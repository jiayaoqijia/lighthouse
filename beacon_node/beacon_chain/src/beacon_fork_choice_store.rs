//! Defines the `BeaconForkChoiceStore` which provides the persistent storage for the `ForkChoice`
//! struct.
//!
//! Additionally, the `BalancesCache` struct is defined; a cache designed to avoid database
//! reads when fork choice requires the validator balances of the justified state.

use crate::{BeaconSnapshot, metrics};
use educe::Educe;
use fixed_bytes::FixedBytesExtended;
use fork_choice::ForkChoiceStore;
use parking_lot::Mutex;
use proto_array::JustifiedBalances;
use safe_arith::{ArithError, SafeArith};
use ssz_derive::{Decode, Encode};
use std::collections::{BTreeSet, VecDeque};
use std::marker::PhantomData;
use std::sync::Arc;
use store::{Error as StoreError, HotColdDB, ItemStore};
use superstruct::superstruct;
use tracing::debug;
use types::{
    AbstractExecPayload, BeaconBlockRef, BeaconState, BeaconStateError, ChainSpec, Checkpoint,
    Epoch, EthSpec, Hash256, RelativeEpoch, Slot,
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
    CommitteeStateSlotMismatch { expected: Slot, actual: Slot },
    UnknownEquivocatingValidator(u64),
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

#[derive(PartialEq, Clone, Debug)]
pub(crate) struct CacheItem {
    pub(crate) block_root: Hash256,
    pub(crate) epoch: Epoch,
    pub(crate) justified_balances: JustifiedBalances,
}

#[derive(PartialEq, Clone, Default, Debug)]
pub struct BalancesCache {
    pub(crate) items: Vec<CacheItem>,
}

impl BalancesCache {
    /// Add an entry for the given epoch boundary block root, built from `state`.
    ///
    /// The caller must pass a `state` at the epoch boundary slot, so that the cached balances
    /// match the checkpoint state exactly (see `on_verified_block`).
    pub fn insert<E: EthSpec>(
        &mut self,
        epoch_boundary_root: Hash256,
        state: &BeaconState<E>,
    ) -> Result<(), Error> {
        let epoch = state.current_epoch();
        if self.position(epoch_boundary_root, epoch).is_none() {
            let item = CacheItem {
                block_root: epoch_boundary_root,
                epoch,
                justified_balances: JustifiedBalances::from_justified_state(state)?,
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
    pub fn get(&mut self, block_root: Hash256, epoch: Epoch) -> Option<JustifiedBalances> {
        let i = self.position(block_root, epoch)?;
        Some(self.items[i].justified_balances.clone())
    }
}

type CommitteeMembership = ((Hash256, Hash256, Slot), Arc<Vec<u64>>);

/// Implements `fork_choice::ForkChoiceStore` in order to provide a persistent backing to the
/// `fork_choice::ForkChoice` struct.
#[derive(Debug, Educe)]
#[educe(PartialEq(bound(E: EthSpec, Hot: ItemStore, Cold: ItemStore)))]
pub struct BeaconForkChoiceStore<E: EthSpec, Hot: ItemStore, Cold: ItemStore> {
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
    /// Derived committee memberships only, bounded and deliberately not persisted.
    #[educe(PartialEq(ignore))]
    committee_memberships: Mutex<VecDeque<CommitteeMembership>>,
    _phantom: PhantomData<E>,
}

impl<E, Hot, Cold> BeaconForkChoiceStore<E, Hot, Cold>
where
    E: EthSpec,
    Hot: ItemStore,
    Cold: ItemStore,
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
            committee_memberships: Mutex::new(VecDeque::new()),
            _phantom: PhantomData,
        })
    }

    /// Save the current state of `Self` to a `PersistedForkChoiceStore` which can be stored to the
    /// on-disk database.
    pub fn to_persisted(&self) -> PersistedForkChoiceStore {
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
        }
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
            committee_memberships: Mutex::new(VecDeque::new()),
            _phantom: PhantomData,
        })
    }
}

