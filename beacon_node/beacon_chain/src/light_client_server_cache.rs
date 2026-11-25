use crate::errors::BeaconChainError;
use crate::{BeaconChainTypes, BeaconStore, metrics};
use hashlink::lru_cache::LruCache;
use parking_lot::{Mutex, RwLock};
use std::sync::Arc;
use tree_hash::TreeHash;
use types::{
    BeaconBlockRef, BeaconState, ChainSpec, Checkpoint, EthSpec, ForkName, Hash256,
    LightClientBootstrap, LightClientFinalityUpdate, LightClientOptimisticUpdate,
    LightClientUpdate, MerkleProof, Slot, SyncAggregate, SyncCommittee,
};

/// A prev block cache miss requires to re-generate the state of the post-parent block. Items in the
/// prev block cache are very small 32 * (6 + 1) = 224 bytes. 32 is an arbitrary number that
/// represents unlikely re-orgs, while keeping the cache very small.
const PREV_BLOCK_CACHE_SIZE: usize = 32;

type SelectedHead<E> = (Hash256, Arc<canonical::BranchUpdates<E>>);
type WrittenCommittee<E> = (u64, Arc<SyncCommittee<E>>);

/// This cache computes light client messages ahead of time, required to satisfy p2p and API
/// requests. These messages include proofs on historical states, so on-demand computation is
/// expensive.
pub struct LightClientServerCache<T: BeaconChainTypes> {
    /// Latest eligible finality update on the selected canonical branch.
    latest_finality_update: RwLock<Option<LightClientFinalityUpdate<T::EthSpec>>>,
    /// Latest eligible optimistic update on the selected canonical branch.
    latest_optimistic_update: RwLock<Option<LightClientOptimisticUpdate<T::EthSpec>>>,
    branches: Mutex<LruCache<Hash256, Arc<canonical::BranchUpdates<T::EthSpec>>>>,
    selected_head: Mutex<Option<SelectedHead<T::EthSpec>>>,
    /// Caches the current sync committee,
    latest_written_current_sync_committee: RwLock<Option<WrittenCommittee<T::EthSpec>>>,
    /// Caches state proofs by block root
    prev_block_cache: Mutex<LruCache<Hash256, LightClientCachedData<T::EthSpec>>>,
    /// Tracks the latest broadcasted finality update
    latest_broadcasted_finality_update: RwLock<Option<LightClientFinalityUpdate<T::EthSpec>>>,
    /// Tracks the latest broadcasted optimistic update
    latest_broadcasted_optimistic_update: RwLock<Option<LightClientOptimisticUpdate<T::EthSpec>>>,
}

impl<T: BeaconChainTypes> LightClientServerCache<T> {
    pub fn new() -> Self {
        Self {
            latest_finality_update: None.into(),
            latest_optimistic_update: None.into(),
            branches: Mutex::new(LruCache::new(PREV_BLOCK_CACHE_SIZE)),
            selected_head: Mutex::new(None),
            latest_written_current_sync_committee: None.into(),
            latest_broadcasted_finality_update: None.into(),
            latest_broadcasted_optimistic_update: None.into(),
            prev_block_cache: LruCache::new(PREV_BLOCK_CACHE_SIZE).into(),
        }
    }

    /// Compute and cache state proofs for latter production of light-client messages. Does not
    /// trigger block replay.
    pub(crate) fn cache_state_data(
        &self,
        spec: &ChainSpec,
        block: BeaconBlockRef<T::EthSpec>,
        block_root: Hash256,
        block_post_state: &mut BeaconState<T::EthSpec>,
    ) -> Result<(), BeaconChainError> {
        let _timer = metrics::start_timer(&metrics::LIGHT_CLIENT_SERVER_CACHE_STATE_DATA_TIMES);
        let fork_name = spec.fork_name_at_slot::<T::EthSpec>(block.slot());
        // Only post-altair
        if fork_name.altair_enabled() {
            // Persist in memory cache for a descendent block
            let cached_data = LightClientCachedData::from_state(block_post_state)?;
            self.prev_block_cache.lock().insert(block_root, cached_data);
        }

        Ok(())
    }

