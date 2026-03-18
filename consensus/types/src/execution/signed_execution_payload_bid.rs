use crate::execution::{ExecutionPayloadBid, ExecutionPayloadBidGloas, ExecutionPayloadBidHeze};
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
    /// The execution payload bid message.
    /// Gloas variant uses ExecutionPayloadBidGloas (no inclusion_list_bits).
    #[superstruct(only(Gloas))]
    pub message_gloas: ExecutionPayloadBidGloas<E>,
    /// The execution payload bid message.
    /// Heze variant uses ExecutionPayloadBidHeze (with inclusion_list_bits).
    #[superstruct(only(Heze))]
    pub message_heze: ExecutionPayloadBidHeze<E>,
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
        // Try Heze first (longer encoding with inclusion_list_bits in message)
        // If that fails (missing inclusion_list_bits), try Gloas
        // This order is critical because the message's blob_kzg_commitments (VariableList)
        // could consume the inclusion_list_bits bytes from Heze data, causing data corruption.
        SignedExecutionPayloadBidHeze::from_ssz_bytes(bytes)
            .map(Self::Heze)
            .or_else(|_| SignedExecutionPayloadBidGloas::from_ssz_bytes(bytes).map(Self::Gloas))
    }
}

// Manual implementation of Default for the concrete variants
impl<E: EthSpec> Default for SignedExecutionPayloadBidGloas<E> {
    fn default() -> Self {
        Self {
            message_gloas: ExecutionPayloadBidGloas::default(),
            signature: Signature::empty(),
        }
    }
}

impl<E: EthSpec> Default for SignedExecutionPayloadBidHeze<E> {
    fn default() -> Self {
        Self {
            message_heze: ExecutionPayloadBidHeze::default(),
            signature: Signature::empty(),
        }
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
            message_gloas: ExecutionPayloadBidGloas::default(),
            signature: Signature::empty(),
        })
    }

    pub fn empty_heze() -> Self {
        Self::Heze(SignedExecutionPayloadBidHeze {
            message_heze: ExecutionPayloadBidHeze::default(),
            signature: Signature::empty(),
        })
    }

    /// Get the message as an ExecutionPayloadBid enum.
    /// This is useful for code that needs to work with both Gloas and Heze variants.
    pub fn message(&self) -> ExecutionPayloadBid<E> {
        match self {
            Self::Gloas(bid) => ExecutionPayloadBid::Gloas(bid.message_gloas.clone()),
            Self::Heze(bid) => ExecutionPayloadBid::Heze(bid.message_heze.clone()),
        }
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
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use ssz::{Decode, Encode};
    use crate::test_utils::TestRandom;

    mod signed_execution_payload_bid_gloas {
        use super::*;
        ssz_and_tree_hash_tests!(SignedExecutionPayloadBidGloas<MainnetEthSpec>);
    }

    mod signed_execution_payload_bid_heze {
        use super::*;
        ssz_and_tree_hash_tests!(SignedExecutionPayloadBidHeze<MainnetEthSpec>);
    }

    /// Test that Heze signed bid cannot be incorrectly decoded as Gloas
    #[test]
    fn test_heze_signed_cannot_decode_as_gloas() {
        let mut rng = StdRng::seed_from_u64(42);
        
        // Create a Heze signed bid with random values
        let heze_bid = SignedExecutionPayloadBidHeze::<MainnetEthSpec>::random_for_test(&mut rng);
        let heze_bytes = heze_bid.as_ssz_bytes();
        eprintln!("Heze signed bid bytes length: {}", heze_bytes.len());

        // Create a Gloas signed bid for comparison
        let gloas_bid = SignedExecutionPayloadBidGloas::<MainnetEthSpec>::random_for_test(&mut rng);
        let gloas_bytes = gloas_bid.as_ssz_bytes();
        eprintln!("Gloas signed bid bytes length: {}", gloas_bytes.len());

        // Heze has inclusion_list_bits in the message, so it should be longer than Gloas
        // Attempting to decode Heze bytes as Gloas should fail
        let result = SignedExecutionPayloadBidGloas::<MainnetEthSpec>::from_ssz_bytes(&heze_bytes);
        match &result {
            Ok(gloas) => {
                eprintln!("ERROR: Heze data decoded as Gloas!");
                eprintln!("Re-encoded Gloas bytes length: {}", gloas.as_ssz_bytes().len());
            }
            Err(e) => {
                eprintln!("CORRECT: Heze data cannot be decoded as Gloas: {:?}", e);
            }
        }
        assert!(
            result.is_err(),
            "Heze signed bid should NOT be decodable as Gloas - this would cause data corruption!"
        );
    }

    /// Test that Gloas signed bid can be decoded correctly using the enum decoder
    #[test]
    fn test_gloas_signed_decode_via_enum() {
        let mut rng = StdRng::seed_from_u64(42);
        
        let gloas_bid = SignedExecutionPayloadBidGloas::<MainnetEthSpec>::random_for_test(&mut rng);
        let gloas_bytes = gloas_bid.as_ssz_bytes();

        // Decode via the enum's Decode implementation
        let decoded: SignedExecutionPayloadBid<MainnetEthSpec> =
            SignedExecutionPayloadBid::from_ssz_bytes(&gloas_bytes).expect("should decode");

        // Should be Gloas variant
        assert!(matches!(decoded, SignedExecutionPayloadBid::Gloas(_)));
    }

    /// Test that Heze signed bid can be decoded correctly using the enum decoder
    #[test]
    fn test_heze_signed_decode_via_enum() {
        let mut rng = StdRng::seed_from_u64(42);
        
        let heze_bid = SignedExecutionPayloadBidHeze::<MainnetEthSpec>::random_for_test(&mut rng);
        let heze_bytes = heze_bid.as_ssz_bytes();

        // Decode via the enum's Decode implementation
        let decoded: SignedExecutionPayloadBid<MainnetEthSpec> =
            SignedExecutionPayloadBid::from_ssz_bytes(&heze_bytes).expect("should decode");

        // Should be Heze variant
        assert!(matches!(decoded, SignedExecutionPayloadBid::Heze(_)));
    }
}