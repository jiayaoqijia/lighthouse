use crate::kzg_ext::KzgCommitments;
use crate::state::BeaconStateError;
use crate::test_utils::TestRandom;
use crate::{Address, EthSpec, ExecutionBlockHash, ForkName, Hash256, SignedRoot, Slot};
use context_deserialize::{ContextDeserialize, context_deserialize};
use educe::Educe;
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize};
use ssz::{Decode, Encode};
use ssz_derive::{Decode, Encode};
use ssz_types::BitVector;
use superstruct::superstruct;
use test_random_derive::TestRandom;
use tree_hash_derive::TreeHash;
use typenum::U16;

/// Size of the inclusion list committee as per EIP-7805.
pub const INCLUSION_LIST_COMMITTEE_SIZE: usize = 16;

/// Superstruct combining ExecutionPayloadBid variants for Gloas and Heze.
/// Gloas variant does NOT have inclusion_list_bits.
/// Heze variant has inclusion_list_bits as per EIP-7805.
// https://github.com/ethereum/consensus-specs/blob/master/specs/gloas/beacon-chain.md#executionpayloadbid
// Modified in Heze:EIP7805 to add inclusion_list_bits
#[superstruct(
    variants(Gloas, Heze),
    variant_attributes(
        derive(
            Default,
            Debug,
            Clone,
            Serialize,
            Deserialize,
            Encode,
            Decode,
            TreeHash,
            Educe,
            TestRandom,
        ),
        context_deserialize(ForkName),
        educe(PartialEq, Hash),
        serde(bound = "E: EthSpec", deny_unknown_fields),
        cfg_attr(
            feature = "arbitrary",
            derive(arbitrary::Arbitrary),
            arbitrary(bound = "E: EthSpec"),
        ),
    ),
    cast_error(
        ty = "BeaconStateError",
        expr = "BeaconStateError::IncorrectStateVariant"
    ),
    partial_getter_error(
        ty = "BeaconStateError",
        expr = "BeaconStateError::IncorrectStateVariant"
    )
)]
#[derive(Debug, Clone, Serialize, Deserialize, Encode, TreeHash, Educe)]
#[educe(PartialEq, Hash)]
#[serde(bound = "E: EthSpec", untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct ExecutionPayloadBid<E: EthSpec> {
    #[superstruct(getter(copy))]
    pub parent_block_hash: ExecutionBlockHash,
    #[superstruct(getter(copy))]
    pub parent_block_root: Hash256,
    #[superstruct(getter(copy))]
    pub block_hash: ExecutionBlockHash,
    #[superstruct(getter(copy))]
    pub prev_randao: Hash256,
    #[serde(with = "serde_utils::address_hex")]
    #[superstruct(getter(copy))]
    pub fee_recipient: Address,
    #[serde(with = "serde_utils::quoted_u64")]
    #[superstruct(getter(copy))]
    pub gas_limit: u64,
    #[serde(with = "serde_utils::quoted_u64")]
    #[superstruct(getter(copy))]
    pub builder_index: u64,
    #[superstruct(getter(copy))]
    pub slot: Slot,
    #[serde(with = "serde_utils::quoted_u64")]
    #[superstruct(getter(copy))]
    pub value: u64,
    #[serde(with = "serde_utils::quoted_u64")]
    #[superstruct(getter(copy))]
    pub execution_payment: u64,
    pub blob_kzg_commitments: KzgCommitments<E>,
    /// [New in Heze:EIP7805] Bitvector indicating which IL committee members' ILs are satisfied.
    /// Each bit corresponds to a committee member index. If bit i is set, the builder claims
    /// to have satisfied the IL from committee member i.
    /// Default is all zeros (no ILs satisfied).
    #[superstruct(only(Heze))]
    #[serde(default)]
    pub inclusion_list_bits: BitVector<U16>,
}

// Manual implementation of Decode for the enum
impl<E: EthSpec> Decode for ExecutionPayloadBid<E> {
    fn is_ssz_fixed_len() -> bool {
        false
    }

