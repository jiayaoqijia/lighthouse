use beacon_chain::AvailabilityProcessingStatus::{Imported, MissingComponents};
use beacon_chain::NotifyExecutionLayer;
use beacon_chain::payload_envelope_verification::{EnvelopeError, EnvelopeSource};
use beacon_chain::test_utils::{
    BeaconChainHarness, fork_name_from_env, generate_data_column_sidecars_from_block, test_spec,
};
use bls::PublicKeyBytes;
use eth2::types::EventKind;
use proto_array::ExecutionStatus;
use std::sync::Arc;
use types::{
    Address, BlockImportSource, Epoch, ExecPayload, ForkName, Hash256, Slot, Spec,
    WithdrawalRequest,
};

type E = Spec;

#[tokio::test]
async fn pre_gloas_block_import_records_payload_gas_limit() {
    if fork_name_from_env() != Some(ForkName::Fulu) {
        return;
    }

    let mut spec = test_spec::<E>();
    spec.gloas_fork_epoch = Some(Epoch::new(1));
    let harness = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(spec))
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let head = harness.chain.head_beacon_block();
    let payload = head
        .message()
        .execution_payload()
        .expect("Fulu block should contain an execution payload");
    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(payload.block_hash()),
        Some(payload.gas_limit())
    );
}

#[tokio::test]
async fn pre_gloas_block_import_skips_cache_without_scheduled_gloas() {
    if fork_name_from_env() != Some(ForkName::Fulu) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let head = harness.chain.head_beacon_block();
    let payload = head
        .message()
        .execution_payload()
        .expect("Fulu block should contain an execution payload");
    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(payload.block_hash()),
        None
    );
}

#[tokio::test]
async fn startup_seeds_gloas_genesis_parent_payload() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    let head = harness.chain.canonical_head.cached_head();
    let genesis_bid = head
        .snapshot
        .beacon_state
        .latest_execution_payload_bid()
        .expect("Gloas genesis should contain an execution payload bid");
    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(genesis_bid.parent_block_hash()),
        Some(genesis_bid.gas_limit())
    );
}

#[tokio::test]
async fn lookup_imports_gloas_payload_after_restart() {
    if !fork_name_from_env().is_some_and(|fork| fork.gloas_enabled()) {
        return;
    }

    let spec = Arc::new(test_spec::<E>());
    let harness = BeaconChainHarness::builder(E::default())
        .spec(spec.clone())
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    harness.execution_block_generator().set_min_blob_count(1);

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, envelope, _) = harness.make_block_with_envelope(state, target_slot).await;
    let block_root = block_contents.0.canonical_root();
    let custody_columns =
        generate_data_column_sidecars_from_block(&block_contents.0, &harness.chain.spec);
    assert!(
        !custody_columns.is_empty(),
        "test block should contain blobs"
    );

    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");
    harness
        .chain
        .persist_fork_choice()
        .expect("fork choice should persist");

    let store = harness.chain.store.clone();
    let slot_clock = harness.chain.slot_clock.clone();
    drop(harness);
    let resumed = BeaconChainHarness::builder(E::default())
        .spec(spec)
        .deterministic_keypairs(64)
        .resumed_ephemeral_store(store)
        .mock_execution_layer()
        .mock_execution_layer_all_payloads_valid()
        .testing_slot_clock(slot_clock)
        .build();
    let chain = &resumed.chain;
    let cache = &chain.pending_payload_cache;
    let envelope = Arc::new(envelope.expect("Gloas block should produce an envelope"));

    assert!(
        cache.get_bid(&block_root).is_none(),
        "the pending bid cache should start empty after restart"
    );
    let column_status = chain
        .process_rpc_custody_columns(custody_columns.clone())
        .await
        .expect("custody columns should be accepted with an empty cache");
    assert!(matches!(column_status, MissingComponents(..)));
    assert!(cache.get_bid(&block_root).is_some());

    // Evict the columns too, so the envelope arrives first.
    cache.do_maintenance(Epoch::new(1)).unwrap();
    assert!(cache.get_bid(&block_root).is_none());
    let verified_envelope = chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Rpc)
        .await
        .expect("envelope should verify");
    let envelope_status = chain
        .process_execution_payload_envelope(
            block_root,
            verified_envelope,
            NotifyExecutionLayer::Yes,
            BlockImportSource::Lookup,
            || Ok(()),
        )
        .await
        .expect("envelope should be accepted after restart");
    assert!(matches!(envelope_status, MissingComponents(..)));
    assert!(cache.get_bid(&block_root).is_some());
    let import_status = chain
        .process_rpc_custody_columns(custody_columns)
        .await
        .expect("custody columns should complete the payload import");
    assert!(matches!(import_status, Imported(..)));
}

