//! Inclusion List API endpoints for FOCIL (EIP-7805).
//!
//! Implements the Beacon API endpoints for Inclusion Lists:
//! - GET /v1/beacon/blocks/{block_id}/inclusion_lists
//! - GET /v1/beacon/states/{state_id}/inclusion_list_committee
//! - POST /v1/beacon/pool/inclusion_lists
//!
//! Route definitions are in lib.rs, this module provides the response types.

use eth2::types::SignedInclusionList as ApiSignedInclusionList;
use types::{EthSpec, Hash256};

/// Response for GET /v1/beacon/blocks/{block_id}/inclusion_lists
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(bound = "E: EthSpec")]
pub struct GetInclusionListsResponse<E: EthSpec> {
    pub inclusion_lists: Vec<ApiSignedInclusionList<E>>,
}

/// Response for GET /v1/beacon/states/{state_id}/inclusion_list_committee
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct GetInclusionListCommitteeResponse {
    #[serde(with = "serde_utils::quoted_u64_vec")]
    pub validators: Vec<u64>,
    pub committee_root: Hash256,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_inclusion_lists_response_serialization() {
        let response = GetInclusionListsResponse::<types::MainnetEthSpec> {
            inclusion_lists: vec![],
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("inclusion_lists"));
    }

    #[test]
    fn test_get_inclusion_list_committee_response_serialization() {
        let response = GetInclusionListCommitteeResponse {
            validators: vec![1, 2, 3],
            committee_root: Hash256::default(),
        };
        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("validators"));
        assert!(json.contains("committee_root"));
    }
}
