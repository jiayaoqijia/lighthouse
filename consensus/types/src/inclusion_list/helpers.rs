//! Inclusion List helpers for FOCIL (EIP-7805).
//!
//! This module implements helper functions for Inclusion List committee selection
//! and signature verification as defined in the Heze fork specification.

use crate::{
    core::{ChainSpec, EthSpec, Hash256, Slot, SignedRoot},
    inclusion_list::{SignedInclusionList, INCLUSION_LIST_COMMITTEE_SIZE},
    state::BeaconState,
};
use ssz_types::FixedVector;
use tree_hash::TreeHash;

/// Validator index type alias.
pub type ValidatorIndex = u64;

/// Returns the inclusion list committee for the given slot.
///
/// The committee is computed by:
/// 1. Getting all beacon committees for the slot
/// 2. Concatenating all validator indices
/// 3. Selecting INCLUSION_LIST_COMMITTEE_SIZE (16) validators using modulo
///
/// Spec: https://github.com/ethereum/consensus-specs/blob/main/specs/heze/beacon-chain.md#get_inclusion_list_committee
pub fn get_inclusion_list_committee<E: EthSpec>(
    state: &BeaconState<E>,
    slot: Slot,
) -> Result<FixedVector<ValidatorIndex, typenum::U16>, String> {
    // Get all beacon committees at this slot
    let committees = state
        .get_beacon_committees_at_slot(slot)
        .map_err(|e| format!("Failed to get beacon committees: {:?}", e))?;

    // Concatenate all committee members
    let mut all_indices: Vec<ValidatorIndex> = Vec::new();
    for committee in committees {
        for &validator_index in committee.committee {
            all_indices.push(validator_index as u64);
        }
    }

    // If no validators, return empty committee
    if all_indices.is_empty() {
        return Ok(FixedVector::default());
    }

    // Select INCLUSION_LIST_COMMITTEE_SIZE validators using modulo
    let mut committee_result = Vec::with_capacity(INCLUSION_LIST_COMMITTEE_SIZE);
    for i in 0..INCLUSION_LIST_COMMITTEE_SIZE {
        let index = i % all_indices.len();
        committee_result.push(all_indices[index]);
    }

    FixedVector::new(committee_result)
        .map_err(|e| format!("Failed to create fixed vector: {:?}", e))
}

/// Returns the root of the inclusion list committee for the given slot.
///
/// This is used to verify that an IL references the correct committee.
pub fn get_inclusion_list_committee_root<E: EthSpec>(
    state: &BeaconState<E>,
    slot: Slot,
) -> Result<Hash256, String> {
    let committee = get_inclusion_list_committee(state, slot)?;
    Ok(committee.tree_hash_root())
}

/// Check if a signed inclusion list has a valid signature.
///
/// Verifies that:
/// 1. The signer is in the inclusion list committee
/// 2. The signature is valid for the DOMAIN_INCLUSION_LIST_COMMITTEE domain
///
/// Spec: https://github.com/ethereum/consensus-specs/blob/main/specs/heze/beacon-chain.md#is_valid_inclusion_list_signature
pub fn is_valid_inclusion_list_signature<E: EthSpec>(
    state: &BeaconState<E>,
    signed_inclusion_list: &SignedInclusionList<E>,
    spec: &ChainSpec,
) -> Result<bool, String> {
    let message = &signed_inclusion_list.message;
    let validator_index = message.validator_index;

    // Get the validator's public key
    let validator = state
        .validators()
        .get(validator_index as usize)
        .ok_or_else(|| format!("Validator index {} out of bounds", validator_index))?;
    
    let pubkey = validator
        .pubkey
        .decompress()
        .map_err(|e| format!("Failed to decompress pubkey: {:?}", e))?;

    // Verify the signer is in the inclusion list committee
    let committee = get_inclusion_list_committee(state, message.slot)?;
    if !committee.contains(&validator_index) {
        return Ok(false);
    }

    // Verify the inclusion list committee root matches
    let expected_committee_root = get_inclusion_list_committee_root(state, message.slot)?;
    if message.inclusion_list_committee_root != expected_committee_root {
        return Ok(false);
    }

    // Compute signing root with DOMAIN_INCLUSION_LIST_COMMITTEE
    let epoch = message.slot.epoch(E::slots_per_epoch());
    let domain = spec.get_domain(
        epoch,
        crate::Domain::InclusionListCommittee,
        &state.fork(),
        state.genesis_validators_root(),
    );
    let signing_root = message.signing_root(domain);

    // Verify BLS signature
    Ok(signed_inclusion_list.signature.verify(&pubkey, signing_root))
}

/// Check if a validator is a member of the inclusion list committee for the given slot.
pub fn is_inclusion_list_committee_member<E: EthSpec>(
    state: &BeaconState<E>,
    slot: Slot,
    validator_index: ValidatorIndex,
) -> Result<bool, String> {
    let committee = get_inclusion_list_committee(state, slot)?;
    Ok(committee.contains(&validator_index))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_committee_size() {
        // The committee should always be of size INCLUSION_LIST_COMMITTEE_SIZE (16)
        assert_eq!(INCLUSION_LIST_COMMITTEE_SIZE, 16);
    }
}