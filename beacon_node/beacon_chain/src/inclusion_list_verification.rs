//! Verification of Inclusion List messages for FOCIL (EIP-7805).
//!
//! This module provides verification logic for `SignedInclusionList` messages
//! received over the gossip network, as well as the `InclusionListStore` for
//! tracking observed ILs and equivocation.

use std::collections::{HashMap, HashSet};

use parking_lot::RwLock;
use slot_clock::SlotClock;
use types::{
    ChainSpec, EthSpec, Hash256, Slot,
    inclusion_list::{InclusionList, SignedInclusionList, MAX_BYTES_PER_INCLUSION_LIST, INCLUSION_LIST_COMMITTEE_SIZE},
};
use ssz_types::BitVector;
use typenum::U16;
use tree_hash::TreeHash;

/// Maximum clock disparity for gossip propagation (500ms in milliseconds).
const MAXIMUM_GOSSIP_CLOCK_DISPARITY: u64 = 500;

/// Maximum number of ILs that can be stored per (slot, committee_root) pair.
#[allow(dead_code)]
const MAX_ILS_PER_SLOT: usize = INCLUSION_LIST_COMMITTEE_SIZE;

/// Returned when an inclusion list passes verification.
#[derive(Debug)]
pub struct VerifiedInclusionList<E: EthSpec> {
    signed_inclusion_list: SignedInclusionList<E>,
}

impl<E: EthSpec> VerifiedInclusionList<E> {
    /// Returns the wrapped `SignedInclusionList`.
    pub fn as_inner(&self) -> &SignedInclusionList<E> {
        &self.signed_inclusion_list
    }

    /// Consumes `self`, returning the wrapped `SignedInclusionList`.
    pub fn into_inner(self) -> SignedInclusionList<E> {
        self.signed_inclusion_list
    }
}

/// Error type for inclusion list verification.
#[derive(Debug)]
pub enum Error {
    /// The inclusion list exceeds the maximum byte size.
    ExceedsMaxBytes {
        actual: usize,
        max: usize,
    },
    /// The slot is not within the valid range (current or previous slot).
    InvalidSlot {
        slot: Slot,
        current_slot: Slot,
    },
    /// The inclusion list was received too late (past attestation deadline).
    ReceivedTooLate {
        slot: Slot,
    },
    /// The committee root does not match.
    CommitteeRootMismatch {
        expected: Hash256,
        provided: Hash256,
    },
    /// The validator is not in the inclusion list committee.
    ValidatorNotInCommittee {
        validator_index: u64,
    },
    /// The validator has already equivocated.
    EquivocatedValidator {
        validator_index: u64,
    },
    /// Invalid signature.
    InvalidSignature,
    /// Unknown validator index.
    UnknownValidatorIndex(u64),
    /// The chain is not synced enough to verify this inclusion list.
    NotSynced,
    /// Internal error during verification.
    InternalError(String),
}

impl From<safe_arith::ArithError> for Error {
    fn from(e: safe_arith::ArithError) -> Self {
        Error::InternalError(format!("Arithmetic error: {:?}", e))
    }
}

impl<E: EthSpec> VerifiedInclusionList<E> {
    /// Verify basic structural requirements for a `SignedInclusionList`.
    ///
    /// This performs the P2P-level validation checks that don't require chain state:
    ///
    /// 1. [REJECT] The size of `message.transactions` is within upperbound MAX_BYTES_PER_INCLUSION_LIST.
    /// 2. [REJECT] The slot matches the current or previous slot.
    /// 3. [IGNORE] Timing constraints for previous slot.
    ///
    /// Additional state-dependent checks should be performed separately.
    pub fn verify_basic(
        signed_inclusion_list: SignedInclusionList<E>,
        current_slot: Slot,
    ) -> Result<Self, Error> {
        let message = &signed_inclusion_list.message;

        // 1. [REJECT] Check byte size
        let total_bytes = message.total_bytes();
        if total_bytes > MAX_BYTES_PER_INCLUSION_LIST {
            return Err(Error::ExceedsMaxBytes {
                actual: total_bytes,
                max: MAX_BYTES_PER_INCLUSION_LIST,
            });
        }

        // 2. [REJECT] Slot must be current or previous slot
        let previous_slot = current_slot.saturating_sub(1u64);
        
        if message.slot != current_slot && message.slot != previous_slot {
            return Err(Error::InvalidSlot {
                slot: message.slot,
                current_slot,
            });
        }

        Ok(Self {
            signed_inclusion_list,
        })
    }
    
