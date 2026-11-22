use crate::{BeaconChain, BeaconChainTypes};
use slot_clock::SlotClock;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use task_executor::TaskExecutor;
use tokio::time::sleep;
use tracing::{debug, error, warn};
use types::{ChainSpec, EthSpec, Slot};

/// Don't run the attestation simulator if the head slot is this many epochs
/// behind the wall-clock slot.
const SYNCING_TOLERANCE_EPOCHS: u64 = 2;

/// Spawns a routine which produces an unaggregated attestation at every slot.
///
/// This routine will run once per slot
pub fn start_attestation_simulator_service<T: BeaconChainTypes>(
    executor: TaskExecutor,
    chain: Arc<BeaconChain<T>>,
) {
    executor.clone().spawn(
        async move { attestation_simulator_service(executor, chain).await },
        "attestation_simulator_service",
    );
}

/// Loop indefinitely, calling `BeaconChain::produce_unaggregated_attestation` each slot at the
/// unaggregated attestation deadline.
async fn attestation_simulator_service<T: BeaconChainTypes>(
    executor: TaskExecutor,
    chain: Arc<BeaconChain<T>>,
) {
    let slot_duration = chain.slot_clock.slot_duration();
    let in_flight = Arc::new(AtomicBool::new(false));

    loop {
        match chain.slot_clock.now_duration() {
            Some(now_duration) => {
                let (attestation_slot, Some(time_to_deadline)) =
                    time_until_attestation_deadline::<T::EthSpec>(
                        &chain.slot_clock,
                        &chain.spec,
                        now_duration,
                    )
                else {
                    error!("Failed to calculate attestation deadline");
                    sleep(slot_duration).await;
                    continue;
                };
                sleep(time_to_deadline).await;

                debug!("Simulating unagg. attestation production");

                // Run the task in the executor
                let inner_chain = chain.clone();
                spawn_simulation_task(&executor, in_flight.clone(), move || {
                    if let Some(slot) = inner_chain.slot_clock.now()
                        && slot == attestation_slot
                    {
                        produce_unaggregated_attestation(inner_chain, attestation_slot);
                    } else {
                        warn!(%attestation_slot, "Missed attestation simulator slot due to lag");
                    }
                });
            }
            None => {
                error!("Failed to read slot clock");
                // If we can't read the slot clock, just wait another slot.
                sleep(slot_duration).await;
            }
        };
    }
}

struct SimulationPermit(Arc<AtomicBool>);

impl Drop for SimulationPermit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn spawn_simulation_task(
    executor: &TaskExecutor,
    in_flight: Arc<AtomicBool>,
    task: impl FnOnce() + Send + 'static,
) -> bool {
    if in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return false;
    }
    let permit = SimulationPermit(in_flight);
    // State/cache reads can wait for fork choice or the database. Keep them off
    // the core executor so lock contention cannot prevent network/exit progress.
    // A single permit also prevents one blocked read from growing a task backlog.
    executor.spawn_blocking(
        move || {
            let _permit = permit;
            task();
        },
        "attestation_simulator_service",
    );
    true
}

fn time_until_attestation_deadline<E: EthSpec>(
    slot_clock: &impl SlotClock,
    chain_spec: &ChainSpec,
    now: Duration,
) -> (Slot, Option<Duration>) {
    let attestation_slot = slot_clock
        .slot_of(now)
        .map_or_else(|| slot_clock.genesis_slot(), |slot| slot + 1);
    let duration_to_attestation_deadline = slot_clock
        .start_of(attestation_slot)
        .and_then(|slot_start| {
            slot_start.checked_add(chain_spec.get_attestation_due::<E>(attestation_slot))
        })
        .and_then(|deadline| deadline.checked_sub(now));
    (attestation_slot, duration_to_attestation_deadline)
}

pub fn produce_unaggregated_attestation<T: BeaconChainTypes>(
    chain: Arc<BeaconChain<T>>,
    current_slot: Slot,
) {
    // Don't run the attestation simulator when the head slot is far behind the
    // wall-clock slot.
    //
    // This helps prevent the simulator from becoming a burden by computing
    // committees from old states.
    let syncing_tolerance_slots = SYNCING_TOLERANCE_EPOCHS * T::EthSpec::slots_per_epoch();
    if chain.best_slot() + syncing_tolerance_slots < current_slot {
        return;
    }

    // Since attestations for different committees are practically identical (apart from the committee index field)
    // Committee 0 is guaranteed to exist. That means there's no need to load the committee.
    let beacon_committee_index = 0;

    // Store the unaggregated attestation in the validator monitor for later processing
    match chain.produce_unaggregated_attestation(current_slot, beacon_committee_index) {
        Ok(unaggregated_attestation) => {
            let data = unaggregated_attestation.data();

            debug!(
                attestation_source = data.source.root.to_string(),
                attestation_target = data.target.root.to_string(),
                "Produce unagg. attestation"
            );

            chain
                .validator_monitor
                .write()
                .set_unaggregated_attestation(unaggregated_attestation);
        }
        Err(e) => {
            debug!(
                error = ?e,
                "Failed to simulate attestation"
            );
        }
    }
}

#[cfg(test)]
mod async_progress_tests {
    use super::*;
    use std::sync::mpsc;
    use task_executor::test_utils::TestRuntime;

    #[tokio::test(flavor = "current_thread")]
    async fn blocked_simulation_preserves_async_progress_and_bounds_pending_work() {
        let runtime = TestRuntime::default();
        let in_flight = Arc::new(AtomicBool::new(false));
        let (entered_tx, entered_rx) = tokio::sync::oneshot::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (heartbeat_tx, heartbeat_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (progress_tx, progress_rx) = tokio::sync::oneshot::channel();
        let releaser = std::thread::spawn(move || {
            started_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            let progressed = heartbeat_rx.recv_timeout(Duration::from_secs(2)).is_ok();
            let _ = release_tx.send(());
            let _ = progress_tx.send(progressed);
        });
        assert!(spawn_simulation_task(
            &runtime.task_executor,
            in_flight.clone(),
            move || {
                started_tx.send(()).unwrap();
                entered_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
            }
        ));
        entered_rx.await.unwrap();
        let duplicate_was_rejected =
            !spawn_simulation_task(&runtime.task_executor, in_flight, || {});
        let _ = heartbeat_tx.send(());
        let async_progressed_before_work_released = progress_rx.await.unwrap();
        releaser.join().unwrap();
        assert!(
            async_progressed_before_work_released,
            "blocking simulation occupied the core executor"
        );
        assert!(
            duplicate_was_rejected,
            "a second simulation queued behind blocked work"
        );
    }
}