impl<E, Hot, Cold> ForkChoiceStore<E> for BeaconForkChoiceStore<E, Hot, Cold>
where
    E: EthSpec,
    Hot: ItemStore,
    Cold: ItemStore,
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
        let epoch = state.current_epoch();
        let epoch_boundary_slot = epoch.start_slot(E::slots_per_epoch());

        if state.slot() == epoch_boundary_slot {
            return self.balances_cache.insert(block_root, state);
        }

        let epoch_boundary_root = *state.get_block_root(epoch_boundary_slot)?;
        if self
            .balances_cache
            .position(epoch_boundary_root, epoch)
            .is_some()
        {
            return Ok(());
        }

        // The epoch boundary slot was skipped. We need to make sure to use the boundary state. If
        // not, a slashing included in first post boundary block would cause us to cache incorrect
        // balances.
        let epoch_boundary_state_root = *state.get_state_root(epoch_boundary_slot)?;
        debug!(
            slot = %epoch_boundary_slot,
            state_root = ?epoch_boundary_state_root,
            "Loading epoch boundary state for balances cache"
        );
        let _timer = metrics::start_timer(&metrics::BALANCES_CACHE_BOUNDARY_STATE_LOAD_TIMES);
        let update_cache = true;
        if let Some(epoch_boundary_state) = self
            .store
            .get_hot_state(&epoch_boundary_state_root, update_cache)
            .map_err(Error::FailedToReadState)?
        {
            self.balances_cache
                .insert(epoch_boundary_root, &epoch_boundary_state)?;
        }

        Ok(())
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

        if let Some(justified_balances) = self.balances_cache.get(
            self.justified_checkpoint.root,
            self.justified_checkpoint.epoch,
        ) {
            metrics::inc_counter(&metrics::BALANCES_CACHE_HITS);
            self.justified_balances = justified_balances;
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

    fn equivocating_committee_balance(
        &self,
        block_root: Hash256,
        state_root: Hash256,
        slot: Slot,
        spec: &ChainSpec,
    ) -> Result<u64, Self::Error> {
        if self.equivocating_indices.is_empty() {
            return Ok(0);
        }
        let key = (block_root, state_root, slot);
        let cached = self
            .committee_memberships
            .lock()
            .iter()
            .find(|(cached_key, _)| *cached_key == key)
            .map(|(_, indices)| indices.clone());
        let members = match cached {
            Some(indices) => indices,
            None => {
                // Never hold the derived-cache lock across state IO or shuffling.
                let mut state = self
                    .store
                    .get_state(&state_root, Some(slot), true)
                    .map_err(Error::FailedToReadState)?
                    .ok_or(Error::MissingState(state_root))?;
                if state.slot() != slot {
                    return Err(Error::CommitteeStateSlotMismatch {
                        expected: slot,
                        actual: state.slot(),
                    });
                }
                state.build_committee_cache(RelativeEpoch::Current, spec)?;
                let count = state.get_committee_count_at_slot(slot)?;
                let mut indices = Vec::new();
                for committee_index in 0..count {
                    indices.extend(
                        state
                            .get_beacon_committee(slot, committee_index)?
                            .committee
                            .iter()
                            .map(|index| *index as u64),
                    );
                }
                indices.sort_unstable();
                let indices = Arc::new(indices);
                let mut cache = self.committee_memberships.lock();
                if let Some((_, existing)) = cache.iter().find(|(cached_key, _)| *cached_key == key)
                {
                    existing.clone()
                } else {
                    if cache.len() == 4 {
                        cache.pop_front();
                    }
                    cache.push_back((key, indices.clone()));
                    indices
                }
            }
        };
        // Re-sum current equivocation/checkpoint data on every call. Cached membership
        // cannot freeze a previous slashing set or previous justified balances.
        let mut balance = 0u64;
        for index in &self.equivocating_indices {
            if members.binary_search(index).is_ok() {
                let position = usize::try_from(*index)
                    .map_err(|_| Error::UnknownEquivocatingValidator(*index))?;
                let value = self
                    .justified_balances
                    .validator_effective_balances
                    .get(position)
                    .ok_or(Error::UnknownEquivocatingValidator(*index))?;
                balance.safe_add_assign(*value)?;
            }
        }
        Ok(balance)
    }

    fn extend_equivocating_indices(&mut self, indices: impl IntoIterator<Item = u64>) {
        self.equivocating_indices.extend(indices);
    }
}

pub type PersistedForkChoiceStore = PersistedForkChoiceStoreV28;

/// A container which allows persisting the `BeaconForkChoiceStore` to the on-disk database.
#[superstruct(variants(V28), variant_attributes(derive(Encode, Decode)), no_enum)]
pub struct PersistedForkChoiceStore {
    pub time: Slot,
    pub finalized_checkpoint: Checkpoint,
    pub justified_checkpoint: Checkpoint,
    pub justified_state_root: Hash256,
    pub unrealized_justified_checkpoint: Checkpoint,
    pub unrealized_justified_state_root: Hash256,
    pub unrealized_finalized_checkpoint: Checkpoint,
    pub proposer_boost_root: Hash256,
    pub equivocating_indices: BTreeSet<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use types::{Spec, Validator};

    type E = Spec;