/// An envelope whose `execution_requests` don't hash to the bid's committed
/// `execution_requests_root` must be rejected by the full gossip verification path.
#[tokio::test]
async fn gossip_rejects_execution_requests_root_mismatch() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let mut signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let block_hash = signed_envelope.message.payload.block_hash;
    signed_envelope
        .message
        .execution_requests
        .withdrawals
        .push(WithdrawalRequest {
            source_address: Address::repeat_byte(0),
            validator_pubkey: PublicKeyBytes::empty(),
            amount: 0,
        })
        .unwrap();

    let result = harness
        .chain
        .verify_envelope_for_gossip(Arc::new(signed_envelope), EnvelopeSource::Gossip)
        .await;
    assert!(matches!(
        result,
        Err(EnvelopeError::ExecutionRequestsRootMismatch { .. })
    ));
    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(block_hash),
        None
    );
}

#[tokio::test]
async fn gossip_verified_envelope_records_payload_gas_limit() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let block_hash = signed_envelope.message.payload.block_hash;
    let gas_limit = signed_envelope.message.payload.gas_limit;
    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(block_hash),
        None
    );

    harness
        .chain
        .verify_envelope_for_gossip(Arc::new(signed_envelope), EnvelopeSource::Gossip)
        .await
        .expect("envelope should pass gossip verification");

    assert_eq!(
        harness
            .chain
            .observed_execution_payloads
            .get_gas_limit(block_hash),
        Some(gas_limit)
    );
}

#[tokio::test]
async fn gossip_ignores_subsequent_envelope_from_same_builder() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let envelope = Arc::new(signed_envelope);

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Gossip)
        .await
        .expect("first envelope should pass gossip verification");
    let second_result = harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Gossip)
        .await;

    assert!(matches!(
        second_result,
        Err(EnvelopeError::EnvelopeAlreadySeen { .. })
    ));
}

#[tokio::test]
async fn gossip_ignores_subsequent_envelope_with_modified_slot() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let envelope = Arc::new(signed_envelope);

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Gossip)
        .await
        .expect("first envelope should pass gossip verification");

    let mut replayed_envelope = (*envelope).clone();
    replayed_envelope.message.payload.slot_number += 1;

    let second_result = harness
        .chain
        .verify_envelope_for_gossip(Arc::new(replayed_envelope), EnvelopeSource::Gossip)
        .await;

    assert!(matches!(
        second_result,
        Err(EnvelopeError::EnvelopeAlreadySeen { .. })
    ));
}

#[tokio::test]
async fn http_envelope_bypasses_deduplication_but_marks_seen() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let envelope = Arc::new(signed_envelope);

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Http)
        .await
        .expect("first HTTP publish should verify");

    // Re-publishing the same envelope is not rejected as a duplicate
    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Http)
        .await
        .expect("re-published envelope should verify");

    // If the envelope is later published on gossip, then the call should return an error
    let gossip_result = harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Gossip)
        .await;

    assert!(matches!(
        gossip_result,
        Err(EnvelopeError::EnvelopeAlreadySeen { .. })
    ));
}

#[tokio::test]
async fn rpc_envelope_bypasses_deduplication_but_marks_seen() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let envelope = Arc::new(signed_envelope);

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Rpc)
        .await
        .expect("first RPC envelope should verify");

    // Re-fetching the same envelope over RPC is not rejected as a duplicate
    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Rpc)
        .await
        .expect("re-fetched envelope should verify");

    // If the envelope is later published on gossip, then the call should return an error
    let gossip_result = harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Gossip)
        .await;

    assert!(matches!(
        gossip_result,
        Err(EnvelopeError::EnvelopeAlreadySeen { .. })
    ));
}

