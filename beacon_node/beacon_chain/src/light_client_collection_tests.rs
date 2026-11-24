//! Unchanged independently generated current Gloas full-node collection bytes.
use crate::block_verification_types::LookupBlock;
use crate::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use crate::{ChainConfig, NotifyExecutionLayer};
use flate2::read::GzDecoder;
use ssz::Encode;
use std::{io::Read, sync::Arc};
use types::*;
type E = MinimalEthSpec;
fn raw(bytes: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}
fn harness() -> BeaconChainHarness<EphemeralHarnessType<E>> {
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
    block
}
fn produce(h: &BeaconChainHarness<EphemeralHarnessType<E>>, block: &SignedBeaconBlock<E>) {
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
    produce(&h, &block);
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
    produce(&h, &block);
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
    produce(&h, &block);
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
