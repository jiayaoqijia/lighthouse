use crate::execution::ExecutionPayloadBid;
use crate::state::BeaconStateError;
use crate::test_utils::TestRandom;
use crate::{EthSpec, ForkName};
use bls::Signature;
use context_deserialize::{ContextDeserialize, context_deserialize};
use educe::Educe;
use rand::RngCore;
use serde::{Deserialize, Deserializer, Serialize};
use ssz::Decode;
use ssz_derive::{Decode, Encode};
use superstruct::superstruct;
use test_random_derive::TestRandom;
use tree_hash_derive::TreeHash;

#[superstruct(
    variants(Gloas, Heze),
    variant_attributes(
        derive(
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
    cast_error(ty = "BeaconStateError", expr = "BeaconStateError::IncorrectStateVariant"),
    partial_getter_error(ty = "BeaconStateError", expr = "BeaconStateError::IncorrectStateVariant")
)]
#[derive(Debug, Clone, Serialize, Deserialize, Encode, TreeHash, Educe)]
#[educe(PartialEq, Hash)]
#[serde(bound = "E: EthSpec", untagged)]
#[ssz(enum_behaviour = "transparent")]
#[tree_hash(enum_behaviour = "transparent")]
pub struct SignedExecutionPayloadBid<E: EthSpec> {
    pub message: ExecutionPayloadBid<E>,
    pub signature: Signature,
}

// Manual implementation of Decode for the enum
impl<E: EthSpec> Decode for SignedExecutionPayloadBid<E> {
    fn is_ssz_fixed_len() -> bool {
        false
    }

    fn ssz_fixed_len() -> usize {
        0
    }

    fn from_ssz_bytes(bytes: &[u8]) -> Result<Self, ssz::DecodeError> {
        // Try Gloas first (shorter encoding without inclusion_list_bits)
        // If that fails, try Heze
        SignedExecutionPayloadBidGloas::from_ssz_bytes(bytes)
            .map(Self::Gloas)
            .or_else(|_| SignedExecutionPayloadBidHeze::from_ssz_bytes(bytes).map(Self::Heze))
    }
}

// Manual implementation of TestRandom for the enum
impl<E: EthSpec> TestRandom for SignedExecutionPayloadBid<E> {
    fn random_for_test(rng: &mut impl RngCore) -> Self {
        // Always generate Gloas variant since Heze is not yet active.
        // Gloas blocks should contain Gloas bids, and Heze blocks should contain Heze bids.
        // When Heze becomes active, this can be updated to randomly choose variants
        // based on the fork context.
        Self::Gloas(SignedExecutionPayloadBidGloas::random_for_test(rng))
    }
}

impl<E: EthSpec> SignedExecutionPayloadBid<E> {
    pub fn empty_gloas() -> Self {
        Self::Gloas(SignedExecutionPayloadBidGloas {
            message: ExecutionPayloadBid::Gloas(Default::default()),
            signature: Signature::empty(),
        })
    }

    pub fn empty_heze() -> Self {
        Self::Heze(SignedExecutionPayloadBidHeze {
            message: ExecutionPayloadBid::Heze(Default::default()),
            signature: Signature::empty(),
        })
    }
}

impl<E: EthSpec> crate::fork::ForkVersionDecode for SignedExecutionPayloadBid<E> {
    fn from_ssz_bytes_by_fork(bytes: &[u8], fork_name: ForkName) -> Result<Self, ssz::DecodeError> {
        match fork_name {
            ForkName::Base | ForkName::Altair | ForkName::Bellatrix | ForkName::Capella 
            | ForkName::Deneb | ForkName::Electra | ForkName::Fulu => {
                Err(ssz::DecodeError::BytesInvalid(format!(
                    "unsupported fork for SignedExecutionPayloadBid: {fork_name}",
                )))
            }
            ForkName::Gloas => SignedExecutionPayloadBidGloas::from_ssz_bytes(bytes).map(Self::Gloas),
            ForkName::Heze => SignedExecutionPayloadBidHeze::from_ssz_bytes(bytes).map(Self::Heze),
        }
    }
}

impl<'de, E: EthSpec> ContextDeserialize<'de, ForkName> for SignedExecutionPayloadBid<E> {
    fn context_deserialize<D>(deserializer: D, context: ForkName) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let convert_err = |e| {
            serde::de::Error::custom(format!("SignedExecutionPayloadBid failed to deserialize: {:?}", e))
        };
        Ok(match context {
            ForkName::Base | ForkName::Altair | ForkName::Bellatrix | ForkName::Capella 
            | ForkName::Deneb | ForkName::Electra | ForkName::Fulu => {
                return Err(serde::de::Error::custom(format!(
                    "SignedExecutionPayloadBid failed to deserialize: unsupported fork '{}'",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MainnetEthSpec;

    mod signed_execution_payload_bid_gloas {
        use super::*;
        ssz_and_tree_hash_tests!(SignedExecutionPayloadBidGloas<MainnetEthSpec>);
    }

    mod signed_execution_payload_bid_heze {
        use super::*;
        ssz_and_tree_hash_tests!(SignedExecutionPayloadBidHeze<MainnetEthSpec>);
    }
}