#[tokio::test]
async fn gossip_seen_envelope_can_be_reverified_via_non_gossip() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();

    harness.extend_to_slot(Slot::new(1)).await;

    let state = harness.get_current_state();
    let target_slot = Slot::new(2);
    harness.advance_slot();
    let (block_contents, opt_envelope, _new_state) =
        harness.make_block_with_envelope(state, target_slot).await;

    let block_root = block_contents.0.canonical_root();
    harness
        .process_block(target_slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let envelope = Arc::new(signed_envelope);
    let event_handler = harness.chain.event_handler.as_ref().unwrap();
    let mut gossip_receiver = event_handler.subscribe_execution_payload_gossip();

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Gossip)
        .await
        .expect("gossip envelope should verify and be marked as seen");

    // Check emitted SSE event
    let gossip_event = gossip_receiver
        .try_recv()
        .expect("first envelope observation should emit execution_payload_gossip");
    if let EventKind::ExecutionPayloadGossip(sse) = gossip_event {
        assert_eq!(sse.slot, target_slot);
        assert_eq!(sse.block_root, block_root);
    } else {
        panic!("expected ExecutionPayloadGossip event, got {gossip_event:?}");
    }

    harness
        .chain
        .verify_envelope_for_gossip(envelope.clone(), EnvelopeSource::Rpc)
        .await
        .expect("RPC lookup should re-verify an envelope already seen on gossip");

    assert!(
        gossip_receiver.try_recv().is_err(),
        "an envelope re-verified over RPC must not re-emit execution_payload_gossip"
    );

    harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Http)
        .await
        .expect("HTTP publish should re-verify an envelope already seen on gossip");

    assert!(
        gossip_receiver.try_recv().is_err(),
        "an envelope re-verified over HTTP must not re-emit execution_payload_gossip"
    );
}

/// Helper: build a Gloas harness with a mock execution layer.
fn gloas_harness() -> BeaconChainHarness<beacon_chain::test_utils::EphemeralHarnessType<E>> {
    BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build()
}

/// Helper: produce the block and envelope for `slot`, import both, and return the block root.
async fn import_block_and_envelope(
    harness: &BeaconChainHarness<beacon_chain::test_utils::EphemeralHarnessType<E>>,
    slot: Slot,
) -> Hash256 {
    let state = harness.get_current_state();
    harness.advance_slot();
    let (block_contents, opt_envelope, _) = harness.make_block_with_envelope(state, slot).await;
    let block_root = block_contents.0.canonical_root();

    let block = block_contents.0.clone();
    harness
        .process_block(slot, block_root, block_contents)
        .await
        .expect("block should be processed");

    // Without its custody columns the envelope never reaches fork choice.
    harness.process_gossip_columns(&block, None).await;

    let signed_envelope = opt_envelope.expect("Gloas block should produce an envelope");
    let gossip_verified = harness
        .chain
        .verify_envelope_for_gossip(Arc::new(signed_envelope), EnvelopeSource::Gossip)
        .await
        .expect("envelope gossip verification should succeed");

    let status = harness
        .chain
        .process_execution_payload_envelope(
            block_root,
            gossip_verified,
            beacon_chain::NotifyExecutionLayer::Yes,
            types::BlockImportSource::Gossip,
            #[allow(clippy::result_large_err)]
            || Ok(()),
        )
        .await
        .expect("envelope import should succeed even when the execution layer is syncing");

    match status {
        beacon_chain::AvailabilityProcessingStatus::Imported(..) => {}
        beacon_chain::AvailabilityProcessingStatus::MissingComponents(..) => {
            panic!("envelope for slot {slot} should import, got {status:?}")
        }
    }

    // The next block builds on the payload status of the head. If the head does not catch up
    // here, every block extends the `EMPTY` variant of its parent.
    harness.chain.recompute_head_at_current_slot().await;

    block_root
}

/// Helper: the execution status that fork choice holds for the payload of a block.
fn execution_status(
    harness: &BeaconChainHarness<beacon_chain::test_utils::EphemeralHarnessType<E>>,
    block_root: Hash256,
) -> ExecutionStatus {
    harness
        .chain
        .canonical_head
        .fork_choice_read_lock()
        .get_block(&block_root)
        .expect("block should be in fork choice")
        .execution_status
}

/// An execution layer that answers `SYNCING` must not stop the import of an envelope. The
/// node holds the payload as `Optimistic` rather than refusing it.
#[tokio::test]
async fn syncing_execution_layer_imports_payload_optimistically() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = gloas_harness();
    harness.extend_to_slot(Slot::new(1)).await;

    harness
        .mock_execution_layer
        .as_ref()
        .expect("mock execution layer")
        .server
        .all_payloads_syncing(true);

    let block_root = import_block_and_envelope(&harness, Slot::new(2)).await;

    assert!(
        is_optimistic(execution_status(&harness, block_root)),
        "a payload the execution layer could not validate must be held as optimistic",
    );
}

