//! Defines the `BeaconForkChoiceStore` which provides the persistent storage for the `ForkChoice`
//! struct.
//!
//! Additionally, the `BalancesCache` struct is defined; a cache designed to avoid database
//! reads when fork choice requires the validator balances of the justified state.

use crate::{BeaconSnapshot, metrics};
use educe::Educe;
use fixed_bytes::FixedBytesExtended;
use fork_choice::ForkChoiceStore;
use proto_array::JustifiedBalances;
use safe_arith::ArithError;
use ssz_derive::{Decode, Encode};
use std::collections::{BTreeSet, HashMap};
use std::marker::PhantomData;
use std::sync::Arc;
use store::{Error as StoreError, HotColdDB, ItemStore};
use superstruct::superstruct;
use types::{
    AbstractExecPayload, BeaconBlockRef, BeaconState, BeaconStateError, Checkpoint, Epoch, EthSpec,
    Hash256, Slot,
};

#[derive(Debug)]
pub enum Error {
    FailedToReadBlock(StoreError),
    MissingBlock(Hash256),
    FailedToReadState(StoreError),
    MissingState(Hash256),
    BeaconStateError(BeaconStateError),
    UnalignedCheckpoint { block_slot: Slot, state_slot: Slot },
    Arith(ArithError),
}

impl From<BeaconStateError> for Error {
    fn from(e: BeaconStateError) -> Self {
        Error::BeaconStateError(e)
    }
}

impl From<ArithError> for Error {
    fn from(e: ArithError) -> Self {
        Error::Arith(e)
    }
}

/// The number of validator balance sets that are cached within `BalancesCache`.
const MAX_BALANCE_CACHE_SIZE: usize = 4;

fn has_two_thirds_supermajority(true_count: usize, committee_size: usize) -> bool {
    true_count.saturating_mul(3) >= committee_size.saturating_mul(2)
}

#[superstruct(
    variants(V8),
    variant_attributes(derive(PartialEq, Clone, Debug, Encode, Decode)),
    no_enum
)]
pub(crate) struct CacheItem {
    pub(crate) block_root: Hash256,
    pub(crate) epoch: Epoch,
    pub(crate) balances: Vec<u64>,
}

pub(crate) type CacheItem = CacheItemV8;

#[superstruct(
    variants(V8),
    variant_attributes(derive(PartialEq, Clone, Default, Debug, Encode, Decode)),
    no_enum
)]
pub struct BalancesCache {
    pub(crate) items: Vec<CacheItemV8>,
}

pub type BalancesCache = BalancesCacheV8;

impl BalancesCache {
    /// Inspect the given `state` and determine the root of the block at the first slot of
    /// `state.current_epoch`. If there is not already some entry for the given block root, then
    /// add the effective balances from the `state` to the cache.
    pub fn process_state<E: EthSpec>(
        &mut self,
        block_root: Hash256,
        state: &BeaconState<E>,
    ) -> Result<(), Error> {
        let epoch = state.current_epoch();
        let epoch_boundary_slot = epoch.start_slot(E::slots_per_epoch());
        let epoch_boundary_root = if epoch_boundary_slot == state.slot() {
            block_root
        } else {
            // This call remains sensible as long as `state.block_roots` is larger than a single
            // epoch.
            *state.get_block_root(epoch_boundary_slot)?
        };

        // Check if there already exists a cache entry for the epoch boundary block of the current
        // epoch. We rely on the invariant that effective balances do not change for the duration
        // of a single epoch, so even if the block on the epoch boundary itself is skipped we can
        // still update its cache entry from any subsequent state in that epoch.
        if self.position(epoch_boundary_root, epoch).is_none() {
            let item = CacheItem {
                block_root: epoch_boundary_root,
                epoch,
                balances: JustifiedBalances::from_justified_state(state)?.effective_balances,
            };

            if self.items.len() == MAX_BALANCE_CACHE_SIZE {
                self.items.remove(0);
            }

            self.items.push(item);
        }

        Ok(())
    }

    fn position(&self, block_root: Hash256, epoch: Epoch) -> Option<usize> {
        self.items
            .iter()
            .position(|item| item.block_root == block_root && item.epoch == epoch)
    }

