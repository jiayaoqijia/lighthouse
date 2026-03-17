//! Inclusion List types for FOCIL (EIP-7805).
//!
//! FOCIL (Fork-choice enforced Inclusion Lists) ensures censorship resistance
//! by requiring block builders to include transactions specified by a committee
//! of validators.

use bls::Signature;
use context_deserialize::context_deserialize;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::VariableList;
use test_random_derive::TestRandom;
use tree_hash_derive::TreeHash;

use crate::{
    core::{EthSpec, Hash256, Slot},
    fork::ForkName,
    test_utils::TestRandom,
};

/// Maximum number of transactions in an Inclusion List.
/// This is bounded by the 8 KiB limit on the total size.
pub const MAX_TRANSACTIONS_PER_INCLUSION_LIST: usize = 16;

/// Maximum bytes per Inclusion List (8 KiB as per EIP-7805).
pub const MAX_BYTES_PER_INCLUSION_LIST: usize = 8192;

/// Transaction type alias for Inclusion List.
/// Each transaction is a variable list of bytes.
pub type IlTransaction<E> = VariableList<u8, <E as EthSpec>::MaxBytesPerTransaction>;

/// List of transactions in an Inclusion List.
pub type IlTransactions<E> = VariableList<IlTransaction<E>, typenum::U16>;

/// InclusionList represents a set of transactions that MUST be included
/// in a subsequent block. Created by an IL committee member for a given slot.
///
/// Spec: https://eips.ethereum.org/EIPS/eip-7805
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode, TreeHash, TestRandom)]
#[context_deserialize(ForkName)]
pub struct InclusionList<E: EthSpec> {
    /// The slot for which this IL is intended.
    pub slot: Slot,
    /// The validator index of the IL committee member who created this IL.
    #[serde(with = "serde_utils::quoted_u64")]
    pub validator_index: u64,
    /// The root of the IL committee for this slot.
    pub inclusion_list_committee_root: Hash256,
    /// The list of transactions to be included.
    #[serde(bound = "E: EthSpec")]
    pub transactions: IlTransactions<E>,
}

impl<E: EthSpec> InclusionList<E> {
    /// Returns the total byte size of all transactions in the IL.
    pub fn total_bytes(&self) -> usize {
        self.transactions.iter().map(|tx| tx.len()).sum()
    }

    /// Returns the number of transactions in the IL.
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }

    /// Checks if the IL exceeds the maximum byte size.
    pub fn exceeds_max_bytes(&self) -> bool {
        self.total_bytes() > MAX_BYTES_PER_INCLUSION_LIST
    }
}

/// SignedInclusionList wraps an InclusionList with a BLS signature.
///
/// The signature is computed over the signing root of the InclusionList
/// with the IL_COMMITTEE domain.
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Encode, Decode, TreeHash, TestRandom)]
#[context_deserialize(ForkName)]
pub struct SignedInclusionList<E: EthSpec> {
    /// The inclusion list message.
    #[serde(bound = "E: EthSpec")]
    pub message: InclusionList<E>,
    /// The BLS signature by the IL committee member.
    pub signature: Signature,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MainnetEthSpec;

    ssz_and_tree_hash_tests!(InclusionList<MainnetEthSpec>);
    ssz_and_tree_hash_tests!(SignedInclusionList<MainnetEthSpec>);
}