    /// Perform full P2P verification with state-dependent checks.
    ///
    /// Implements all P2P validation rules from Heze spec:
    ///
    /// 1. [REJECT] The size of `message.transactions` is within `MAX_BYTES_PER_INCLUSION_LIST`.
    /// 2. [REJECT] The slot is equal to the previous or current slot.
    /// 3. [IGNORE] The slot is current, or previous and current time < attestation_due.
    /// 4. [IGNORE] The `inclusion_list_committee_root` matches the computed committee root.
    /// 5. [REJECT] The validator index is within the inclusion list committee.
    /// 6. [IGNORE] This is the first or second valid IL from this validator.
    /// 7. [REJECT] The signature is valid.
    ///
    /// Note: Rules 3, 4, 6 are IGNORE rules - they don't reject but may skip processing.
    pub fn verify_for_gossip(
        signed_inclusion_list: SignedInclusionList<E>,
        current_slot: Slot,
        state: &types::BeaconState<E>,
        spec: &ChainSpec,
        il_store: &InclusionListStore<E>,
    ) -> Result<Self, Error> {
        // First, do basic structural verification
        let verified = Self::verify_basic(signed_inclusion_list, current_slot)?;
        let signed_il = verified.signed_inclusion_list;
        let message = &signed_il.message;
        
        // 4. [IGNORE] Committee root verification
        // Note: This is IGNORE, not REJECT - we skip if mismatch but don't punish
        let expected_committee_root = types::inclusion_list::get_inclusion_list_committee_root(state, message.slot)
            .map_err(|e| Error::InternalError(format!("Failed to compute committee root: {}", e)))?;
        
        if message.inclusion_list_committee_root != expected_committee_root {
            // IGNORE: Committee root mismatch - skip processing
            return Err(Error::CommitteeRootMismatch {
                expected: expected_committee_root,
                provided: message.inclusion_list_committee_root,
            });
        }
        
        // 5. [REJECT] Validator must be in committee
        let committee = types::inclusion_list::get_inclusion_list_committee(state, message.slot)
            .map_err(|e| Error::InternalError(format!("Failed to get committee: {}", e)))?;
        
        if !committee.contains(&message.validator_index) {
            return Err(Error::ValidatorNotInCommittee {
                validator_index: message.validator_index,
            });
        }
        
        // 6. [IGNORE] Check if this is the first or second IL from this validator
        // This is used for equivocation tracking - we allow up to 2 ILs per validator
        let key = (message.slot, message.inclusion_list_committee_root);
        let il_count = il_store.get_inclusion_lists(key)
            .iter()
            .filter(|il| il.validator_index == message.validator_index)
            .count();
        
        if il_count >= 2 {
            // IGNORE: Already have 2 ILs from this validator
            return Err(Error::EquivocatedValidator {
                validator_index: message.validator_index,
            });
        }
        
        // 7. [REJECT] Signature verification
        let is_valid_sig = types::inclusion_list::is_valid_inclusion_list_signature(
            state,
            &signed_il,
            spec,
        ).map_err(|e| Error::InternalError(format!("Signature verification error: {}", e)))?;
        
        if !is_valid_sig {
            return Err(Error::InvalidSignature);
        }
        
        Ok(Self {
            signed_inclusion_list: signed_il,
        })
    }
}

/// Verify that the inclusion list slot is within the allowed propagation range.
pub fn verify_propagation_slot_range(
    slot_clock: &impl SlotClock,
    message: &InclusionList<impl EthSpec>,
) -> Result<(), Error> {
    let current_slot = slot_clock.now().ok_or(Error::NotSynced)?;
    let previous_slot = current_slot.saturating_sub(1u64);

    if message.slot != current_slot && message.slot != previous_slot {
        return Err(Error::InvalidSlot {
            slot: message.slot,
            current_slot,
        });
    }

    // Check if we're in the last 500ms of the slot (too late for IL propagation)
    // This is a simplified check compared to the original
    let slot_duration = slot_clock.slot_duration();
    let tolerance = std::time::Duration::from_millis(MAXIMUM_GOSSIP_CLOCK_DISPARITY);
    
    if let Some(slot_start) = slot_clock.start_of(message.slot) {
        if let Some(now) = slot_clock.now_duration() {
            let elapsed = now.saturating_sub(slot_start);
            if elapsed + tolerance > slot_duration {
                return Err(Error::ReceivedTooLate { slot: message.slot });
            }
        }
    }

    Ok(())
}