    /// Get the balances for the given `block_root`, if any.
    ///
    /// If some balances are found, they are cloned from the cache.
    pub fn get(&mut self, block_root: Hash256, epoch: Epoch) -> Option<Vec<u64>> {
        let i = self.position(block_root, epoch)?;
        Some(self.items[i].balances.clone())
    }
}

/// Implements `fork_choice::ForkChoiceStore` in order to provide a persistent backing to the
/// `fork_choice::ForkChoice` struct.
#[derive(Debug, Educe)]
#[educe(PartialEq(bound(E: EthSpec, Hot: ItemStore<E>, Cold: ItemStore<E>)))]
pub struct BeaconForkChoiceStore<E: EthSpec, Hot: ItemStore<E>, Cold: ItemStore<E>> {
    #[educe(PartialEq(ignore))]
    store: Arc<HotColdDB<E, Hot, Cold>>,
    balances_cache: BalancesCache,
    time: Slot,
    finalized_checkpoint: Checkpoint,
    justified_checkpoint: Checkpoint,
    justified_balances: JustifiedBalances,
    justified_state_root: Hash256,
    unrealized_justified_checkpoint: Checkpoint,
    unrealized_justified_state_root: Hash256,
    unrealized_finalized_checkpoint: Checkpoint,
    proposer_boost_root: Hash256,
    equivocating_indices: BTreeSet<u64>,
    /// [New in Heze:EIP7805] Tracks whether execution payloads satisfy inclusion list constraints.
    /// Maps block root -> satisfaction status.
    payload_inclusion_list_satisfaction: HashMap<Hash256, bool>,
    /// [New in Gloas:EIP7732] PTC votes for payload timeliness.
    /// Maps block root -> bitvector of PTC votes (true = payload present vote).
    payload_timeliness_vote: HashMap<Hash256, Vec<bool>>,
    /// [New in Gloas:EIP7732] PTC votes for data availability.
    /// Maps block root -> bitvector of PTC votes (true = data available vote).
    payload_data_availability_vote: HashMap<Hash256, Vec<bool>>,
    _phantom: PhantomData<E>,
}

