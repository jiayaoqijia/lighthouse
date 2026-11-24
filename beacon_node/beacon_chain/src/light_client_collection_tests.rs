//! Unchanged independently generated current Gloas full-node collection bytes.
use crate::block_verification_types::LookupBlock;
use crate::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use crate::{ChainConfig, NotifyExecutionLayer};
use flate2::read::GzDecoder;
use ssz::Encode;
use std::{io::Read, sync::Arc};
use store::{DBColumn, HotColdDB, KeyValueStore, MemoryStore};
use types::*;
type E = MinimalEthSpec;
fn raw(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}
fn harness() -> BeaconChainHarness<EphemeralHarnessType<E>> {
    harness_with_sender(None)
}
fn harness_with_sender(
    sender: Option<futures::channel::mpsc::Sender<crate::LightClientProducerEvent<E>>>,
) -> BeaconChainHarness<EphemeralHarnessType<E>> {
    let config: Config = yaml_serde::from_slice(include_bytes!(
        "../tests/testdata/gloas-light-client-collection/config.yaml"
    ))
    .unwrap();
    let spec = Arc::new(ChainSpec::from_config::<E>(&config).unwrap());
    let state = BeaconState::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/state.ssz.gz"
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
            enable_light_client_server: sender.is_some(),
            ..Default::default()
        })
        .initial_mutator(Box::new(move |builder| match sender {
            Some(sender) => builder.light_client_server_tx(sender),
            None => builder,
        }))
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
    block
}
async fn produce(h: &BeaconChainHarness<EphemeralHarnessType<E>>, block: &SignedBeaconBlock<E>) {
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(
        h.chain.canonical_head.cached_head().head_block_root(),
        block.canonical_root()
    );
    h.chain
        .recompute_and_cache_light_client_updates((
            block.parent_root(),
            block.slot(),
            block.message().body().sync_aggregate().unwrap().clone(),
        ))
        .unwrap();
}
#[tokio::test]
async fn insufficient_sync_participation_keeps_bootstrap_but_no_updates() {
    let h = harness();
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz"),
    )
    .await;
    assert_eq!(
        block
            .message()
            .body()
            .sync_aggregate()
            .unwrap()
            .sync_committee_bits
            .num_set_bits(),
        0
    );
    produce(&h, &block).await;
    assert!(
        h.chain.get_light_client_updates(0, 1).unwrap().is_empty(),
        "zero-participant block created a period update"
    );
    assert!(
        h.chain
            .light_client_server_cache
            .get_latest_finality_update()
            .is_none()
    );
    assert!(
        h.chain
            .light_client_server_cache
            .get_latest_optimistic_update()
            .is_none()
    );
    let (bootstrap, _) = h
        .chain
        .light_client_server_cache
        .get_light_client_bootstrap(&h.chain.store, &h.chain.genesis_block_root, 0, &h.spec)
        .unwrap()
        .unwrap();
    assert_eq!(
        bootstrap.as_ssz_bytes(),
        raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/bootstrap-0.ssz.gz"
        ))
    );
}
#[tokio::test]
async fn current_reference_genesis_finality_and_first_qualified_updates() {
    let h = harness();
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz"),
    )
    .await;
    produce(&h, &block).await;
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-2.ssz.gz"),
    )
    .await;
    assert!(
        block
            .message()
            .body()
            .sync_aggregate()
            .unwrap()
            .sync_committee_bits
            .num_set_bits()
            > 0
    );
    produce(&h, &block).await;
    let updates = h.chain.get_light_client_updates(0, 1).unwrap();
    assert_eq!(updates.len(), 1);
    assert_eq!(
        updates[0].as_ssz_bytes(),
        raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/update-0.ssz.gz"
        ))
    );
    assert_eq!(
        h.chain
            .light_client_server_cache
            .get_latest_finality_update()
            .expect("genesis finality proof missing")
            .as_ssz_bytes(),
        raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/finality-0.ssz.gz"
        ))
    );
    assert_eq!(
        h.chain
            .light_client_server_cache
            .get_latest_optimistic_update()
            .unwrap()
            .as_ssz_bytes(),
        raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/optimistic-0.ssz.gz"
        ))
    );
}

#[tokio::test]
async fn legacy_zero_participant_period_update_is_not_served() {
    let h = harness();
    let mut update = LightClientUpdate::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/update-0.ssz.gz"
        )),
        &ForkName::Gloas,
    )
    .unwrap();
    match &mut update {
        LightClientUpdate::Gloas(update) => {
            update.sync_aggregate.sync_committee_bits = Default::default()
        }
        _ => unreachable!(),
    }
    h.chain.store.store_light_client_update(0, &update).unwrap();
    assert_eq!(
        h.chain.store.get_light_client_updates(0, 1).unwrap().len(),
        1,
        "legacy negative fixture was not actually stored"
    );
    assert!(
        h.chain.get_light_client_updates(0, 1).unwrap().is_empty(),
        "legacy invalid update was served from durable state"
    );
}

