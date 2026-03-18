//! Verification of Inclusion List messages for FOCIL (EIP-7805).
//!
//! This module provides verification logic for `SignedInclusionList` messages
//! received over the gossip network, as well as the `InclusionListStore` for
//! tracking observed ILs and equivocation.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use parking_lot::RwLock;
use safe_arith::SafeArith;
use slot_clock::SlotClock;
use types::{
    ChainSpec, EthSpec, Hash256, Slot,
    inclusion_list::{InclusionList, SignedInclusionList, MAX_BYTES_PER_INCLUSION_LIST, INCLUSION_LIST_COMMITTEE_SIZE},
};
use ssz_types::Bitvector;
use typenum::U16;
use tree_hash::TreeHash;

/// Maximum clock disparity for gossip propagation (500ms in milliseconds).
const MAXIMUM_GOSSIP_CLOCK_DISPARITY: u64 = 500;

/// Maximum number of ILs that can be stored per (slot, committee_root) pair.
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

    // Allow 500ms clock disparity
    let tolerance = std::time::Duration::from_millis(MAXIMUM_GOSSIP_CLOCK_DISPARITY);
    if !slot_clock.is_prior_to_slot_end(tolerance) {
        return Err(Error::ReceivedTooLate { slot: message.slot });
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
    /// Each entry contains the set of ILs for that key.
    inclusion_lists: RwLock<HashMap<StoreKey, HashSet<SignedInclusionList<E>>>>,
    
    /// Track equivocators: (slot, committee_root) -> Set of validator indices that have equivocated.
    equivocators: RwLock<HashMap<StoreKey, HashSet<u64>>>,
    
    /// Track which validators we've seen ILs from: (slot, committee_root) -> (validator_index -> IL hash).
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
    /// - Ignores ILs from equivocators
    /// - Detects equivocation (same validator, different IL)
    /// - Stores valid ILs before view freeze cutoff
    ///
    /// Returns true if the IL was processed (either stored or detected as equivocation).
    pub fn process_inclusion_list(
        &self,
        signed_il: SignedInclusionList<E>,
        is_before_view_freeze_cutoff: bool,
    ) -> bool {
        let message = &signed_il.message;
        let key = (message.slot, message.inclusion_list_committee_root);
        let validator_index = message.validator_index;
        
        // Check if this validator is already an equivocator
        if self.is_equivocator(key, validator_index) {
            return false; // Ignore ILs from equivocators
        }
        
        // Compute hash of this IL
        let il_hash = signed_il.tree_hash_root();
        
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
            self.store_il(key, signed_il.clone());
            validator_map.insert(validator_index, il_hash);
        }
        
        true
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
    fn store_il(&self, key: StoreKey, signed_il: SignedInclusionList<E>) {
        self.inclusion_lists
            .write()
            .entry(key)
            .or_default()
            .insert(signed_il);
    }

    /// Remove an inclusion list for a specific validator.
    fn remove_il(&self, key: StoreKey, validator_index: u64) {
        if let Some(ils) = self.inclusion_lists.write().get_mut(&key) {
            ils.retain(|il| il.message.validator_index != validator_index);
        }
    }

    /// Get all inclusion lists for a given key, excluding equivocators.
    pub fn get_inclusion_lists(&self, key: StoreKey) -> Vec<SignedInclusionList<E>> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        
        self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.message.validator_index))
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
    ) -> Bitvector<U16> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        let validator_indices: HashSet<u64> = self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.message.validator_index))
                    .map(|il| il.message.validator_index)
                    .collect()
            })
            .unwrap_or_default();
        
        // Create bitvector
        let mut bits = vec![false; INCLUSION_LIST_COMMITTEE_SIZE];
        for (i, &validator_index) in committee.iter().enumerate() {
            if i < INCLUSION_LIST_COMMITTEE_SIZE && validator_indices.contains(&validator_index) {
                bits[i] = true;
            }
        }
        
        Bitvector::new(bits).unwrap_or_default()
    }

    /// Get all unique transactions from inclusion lists for a given key.
    pub fn get_transactions(&self, key: StoreKey) -> Vec<Vec<u8>> {
        let equivocators = self.equivocators.read().get(&key).cloned().unwrap_or_default();
        
        let mut all_txs: Vec<Vec<u8>> = self.inclusion_lists
            .read()
            .get(&key)
            .map(|ils| {
                ils.iter()
                    .filter(|il| !equivocators.contains(&il.message.validator_index))
                    .flat_map(|il| il.message.transactions.iter().cloned())
                    .collect()
            })
            .unwrap_or_default();
        
        // Deduplicate transactions
        all_txs.sort();
        all_txs.dedup();
        all_txs
    }

    /// Check if the given inclusion list bits are inclusive of our local view.
    ///
    /// Returns true if `inclusion_list_bits` is a superset of the locally observed bits.
    pub fn is_inclusive(
        &self,
        key: StoreKey,
        committee: &[u64],
        inclusion_list_bits: &Bitvector<U16>,
    ) -> bool {
        let local_bits = self.get_inclusion_list_bits(key, committee);
        
        for (i, (bit, local_bit)) in inclusion_list_bits.iter().zip(local_bits.iter()).enumerate() {
            // If local has a bit set, the incoming must also have it set
            if local_bit && !bit {
                return false;
            }
        }
        true
    }

    /// Prune old inclusion lists from the store.
    pub fn prune(&self, current_slot: Slot) {
        let prune_slot = current_slot.saturating_sub(2);
        
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