    /// Construct a branch candidate and retain bootstrap proof data. Publication
    /// is separate so noncanonical branches cannot change the serving view.
    fn build_update(
        &self,
        store: BeaconStore<T>,
        block_slot: Slot,
        block_parent_root: &Hash256,
        sync_aggregate: &SyncAggregate<T::EthSpec>,
        chain_spec: &ChainSpec,
    ) -> Result<Option<canonical::Update<T::EthSpec>>, BeaconChainError> {
        metrics::inc_counter(&metrics::LIGHT_CLIENT_SERVER_CACHE_PROCESSING_REQUESTS);
        let _timer =
            metrics::start_timer(&metrics::LIGHT_CLIENT_SERVER_CACHE_RECOMPUTE_UPDATES_TIMES);

        let attested_block_root = block_parent_root;

        let attested_block = store.get_blinded_block(attested_block_root)?.ok_or(
            BeaconChainError::DBInconsistent(format!(
                "Block not available {:?}",
                attested_block_root
            )),
        )?;

        if !chain_spec
            .fork_name_at_slot::<T::EthSpec>(attested_block.slot())
            .altair_enabled()
        {
            return Ok(None);
        }

        let cached_parts = self.get_or_compute_prev_block_cache(
            store.clone(),
            attested_block_root,
            &attested_block.state_root(),
            attested_block.slot(),
        )?;

        let finalized_period = cached_parts
            .finalized_checkpoint
            .epoch
            .sync_committee_period(chain_spec)?;

        store.store_sync_committee_branch(
            attested_block.message().tree_hash_root(),
            &cached_parts.current_sync_committee_branch,
        )?;

        let attested_period = attested_block
            .slot()
            .epoch(T::EthSpec::slots_per_epoch())
            .sync_committee_period(chain_spec)?;
        self.store_current_sync_committee(
            &store,
            &cached_parts,
            attested_period,
            finalized_period,
        )?;

        // Bootstrap committee data above remains useful even if no eligible
        // update can be formed from this block's aggregate.
        if (sync_aggregate.sync_committee_bits.num_set_bits() as u64)
            < chain_spec.min_sync_committee_participants
        {
            return Ok(None);
        }

        let maybe_finalized_block = store.get_blinded_block(&cached_parts.finalized_block_root)?;
        let finality_available =
            cached_parts.finalized_block_root.is_zero() || maybe_finalized_block.is_some();
        let sync_period = block_slot
            .epoch(T::EthSpec::slots_per_epoch())
            .sync_committee_period(chain_spec)?;
        let same_period = attested_period == sync_period;
        let (next_committee, next_branch) = if same_period {
            (
                cached_parts.next_sync_committee.clone(),
                cached_parts.next_sync_committee_branch.clone(),
            )
        } else {
            (
                Arc::new(SyncCommittee::temporary()),
                vec![Hash256::ZERO; cached_parts.next_sync_committee_branch.len()],
            )
        };
        let finality_branch = if finality_available {
            cached_parts.finality_branch.clone()
        } else {
            vec![Hash256::ZERO; cached_parts.finality_branch.len()]
        };
        let new_light_client_update = LightClientUpdate::new(
            sync_aggregate,
            block_slot,
            next_committee,
            next_branch,
            finality_branch,
            &attested_block,
            maybe_finalized_block.as_ref(),
            chain_spec,
        )?;

        metrics::inc_counter(&metrics::LIGHT_CLIENT_SERVER_CACHE_PROCESSING_SUCCESSES);
        Ok(Some(canonical::Update {
            update: Arc::new(new_light_client_update),
            finality_available,
            period: same_period.then_some(sync_period),
        }))
    }

    /// Isolated verifier fixtures assemble a valid sync aggregate before its
    /// signature block exists. This component helper is never the production
    /// serving path; native nodes publish only `recompute_for_head` results.
    pub(crate) fn cache_aggregate_for_test(
        &self,
        store: BeaconStore<T>,
        slot: Slot,
        parent: &Hash256,
        aggregate: &SyncAggregate<T::EthSpec>,
        spec: &ChainSpec,
    ) -> Result<(), BeaconChainError> {
        if let Some(built) = self.build_update(store.clone(), slot, parent, aggregate, spec)? {
            *self.latest_optimistic_update.write() = Some((*built.update).clone().into());
            if built.finality_available {
                *self.latest_finality_update.write() = Some((*built.update).clone().into());
            }
            if let Some(period) = built.period {
                let better = store
                    .get_light_client_update(period)?
                    .map(|old| old.is_better_light_client_update(&built.update, spec))
                    .transpose()?
                    .unwrap_or(true);
                if better {
                    store.store_light_client_update(period, &built.update)?;
                }
            }
        }
        Ok(())
    }

    fn store_current_sync_committee(
        &self,
        store: &BeaconStore<T>,
        cached_parts: &LightClientCachedData<T::EthSpec>,
        sync_committee_period: u64,
        finalized_period: u64,
    ) -> Result<(), BeaconChainError> {
        if let Some((latest_period, latest_sync_committee)) =
            self.latest_written_current_sync_committee.read().clone()
            && latest_period == sync_committee_period
            && latest_sync_committee == cached_parts.current_sync_committee
        {
            return Ok(());
        };

        if finalized_period + 1 >= sync_committee_period {
            store.store_sync_committee(
                sync_committee_period,
                &cached_parts.current_sync_committee,
            )?;
            *self.latest_written_current_sync_committee.write() = Some((
                sync_committee_period,
                cached_parts.current_sync_committee.clone(),
            ));
        }

        Ok(())
    }