impl<E, Hot, Cold> BeaconForkChoiceStore<E, Hot, Cold>
where
    E: EthSpec,
    Hot: ItemStore<E>,
    Cold: ItemStore<E>,
{
    /// Initialize `Self` from some `anchor` checkpoint which may or may not be the genesis state.
    ///
    /// ## Specification
    ///
    /// Equivalent to:
    ///
    /// https://github.com/ethereum/eth2.0-specs/blob/v0.12.1/specs/phase0/fork-choice.md#get_forkchoice_store
    ///
    /// ## Notes:
    ///
    /// It is assumed that `anchor` is already persisted in `store`.
    pub fn get_forkchoice_store(
        store: Arc<HotColdDB<E, Hot, Cold>>,
        anchor: BeaconSnapshot<E>,
    ) -> Result<Self, Error> {
        let unadvanced_state_root = anchor.beacon_state_root();
        let mut anchor_state = anchor.beacon_state;
        let mut anchor_block_header = anchor_state.latest_block_header().clone();

        // The anchor state MUST be on an epoch boundary (it should be advanced by the caller).
        if !anchor_state
            .slot()
            .as_u64()
            .is_multiple_of(E::slots_per_epoch())
        {
            return Err(Error::UnalignedCheckpoint {
                block_slot: anchor_block_header.slot,
                state_slot: anchor_state.slot(),
            });
        }

        // Compute the accurate block root for the checkpoint block.
        if anchor_block_header.state_root.is_zero() {
            anchor_block_header.state_root = unadvanced_state_root;
        }
        let anchor_block_root = anchor_block_header.canonical_root();
        let anchor_epoch = anchor_state.current_epoch();
        let justified_checkpoint = Checkpoint {
            epoch: anchor_epoch,
            root: anchor_block_root,
        };
        let finalized_checkpoint = justified_checkpoint;
        let justified_balances = JustifiedBalances::from_justified_state(&anchor_state)?;
        let justified_state_root = anchor_state.canonical_root()?;

        Ok(Self {
            store,
            balances_cache: <_>::default(),
            time: anchor_state.slot(),
            justified_checkpoint,
            justified_balances,
            justified_state_root,
            finalized_checkpoint,
            unrealized_justified_checkpoint: justified_checkpoint,
            unrealized_justified_state_root: justified_state_root,
            unrealized_finalized_checkpoint: finalized_checkpoint,
            proposer_boost_root: Hash256::zero(),
            equivocating_indices: BTreeSet::new(),
            // [New in Heze:EIP7805] Initialize with anchor block root as satisfied
            payload_inclusion_list_satisfaction: [(anchor_block_root, true)].into_iter().collect(),
            // [New in Gloas:EIP7732] Initialize anchor block with all PTC votes as true
            payload_timeliness_vote: [(anchor_block_root, vec![true; E::ptc_size()])]
                .into_iter()
                .collect(),
            payload_data_availability_vote: [(anchor_block_root, vec![true; E::ptc_size()])]
                .into_iter()
                .collect(),
            _phantom: PhantomData,
        })
    }

    /// Save the current state of `Self` to a `PersistedForkChoiceStore` which can be stored to the
    /// on-disk database.
    pub fn to_persisted(&self) -> PersistedForkChoiceStore {
        // Convert HashMap to Vec for SSZ serialization
        let il_satisfaction: Vec<(Hash256, bool)> = self
            .payload_inclusion_list_satisfaction
            .iter()
            .map(|(&k, &v)| (k, v))
            .collect();

        // [New in Gloas:EIP7732] Convert PTC votes to Vec for SSZ serialization
        let ptc_timeliness: Vec<(Hash256, Vec<bool>)> = self
            .payload_timeliness_vote
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect();

        let ptc_data_availability: Vec<(Hash256, Vec<bool>)> = self
            .payload_data_availability_vote
            .iter()
            .map(|(&k, v)| (k, v.clone()))
            .collect();

        PersistedForkChoiceStore {
            time: self.time,
            finalized_checkpoint: self.finalized_checkpoint,
            justified_checkpoint: self.justified_checkpoint,
            justified_state_root: self.justified_state_root,
            unrealized_justified_checkpoint: self.unrealized_justified_checkpoint,
            unrealized_justified_state_root: self.unrealized_justified_state_root,
            unrealized_finalized_checkpoint: self.unrealized_finalized_checkpoint,
            proposer_boost_root: self.proposer_boost_root,
            equivocating_indices: self.equivocating_indices.clone(),
            // [New in Heze:EIP7805]
            payload_inclusion_list_satisfaction: il_satisfaction,
            // [New in Gloas:EIP7732]
            payload_timeliness_vote: ptc_timeliness,
            payload_data_availability_vote: ptc_data_availability,
        }
    }

    /// Restore `Self` from a previously-generated `PersistedForkChoiceStore`.
    ///
    /// DEPRECATED. Can be deleted once migrations no longer require it.
    pub fn from_persisted_v17(
        persisted: PersistedForkChoiceStoreV17,
        justified_state_root: Hash256,
        unrealized_justified_state_root: Hash256,
        store: Arc<HotColdDB<E, Hot, Cold>>,
    ) -> Result<Self, Error> {
        let justified_balances =
            JustifiedBalances::from_effective_balances(persisted.justified_balances)?;

        Ok(Self {
            store,
            balances_cache: <_>::default(),
            time: persisted.time,
            finalized_checkpoint: persisted.finalized_checkpoint,
            justified_checkpoint: persisted.justified_checkpoint,
            justified_balances,
            justified_state_root,
            unrealized_justified_checkpoint: persisted.unrealized_justified_checkpoint,
            unrealized_justified_state_root,
            unrealized_finalized_checkpoint: persisted.unrealized_finalized_checkpoint,
            proposer_boost_root: persisted.proposer_boost_root,
            equivocating_indices: persisted.equivocating_indices,
            // [New in Heze:EIP7805] Default to empty for migrated stores
            payload_inclusion_list_satisfaction: HashMap::new(),
            // [New in Gloas:EIP7732] Default to empty for migrated stores
            payload_timeliness_vote: HashMap::new(),
            payload_data_availability_vote: HashMap::new(),
            _phantom: PhantomData,
        })
    }

    /// Restore `Self` from a previously-generated `PersistedForkChoiceStore`.
    pub fn from_persisted(
        persisted: PersistedForkChoiceStore,
        store: Arc<HotColdDB<E, Hot, Cold>>,
    ) -> Result<Self, Error> {
        let justified_checkpoint = persisted.justified_checkpoint;
        let justified_state_root = persisted.justified_state_root;

        let update_cache = true;
        let justified_state = store
            .get_hot_state(&justified_state_root, update_cache)
            .map_err(Error::FailedToReadState)?
            .ok_or(Error::MissingState(justified_state_root))?;

        let justified_balances = JustifiedBalances::from_justified_state(&justified_state)?;

        // Convert Vec back to HashMap
        let il_satisfaction: HashMap<Hash256, bool> = persisted
            .payload_inclusion_list_satisfaction
            .into_iter()
            .collect();

        // [New in Gloas:EIP7732] Convert PTC votes back to HashMap
        let ptc_timeliness: HashMap<Hash256, Vec<bool>> =
            persisted.payload_timeliness_vote.into_iter().collect();

        let ptc_data_availability: HashMap<Hash256, Vec<bool>> = persisted
            .payload_data_availability_vote
            .into_iter()
            .collect();

        Ok(Self {
            store,
            balances_cache: <_>::default(),
            time: persisted.time,
            finalized_checkpoint: persisted.finalized_checkpoint,
            justified_checkpoint,
            justified_balances,
            justified_state_root,
            unrealized_justified_checkpoint: persisted.unrealized_justified_checkpoint,
            unrealized_justified_state_root: persisted.unrealized_justified_state_root,
            unrealized_finalized_checkpoint: persisted.unrealized_finalized_checkpoint,
            proposer_boost_root: persisted.proposer_boost_root,
            equivocating_indices: persisted.equivocating_indices,
            // [New in Heze:EIP7805]
            payload_inclusion_list_satisfaction: il_satisfaction,
            // [New in Gloas:EIP7732]
            payload_timeliness_vote: ptc_timeliness,
            payload_data_availability_vote: ptc_data_availability,
            _phantom: PhantomData,
        })
    }
}

