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
    // Should only receive events for recent blocks, import_block filters by blocks close to clock.
    //
    // Intents to process SyncAggregates of all recent blocks sequentially, without skipping.
    // Uses a bounded receiver, so may drop some SyncAggregates if very overloaded. This is okay
    // since only the most recent updates have value.
    while let Some(event) = light_client_server_rv.next().await {
        let parent_root = event.0;

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