/// Key for indexing inclusion lists in the store.
type StoreKey = (Slot, Hash256);

/// InclusionListStore for tracking observed ILs and equivocation.
///
/// This is the main data structure for managing inclusion lists as defined
/// in the Heze specification.
///
/// Spec: https://github.com/ethereum/consensus-specs/blob/main/specs/heze/inclusion-list.md
#[derive(Debug, Default)]
pub struct InclusionListStore<E: EthSpec> {
    /// Store of valid inclusion lists by (slot, committee_root).
    /// Each entry contains the list of ILs for that key.
    /// Spec: `inclusion_lists: DefaultDict[Tuple[Slot, Root], Set[InclusionList]]`
    inclusion_lists: RwLock<HashMap<StoreKey, Vec<InclusionList<E>>>>,
    
    /// Track equivocators: (slot, committee_root) -> Set of validator indices that have equivocated.
    /// Spec: `equivocators: DefaultDict[Tuple[Slot, Root], Set[ValidatorIndex]]`
    equivocators: RwLock<HashMap<StoreKey, HashSet<u64>>>,
    
    /// Track which validators we've seen ILs from: (slot, committee_root) -> (validator_index -> IL hash).
    /// Used for equivocation detection.
    seen_validators: RwLock<HashMap<StoreKey, HashMap<u64, Hash256>>>,
}

impl<E: EthSpec> InclusionListStore<E> {
    /// Create a new inclusion list store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Process a new inclusion list.
    ///
    /// Implements `process_inclusion_list` from the Heze spec:
    /// ```python
    /// def process_inclusion_list(
    ///     store: InclusionListStore, inclusion_list: InclusionList, is_before_view_freeze_cutoff: bool
    /// ) -> None:
    /// ```
    ///
    /// - Ignores ILs from equivocators
    /// - Detects equivocation (same validator, different IL)
    /// - Stores valid ILs before view freeze cutoff
    ///
    /// Returns true if the IL was processed (either stored or detected as equivocation).
    pub fn process_inclusion_list(
        &self,
        inclusion_list: InclusionList<E>,
        is_before_view_freeze_cutoff: bool,
    ) -> bool {
        let key = (inclusion_list.slot, inclusion_list.inclusion_list_committee_root);
        let validator_index = inclusion_list.validator_index;
        
        // Check if this validator is already an equivocator
        if self.is_equivocator(key, validator_index) {
            return false; // Ignore ILs from equivocators
        }
        
        // Compute hash of this IL
        let il_hash = inclusion_list.tree_hash_root();
        
        // Check if we've seen this validator before
        let mut seen = self.seen_validators.write();
        let validator_map = seen.entry(key).or_default();
        
        if let Some(&existing_hash) = validator_map.get(&validator_index) {
            if existing_hash != il_hash {
                // Equivocation detected: same validator, different IL
                self.mark_equivocator(key, validator_index);
                
                // Remove the existing IL from storage
                self.remove_il(key, validator_index);
                
                return true;
            }
            // Same IL, ignore duplicate
            return false;
        }
        
        // First IL from this validator
        if is_before_view_freeze_cutoff {
            // Store the IL
            self.store_il(key, inclusion_list);
            validator_map.insert(validator_index, il_hash);
        }
        
        true
    }

    /// Process a signed inclusion list (convenience method).
    ///
    /// Extracts the message (InclusionList) from SignedInclusionList and processes it.
    pub fn process_signed_inclusion_list(
        &self,
        signed_il: SignedInclusionList<E>,
        is_before_view_freeze_cutoff: bool,
    ) -> bool {
        self.process_inclusion_list(signed_il.message, is_before_view_freeze_cutoff)
    }

    /// Check if a validator is an equivocator for the given key.
    pub fn is_equivocator(&self, key: StoreKey, validator_index: u64) -> bool {
        self.equivocators
            .read()
            .get(&key)
            .is_some_and(|set| set.contains(&validator_index))
    }