    fn ssz_fixed_len() -> usize {
        <Self as Decode>::from_ssz_bytes(&[])
            .map(|_| 0)
            .unwrap_or(0)
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, ssz::DecodeError> {
        // Try Gloas first (shorter encoding without inclusion_list_bits).
        // Only try Heze if Gloas fails AND there are enough extra bytes for inclusion_list_bits.
        // Heze has an extra BitVector<U16> field (2 bytes) compared to Gloas.
        // This order prevents Gloas data from being incorrectly decoded as Heze.
        let gloas_result = ExecutionPayloadBidGloas::from_ssz_bytes(bytes);
        
        match gloas_result {
            Ok(gloas) => {
                // Gloas decoded successfully. Check if there might be extra bytes
                // that could indicate this is actually Heze data.
                // Re-encode to check consumed bytes.
                let gloas_bytes = gloas.as_ssz_bytes();
                if bytes.len() > gloas_bytes.len() + 2 {
                    // There are more bytes than Gloas would use, try Heze
                    if let Ok(heze) = ExecutionPayloadBidHeze::from_ssz_bytes(bytes) {
                        let heze_bytes = heze.as_ssz_bytes();
                        // Use Heze only if it consumes all bytes exactly
                        if heze_bytes.len() == bytes.len() {
                            return Ok(Self::Heze(heze));
                        }
                    }
                }
                Ok(Self::Gloas(gloas))
            }
            Err(_) => {
                // Gloas failed, try Heze
                ExecutionPayloadBidHeze::from_ssz_bytes(bytes).map(Self::Heze)
            }
        }
    }
}

// Manual implementation of TestRandom for the enum
impl<E: EthSpec> TestRandom for ExecutionPayloadBid<E> {
    fn random_for_test(rng: &mut impl RngCore) -> Self {
        // Always generate Gloas variant since Heze is not yet active.
        // Gloas blocks should contain Gloas bids, and Heze blocks should contain Heze bids.
        // When Heze becomes active, this can be updated to randomly choose variants
        // based on the fork context.
        Self::Gloas(ExecutionPayloadBidGloas::random_for_test(rng))
    }
}

impl<E: EthSpec> crate::fork::ForkVersionDecode for ExecutionPayloadBid<E> {
    fn from_ssz_bytes_by_fork(bytes: &[u8], fork_name: ForkName) -> Result<Self, ssz::DecodeError> {
        match fork_name {
            ForkName::Base
            | ForkName::Altair
            | ForkName::Bellatrix
            | ForkName::Capella
            | ForkName::Deneb
            | ForkName::Electra
            | ForkName::Fulu => Err(ssz::DecodeError::BytesInvalid(format!(
                "unsupported fork for ExecutionPayloadBid: {fork_name}",
            ))),
            ForkName::Gloas => ExecutionPayloadBidGloas::from_ssz_bytes(bytes).map(Self::Gloas),
            ForkName::Heze => ExecutionPayloadBidHeze::from_ssz_bytes(bytes).map(Self::Heze),
        }
    }
}

impl<'de, E: EthSpec> ContextDeserialize<'de, ForkName> for ExecutionPayloadBid<E> {
    fn context_deserialize<D>(deserializer: D, context: ForkName) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let convert_err = |e| {
            serde::de::Error::custom(format!(
                "ExecutionPayloadBid failed to deserialize: {:?}",
                e
            ))
        };
        Ok(match context {
            ForkName::Base
            | ForkName::Altair
            | ForkName::Bellatrix
            | ForkName::Capella
            | ForkName::Deneb
            | ForkName::Electra
            | ForkName::Fulu => {
                return Err(serde::de::Error::custom(format!(
                    "ExecutionPayloadBid failed to deserialize: unsupported fork '{}'",
                    context
                )));
            }
            ForkName::Gloas => {
                Self::Gloas(Deserialize::deserialize(deserializer).map_err(convert_err)?)
            }
            ForkName::Heze => {
                Self::Heze(Deserialize::deserialize(deserializer).map_err(convert_err)?)
            }
        })
    }
}

impl<E: EthSpec> Default for ExecutionPayloadBid<E> {
    fn default() -> Self {
        // Default to Gloas variant since Heze is not yet active.
        Self::Gloas(ExecutionPayloadBidGloas::default())
    }
}

impl<E: EthSpec> SignedRoot for ExecutionPayloadBid<E> {}
impl<E: EthSpec> SignedRoot for ExecutionPayloadBidGloas<E> {}
impl<E: EthSpec> SignedRoot for ExecutionPayloadBidHeze<E> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MainnetEthSpec;
    use ssz::{Decode, Encode};

    mod execution_payload_bid_gloas {
        use super::*;
        ssz_and_tree_hash_tests!(ExecutionPayloadBidGloas<MainnetEthSpec>);
    }

    mod execution_payload_bid_heze {
        use super::*;
        ssz_and_tree_hash_tests!(ExecutionPayloadBidHeze<MainnetEthSpec>);
    }

    /// Test that Heze data cannot be incorrectly decoded as Gloas
    #[test]
    fn test_heze_cannot_decode_as_gloas() {
        // Create a Heze bid with default values
        let heze_bid = ExecutionPayloadBidHeze::<MainnetEthSpec>::default();
        let heze_bytes = heze_bid.as_ssz_bytes();

        // Heze has inclusion_list_bits, so it should be longer than Gloas
        // Attempting to decode Heze bytes as Gloas should fail
        let result = ExecutionPayloadBidGloas::<MainnetEthSpec>::from_ssz_bytes(&heze_bytes);
        assert!(
            result.is_err(),
            "Heze data should NOT be decodable as Gloas - this would cause data corruption!"
        );
    }

    /// Test that Gloas data can be decoded correctly using the enum decoder
    #[test]
    fn test_gloas_decode_via_enum() {
        let gloas_bid = ExecutionPayloadBidGloas::<MainnetEthSpec>::default();
        let gloas_bytes = gloas_bid.as_ssz_bytes();

        // Decode via the enum's Decode implementation
        let decoded: ExecutionPayloadBid<MainnetEthSpec> =
            ExecutionPayloadBid::from_ssz_bytes(&gloas_bytes).expect("should decode");

        // Should be Gloas variant
        assert!(matches!(decoded, ExecutionPayloadBid::Gloas(_)));
    }

    /// Test that Heze data can be decoded correctly using the enum decoder
    #[test]
    fn test_heze_decode_via_enum() {
        let heze_bid = ExecutionPayloadBidHeze::<MainnetEthSpec>::default();
        let heze_bytes = heze_bid.as_ssz_bytes();

        // Decode via the enum's Decode implementation
        let decoded: ExecutionPayloadBid<MainnetEthSpec> =
            ExecutionPayloadBid::from_ssz_bytes(&heze_bytes).expect("should decode");

        // Should be Heze variant
        assert!(matches!(decoded, ExecutionPayloadBid::Heze(_)));
    }
}
