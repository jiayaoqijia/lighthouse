//! Inclusion List service for FOCIL (EIP-7805).
//!
//! This service handles the production and broadcasting of inclusion lists
//! by validators who are members of the inclusion list committee.
//!
//! Per EIP-7805 spec:
//! - IL should be produced and broadcast immediately after processing the block
//!   for the current slot and confirming it as the head.
//! - If no block is received by (submission_due - 1000ms), use local head.
//! - Must broadcast by submission_deadline (~67% of slot = ~8s for 12s slot).

use crate::duties_service::DutiesService;
use beacon_node_fallback::{ApiTopic, BeaconNodeFallback, beacon_head_monitor::HeadEvent};
use bls::PublicKeyBytes;
use eth2::types::StateId;
use slot_clock::SlotClock;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use task_executor::TaskExecutor;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::time::{Duration, Instant, sleep};
use tracing::{debug, error, info, trace, warn};
use types::{
    ChainSpec, EthSpec, Hash256, IlTransaction, IlTransactions, InclusionList, SignedInclusionList,
    Slot,
};
use validator_store::ValidatorStore;

/// Basis points for inclusion list submission deadline (~67% of slot duration).
pub const INCLUSION_LIST_SUBMISSION_DUE_BPS: u64 = 6667;

/// Basis points for view freeze cutoff (75% of slot duration).
pub const VIEW_FREEZE_CUTOFF_BPS: u64 = 7500;

/// Basis points for proposer inclusion list cutoff (~92% of slot duration).
pub const PROPOSER_INCLUSION_LIST_CUTOFF_BPS: u64 = 9167;

/// Number of milliseconds before the submission deadline to fallback to local head.
/// Per spec: "If no block is received by get_inclusion_list_submission_due_ms(epoch) - 1000 
/// milliseconds into the slot, the validator should run get_head"
const HEAD_CHECK_MARGIN_MS: u64 = 1000;

pub struct InclusionListService<S: ValidatorStore, T: SlotClock + 'static> {
    inner: Arc<Inner<S, T>>,
}

impl<S: ValidatorStore, T: SlotClock + 'static> Clone for InclusionListService<S, T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<S: ValidatorStore, T: SlotClock + 'static> Deref for InclusionListService<S, T> {
    type Target = Inner<S, T>;

    fn deref(&self) -> &Self::Target {
        self.inner.deref()
    }
}

pub struct Inner<S: ValidatorStore, T: SlotClock + 'static> {
    duties_service: Arc<DutiesService<S, T>>,
    validator_store: Arc<S>,
    slot_clock: T,
    beacon_nodes: Arc<BeaconNodeFallback<T>>,
    executor: TaskExecutor,
    /// Boolean to track whether the service has posted subscriptions to the BN at least once.
    #[allow(dead_code)]
    first_subscription_done: AtomicBool,
    /// Receiver for head events from beacon node.
    head_monitor_rx: Option<Mutex<mpsc::Receiver<HeadEvent>>>,
}