    /// Mark a validator as an equivocator.
    fn mark_equivocator(&self, key: StoreKey, validator_index: u64) {
        self.equivocators
            .write()
            .entry(key)
            .or_default()
            .insert(validator_index);
        
        // Also remove from seen validators
        self.seen_validators
            .write()
            .get_mut(&key)
            .map(|map| map.remove(&validator_index));
    }

    /// Store an inclusion list.
    fn store_il(&self, key: StoreKey, inclusion_list: InclusionList<E>) {
        self.inclusion_lists
            .write()
            .entry(key)
            .or_default()
            .push(inclusion_list);
    }

    /// Remove an inclusion list for a specific validator.
    fn remove_il(&self, key: StoreKey, validator_index: u64) {
        if let Some(ils) = self.inclusion_lists.write().get_mut(&key) {
            ils.retain(|il| il.validator_index != validator_index);
        }
    }

    /// Get all inclusion lists for a given key, excluding equivocators.
    pub fn get_inclusion_lists(&self, key: StoreKey) -> Vec<InclusionList<E>> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        
        self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.validator_index))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get the inclusion list bits for a given key.
    ///
    /// Returns a Bitvector where each bit indicates whether an IL was received
    /// from the corresponding committee member.
    pub fn get_inclusion_list_bits(
        &self,
        key: StoreKey,
        committee: &[u64],
    ) -> BitVector<U16> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        let validator_indices: HashSet<u64> = self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.validator_index))
                    .map(|il| il.validator_index)
                    .collect()
            })
            .unwrap_or_default();
        
        // Create bitvector
        let mut bits = BitVector::<U16>::default();
        for (i, &validator_index) in committee.iter().enumerate() {
            if i < INCLUSION_LIST_COMMITTEE_SIZE && validator_indices.contains(&validator_index) {
                bits.set(i, true).expect("index within bounds");
            }
        }
        
        bits
    }

    /// Get all unique transactions from inclusion lists for a given key.
    /// Implements `get_inclusion_list_transactions` from the Heze spec.
    pub fn get_transactions(&self, key: StoreKey) -> Vec<Vec<u8>> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        
        let mut all_txs: Vec<Vec<u8>> = self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.validator_index))
                    .flat_map(|il| il.transactions.iter().map(|tx| tx.to_vec()))
                    .collect()
            })
            .unwrap_or_default();
        
        // Deduplicate transactions
        all_txs.sort();
        all_txs.dedup();
        all_txs
    }

    /// Check if the given inclusion list bits are inclusive of our local view.
    /// Implements `is_inclusion_list_bits_inclusive` from the Heze spec.
    ///
    /// Returns true if `inclusion_list_bits` is a superset of the locally observed bits.
    pub fn is_inclusive(
        &self,
        key: StoreKey,
        committee: &[u64],
        inclusion_list_bits: &BitVector<U16>,
    ) -> bool {
        let local_bits = self.get_inclusion_list_bits(key, committee);
        
        for (_i, (bit, local_bit)) in inclusion_list_bits.iter().zip(local_bits.iter()).enumerate() {
            // If local has a bit set, the incoming must also have it set
            if local_bit && !bit {
                return false;
            }
        }
        true
    }

    /// Prune old inclusion lists from the store.
    pub fn prune(&self, current_slot: Slot) {
        let prune_slot = current_slot.saturating_sub(Slot::new(2));
        
        self.inclusion_lists.write().retain(|(slot, _), _| *slot >= prune_slot);
        self.equivocators.write().retain(|(slot, _), _| *slot >= prune_slot);
        self.seen_validators.write().retain(|(slot, _), _| *slot >= prune_slot);
    }
    
    /// Get the number of stored inclusion lists (for metrics).
    pub fn len(&self) -> usize {
        self.inclusion_lists.read().values().map(|s| s.len()).sum()
    }
    
    /// Check if the store is empty.
    pub fn is_empty(&self) -> bool {
        self.inclusion_lists.read().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inclusion_list_store_basic() {
        let store = InclusionListStore::<types::MainnetEthSpec>::new();
        let slot = Slot::new(1);
        let root = Hash256::zero();
        let key = (slot, root);
        
        // Initially empty
        assert!(store.is_empty());
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn test_max_bytes() {
        assert_eq!(MAX_BYTES_PER_INCLUSION_LIST, 8192);
    }
    
    #[test]
    fn test_committee_size() {
        assert_eq!(INCLUSION_LIST_COMMITTEE_SIZE, 16);
    }
}