/// A later valid payload promotes every optimistic payload below it. The node does not fetch
/// an old payload again.
#[tokio::test]
async fn a_later_valid_payload_promotes_its_optimistic_ancestors() {
    if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
        return;
    }

    let harness = gloas_harness();
    harness.extend_to_slot(Slot::new(1)).await;

    let mock = harness
        .mock_execution_layer
        .as_ref()
        .expect("mock execution layer");

    // The node imports two slots while the execution layer answers `SYNCING`.
    mock.server.all_payloads_syncing(true);
    let first_root = import_block_and_envelope(&harness, Slot::new(2)).await;
    let second_root = import_block_and_envelope(&harness, Slot::new(3)).await;

    assert!(is_optimistic(execution_status(&harness, first_root)));
    assert!(is_optimistic(execution_status(&harness, second_root)));

    // The execution layer catches up and validates the next payload.
    mock.server.all_payloads_valid();
    let third_root = import_block_and_envelope(&harness, Slot::new(4)).await;

    assert!(
        is_valid_and_post_bellatrix(execution_status(&harness, third_root)),
        "the payload the execution layer validated must be valid",
    );
    assert!(
        is_valid_and_post_bellatrix(execution_status(&harness, second_root)),
        "its parent's payload is vouched for by the valid descendant",
    );
    assert!(
        is_valid_and_post_bellatrix(execution_status(&harness, first_root)),
        "promotion must walk the whole ancestry, not just one step",
    );
}

fn is_valid_and_post_bellatrix(status: ExecutionStatus) -> bool {
    matches!(status, ExecutionStatus::Valid(_))
}

fn is_optimistic(status: ExecutionStatus) -> bool {
    matches!(status, ExecutionStatus::Optimistic(_))
}

/// Explicit Gloas genesis: these assertions never disappear behind a FORK_NAME guard.
fn binding_harness() -> BeaconChainHarness<beacon_chain::test_utils::EphemeralHarnessType<E>> {
    use types::EthSpec;
    BeaconChainHarness::builder(E::default())
        .spec(Arc::new(
            ForkName::Gloas.make_genesis_spec(E::default_spec()),
        ))
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build()
}

/// Even a real proposer signature must not admit content inconsistent with the beacon state.
/// Failed admission must leave both deduplication and the gas-limit cache untouched.
#[tokio::test]
async fn signed_inconsistent_envelopes_cannot_poison_observation() {
    use types::{Domain, ExecutionPayloadEnvelope, SignedRoot, Withdrawal};

    for source in [
        EnvelopeSource::Gossip,
        EnvelopeSource::Rpc,
        EnvelopeSource::Http,
    ] {
        let harness = binding_harness();
        harness.extend_to_slot(Slot::new(1)).await;
        let state = harness.get_current_state();
        let slot = Slot::new(2);
        harness.advance_slot();
        let (contents, original, post_state) = harness.make_block_with_envelope(state, slot).await;
        let proposer = contents.0.message().proposer_index() as usize;
        let root = contents.0.canonical_root();
        harness.process_block(slot, root, contents).await.unwrap();
        let original = original.unwrap();
        let hash = original.message.payload.block_hash;
        let domain = harness.spec.get_domain(
            original.epoch(),
            Domain::BeaconBuilder,
            &post_state.fork(),
            post_state.genesis_validators_root(),
        );
        let mut cases = Vec::new();
        let mut add = |name: &'static str, change: fn(&mut ExecutionPayloadEnvelope<E>)| {
            let mut envelope = original.clone();
            change(&mut envelope.message);
            envelope.signature = harness.validator_keypairs[proposer]
                .sk
                .sign(envelope.message.signing_root(domain));
            assert!(
                envelope
                    .verify_signature_with_state(&post_state, &harness.spec)
                    .unwrap(),
                "{name}: fixture must have a valid real signature"
            );
            cases.push((name, envelope));
        };
        add("ParentBeaconBlockRootMismatch", |m| {
            m.parent_beacon_block_root = Hash256::repeat_byte(0x71)
        });
        add("GasLimitMismatch", |m| m.payload.gas_limit += 1);
        add("PrevRandaoMismatch", |m| {
            m.payload.prev_randao = Hash256::repeat_byte(0x72)
        });
        add("ParentHashMismatch", |m| {
            m.payload.parent_hash = types::ExecutionBlockHash::repeat_byte(0x73)
        });
        add("TimestampMismatch", |m| m.payload.timestamp += 1);
        add("WithdrawalsRootMismatch", |m| {
            m.payload
                .withdrawals
                .push(Withdrawal {
                    index: 7,
                    validator_index: 8,
                    address: Address::repeat_byte(9),
                    amount: 10,
                })
                .unwrap();
        });
        add("EmptyTransaction", |m| {
            m.payload.transactions.push(Default::default()).unwrap()
        });
        for (name, envelope) in &cases {
            let result = harness
                .chain
                .verify_envelope_for_gossip(Arc::new(envelope.clone()), source)
                .await;
            assert!(
                result.is_err(),
                "{source:?} {name}: correctly signed inconsistent envelope was admitted"
            );
            let error = result.unwrap_err();
            assert!(
                format!("{error:?}").contains(name),
                "{source:?} {name}: wrong guard: {error:?}"
            );
            assert!(
                error.penalize_peer(),
                "inconsistent signed content must be rejected"
            );
            assert_eq!(
                harness
                    .chain
                    .observed_execution_payloads
                    .get_gas_limit(hash),
                None,
                "{name}: failure poisoned gas-limit cache"
            );
        }
        harness
            .chain
            .verify_envelope_for_gossip(Arc::new(original.clone()), EnvelopeSource::Gossip)
            .await
            .expect("invalid attempts must not suppress the authentic envelope");
        assert_eq!(
            harness
                .chain
                .observed_execution_payloads
                .get_gas_limit(hash),
            Some(original.message.payload.gas_limit)
        );
        // Non-gossip recovery paths recheck content even after a valid observation.
        for repeat_source in [EnvelopeSource::Rpc, EnvelopeSource::Http] {
            for (name, envelope) in &cases {
                let error = harness
                    .chain
                    .verify_envelope_for_gossip(Arc::new(envelope.clone()), repeat_source)
                    .await
                    .unwrap_err();
                assert!(
                    format!("{error:?}").contains(name),
                    "cached {repeat_source:?} {name}: {error:?}"
                );
                assert_eq!(
                    harness
                        .chain
                        .observed_execution_payloads
                        .get_gas_limit(hash),
                    Some(original.message.payload.gas_limit)
                );
            }
        }
    }
}