    /// Retrieves prev block cached data from cache. If not present re-computes by retrieving the
    /// parent state, and inserts an entry to the cache.
    ///
    /// In separate function since FnOnce of get_or_insert can not be fallible.
    fn get_or_compute_prev_block_cache(
        &self,
        store: BeaconStore<T>,
        block_root: &Hash256,
        block_state_root: &Hash256,
        block_slot: Slot,
    ) -> Result<LightClientCachedData<T::EthSpec>, BeaconChainError> {
        // Attempt to get the value from the cache first.
        if let Some(cached_parts) = self.prev_block_cache.lock().get(block_root) {
            return Ok(cached_parts.clone());
        }
        metrics::inc_counter(&metrics::LIGHT_CLIENT_SERVER_CACHE_PREV_BLOCK_CACHE_MISS);

        // Compute the value, handling potential errors.
        // This state should already be cached. By electing not to cache it here
        // we remove any chance of the light client server from affecting the state cache.
        // We'd like the light client server to be as minimally invasive as possible.
        let mut state = store
            .get_state(block_state_root, Some(block_slot), false)?
            .ok_or_else(|| {
                BeaconChainError::DBInconsistent(format!("Missing state {:?}", block_state_root))
            })?;
        let new_value = LightClientCachedData::from_state(&mut state)?;

        // Insert value and return owned
        self.prev_block_cache
            .lock()
            .insert(*block_root, new_value.clone());
        Ok(new_value)
    }

    /// Checks if we've already broadcasted the latest finality update.
    /// If we haven't, update the `latest_broadcasted_finality_update` cache
    /// and return the latest finality update for broadcasting, else return `None`.
    pub fn should_broadcast_latest_finality_update(
        &self,
    ) -> Option<LightClientFinalityUpdate<T::EthSpec>> {
        if let Some(latest_finality_update) = self.get_latest_finality_update() {
            let latest_broadcasted_finality_update = self.get_latest_broadcasted_finality_update();
            match latest_broadcasted_finality_update {
                Some(latest_broadcasted_finality_update) => {
                    if latest_broadcasted_finality_update != latest_finality_update {
                        self.set_latest_broadcasted_finality_update(latest_finality_update.clone());
                        return Some(latest_finality_update);
                    }
                }
                None => {
                    self.set_latest_broadcasted_finality_update(latest_finality_update.clone());
                    return Some(latest_finality_update);
                }
            }
        }

        None
    }

    pub fn get_latest_finality_update(&self) -> Option<LightClientFinalityUpdate<T::EthSpec>> {
        self.latest_finality_update.read().clone()
    }

    pub fn get_latest_broadcasted_optimistic_update(
        &self,
    ) -> Option<LightClientOptimisticUpdate<T::EthSpec>> {
        self.latest_broadcasted_optimistic_update.read().clone()
    }

    pub fn get_latest_broadcasted_finality_update(
        &self,
    ) -> Option<LightClientFinalityUpdate<T::EthSpec>> {
        self.latest_broadcasted_finality_update.read().clone()
    }

    pub fn set_latest_broadcasted_optimistic_update(
        &self,
        optimistic_update: LightClientOptimisticUpdate<T::EthSpec>,
    ) {
        *self.latest_broadcasted_optimistic_update.write() = Some(optimistic_update.clone());
    }

    pub fn set_latest_broadcasted_finality_update(
        &self,
        finality_update: LightClientFinalityUpdate<T::EthSpec>,
    ) {
        *self.latest_broadcasted_finality_update.write() = Some(finality_update.clone());
    }

    /// Checks if we've already broadcasted the latest optimistic update.
    /// If we haven't, update the `latest_broadcasted_optimistic_update` cache
    /// and return the latest optimistic update for broadcasting, else return `None`.
    pub fn should_broadcast_latest_optimistic_update(
        &self,
    ) -> Option<LightClientOptimisticUpdate<T::EthSpec>> {
        if let Some(latest_optimistic_update) = self.get_latest_optimistic_update() {
            let latest_broadcasted_optimistic_update =
                self.get_latest_broadcasted_optimistic_update();
            match latest_broadcasted_optimistic_update {
                Some(latest_broadcasted_optimistic_update) => {
                    if latest_broadcasted_optimistic_update != latest_optimistic_update {
                        self.set_latest_broadcasted_optimistic_update(
                            latest_optimistic_update.clone(),
                        );
                        return Some(latest_optimistic_update);
                    }
                }
                None => {
                    self.set_latest_broadcasted_optimistic_update(latest_optimistic_update.clone());
                    return Some(latest_optimistic_update);
                }
            }
        }

        None
    }

