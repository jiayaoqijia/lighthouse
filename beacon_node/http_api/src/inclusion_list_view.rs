use crate::block_id::BlockId;
use beacon_chain::{BeaconChain, BeaconChainTypes, inclusion_list_store::ObservedInclusionList};
use eth2::types::GenericResponse;
use serde::{Deserialize, Serialize};
use types::{Hash256, Slot};

#[derive(Deserialize, Serialize)]
pub(crate) struct InclusionListView {
    block_root: Hash256,
    slot: Slot,
    dependent_root: Hash256,
    #[serde(with = "serde_utils::quoted_u64_vec")]
    committee: Vec<u64>,
    inclusion_lists: Vec<ObservedInclusionList>,
}

pub(crate) fn for_block<T: BeaconChainTypes>(
    id: BlockId,
    chain: &BeaconChain<T>,
) -> Result<GenericResponse<InclusionListView>, warp::Rejection> {
    let (block, _, _) = id.blinded_block(chain)?;
    if block.slot() == Slot::new(0) {
        return Err(warp_utils::reject::custom_not_found(
            "Genesis has no previous slot".into(),
        ));
    }
    let slot = block.slot().saturating_sub(1u64);
    if !chain
        .spec
        .fork_name_at_slot::<T::EthSpec>(slot)
        .heze_enabled()
    {
        return Err(warp_utils::reject::custom_not_found(
            "Previous slot predates Heze".into(),
        ));
    }

    let (committee, dependent_root) = chain
        .inclusion_list_committee(block.message().parent_root(), slot)
        .map_err(|err| {
            warp_utils::reject::custom_not_found(format!("Committee unavailable: {err:?}"))
        })?;
    let inclusion_lists = chain
        .inclusion_list_store
        .read()
        .observed_inclusion_lists(slot, dependent_root, &committee)
        .ok_or_else(|| warp_utils::reject::custom_not_found("Inclusion-list slot pruned".into()))?;

    Ok(GenericResponse::from(InclusionListView {
        block_root: block.canonical_root(),
        slot,
        dependent_root,
        committee: committee.iter().copied().collect(),
        inclusion_lists,
    }))
}