/// The signature authenticates the complete message, including body fields that don't change
/// the claimed block hash. Admission never treats that claimed hash as content authentication.
#[tokio::test]
async fn stale_signatures_do_not_authenticate_replaced_payload_contents() {
    use types::ExecutionPayloadEnvelope;
    let harness = binding_harness();
    harness.extend_to_slot(Slot::new(1)).await;
    let state = harness.get_current_state();
    harness.advance_slot();
    let slot = Slot::new(2);
    let (contents, original, _) = harness.make_block_with_envelope(state, slot).await;
    let root = contents.0.canonical_root();
    harness.process_block(slot, root, contents).await.unwrap();
    let original = original.unwrap();
    let hash = original.message.payload.block_hash;
    let mut cases = Vec::new();
    let mut add = |name: &'static str, change: fn(&mut ExecutionPayloadEnvelope<E>)| {
        let mut envelope = original.clone();
        change(&mut envelope.message);
        assert_eq!(envelope.message.payload.block_hash, hash);
        cases.push((name, envelope));
    };
    add("state root", |m| {
        m.payload.state_root = Hash256::repeat_byte(0x61)
    });
    add("receipts root", |m| {
        m.payload.receipts_root = Hash256::repeat_byte(0x62)
    });
    add("transactions", |m| {
        m.payload
            .transactions
            .push(ssz_types::ProgressiveVariableList::new(vec![1]).unwrap())
            .unwrap()
    });
    add("gas used", |m| m.payload.gas_used += 1);
    add("fee recipient", |m| {
        m.payload.fee_recipient = Address::repeat_byte(0x63)
    });
    add("block number", |m| m.payload.block_number += 1);
    add("blob gas", |m| m.payload.blob_gas_used += 1);
    add("excess blob gas", |m| m.payload.excess_blob_gas += 1);
    add("access list", |m| {
        m.payload.block_access_list.push(1).unwrap()
    });
    for source in [
        EnvelopeSource::Gossip,
        EnvelopeSource::Rpc,
        EnvelopeSource::Http,
    ] {
        for (name, envelope) in &cases {
            let error = harness
                .chain
                .verify_envelope_for_gossip(Arc::new(envelope.clone()), source)
                .await
                .unwrap_err();
            assert!(
                matches!(error, EnvelopeError::BadSignature),
                "{source:?} {name}: {error:?}"
            );
            assert_eq!(
                harness
                    .chain
                    .observed_execution_payloads
                    .get_gas_limit(hash),
                None
            );
        }
    }
    harness
        .chain
        .verify_envelope_for_gossip(Arc::new(original.clone()), EnvelopeSource::Gossip)
        .await
        .unwrap();
    for source in [EnvelopeSource::Rpc, EnvelopeSource::Http] {
        for (name, envelope) in &cases {
            let error = harness
                .chain
                .verify_envelope_for_gossip(Arc::new(envelope.clone()), source)
                .await
                .unwrap_err();
            assert!(
                matches!(error, EnvelopeError::BadSignature),
                "cached {source:?} {name}: {error:?}"
            );
        }
    }
}