impl<E, Hot, Cold> ForkChoiceStore<E> for BeaconForkChoiceStore<E, Hot, Cold>
where
    E: EthSpec,
    Hot: ItemStore<E>,
    Cold: ItemStore<E>,
{
    type Error = Error;

    fn get_current_slot(&self) -> Slot {
        self.time
    }

    fn set_current_slot(&mut self, slot: Slot) {
        self.time = slot
    }

    fn on_verified_block<Payload: AbstractExecPayload<E>>(
        &mut self,
        _block: BeaconBlockRef<E, Payload>,
        block_root: Hash256,
        state: &BeaconState<E>,
    ) -> Result<(), Self::Error> {
        self.balances_cache.process_state(block_root, state)
    }

    fn justified_checkpoint(&self) -> &Checkpoint {
        &self.justified_checkpoint
    }

    fn justified_state_root(&self) -> Hash256 {
        self.justified_state_root
    }

    fn justified_balances(&self) -> &JustifiedBalances {
        &self.justified_balances
    }

    fn finalized_checkpoint(&self) -> &Checkpoint {
        &self.finalized_checkpoint
    }

    fn unrealized_justified_checkpoint(&self) -> &Checkpoint {
        &self.unrealized_justified_checkpoint
    }

    fn unrealized_justified_state_root(&self) -> Hash256 {
        self.unrealized_justified_state_root
    }

    fn unrealized_finalized_checkpoint(&self) -> &Checkpoint {
        &self.unrealized_finalized_checkpoint
    }

    fn proposer_boost_root(&self) -> Hash256 {
        self.proposer_boost_root
    }

    fn set_finalized_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.finalized_checkpoint = checkpoint
    }

    fn set_justified_checkpoint(
        &mut self,
        checkpoint: Checkpoint,
        justified_state_root: Hash256,
    ) -> Result<(), Error> {
        self.justified_checkpoint = checkpoint;
        self.justified_state_root = justified_state_root;

        if let Some(balances) = self.balances_cache.get(
            self.justified_checkpoint.root,
            self.justified_checkpoint.epoch,
        ) {
            // NOTE: could avoid this re-calculation by introducing a `PersistedCacheItem`.
            metrics::inc_counter(&metrics::BALANCES_CACHE_HITS);
            self.justified_balances = JustifiedBalances::from_effective_balances(balances)?;
        } else {
            metrics::inc_counter(&metrics::BALANCES_CACHE_MISSES);

            // Justified state is reasonably useful to cache, it might be finalized soon.
            let update_cache = true;
            let state = self
                .store
                .get_hot_state(&self.justified_state_root, update_cache)
                .map_err(Error::FailedToReadState)?
                .ok_or(Error::MissingState(self.justified_state_root))?;

            self.justified_balances = JustifiedBalances::from_justified_state(&state)?;
        }

        Ok(())
    }

    fn set_unrealized_justified_checkpoint(&mut self, checkpoint: Checkpoint, state_root: Hash256) {
        self.unrealized_justified_checkpoint = checkpoint;
        self.unrealized_justified_state_root = state_root;
    }

    fn set_unrealized_finalized_checkpoint(&mut self, checkpoint: Checkpoint) {
        self.unrealized_finalized_checkpoint = checkpoint;
    }

    fn set_proposer_boost_root(&mut self, proposer_boost_root: Hash256) {
        self.proposer_boost_root = proposer_boost_root;
    }

    fn equivocating_indices(&self) -> &BTreeSet<u64> {
        &self.equivocating_indices
    }

    fn extend_equivocating_indices(&mut self, indices: impl IntoIterator<Item = u64>) {
        self.equivocating_indices.extend(indices);
    }

    /// [New in Heze:EIP7805] Check if the payload at the given root satisfies IL constraints.
    fn is_payload_inclusion_list_satisfied(&self, block_root: Hash256) -> Option<bool> {
        self.payload_inclusion_list_satisfaction
            .get(&block_root)
            .copied()
    }

    /// [New in Heze:EIP7805] Record IL satisfaction status for a payload.
    fn set_payload_inclusion_list_satisfaction(&mut self, block_root: Hash256, satisfied: bool) {
        self.payload_inclusion_list_satisfaction
            .insert(block_root, satisfied);
    }

    /// [New in Heze:EIP7805] Get the entire payload_inclusion_list_satisfaction map.
    fn payload_inclusion_list_satisfaction(&self) -> &HashMap<Hash256, bool> {
        &self.payload_inclusion_list_satisfaction
    }

    // ===== [New in Gloas:EIP7732] PTC (Payload Timeliness Committee) methods =====

    /// [New in Gloas:EIP7732] Get the PTC timeliness votes for a block root.
    fn payload_timeliness_vote(&self, block_root: Hash256) -> Option<&Vec<bool>> {
        self.payload_timeliness_vote.get(&block_root)
    }

    /// [New in Gloas:EIP7732] Record a PTC timeliness vote from a committee member.
    fn set_payload_timeliness_vote(&mut self, block_root: Hash256, index: usize, vote: bool) {
        let votes = self
            .payload_timeliness_vote
            .entry(block_root)
            .or_insert_with(|| vec![false; E::ptc_size()]);
        if index < votes.len() {
            votes[index] = vote;
        }
    }

    /// [New in Gloas:EIP7732] Get the PTC data availability votes for a block root.
    fn payload_data_availability_vote(&self, block_root: Hash256) -> Option<&Vec<bool>> {
        self.payload_data_availability_vote.get(&block_root)
    }

    /// [New in Gloas:EIP7732] Record a PTC data availability vote from a committee member.
    fn set_payload_data_availability_vote(
        &mut self,
        block_root: Hash256,
        index: usize,
        vote: bool,
    ) {
        let votes = self
            .payload_data_availability_vote
            .entry(block_root)
            .or_insert_with(|| vec![false; E::ptc_size()]);
        if index < votes.len() {
            votes[index] = vote;
        }
    }

    /// [New in Gloas:EIP7732] Check if payload is timely based on PTC votes.
    /// Returns true if >= 2/3 of PTC voted for timeliness.
    fn is_payload_timely(&self, ptc_size: usize, block_root: Hash256) -> bool {
        match self.payload_timeliness_vote.get(&block_root) {
            Some(votes) => {
                let true_count = votes.iter().filter(|&&v| v).count();
                has_two_thirds_supermajority(true_count, ptc_size)
            }
            None => false,
        }
    }

    /// [New in Gloas:EIP7732] Check if payload data is available based on PTC votes.
    /// Returns true if >= 2/3 of PTC voted for data availability.
    fn is_payload_data_available(&self, ptc_size: usize, block_root: Hash256) -> bool {
        match self.payload_data_availability_vote.get(&block_root) {
            Some(votes) => {
                let true_count = votes.iter().filter(|&&v| v).count();
                has_two_thirds_supermajority(true_count, ptc_size)
            }
            None => false,
        }
    }

    /// [New in Gloas:EIP7732] Initialize PTC voting arrays for a new block.
    ///
    /// Uses `or_insert_with` to avoid overwriting votes that may have been recorded
    /// before block processing (e.g., if PTC attestation messages arrived early).
    fn initialize_ptc_votes(&mut self, block_root: Hash256, ptc_size: usize) {
        // Only initialize if not already present (votes may have been recorded earlier)
        self.payload_timeliness_vote
            .entry(block_root)
            .or_insert_with(|| vec![false; ptc_size]);
        self.payload_data_availability_vote
            .entry(block_root)
            .or_insert_with(|| vec![false; ptc_size]);
    }
}

