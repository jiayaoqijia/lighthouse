use crate::kzg_ext::KzgCommitments;
use crate::test_utils::TestRandom;
use crate::{Address, EthSpec, ExecutionBlockHash, ForkName, Hash256, SignedRoot, Slot};
use context_deserialize::context_deserialize;
use educe::Educe;
use serde::{Deserialize, Serialize};
use ssz_derive::{Decode, Encode};
use ssz_types::BitVector;
use test_random_derive::TestRandom;
use tree_hash_derive::TreeHash;
use typenum::U16;

/// Size of the inclusion list committee as per EIP-7805.
pub const INCLUSION_LIST_COMMITTEE_SIZE: usize = 16;

#[derive(
    Default, Debug, Clone, Serialize, Encode, Decode, Deserialize, TreeHash, Educe, TestRandom,
)]
#[cfg_attr(
    feature = "arbitrary",
    derive(arbitrary::Arbitrary),
    arbitrary(bound = "E: EthSpec")
)]
#[educe(PartialEq, Hash)]
#[serde(bound = "E: EthSpec")]
#[context_deserialize(ForkName)]
// https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/beacon-chain.md#executionpayloadbid
// Modified in Heze:EIP7805 to add inclusion_list_bits
pub struct ExecutionPayloadBid<E: EthSpec> {
    pub parent_block_hash: ExecutionBlockHash,
    pub parent_block_root: Hash256,
    pub block_hash: ExecutionBlockHash,
    pub prev_randao: Hash256,
    #[serde(with = "serde_utils::address_hex")]
    pub fee_recipient: Address,
    #[serde(with = "serde_utils::quoted_u64")]
    pub gas_limit: u64,
    #[serde(with = "serde_utils::quoted_u64")]
    pub builder_index: u64,
    pub slot: Slot,
    #[serde(with = "serde_utils::quoted_u64")]
    pub value: u64,
    #[serde(with = "serde_utils::quoted_u64")]
    pub execution_payment: u64,
    pub blob_kzg_commitments: KzgCommitments<E>,
    /// [New in Heze:EIP7805] Bitvector indicating which IL committee members' ILs are satisfied.
    /// Each bit corresponds to a committee member index. If bit i is set, the builder claims
    /// to have satisfied the IL from committee member i.
    /// Default is all zeros (no ILs satisfied).
    /// Note: For pre-Heze (Gloas), this field should be all zeros.
    #[serde(default = "default_inclusion_list_bits")]
    pub inclusion_list_bits: BitVector<U16>,
}

fn default_inclusion_list_bits() -> BitVector<U16> {
    BitVector::default()
}

impl<E: EthSpec> SignedRoot for ExecutionPayloadBid<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MainnetEthSpec;

    ssz_and_tree_hash_tests!(ExecutionPayloadBid<MainnetEthSpec>);
}
