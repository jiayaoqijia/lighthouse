//! Verification of Inclusion List messages for FOCIL (EIP-7805).
//!
//! This module provides verification logic for `SignedInclusionList` messages
//! received over the gossip network.

use std::collections::HashSet;
use std::sync::Arc;

use parking_lot::RwLock;
use safe_arith::SafeArith;
use slot_clock::SlotClock;
use types::{
    ChainSpec, EthSpec, Hash256, Slot,
    inclusion_list::{InclusionList, SignedInclusionList, MAX_BYTES_PER_INCLUSION_LIST},
};

/// Maximum clock disparity for gossip propagation (500ms in milliseconds).
const MAXIMUM_GOSSIP_CLOCK_DISPARITY: u64 = 500;

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

/// Store for tracking observed inclusion lists and equivocation.
#[derive(Debug, Default)]
pub struct ObservedInclusionLists {
    /// Maps (slot, committee_root) -> (validator_index -> count)
    observed: RwLock<HashSet<(Slot, Hash256, u64)>>,
    /// Track equivocators: (slot, committee_root) -> Set of equivocator validator indices
    equivocators: RwLock<HashSet<(Slot, Hash256, u64)>>,
}

impl ObservedInclusionLists {
    /// Create a new observed inclusion lists store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this validator has already been observed for this slot/committee.
    pub fn is_known(&self, slot: Slot, committee_root: Hash256, validator_index: u64) -> bool {
        self.observed.read().contains(&(slot, committee_root, validator_index))
    }

    /// Check if this validator is an equivocator.
    pub fn is_equivocator(&self, slot: Slot, committee_root: Hash256, validator_index: u64) -> bool {
        self.equivocators.read().contains(&(slot, committee_root, validator_index))
    }

    /// Observe an inclusion list. Returns true if this is an equivocation.
    pub fn observe(&self, slot: Slot, committee_root: Hash256, validator_index: u64) -> bool {
        let key = (slot, committee_root, validator_index);
        
        // Check if already marked as equivocator
        if self.equivocators.read().contains(&key) {
            return true;
        }

        // Check if we've seen this validator before
        if self.observed.read().contains(&key) {
            // This is the second IL from this validator - mark as equivocator
            self.equivocators.write().insert(key);
            self.observed.write().remove(&key);
            return true;
        }

        // First time seeing this validator
        self.observed.write().insert(key);
        false
    }

    /// Prune old inclusion lists from the store.
    pub fn prune(&self, current_slot: Slot) {
        let prune_slot = current_slot.saturating_sub(2);
        self.observed.write().retain(|(slot, _, _)| *slot >= prune_slot);
        self.equivocators.write().retain(|(slot, _, _)| *slot >= prune_slot);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_observed_inclusion_lists() {
        let store = ObservedInclusionLists::new();
        let slot = Slot::new(1);
        let root = Hash256::zero();
        let validator = 0u64;

        // First observation should not be equivocation
        assert!(!store.observe(slot, root, validator));
        assert!(store.is_known(slot, root, validator));
        assert!(!store.is_equivocator(slot, root, validator));

        // Second observation should be equivocation
        assert!(store.observe(slot, root, validator));
        assert!(!store.is_known(slot, root, validator));
        assert!(store.is_equivocator(slot, root, validator));
    }

    #[test]
    fn test_max_bytes() {
        assert_eq!(MAX_BYTES_PER_INCLUSION_LIST, 8192);
    }
}