pub type PersistedForkChoiceStore = PersistedForkChoiceStoreV28;

#[cfg(test)]
mod tests {
    use super::has_two_thirds_supermajority;

    #[test]
    fn supermajority_helper_accepts_exact_two_thirds() {
        assert!(has_two_thirds_supermajority(2, 3));
        assert!(has_two_thirds_supermajority(4, 6));
    }

    #[test]
    fn supermajority_helper_rejects_less_than_two_thirds() {
        assert!(!has_two_thirds_supermajority(1, 2));
        assert!(!has_two_thirds_supermajority(3, 5));
    }
}

/// A container which allows persisting the `BeaconForkChoiceStore` to the on-disk database.
#[superstruct(
    variants(V17, V28),
    variant_attributes(derive(Encode, Decode)),
    no_enum
)]
pub struct PersistedForkChoiceStore {
    /// The balances cache was removed from disk storage in schema V28.
    #[superstruct(only(V17))]
    pub balances_cache: BalancesCacheV8,
    pub time: Slot,
    pub finalized_checkpoint: Checkpoint,
    pub justified_checkpoint: Checkpoint,
    /// The justified balances were removed from disk storage in schema V28.
    #[superstruct(only(V17))]
    pub justified_balances: Vec<u64>,
    /// The justified state root is stored so that it can be used to load the justified balances.
    #[superstruct(only(V28))]
    pub justified_state_root: Hash256,
    pub unrealized_justified_checkpoint: Checkpoint,
    #[superstruct(only(V28))]
    pub unrealized_justified_state_root: Hash256,
    pub unrealized_finalized_checkpoint: Checkpoint,
    pub proposer_boost_root: Hash256,
    pub equivocating_indices: BTreeSet<u64>,
    /// [New in Heze:EIP7805] Tracks whether execution payloads satisfy inclusion list constraints.
    /// Stored as a vector of (root, satisfied) pairs for SSZ compatibility.
    #[superstruct(only(V28))]
    pub payload_inclusion_list_satisfaction: Vec<(Hash256, bool)>,
    /// [New in Gloas:EIP7732] PTC votes for payload timeliness.
    /// Stored as a vector of (root, votes) pairs for SSZ compatibility.
    /// votes is a vector of booleans representing PTC votes.
    #[superstruct(only(V28))]
    pub payload_timeliness_vote: Vec<(Hash256, Vec<bool>)>,
    /// [New in Gloas:EIP7732] PTC votes for data availability.
    /// Stored as a vector of (root, votes) pairs for SSZ compatibility.
    /// votes is a vector of booleans representing PTC votes.
    #[superstruct(only(V28))]
    pub payload_data_availability_vote: Vec<(Hash256, Vec<bool>)>,
}

// Convert V28 to V17 by adding balances and removing justified state roots.
impl From<(PersistedForkChoiceStoreV28, JustifiedBalances)> for PersistedForkChoiceStoreV17 {
    fn from((v28, balances): (PersistedForkChoiceStoreV28, JustifiedBalances)) -> Self {
        Self {
            balances_cache: Default::default(),
            time: v28.time,
            finalized_checkpoint: v28.finalized_checkpoint,
            justified_checkpoint: v28.justified_checkpoint,
            justified_balances: balances.effective_balances,
            unrealized_justified_checkpoint: v28.unrealized_justified_checkpoint,
            unrealized_finalized_checkpoint: v28.unrealized_finalized_checkpoint,
            proposer_boost_root: v28.proposer_boost_root,
            equivocating_indices: v28.equivocating_indices,
        }
    }
}
