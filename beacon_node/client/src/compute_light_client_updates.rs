use beacon_chain::{BeaconChain, BeaconChainTypes, LightClientProducerEvent};
use beacon_processor::work_reprocessing_queue::ReprocessQueueMessage;
use beacon_processor::{BeaconProcessorSend, Work, WorkEvent};
use futures::StreamExt;
use futures::channel::mpsc::Receiver;
use std::sync::Arc;
use tracing::{debug, error};

// Each `LightClientProducerEvent` is ~200 bytes. With the light_client server producing only recent
// updates it is okay to drop some events in case of overloading. In normal network conditions
// there's one event emitted per block at most every 12 seconds, while consuming the event should
// take a few milliseconds. 32 is a small enough arbitrary number.
pub(crate) const LIGHT_CLIENT_SERVER_CHANNEL_CAPACITY: usize = 32;

pub async fn compute_light_client_updates<T: BeaconChainTypes>(
    chain: Arc<BeaconChain<T>>,
    mut light_client_server_rv: Receiver<LightClientProducerEvent<T::EthSpec>>,
    beacon_processor_send: BeaconProcessorSend<T::EthSpec>,
) {
    // Events are wakeups: the selected head can advance while work is queued.
    // Reconstruction derives all relevant aggregates on that canonical branch.
    while let Some(event) = light_client_server_rv.next().await {
        let inner = chain.clone();
        let Some(task) = chain.task_executor.clone().spawn_blocking_handle(
            move || inner.recompute_and_cache_light_client_updates(event),
            "light_client_canonical_updates",
        ) else {
            break;
        };
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                debug!(?error, "Failed to compute canonical light-client updates");
                continue;
            }
            Err(error) => {
                debug!(?error, "Light-client reconstruction task failed");
                continue;
            }
        }

        // Wake only consumers of data that was actually materialized. Using the
        // old event's parent can miss the new update, or drain a pending update
        // before any eligible aggregate exists.
        let Some(update) = chain
            .light_client_server_cache
            .get_latest_optimistic_update()
        else {
            continue;
        };
        let parent_root = update.get_canonical_root();
        let msg = ReprocessQueueMessage::NewLightClientOptimisticUpdate { parent_root };
        if beacon_processor_send
            .try_send(WorkEvent {
                drop_during_sync: true,
                work: Work::Reprocess(msg),
            })
            .is_err()
        {
            error!(%parent_root,"Failed to inform light client update")
        };
    }
}

#[cfg(test)]
#[path = "light_client_updates_tests.rs"]
mod tests;
