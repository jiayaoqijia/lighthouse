//! Real producer/channel consumers using unchanged current-reference signed blocks.
use super::*;
use beacon_chain::block_verification_types::LookupBlock;
use beacon_chain::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use beacon_chain::{ChainConfig, NotifyExecutionLayer};
use flate2::read::GzDecoder;
use std::io::Read;
use types::*;

type E = MinimalEthSpec;
fn raw(bytes: &[u8]) -> Vec<u8> {
    let mut output = vec![];
    GzDecoder::new(bytes).read_to_end(&mut output).unwrap();
    output
}
fn harness() -> BeaconChainHarness<EphemeralHarnessType<E>> {
    let config: Config = yaml_serde::from_slice(include_bytes!(
        "../../beacon_chain/tests/testdata/gloas-light-client-collection/config.yaml"
    ))
    .unwrap();
    let spec = Arc::new(ChainSpec::from_config::<E>(&config).unwrap());
    let state = BeaconState::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../../beacon_chain/tests/testdata/gloas-light-client-collection/state.ssz.gz"
        )),
        &spec,
    )
    .unwrap();
    let genesis_time = state.genesis_time();
    BeaconChainHarness::builder(E::default())
        .spec(spec)
        .keypairs(vec![])
        .chain_config(ChainConfig {
            archive: true,
            ..Default::default()
        })
        .genesis_state_ephemeral_store(state)
        .mock_execution_layer()
        .mock_execution_layer_all_payloads_valid()
        .recalculate_fork_times_with_genesis(genesis_time)
        .build()
}
async fn import(
    h: &BeaconChainHarness<EphemeralHarnessType<E>>,
    bytes: &[u8],
) -> SignedBeaconBlock<E> {
    let block = SignedBeaconBlock::<E>::from_ssz_bytes(&raw(bytes), &h.spec).unwrap();
    h.set_current_slot(block.slot());
    let root = block.canonical_root();
    h.chain
        .process_block(
            root,
            LookupBlock::new(root, Arc::new(block.clone())),
            NotifyExecutionLayer::Yes,
            BlockImportSource::Lookup,
            || Ok(()),
        )
        .await
        .unwrap();
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(h.chain.canonical_head.cached_head().head_block_root(), root);
    block
}
fn event(block: &SignedBeaconBlock<E>) -> LightClientProducerEvent<E> {
    (
        block.parent_root(),
        block.slot(),
        block.message().body().sync_aggregate().unwrap().clone(),
    )
}
async fn run(
    h: &BeaconChainHarness<EphemeralHarnessType<E>>,
    event: LightClientProducerEvent<E>,
) -> tokio::sync::mpsc::Receiver<WorkEvent<E>> {
    let (mut tx, rx) = futures::channel::mpsc::channel(1);
    tx.try_send(event).unwrap();
    drop(tx);
    let (processor, result) = tokio::sync::mpsc::channel(4);
    compute_light_client_updates(h.chain.clone(), rx, BeaconProcessorSend(processor)).await;
    result
}

#[tokio::test]
async fn queued_older_event_notifies_the_actual_canonical_update_root() {
    let h = harness();
    let first = import(
        &h,
        include_bytes!(
            "../../beacon_chain/tests/testdata/gloas-light-client-collection/block-2.ssz.gz"
        ),
    )
    .await;
    let stale = event(&first);
    let middle = import(
        &h,
        include_bytes!(
            "../../beacon_chain/tests/testdata/gloas-light-client-collection/block-4.ssz.gz"
        ),
    )
    .await;
    let second = import(
        &h,
        include_bytes!(
            "../../beacon_chain/tests/testdata/gloas-light-client-collection/block-5.ssz.gz"
        ),
    )
    .await;
    assert_eq!(middle.parent_root(), first.canonical_root());
    assert_eq!(second.parent_root(), middle.canonical_root());
    assert!(
        second
            .message()
            .body()
            .sync_aggregate()
            .unwrap()
            .sync_committee_bits
            .num_set_bits()
            > 0
    );
    assert_ne!(
        stale.0,
        second.parent_root(),
        "fixture does not distinguish the queued event from the current update"
    );
    let mut results = run(&h, stale).await;
    let notification = results
        .try_recv()
        .expect("actual worker did not notify the processor");
    let Work::Reprocess(ReprocessQueueMessage::NewLightClientOptimisticUpdate { parent_root }) =
        notification.work
    else {
        panic!("unexpected producer notification")
    };
    let actual = h
        .chain
        .light_client_server_cache
        .get_latest_optimistic_update()
        .unwrap();
    assert_eq!(actual.get_canonical_root(), second.parent_root());
    assert_eq!(
        parent_root,
        actual.get_canonical_root(),
        "worker notified the old queued event instead of the actual cached update"
    );
    assert!(results.try_recv().is_err());
}

#[tokio::test]
async fn insufficient_participation_does_not_notify_a_nonexistent_update() {
    let h = harness();
    let first = import(
        &h,
        include_bytes!(
            "../../beacon_chain/tests/testdata/gloas-light-client-collection/block-1.ssz.gz"
        ),
    )
    .await;
    assert_eq!(
        first
            .message()
            .body()
            .sync_aggregate()
            .unwrap()
            .sync_committee_bits
            .num_set_bits(),
        0
    );
    let mut results = run(&h, event(&first)).await;
    assert!(
        h.chain
            .light_client_server_cache
            .get_latest_optimistic_update()
            .is_none()
    );
    assert!(
        results.try_recv().is_err(),
        "worker announced data that does not exist"
    );
}
