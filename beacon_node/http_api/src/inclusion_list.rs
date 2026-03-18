//! Inclusion List API endpoints for FOCIL (EIP-7805).
//!
//! Implements the Beacon API endpoints for Inclusion Lists:
//! - GET /v1/beacon/blocks/{block_id}/inclusion_lists
//! - POST /v1/beacon/pool/inclusion_lists

use crate::{ApiError, ApiResult, Path, Query, Version};
use beacon_chain::{BeaconChain, BeaconChainTypes};
use eth2::types::{InclusionList as ApiInclusionList, SignedInclusionList as ApiSignedInclusionList};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use types::{EthSpec, Hash256, Slot};

/// Response for GET /v1/beacon/blocks/{block_id}/inclusion_lists
#[derive(Debug, Serialize, Deserialize)]
pub struct GetInclusionListsResponse<E: EthSpec> {
    pub inclusion_lists: Vec<ApiSignedInclusionList<E>>,
}

/// Response for GET /v1/beacon/states/{state_id}/inclusion_list_committee
#[derive(Debug, Serialize, Deserialize)]
pub struct GetInclusionListCommitteeResponse {
    #[serde(with = "serde_utils::quoted_u64_vec")]
    pub validators: Vec<u64>,
    pub committee_root: Hash256,
}

/// Get inclusion lists for a block.
///
/// GET /v1/beacon/blocks/{block_id}/inclusion_lists
pub fn get_inclusion_lists<T: BeaconChainTypes>(
    block_id: Path<String>,
    chain: Arc<BeaconChain<T>>,
) -> ApiResult<Version::V1<GetInclusionListsResponse<T::EthSpec>>> {
    // Parse block_id
    let block_id = block_id.into_inner();
    
    // In production, this would fetch the inclusion lists from:
    // 1. The InclusionListStore for pending ILs
    // 2. The block's IL bits for finalized blocks
    // For now, return empty list as this is a stub implementation
    
    let response = GetInclusionListsResponse {
        inclusion_lists: vec![],
    };
    
    Ok(Version::V1(response))
}

/// Get inclusion list committee for a state.
///
/// GET /v1/beacon/states/{state_id}/inclusion_list_committee
pub fn get_inclusion_list_committee<T: BeaconChainTypes>(
    state_id: Path<String>,
    slot: Query<Option<Slot>>,
    chain: Arc<BeaconChain<T>>,
) -> ApiResult<Version::V1<GetInclusionListCommitteeResponse>> {
    let state_id = state_id.into_inner();
    let slot = slot.into_inner().unwrap_or_else(|| chain.slot().unwrap_or_default());
    
    // In production, this would:
    // 1. Get the state at state_id
    // 2. Compute the inclusion list committee for the slot
    // 3. Return the committee and root
    
    // For now, return a stub response
    let response = GetInclusionListCommitteeResponse {
        validators: vec![],
        committee_root: Hash256::zero(),
    };
    
    Ok(Version::V1(response))
}

/// Submit an inclusion list.
///
/// POST /v1/beacon/pool/inclusion_lists
pub fn submit_inclusion_list<T: BeaconChainTypes>(
    signed_inclusion_list: ApiSignedInclusionList<T::EthSpec>,
    chain: Arc<BeaconChain<T>>,
) -> ApiResult<()> {
    // In production, this would:
    // 1. Verify the inclusion list
    // 2. Add it to the InclusionListStore
    // 3. Broadcast it to the gossip network
    
    // For now, just accept it (stub)
    Ok(())
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
}