#[tokio::test]
async fn actual_canonical_reorg_clears_other_branch_updates() {
    let h = harness();
    for raw in [
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz").as_slice(),
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-2.ssz.gz").as_slice(),
    ] {
        let block = import(&h, raw).await;
        h.chain.recompute_head_at_current_slot().await;
        assert_eq!(
            h.chain.canonical_head.cached_head().head_block_root(),
            block.canonical_root(),
            "fixture branch was not actually selected by native fork choice"
        );
        produce(&h, &block).await;
    }
    assert_eq!(h.chain.get_light_client_updates(0, 1).unwrap().len(), 1);
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-3.ssz.gz"),
    )
    .await;
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(
        h.chain.canonical_head.cached_head().head_block_root(),
        block.canonical_root(),
        "native reorg precondition missing"
    );
    produce(&h, &block).await;
    assert!(
        h.chain.get_light_client_updates(0, 1).unwrap().is_empty(),
        "period update leaked from the old canonical branch"
    );
    assert!(
        h.chain
            .light_client_server_cache
            .get_latest_finality_update()
            .is_none(),
        "finality update leaked from the old branch"
    );
    assert!(
        h.chain
            .light_client_server_cache
            .get_latest_optimistic_update()
            .is_none(),
        "optimistic update leaked from the old branch"
    );
}

#[tokio::test]
async fn cached_bootstrap_branch_rejects_a_mismatched_period_committee() {
    let h = harness();
    let expected = raw(include_bytes!(
        "../tests/testdata/gloas-light-client-collection/bootstrap-0.ssz.gz"
    ));
    let (first, _) = h
        .chain
        .get_light_client_bootstrap(&h.chain.genesis_block_root)
        .unwrap()
        .unwrap();
    assert_eq!(first.as_ssz_bytes(), expected);
    let state = h.get_current_state();
    let mut wrong = state.current_sync_committee().unwrap().as_ref().clone();
    let different = wrong
        .pubkeys
        .iter()
        .position(|key| *key != wrong.pubkeys[0])
        .unwrap();
    wrong.pubkeys[0] = wrong.pubkeys[different];
    h.chain.store.store_sync_committee(0, &wrong).unwrap();
    for _ in 0..2 {
        let (actual, _) = h
            .chain
            .get_light_client_bootstrap(&h.chain.genesis_block_root)
            .unwrap()
            .unwrap();
        assert_eq!(
            actual.as_ssz_bytes(),
            expected,
            "cached branch paired with another period committee"
        );
    }
}
#[tokio::test]
async fn identical_committee_content_is_persisted_for_each_period() {
    let h = harness();
    let mut state = h.get_current_state();
    let cached = super::LightClientCachedData::from_state(&mut state).unwrap();
    let cache = &h.chain.light_client_server_cache;
    cache
        .store_current_sync_committee(&h.chain.store, &cached, 0, 0)
        .unwrap();
    cache
        .store_current_sync_committee(&h.chain.store, &cached, 1, 0)
        .unwrap();
    assert_eq!(
        h.chain.store.get_sync_committee(1).unwrap().unwrap(),
        *cached.current_sync_committee
    );
}

#[tokio::test]
async fn failed_reorg_batch_keeps_previous_view_and_retries_atomically() {
    let h = harness();
    for bytes in [
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz").as_slice(),
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-2.ssz.gz").as_slice(),
    ] {
        let block = import(&h, bytes).await;
        produce(&h, &block).await;
    }
    let previous = h.chain.get_light_client_updates(0, 1).unwrap()[0].as_ssz_bytes();
    let latest = h
        .chain
        .light_client_server_cache
        .get_latest_optimistic_update()
        .unwrap();
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-3.ssz.gz"),
    )
    .await;
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(
        h.chain.canonical_head.cached_head().head_block_root(),
        block.canonical_root()
    );
    h.chain.store.hot_db.inject_fault_on_next_atomic_write();
    let cache = &h.chain.light_client_server_cache;
    assert!(
        cache
            .recompute_for_head(h.chain.store.clone(), block.canonical_root(), &h.spec)
            .is_err()
    );
    assert_eq!(
        h.chain.get_light_client_updates(0, 1).unwrap()[0].as_ssz_bytes(),
        previous
    );
    assert_eq!(cache.get_latest_optimistic_update().unwrap(), latest);
    cache
        .recompute_for_head(h.chain.store.clone(), block.canonical_root(), &h.spec)
        .unwrap();
    assert!(h.chain.get_light_client_updates(0, 1).unwrap().is_empty());
    assert!(cache.get_latest_optimistic_update().is_none());
    assert!(cache.get_latest_finality_update().is_none());
}