impl<S: ValidatorStore + 'static, T: SlotClock + 'static> InclusionListService<S, T> {
    pub fn new(
        duties_service: Arc<DutiesService<S, T>>,
        validator_store: Arc<S>,
        slot_clock: T,
        beacon_nodes: Arc<BeaconNodeFallback<T>>,
        executor: TaskExecutor,
        head_monitor_rx: Option<mpsc::Receiver<HeadEvent>>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                duties_service,
                validator_store,
                slot_clock,
                beacon_nodes,
                executor,
                first_subscription_done: AtomicBool::new(false),
                head_monitor_rx: head_monitor_rx.map(Mutex::new),
            }),
        }
    }

    /// Check if the Heze fork has been activated and therefore inclusion list duties should be performed.
    ///
    /// Slot clock errors are mapped to `false`.
    fn heze_fork_activated(&self) -> bool {
        let result = self.duties_service
            .spec
            .heze_fork_epoch
            .and_then(|fork_epoch| {
                let current_epoch = self.slot_clock.now()?.epoch(S::E::slots_per_epoch());
                let activated = current_epoch >= fork_epoch;
                debug!(
                    current_epoch = %current_epoch,
                    heze_fork_epoch = %fork_epoch,
                    activated = activated,
                    "EIP7805: Checking Heze fork activation"
                );
                Some(activated)
            })
            .unwrap_or(false);
        result
    }

    pub fn start_update_service(self, spec: &ChainSpec) -> Result<(), String> {
        if !self.heze_fork_activated() {
            info!("Inclusion list service not started: Heze fork not active");
            return Ok(());
        }

        let _slot_duration = spec.get_slot_duration();
        let _duration_to_next_slot = self
            .slot_clock
            .duration_to_next_slot()
            .ok_or("Unable to determine duration to next slot")?;

        let executor = self.executor.clone();
        let spec = spec.clone();

        executor.spawn(
            async move {
                info!("Inclusion list service started");

                loop {
                    // Wait for the next slot
                    if let Some(duration_to_next_slot) = self.slot_clock.duration_to_next_slot() {
                        sleep(duration_to_next_slot).await;
                    } else {
                        error!("Unable to read slot clock");
                        sleep(Duration::from_secs(1)).await;
                        continue;
                    }

                    // Get current slot
                    let Some(current_slot) = self.slot_clock.now() else {
                        error!("Unable to read slot clock");
                        continue;
                    };

                    // Check if Heze is active for this slot
                    if !self.heze_fork_activated() {
                        trace!("Heze fork not active, skipping inclusion list production");
                        continue;
                    }

                    // Get inclusion list committee assignments for this slot
                    let assignments = self.get_inclusion_list_assignments(current_slot).await;

                    if assignments.is_empty() {
                        trace!(slot = %current_slot, "No inclusion list duties for this slot");
                        continue;
                    }

                    // Per EIP-7805 spec:
                    // 1. Wait for block and process immediately when received
                    // 2. If no block by (submission_due - 1000ms), use local head
                    // 3. Must broadcast by submission_due

                    let submission_due_ms = get_inclusion_list_submission_due_ms(&spec);
                    let fallback_deadline_ms = submission_due_ms.saturating_sub(HEAD_CHECK_MARGIN_MS);

                    // Calculate the deadline instant
                    // Use slot_clock to determine if we can calculate the deadline
                    let deadline_instant = if self.slot_clock.start_of(current_slot).is_some() {
                        let now = Instant::now();
                        let elapsed = now.elapsed();
                        let deadline_duration = Duration::from_millis(fallback_deadline_ms);
                        // Calculate when the deadline is from now
                        if deadline_duration > elapsed {
                            Instant::now() + (deadline_duration - elapsed)
                        } else {
                            // Already past the deadline, process immediately with local head
                            Instant::now()
                        }
                    } else {
                        // Can't determine slot start, use a reasonable timeout
                        Instant::now() + Duration::from_millis(fallback_deadline_ms)
                    };

                    // Try to receive a head event (block) before the deadline
                    let head_event = self.wait_for_head_event(current_slot, deadline_instant).await;

                    match head_event {
                        Some(event) => {
                            debug!(
                                slot = %current_slot,
                                beacon_block_root = ?event.beacon_block_root,
                                "EIP7805: Received block, processing inclusion list immediately"
                            );
                        }
                        None => {
                            // Timeout - use local head
                            debug!(
                                slot = %current_slot,
                                "EIP7805: No block received, using local head for inclusion list"
                            );
                        }
                    }

                    // Produce and broadcast inclusion lists
                    for (validator_index, pubkey) in assignments {
                        match self
                            .produce_and_broadcast_inclusion_list(
                                current_slot,
                                validator_index,
                                pubkey,
                                &spec,
                            )
                            .await
                        {
                            Ok(_) => {
                                info!(
                                    slot = %current_slot,
                                    validator_index = validator_index,
                                    "Successfully produced and broadcast inclusion list"
                                );
                            }
                            Err(e) => {
                                warn!(
                                    slot = %current_slot,
                                    validator_index = validator_index,
                                    error = %e,
                                    "Failed to produce inclusion list"
                                );
                            }
                        }
                    }
                }
            },
            "inclusion_list_service",
        );

        Ok(())
    }

    /// Wait for a head event (block) for the current slot until the deadline.
    ///
    /// Per EIP-7805 spec:
    /// "If a validator is in the current inclusion list committee, the validator should
    /// create and broadcast the signed_inclusion_list to the global inclusion_list
    /// subnet by get_inclusion_list_submission_due_ms(epoch) milliseconds into the slot
    /// after processing the block for the current slot and confirming it as the head."
    ///
    /// Returns Some(HeadEvent) if a block for the current slot was received before deadline.
    /// Returns None if timeout or no head monitor available (fallback to local head).
    async fn wait_for_head_event(&self, current_slot: Slot, deadline: Instant) -> Option<HeadEvent> {
        let Some(receiver) = &self.head_monitor_rx else {
            // No head monitor configured, use local head immediately
            debug!(
                slot = %current_slot,
                "EIP7805: No head monitor configured, using local head"
            );
            return None;
        };

        let mut receiver = receiver.lock().await;

        loop {
            // Calculate remaining time until deadline
            let now = Instant::now();
            if now >= deadline {
                debug!(
                    slot = %current_slot,
                    "EIP7805: Deadline reached, falling back to local head"
                );
                return None;
            }

            let remaining = deadline - now;

            // Wait for either a head event or timeout
            tokio::select! {
                result = receiver.recv() => {
                    match result {
                        Some(head_event) => {
                            // Check if this event is for the current slot
                            if head_event.slot == current_slot {
                                debug!(
                                    slot = %current_slot,
                                    beacon_block_root = ?head_event.beacon_block_root,
                                    "EIP7805: Received head event for current slot"
                                );
                                return Some(head_event);
                            } else {
                                // Event for different slot, continue waiting
                                trace!(
                                    current_slot = %current_slot,
                                    event_slot = %head_event.slot,
                                    "EIP7805: Ignoring head event for different slot"
                                );
                                continue;
                            }
                        }
                        None => {
                            warn!(
                                slot = %current_slot,
                                "EIP7805: Head monitor channel closed"
                            );
                            return None;
                        }
                    }
                }
                _ = sleep(remaining) => {
                    debug!(
                        slot = %current_slot,
                        "EIP7805: Timeout waiting for block, using local head"
                    );
                    return None;
                }
            }
        }
    }

    /// Get inclusion list committee assignments for the given slot.
    async fn get_inclusion_list_assignments(&self, slot: Slot) -> Vec<(u64, PublicKeyBytes)> {
        use validator_store::DoppelgangerStatus;

        debug!(slot = %slot, "EIP7805: Fetching inclusion list committee assignments");

        // Query beacon node for inclusion list committee assignments
        let committee_response = self
            .beacon_nodes
            .first_success(|beacon_node| {
                let slot = slot;
                async move {
                    beacon_node
                        .get_beacon_states_inclusion_list_committee(StateId::Head, Some(slot))
                        .await
                        .map_err(|e| format!("Failed to get IL committee: {}", e))
                }
            })
            .await;

        match committee_response {
            Ok(response) => {
                let committee_indices: std::collections::HashSet<u64> =
                    response.data.validators.into_iter().collect();
                
                debug!(
                    slot = %slot,
                    committee_size = committee_indices.len(),
                    committee_root = ?response.data.committee_root,
                    "EIP7805: Received inclusion list committee"
                );

                // Get all our voting pubkeys
                let our_pubkeys: Vec<PublicKeyBytes> = self
                    .validator_store
                    .voting_pubkeys(DoppelgangerStatus::ignored);

                debug!(
                    slot = %slot,
                    our_validator_count = our_pubkeys.len(),
                    "EIP7805: Checking our validators against committee"
                );

                // Check which of our validators are in the committee
                let mut assignments = Vec::new();
                for pubkey in our_pubkeys {
                    if let Some(index) = self.validator_store.validator_index(&pubkey) {
                        if committee_indices.contains(&index) {
                            debug!(
                                slot = %slot,
                                validator_index = index,
                                "EIP7805: Our validator is in IL committee"
                            );
                            assignments.push((index, pubkey));
                        }
                    }
                }

                if !assignments.is_empty() {
                    info!(
                        slot = %slot,
                        assignment_count = assignments.len(),
                        "EIP7805: Have inclusion list duties for this slot"
                    );
                }

                assignments
            }
            Err(e) => {
                warn!(%slot, error = %e, "EIP7805: Failed to get inclusion list committee");
                Vec::new()
            }
        }
    }

    /// Produce and broadcast an inclusion list for the given validator.
    async fn produce_and_broadcast_inclusion_list(
        &self,
        slot: Slot,
        validator_index: u64,
        pubkey: PublicKeyBytes,
        _spec: &ChainSpec,
    ) -> Result<(), String> {
        debug!(
            slot = %slot,
            validator_index = validator_index,
            "EIP7805: Starting inclusion list production"
        );

        // 1. Get the inclusion list committee root from beacon node
        let committee_root = self.get_inclusion_list_committee_root(slot).await?;
        debug!(
            slot = %slot,
            validator_index = validator_index,
            committee_root = ?committee_root,
            "EIP7805: Got committee root"
        );

        // 2. Get inclusion list transactions from execution engine
        let transactions = self.get_inclusion_list_transactions().await?;
        debug!(
            slot = %slot,
            validator_index = validator_index,
            tx_count = transactions.len(),
            "EIP7805: Got inclusion list transactions"
        );

        // 3. Build the inclusion list
        let il_transactions: Vec<IlTransaction<S::E>> = transactions
            .into_iter()
            .filter_map(|tx| IlTransaction::<S::E>::new(tx).ok())
            .collect();

        let il_transactions = IlTransactions::<S::E>::new(il_transactions).unwrap_or_default();

        let inclusion_list = InclusionList::<S::E> {
            slot,
            validator_index,
            inclusion_list_committee_root: committee_root,
            transactions: il_transactions,
        };

        debug!(
            slot = %slot,
            validator_index = validator_index,
            tx_count = inclusion_list.transactions.len(),
            "EIP7805: Built inclusion list"
        );

        // 4. Sign the inclusion list using ValidatorStore
        let signed_inclusion_list = self
            .validator_store
            .sign_inclusion_list(pubkey, inclusion_list)
            .await
            .map_err(|e| format!("Failed to sign inclusion list: {:?}", e))?;

        debug!(
            slot = %slot,
            validator_index = validator_index,
            "EIP7805: Signed inclusion list"
        );

        // 5. Broadcast to the network via beacon node
        self.broadcast_inclusion_list(signed_inclusion_list).await?;

        info!(
            slot = %slot,
            validator_index = validator_index,
            "EIP7805: Successfully produced and broadcast inclusion list"
        );

        Ok(())
    }

    /// Get the inclusion list committee root for the given slot.
    async fn get_inclusion_list_committee_root(&self, slot: Slot) -> Result<Hash256, String> {
        // Query beacon node for the committee root
        let response = self
            .beacon_nodes
            .first_success(|beacon_node| {
                let slot = slot;
                async move {
                    beacon_node
                        .get_beacon_states_inclusion_list_committee(StateId::Head, Some(slot))
                        .await
                        .map_err(|e| format!("Failed to get IL committee: {}", e))
                }
            })
            .await
            .map_err(|e| format!("Failed to query beacon node: {}", e))?;

        Ok(response.data.committee_root)
    }

    /// Get inclusion list transactions from the execution engine.
    async fn get_inclusion_list_transactions(&self) -> Result<Vec<Vec<u8>>, String> {
        // Query beacon node which forwards to execution engine for inclusion list transactions
        // This uses the engine_getInclusionListV1 method via the beacon node's HTTP API
        let response = self
            .beacon_nodes
            .first_success(|beacon_node| async move {
                beacon_node
                    .get_validator_inclusion_list_transactions()
                    .await
                    .map_err(|e| format!("Failed to get inclusion list transactions: {}", e))
            })
            .await
            .map_err(|e| format!("Failed to query beacon node: {}", e))?;

        // Decode hex-encoded transactions back to bytes
        let transactions: Vec<Vec<u8>> = response
            .data
            .transactions
            .into_iter()
            .filter_map(|hex_tx| {
                // Remove "0x" prefix if present and decode hex
                let hex_str = hex_tx.strip_prefix("0x").unwrap_or(&hex_tx);
                hex::decode(hex_str).ok()
            })
            .collect();

        debug!(
            tx_count = transactions.len(),
            "EIP7805: Retrieved inclusion list transactions"
        );

        Ok(transactions)
    }

    /// Broadcast the signed inclusion list to the network.
    async fn broadcast_inclusion_list(
        &self,
        signed_inclusion_list: SignedInclusionList<S::E>,
    ) -> Result<(), String> {
        debug!(
            slot = %signed_inclusion_list.message.slot,
            validator_index = signed_inclusion_list.message.validator_index,
            "EIP7805: Broadcasting inclusion list"
        );

        let slot = signed_inclusion_list.message.slot;
        let validator_index = signed_inclusion_list.message.validator_index;

        // Submit to beacon node for gossip propagation
        self.beacon_nodes
            .request(ApiTopic::InclusionList, move |beacon_node| {
                let signed_inclusion_list = signed_inclusion_list.clone();
                async move {
                    beacon_node
                        .post_beacon_pool_inclusion_lists::<S::E>(&signed_inclusion_list)
                        .await
                        .map_err(|e| format!("Failed to broadcast inclusion list: {}", e))
                }
            })
            .await
            .map_err(|e| format!("Failed to submit inclusion list to beacon node: {}", e))?;

        info!(
            slot = %slot,
            validator_index = validator_index,
            "EIP7805: Successfully broadcast inclusion list"
        );

        Ok(())
    }
}

