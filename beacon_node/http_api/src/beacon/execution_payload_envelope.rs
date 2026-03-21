use crate::task_spawner::{Priority, TaskSpawner};
use crate::utils::{ChainFilter, EthV1Filter, NetworkTxFilter, ResponseFilter, TaskSpawnerFilter};
use beacon_chain::{BeaconChain, BeaconChainTypes, NotifyExecutionLayer};
use bytes::Bytes;
use eth2::{CONTENT_TYPE_HEADER, SSZ_CONTENT_TYPE_HEADER};
use lighthouse_network::PubsubMessage;
use network::NetworkMessage;
use ssz::Decode;
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tracing::{debug, info, warn};
use types::{SignedExecutionPayloadEnvelope, block::BlockImportSource};
use warp::{Filter, Rejection, Reply, reply::Response};

// POST beacon/execution_payload_envelope (SSZ)
pub(crate) fn post_beacon_execution_payload_envelope_ssz<T: BeaconChainTypes>(
    eth_v1: EthV1Filter,
    task_spawner_filter: TaskSpawnerFilter<T>,
    chain_filter: ChainFilter<T>,
    network_tx_filter: NetworkTxFilter<T>,
) -> ResponseFilter {
    eth_v1
        .and(warp::path("beacon"))
        .and(warp::path("execution_payload_envelope"))
        .and(warp::path::end())
        .and(warp::header::exact(
            CONTENT_TYPE_HEADER,
            SSZ_CONTENT_TYPE_HEADER,
        ))
        .and(warp::body::bytes())
        .and(task_spawner_filter)
        .and(chain_filter)
        .and(network_tx_filter)
        .then(
            |body_bytes: Bytes,
             task_spawner: TaskSpawner<T::EthSpec>,
             chain: Arc<BeaconChain<T>>,
             network_tx: UnboundedSender<NetworkMessage<T::EthSpec>>| {
                task_spawner.spawn_async_with_rejection(Priority::P0, async move {
                    let envelope =
                        SignedExecutionPayloadEnvelope::<T::EthSpec>::from_ssz_bytes(&body_bytes)
                            .map_err(|e| {
                            warp_utils::reject::custom_bad_request(format!("invalid SSZ: {e:?}"))
                        })?;
                    publish_execution_payload_envelope(envelope, chain, &network_tx).await
                })
            },
        )
        .boxed()
}

// POST beacon/execution_payload_envelope
pub(crate) fn post_beacon_execution_payload_envelope<T: BeaconChainTypes>(
    eth_v1: EthV1Filter,
    task_spawner_filter: TaskSpawnerFilter<T>,
    chain_filter: ChainFilter<T>,
    network_tx_filter: NetworkTxFilter<T>,
) -> ResponseFilter {
    eth_v1
        .and(warp::path("beacon"))
        .and(warp::path("execution_payload_envelope"))
        .and(warp::path::end())
        .and(warp::body::json())
        .and(task_spawner_filter.clone())
        .and(chain_filter.clone())
        .and(network_tx_filter.clone())
        .then(
            |envelope: SignedExecutionPayloadEnvelope<T::EthSpec>,
             task_spawner: TaskSpawner<T::EthSpec>,
             chain: Arc<BeaconChain<T>>,
             network_tx: UnboundedSender<NetworkMessage<T::EthSpec>>| {
                task_spawner.spawn_async_with_rejection(Priority::P0, async move {
                    publish_execution_payload_envelope(envelope, chain, &network_tx).await
                })
            },
        )
        .boxed()
}
/// Publishes a signed execution payload envelope to the network and processes it locally.
///
/// This is critical for locally produced envelopes: they must be processed locally
/// (sent to the EL via engine_newPayload) just like gossip-received envelopes.
pub async fn publish_execution_payload_envelope<T: BeaconChainTypes>(
    envelope: SignedExecutionPayloadEnvelope<T::EthSpec>,
    chain: Arc<BeaconChain<T>>,
    network_tx: &UnboundedSender<NetworkMessage<T::EthSpec>>,
) -> Result<Response, Rejection> {
    let slot = envelope.message.slot;
    let beacon_block_root = envelope.message.beacon_block_root;

    // TODO(gloas): Replace this check once we have gossip validation.
    if !chain.spec.is_gloas_scheduled() {
        return Err(warp_utils::reject::custom_bad_request(
            "Execution payload envelopes are not supported before the Gloas fork".into(),
        ));
    }

    info!(
        %slot,
        %beacon_block_root,
        builder_index = envelope.message.builder_index,
        "Publishing signed execution payload envelope to network"
    );

    // First, verify the envelope (same as gossip verification)
    let envelope_arc = Arc::new(envelope);
    let verified_envelope = chain
        .verify_envelope_for_gossip(envelope_arc.clone())
        .await
        .map_err(|e| {
            warn!(%slot, error = ?e, "Failed to verify execution payload envelope");
            warp_utils::reject::custom_bad_request(format!(
                "Invalid execution payload envelope: {e}"
            ))
        })?;

    // Publish to the network
    crate::utils::publish_pubsub_message(
        network_tx,
        PubsubMessage::ExecutionPayload(Box::new((*envelope_arc).clone())),
    )
    .map_err(|_| {
        warn!(%slot, "Failed to publish execution payload envelope to network");
        warp_utils::reject::custom_server_error(
            "Unable to publish execution payload envelope to network".into(),
        )
    })?;

    // Process the envelope locally (this sends it to the EL)
    debug!(%slot, %beacon_block_root, "Processing locally produced envelope");
    
    let result = chain
        .process_execution_payload_envelope(
            beacon_block_root,
            verified_envelope,
            NotifyExecutionLayer::Yes,
            BlockImportSource::HttpApi,
            || Ok(()),
        )
        .await;

    match result {
        Ok(_) => {
            debug!(%slot, %beacon_block_root, "Locally produced envelope processed successfully");
        }
        Err(e) => {
            warn!(%slot, %beacon_block_root, error = ?e, "Failed to process locally produced envelope");
        }
    }

    Ok(warp::reply().into_response())
}