#[tokio::test]
async fn cold_cache_reconciles_noncanonical_durable_records() {
    let h = harness();
    for bytes in [
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz").as_slice(),
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-2.ssz.gz").as_slice(),
    ] {
        let block = import(&h, bytes).await;
        produce(&h, &block).await;
    }
    assert_eq!(h.chain.get_light_client_updates(0, 1).unwrap().len(), 1);
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-3.ssz.gz"),
    )
    .await;
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(
        h.chain.canonical_head.cached_head().head_block_root(),
        block.canonical_root()
    );
    let stale = h.chain.get_light_client_updates(0, 1).unwrap().remove(0);
    h.chain.store.store_light_client_update(99, &stale).unwrap();
    let cold = super::LightClientServerCache::<EphemeralHarnessType<E>>::new();
    cold.recompute_for_head(h.chain.store.clone(), block.canonical_root(), &h.spec)
        .unwrap();
    assert!(h.chain.get_light_client_updates(0, 1).unwrap().is_empty());
    assert!(cold.get_latest_optimistic_update().is_none());
    assert!(h.chain.store.get_light_client_update(99).unwrap().is_none());
}

#[tokio::test]
async fn canonical_head_change_schedules_a_new_serving_projection() {
    let (sender, mut receiver) = futures::channel::mpsc::channel(16);
    let h = harness_with_sender(Some(sender));
    let block = import(
        &h,
        include_bytes!("../tests/testdata/gloas-light-client-collection/block-1.ssz.gz"),
    )
    .await;
    // Consume import notifications before selecting the new head. Import-time
    // computation alone can observe the previous head and must not be relied on.
    while receiver.try_recv().is_ok() {}
    h.chain.recompute_head_at_current_slot().await;
    assert_eq!(
        h.chain.canonical_head.cached_head().head_block_root(),
        block.canonical_root()
    );
    let event = receiver
        .try_recv()
        .expect("head selection emitted no light-client work");
    assert_eq!(event.0, block.parent_root());
    assert_eq!(event.1, block.slot());
    h.chain
        .recompute_and_cache_light_client_updates(event)
        .unwrap();
}

#[tokio::test]
async fn durable_update_schema_handles_a_fork_inside_a_committee_period() {
    let h = harness();
    let mut spec = (*h.spec).clone();
    spec.gloas_fork_epoch = Some(Epoch::new(1));
    let mut update = LightClientUpdate::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-light-client-collection/update-0.ssz.gz"
        )),
        &ForkName::Gloas,
    )
    .unwrap();
    let LightClientUpdate::Gloas(inner) = &mut update else {
        unreachable!()
    };
    inner.attested_header.beacon.slot = Slot::new(E::slots_per_epoch());
    inner.signature_slot = inner.attested_header.beacon.slot + 1;
    assert_ne!(spec.fork_name_at_epoch(Epoch::new(0)), ForkName::Gloas);
    let store = HotColdDB::<E, MemoryStore, MemoryStore>::open_ephemeral(
        Default::default(),
        Arc::new(spec),
    )
    .unwrap();
    store.store_light_client_update(0, &update).unwrap();
    assert_eq!(
        store
            .get_light_client_update(0)
            .unwrap()
            .unwrap()
            .as_ssz_bytes(),
        update.as_ssz_bytes()
    );
    assert_eq!(
        store.get_light_client_updates(0, 1).unwrap()[0].as_ssz_bytes(),
        update.as_ssz_bytes()
    );
    let tagged = store
        .hot_db
        .get_bytes(DBColumn::LightClientUpdate, &0u64.to_be_bytes())
        .unwrap()
        .unwrap();
    assert_ne!(
        tagged,
        update.as_ssz_bytes(),
        "new write omitted schema identity"
    );
    // An actual legacy untagged row must decode with its attested-slot schema.
    store
        .hot_db
        .put_bytes(
            DBColumn::LightClientUpdate,
            &0u64.to_be_bytes(),
            &update.as_ssz_bytes(),
        )
        .unwrap();
    assert_eq!(
        store
            .get_light_client_update(0)
            .unwrap()
            .unwrap()
            .as_ssz_bytes(),
        update.as_ssz_bytes()
    );
    for malformed in [
        b"LCUP".to_vec(),
        b"LCUP\x02\x05".to_vec(),
        b"LCUP\x01\xff".to_vec(),
        vec![0; 17],
    ] {
        store
            .hot_db
            .put_bytes(DBColumn::LightClientUpdate, &0u64.to_be_bytes(), &malformed)
            .unwrap();
        assert!(store.get_light_client_update(0).is_err());
        assert!(store.get_light_client_updates(0, 1).is_err());
    }
}
