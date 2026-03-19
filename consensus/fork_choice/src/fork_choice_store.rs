use proto_array::JustifiedBalances;
use std::collections::{BTreeSet, HashMap};
use std::fmt::Debug;
use types::{AbstractExecPayload, BeaconBlockRef, BeaconState, Checkpoint, EthSpec, Hash256, Slot};

/// Approximates the `Store` in "Ethereum 2.0 Phase 0 -- Beacon Chain Fork Choice":
///
/// https://github.com/ethereum/eth2.0-specs/blob/v0.12.1/specs/phase0/fork-choice.md#store
///
/// ## Detail
///
/// This is only an approximation for two reasons:
///
/// - This crate stores the actual block DAG in `ProtoArrayForkChoice`.
/// - `time` is represented using `Slot` instead of UNIX epoch `u64`.
///
/// ## Motiviation
///
/// The primary motivation for defining this as a trait to be implemented upstream rather than a
/// concrete struct is to allow this crate to be free from "impure" on-disk database logic,
/// hopefully making auditing easier.
pub trait ForkChoiceStore<E: EthSpec>: Sized {
    type Error: Debug;

    /// Returns the last value passed to `Self::set_current_slot`.
    fn get_current_slot(&self) -> Slot;

    /// Set the value to be returned by `Self::get_current_slot`.
    ///
    /// ## Notes
    ///
    /// This should only ever be called from within `ForkChoice::on_tick`.
    fn set_current_slot(&mut self, slot: Slot);

    /// Called whenever `ForkChoice::on_block` has verified a block, but not yet added it to fork
    /// choice. Allows the implementer to performing caching or other housekeeping duties.
    fn on_verified_block<Payload: AbstractExecPayload<E>>(
        &mut self,
        block: BeaconBlockRef<E, Payload>,
        block_root: Hash256,
        state: &BeaconState<E>,
    ) -> Result<(), Self::Error>;

    /// Returns the `justified_checkpoint`.
    fn justified_checkpoint(&self) -> &Checkpoint;

    /// Returns the state root of the justified checkpoint.
    fn justified_state_root(&self) -> Hash256;

    /// Returns balances from the `state` identified by `justified_checkpoint.root`.
    fn justified_balances(&self) -> &JustifiedBalances;

    /// Returns the `finalized_checkpoint`.
    fn finalized_checkpoint(&self) -> &Checkpoint;

    /// Returns the `unrealized_justified_checkpoint`.
    fn unrealized_justified_checkpoint(&self) -> &Checkpoint;

    /// Returns the state root of the unrealized justified checkpoint.
    fn unrealized_justified_state_root(&self) -> Hash256;

    /// Returns the `unrealized_finalized_checkpoint`.
    fn unrealized_finalized_checkpoint(&self) -> &Checkpoint;

    /// Returns the `proposer_boost_root`.
    fn proposer_boost_root(&self) -> Hash256;

    /// Sets `finalized_checkpoint`.
    fn set_finalized_checkpoint(&mut self, checkpoint: Checkpoint);

    /// Sets the `justified_checkpoint`.
    fn set_justified_checkpoint(
        &mut self,
        checkpoint: Checkpoint,
        state_root: Hash256,
    ) -> Result<(), Self::Error>;

    /// Sets the `unrealized_justified_checkpoint`.
    fn set_unrealized_justified_checkpoint(&mut self, checkpoint: Checkpoint, state_root: Hash256);

    /// Sets the `unrealized_finalized_checkpoint`.
    fn set_unrealized_finalized_checkpoint(&mut self, checkpoint: Checkpoint);

    /// Sets the proposer boost root.
    fn set_proposer_boost_root(&mut self, proposer_boost_root: Hash256);

    /// Gets the equivocating indices.
    fn equivocating_indices(&self) -> &BTreeSet<u64>;

    /// Adds to the set of equivocating indices.
    fn extend_equivocating_indices(&mut self, indices: impl IntoIterator<Item = u64>);

    // ===== [New in Heze:EIP7805] Inclusion List satisfaction tracking methods =====

    /// Check if the execution payload at the given block root satisfies inclusion list constraints.
    /// Returns `None` if the block is unknown or not yet processed.
    fn is_payload_inclusion_list_satisfied(&self, block_root: Hash256) -> Option<bool>;

    /// Record the inclusion list satisfaction status for an execution payload.
    fn set_payload_inclusion_list_satisfaction(&mut self, block_root: Hash256, satisfied: bool);

    /// Get a reference to the entire payload_inclusion_list_satisfaction map.
    fn payload_inclusion_list_satisfaction(&self) -> &HashMap<Hash256, bool>;

    // ===== [New in Gloas:EIP7732] PTC (Payload Timeliness Committee) methods =====

    /// Get the PTC timeliness votes for a block root.
    /// Returns `None` if no votes have been recorded for this block.
    fn payload_timeliness_vote(&self, block_root: Hash256) -> Option<&Vec<bool>>;

    /// Record a PTC timeliness vote from a committee member.
    /// `index` is the PTC committee index (0 to PTC_SIZE-1).
    /// `vote` is true if the payload was present (timely), false otherwise.
    fn set_payload_timeliness_vote(&mut self, block_root: Hash256, index: usize, vote: bool);

    /// Get the PTC data availability votes for a block root.
    /// Returns `None` if no votes have been recorded for this block.
    fn payload_data_availability_vote(&self, block_root: Hash256) -> Option<&Vec<bool>>;

    /// Record a PTC data availability vote from a committee member.
    /// `index` is the PTC committee index (0 to PTC_SIZE-1).
    /// `vote` is true if the data was available, false otherwise.
    fn set_payload_data_availability_vote(&mut self, block_root: Hash256, index: usize, vote: bool);

    /// Check if payload is timely based on PTC votes.
    /// Returns true if >= 2/3 of PTC voted for timeliness.
    fn is_payload_timely(&self, ptc_size: usize, block_root: Hash256) -> bool;

    /// Check if payload data is available based on PTC votes.
    /// Returns true if >= 2/3 of PTC voted for data availability.
    fn is_payload_data_available(&self, ptc_size: usize, block_root: Hash256) -> bool;

    /// Initialize PTC voting arrays for a new block.
    fn initialize_ptc_votes(&mut self, block_root: Hash256, ptc_size: usize);
}