/// Calculate the submission due time in milliseconds from slot start.
pub fn get_inclusion_list_submission_due_ms(spec: &ChainSpec) -> u64 {
    // INCLUSION_LIST_SUBMISSION_DUE_BPS = 6667 (67% of slot duration)
    let slot_duration_ms = spec.seconds_per_slot * 1000;
    (slot_duration_ms * INCLUSION_LIST_SUBMISSION_DUE_BPS) / 10000
}

/// Calculate the view freeze cutoff time in milliseconds from slot start.
pub fn get_view_freeze_cutoff_ms(spec: &ChainSpec) -> u64 {
    // VIEW_FREEZE_CUTOFF_BPS = 7500 (75% of slot duration)
    let slot_duration_ms = spec.seconds_per_slot * 1000;
    (slot_duration_ms * VIEW_FREEZE_CUTOFF_BPS) / 10000
}

/// Calculate the proposer inclusion list cutoff time in milliseconds from slot start.
pub fn get_proposer_inclusion_list_cutoff_ms(spec: &ChainSpec) -> u64 {
    // PROPOSER_INCLUSION_LIST_CUTOFF_BPS = 9167 (92% of slot duration)
    let slot_duration_ms = spec.seconds_per_slot * 1000;
    (slot_duration_ms * PROPOSER_INCLUSION_LIST_CUTOFF_BPS) / 10000
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_calculations() {
        // Test with 12 second slot
        let spec = ChainSpec::mainnet();

        // Submission due: ~67% of 12s = ~8s = 8000ms
        let submission_due = get_inclusion_list_submission_due_ms(&spec);
        assert!(submission_due > 7000 && submission_due < 9000);

        // View freeze cutoff: 75% of 12s = 9s = 9000ms
        let view_freeze = get_view_freeze_cutoff_ms(&spec);
        assert_eq!(view_freeze, 9000);

        // Proposer cutoff: ~92% of 12s = ~11s = 11000ms
        let proposer_cutoff = get_proposer_inclusion_list_cutoff_ms(&spec);
        assert!(proposer_cutoff > 10000 && proposer_cutoff < 12000);
    }
}
