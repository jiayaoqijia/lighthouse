use crate::duties_service::DutiesService;
use eth2::types::InclusionListDuty;
use futures::future::join_all;
use slot_clock::SlotClock;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::{Instant, sleep, timeout_at};
use tracing::warn;
use types::{ForkName, Hash256, Slot};
use validator_store::ValidatorStore;

pub fn start<S: ValidatorStore + 'static, T: SlotClock + 'static>(
    duties: Arc<DutiesService<S, T>>,
) {
    if !duties.spec.is_heze_scheduled() {
        return;
    }
    duties.executor.clone().spawn(
        async move {
            let clock = &duties.slot_clock;
            let mut last_slot = clock.now();
            loop {
                sleep(
                    clock
                        .duration_to_next_slot()
                        .unwrap_or_else(|| clock.slot_duration()),
                )
                .await;
                let Some(slot) = clock.now() else { continue };
                if last_slot.is_some_and(|previous| previous >= slot) {
                    continue;
                }
                last_slot = Some(slot);
                let fork = duties.spec.fork_name_at_slot::<S::E>(slot);
                if !fork.heze_enabled() {
                    continue;
                }
                let Some(remaining) =
                    remaining_until_due(clock, slot, duties.spec.get_inclusion_list_due())
                else {
                    continue;
                };
                let deadline = Instant::now() + remaining;
                // Give the BN time to import the current slot's block while reserving time
                // for the Engine request, signing and publication.
                sleep(remaining / 2).await;
                let Some((dependent_root, assignments)) = duties.get_il_duties_for_slot(slot)
                else {
                    continue;
                };
                if timeout_at(
                    deadline,
                    join_all(
                        assignments
                            .into_iter()
                            .map(|duty| publish(&duties, fork, dependent_root, duty)),
                    ),
                )
                .await
                .is_err()
                {
                    warn!(%slot, "Inclusion list publication deadline elapsed");
                }
            }
        },
        "inclusion_list_service",
    );
}

fn remaining_until_due(clock: &impl SlotClock, slot: Slot, due: Duration) -> Option<Duration> {
    clock
        .start_of(slot)?
        .checked_add(due)?
        .checked_sub(clock.now_duration()?)
}

async fn publish<S: ValidatorStore + 'static, T: SlotClock + 'static>(
    duties: &DutiesService<S, T>,
    fork: ForkName,
    dependent_root: Hash256,
    duty: InclusionListDuty,
) {
    let slot = duty.slot;
    let validator_index = duty.validator_index;
    let candidate = duties
        .beacon_nodes
        .first_success(|beacon_node| async move {
            let list = beacon_node
                .get_validator_inclusion_list(slot, validator_index, dependent_root)
                .await
                .map_err(|error| format!("{error:?}"))?
                .data;
            if list.slot != slot
                || list.validator_index != validator_index
                || list.dependent_root != dependent_root
            {
                return Err("Beacon node returned a mismatched inclusion list duty".into());
            }
            Ok::<_, String>(list)
        })
        .await;
    let candidate = match candidate {
        Ok(candidate) if !candidate.transactions.is_empty() => candidate,
        Ok(_) => return,
        Err(error) => {
            warn!(%slot, validator_index, %error, "Unable to retrieve inclusion list");
            return;
        }
    };
    let signed = match duties
        .validator_store
        .sign_inclusion_list(duty.pubkey, candidate)
        .await
    {
        Ok(list) => list,
        Err(error) => {
            warn!(%slot, validator_index, ?error, "Unable to sign inclusion list");
            return;
        }
    };
    if let Err(error) = duties
        .beacon_nodes
        .broadcast(|beacon_node| {
            let signed = &signed;
            async move {
                beacon_node
                    .post_validator_inclusion_list(signed, fork)
                    .await
            }
        })
        .await
    {
        warn!(%slot, validator_index, %error, "Unable to publish inclusion list to all beacon nodes");
    }
}