    pub fn get_latest_optimistic_update(&self) -> Option<LightClientOptimisticUpdate<T::EthSpec>> {
        self.latest_optimistic_update.read().clone()
    }

    /// Fetches a light client bootstrap for a given finalized checkpoint `block_root`. We eagerly persist
    /// `sync_committee_branch and `sync_committee` to allow for a more efficient bootstrap construction.
    ///
    /// Missing proof data can be recovered while the block's authenticated immediate post-state
    /// remains available, including checkpoints affected by older pruning behavior.
    #[allow(clippy::type_complexity)]
    pub fn get_light_client_bootstrap(
        &self,
        store: &BeaconStore<T>,
        block_root: &Hash256,
        finalized_period: u64,
        chain_spec: &ChainSpec,
    ) -> Result<Option<(LightClientBootstrap<T::EthSpec>, ForkName)>, BeaconChainError> {
        let Some(block) = store.get_blinded_block(block_root)? else {
            return Ok(None);
        };

        let (_, slot) = (block.state_root(), block.slot());

        let fork_name = chain_spec.fork_name_at_slot::<T::EthSpec>(slot);

        let sync_committee_period = block
            .slot()
            .epoch(T::EthSpec::slots_per_epoch())
            .sync_committee_period(chain_spec)?;

        if sync_committee_period > finalized_period {
            return Err(BeaconChainError::LightClientBootstrapError(format!(
                "The blocks sync committee period {sync_committee_period} is greater than the current finalized period {finalized_period}"
            )));
        }

        let branch = store.get_sync_committee_branch(block_root)?;
        let committee = store.get_sync_committee(sync_committee_period)?;
        let (current_sync_committee_branch, current_sync_committee) = match (branch, committee) {
            (Some(branch), Some(committee))
                if merkle_proof::verify_merkle_proof(
                    committee.tree_hash_root(),
                    &branch,
                    branch.len(),
                    types::light_client::consts::CURRENT_SYNC_COMMITTEE_FIELD_INDEX,
                    block.state_root(),
                ) =>
            {
                (branch, Arc::new(committee))
            }
            _ => {
                // Recover previously pruned proof data from this block's immediate post-state.
                let Some(mut state) = store.get_state(&block.state_root(), Some(slot), false)?
                else {
                    return Err(BeaconChainError::LightClientBootstrapError(format!(
                        "Post-state for bootstrap block {block_root:?} is unavailable"
                    )));
                };
                if state.slot() != slot || state.canonical_root()? != block.state_root() {
                    return Err(BeaconChainError::LightClientBootstrapError(format!(
                        "Post-state does not match bootstrap block {block_root:?}"
                    )));
                }
                let mut header = state.latest_block_header().clone();
                header.state_root = block.state_root();
                if header.canonical_root() != *block_root {
                    return Err(BeaconChainError::LightClientBootstrapError(format!(
                        "Post-state header does not match bootstrap block {block_root:?}"
                    )));
                }
                let branch = state.compute_current_sync_committee_proof()?;
                let committee = state.current_sync_committee()?.clone();
                store.store_sync_committee_branch(*block_root, &branch)?;
                // A requested side branch must not replace the global period committee.
                (branch, committee)
            }
        };

        let light_client_bootstrap = LightClientBootstrap::new(
            &block,
            current_sync_committee,
            current_sync_committee_branch,
            chain_spec,
        )?;

        Ok(Some((light_client_bootstrap, fork_name)))
    }
}

impl<T: BeaconChainTypes> Default for LightClientServerCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct LightClientCachedData<E: EthSpec> {
    finalized_checkpoint: Checkpoint,
    finality_branch: MerkleProof,
    next_sync_committee_branch: MerkleProof,
    current_sync_committee_branch: MerkleProof,
    next_sync_committee: Arc<SyncCommittee<E>>,
    current_sync_committee: Arc<SyncCommittee<E>>,
    finalized_block_root: Hash256,
}

impl<E: EthSpec> LightClientCachedData<E> {
    fn from_state(state: &mut BeaconState<E>) -> Result<Self, BeaconChainError> {
        Ok(Self {
            finalized_checkpoint: state.finalized_checkpoint(),
            finality_branch: state.compute_finalized_root_proof()?,
            next_sync_committee: state.next_sync_committee()?.clone(),
            current_sync_committee: state.current_sync_committee()?.clone(),
            next_sync_committee_branch: state.compute_next_sync_committee_proof()?,
            current_sync_committee_branch: state.compute_current_sync_committee_proof()?,
            finalized_block_root: state.finalized_checkpoint().root,
        })
    }
}

#[cfg(test)]
#[path = "light_client_collection_tests.rs"]
mod current_reference_tests;

#[path = "light_client_canonical_cache.rs"]
mod canonical;