    #[test]
    fn balances_cache_hit_matches_justified_state() {
        let spec = E::default_spec();
        let mut state: BeaconState<E> = BeaconState::new(0, <_>::default(), &spec);
        for i in 0..4u64 {
            state
                .validators_mut()
                .push(Validator {
                    effective_balance: 32_000_000_000,
                    slashed: i == 1,
                    activation_epoch: Epoch::new(0),
                    exit_epoch: spec.far_future_epoch,
                    withdrawable_epoch: spec.far_future_epoch,
                    ..Default::default()
                })
                .unwrap();
        }

        let block_root = Hash256::repeat_byte(1);
        let mut cache = BalancesCache::default();
        cache.insert(block_root, &state).unwrap();

        let cached = cache.get(block_root, state.current_epoch()).unwrap();
        let expected = JustifiedBalances::from_justified_state(&state).unwrap();
        assert_eq!(cached, expected);
        assert!(!cached.slashed_balances.is_empty());
        assert!(cache.get(block_root, state.current_epoch() + 1).is_none());
    }
    fn reference_committee_store<E: EthSpec>(preset: &str, bytes: &[u8]) {
        use store::{KeyValueStore, MemoryStore, StoreConfig};
        use types::{BeaconBlock, ForkName, SignedBeaconBlock};
        let spec = ForkName::Gloas.make_genesis_spec(E::default_spec());
        let mut state = BeaconState::<E>::from_ssz_bytes(bytes, &spec).unwrap();
        let root = state.canonical_root().unwrap();
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../docs/qualification/gloas-committee-weight-20261122/reference/cases.json"
        ))
        .unwrap();
        let case = fixture["cases"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["preset"] == preset)
            .unwrap();
        let block_root: Hash256 = case["head_root"].as_str().unwrap().parse().unwrap();
        let db = Arc::new(
            HotColdDB::<E, MemoryStore, MemoryStore>::open_ephemeral(
                StoreConfig::default(),
                spec.clone().into(),
            )
            .unwrap(),
        );
        let anchor_op = db
            .init_anchor_info(Hash256::zero(), Slot::new(0), Slot::new(0), false)
            .unwrap();
        db.hot_db.do_atomically(vec![anchor_op]).unwrap();
        db.put_state(&root, &state).unwrap();
        let mut block = BeaconBlock::<E>::empty(&spec);
        *block.state_root_mut() = root;
        let snapshot = BeaconSnapshot::new(
            Arc::new(SignedBeaconBlock::from_block(
                block,
                bls::Signature::empty(),
            )),
            None,
            block_root,
            state,
        );
        let mut live = BeaconForkChoiceStore::get_forkchoice_store(db.clone(), snapshot).unwrap();
        // Empty equivocation sets require no state IO.
        assert_eq!(
            live.equivocating_committee_balance(
                block_root,
                Hash256::repeat_byte(0x81),
                Slot::new(0),
                &spec
            )
            .unwrap(),
            0
        );
        live.extend_equivocating_indices(
            case["equivocating_indices"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap()),
        );
        let expected = case["equivocation_committee_weight"].as_u64().unwrap();
        assert_eq!(
            live.equivocating_committee_balance(block_root, root, Slot::new(0), &spec)
                .unwrap(),
            expected
        );
        let actual_members = live.committee_memberships.lock().front().unwrap().1.clone();
        let expected_members: Vec<u64> = case["committee_members"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(*actual_members, expected_members);
        let restored = BeaconForkChoiceStore::from_persisted(live.to_persisted(), db).unwrap();
        assert!(restored.committee_memberships.lock().is_empty());
        assert_eq!(
            restored
                .equivocating_committee_balance(block_root, root, Slot::new(0), &spec)
                .unwrap(),
            expected
        );
        // The cache retains membership, never an old slashing set or old checkpoint balances.
        let members = expected_members;
        let extra = *members
            .iter()
            .find(|i| !live.equivocating_indices.contains(i))
            .unwrap();
        live.extend_equivocating_indices([extra]);
        let expected =
            expected + live.justified_balances.validator_effective_balances[extra as usize];
        assert_eq!(
            live.equivocating_committee_balance(block_root, root, Slot::new(0), &spec)
                .unwrap(),
            expected
        );
        live.justified_balances.validator_effective_balances[extra as usize] = 7;
        assert_eq!(
            live.equivocating_committee_balance(block_root, root, Slot::new(0), &spec)
                .unwrap(),
            case["equivocation_committee_weight"].as_u64().unwrap() + 7
        );
        let outside = (0..64).find(|i| !members.contains(i)).unwrap();
        live.extend_equivocating_indices([outside]);
        assert_eq!(
            live.equivocating_committee_balance(block_root, root, Slot::new(0), &spec)
                .unwrap(),
            case["equivocation_committee_weight"].as_u64().unwrap() + 7
        );
        assert!(matches!(
            live.equivocating_committee_balance(block_root, root, Slot::new(1), &spec),
            Err(Error::CommitteeStateSlotMismatch { .. })
        ));
        assert!(
            live.equivocating_committee_balance(
                block_root,
                Hash256::repeat_byte(0x81),
                Slot::new(0),
                &spec
            )
            .is_err()
        );
        for n in 1..=5 {
            live.equivocating_committee_balance(Hash256::repeat_byte(n), root, Slot::new(0), &spec)
                .unwrap();
        }
        assert_eq!(live.committee_memberships.lock().len(), 4);
    }

    #[test]
    fn signed_reference_committee_membership_survives_cold_restore_and_current_inputs() {
        reference_committee_store::<types::MinimalEthSpec>(
            "minimal",
            include_bytes!(
                "../../../docs/qualification/gloas-committee-weight-20261122/reference/minimal-head-state.ssz"
            ),
        );
        reference_committee_store::<types::MainnetEthSpec>(
            "mainnet",
            include_bytes!(
                "../../../docs/qualification/gloas-committee-weight-20261122/reference/mainnet-head-state.ssz"
            ),
        );
    }
}
