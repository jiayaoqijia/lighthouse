//! Tests for the inclusion list store reads on `BeaconChain`.
//!
//! These do not need a Heze state: the committee derives from the ordinary attester shuffling and
//! the store holds no fork-specific data.

use beacon_chain::inclusion_list_store::ProposerInclusionLists;
use beacon_chain::inclusion_list_verification::gossip_verified_inclusion_list::GossipVerifiedInclusionList;
use beacon_chain::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use beacon_chain::{BeaconChainError, WhenSlotSkipped};
use bls::Signature;
use execution_layer::json_structures::JsonPayloadAttributes;
use ssz_types::ProgressiveVariableList;
use std::sync::{Arc, Mutex};
use types::{
    EthSpec, ForkName, Hash256, InclusionList, ProgressiveTransactions, RelativeEpoch,
    SignedExecutionPayloadBidRef, SignedInclusionList, Slot, Spec,
};

type E = Spec;

/// 8 validators per slot on minimal, fewer than the committee size, so positions repeat.
const VALIDATOR_COUNT: usize = 64;

#[tokio::test]
async fn heze_checkpoint_envelope_without_pre_anchor_parent() {
    use beacon_chain::builder::BeaconChainBuilder;
    use beacon_chain::payload_envelope_verification::{EnvelopeError, EnvelopeSource};
    use beacon_chain::test_utils::{
        generate_data_column_indices_rand_order, generate_data_column_sidecars_from_block, get_kzg,
        mock_execution_layer_from_parts,
    };
    use beacon_chain::{ChainConfig, NotifyExecutionLayer};
    use execution_layer::{PayloadStatusV1, PayloadStatusV1Status};
    use rand::{SeedableRng, rngs::StdRng};
    use slot_clock::{SlotClock, TestingSlotClock};
    use std::time::Duration;
    use store::{HotColdDB, StoreConfig};
    use types::BlockImportSource;

    let spec = Arc::new(ForkName::Heze.make_genesis_spec(E::default_spec()));
    let source = BeaconChainHarness::builder(E::default())
        .spec(spec.clone())
        .deterministic_keypairs(VALIDATOR_COUNT)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    source.execution_block_generator().set_min_blob_count(1);
    let genesis_state = source.get_current_state();
    let slot = Slot::new(E::slots_per_epoch());
    source.extend_to_slot(slot).await;
    let root = source.head_block_root();
    let block = source.chain.store.get_full_block(&root).unwrap().unwrap();
    let state = source
        .chain
        .store
        .get_state(&block.state_root(), Some(slot), true)
        .unwrap()
        .unwrap();
    let envelope = source
        .chain
        .store
        .get_signed_payload_envelope(&root)
        .unwrap()
        .unwrap();
    let payload_hash = envelope.message.payload.block_hash;
    let mock = mock_execution_layer_from_parts(spec.clone(), source.runtime.task_executor.clone());
    mock.server.set_payload_statuses(
        payload_hash,
        PayloadStatusV1 {
            status: PayloadStatusV1Status::Valid,
            latest_valid_hash: Some(payload_hash),
            validation_error: None,
            inclusion_list_satisfied: Some(true),
        },
    );
    let clock = TestingSlotClock::new(
        Slot::new(0),
        Duration::from_secs(source.chain.genesis_time),
        spec.get_slot_duration(),
    );
    clock.set_slot(slot.as_u64());
    let (shutdown_tx, _shutdown_rx) = futures::channel::mpsc::channel(1);
    let chain = Arc::new(
        BeaconChainBuilder::<EphemeralHarnessType<E>>::new(E::default(), get_kzg(&spec))
            .chain_config(ChainConfig {
                optimistic_finalized_sync: false,
                // The mock EL's synthetic block hash is not a real RLP hash.
                verify_envelope_payload_hash_on_cl: false,
                ..ChainConfig::default()
            })
            .store(Arc::new(
                HotColdDB::open_ephemeral(StoreConfig::default(), spec.clone()).unwrap(),
            ))
            .custom_spec(spec.clone())
            .task_executor(source.runtime.task_executor.clone())
            .weak_subjectivity_state(state, block.clone(), None, genesis_state)
            .unwrap()
            .slot_clock(clock)
            .shutdown_sender(shutdown_tx)
            .execution_layer(Some(mock.el))
            .ordered_custody_column_indices(generate_data_column_indices_rand_order::<E>())
            .rng(Box::new(StdRng::seed_from_u64(42)))
            .build()
            .unwrap(),
    );
    assert!(
        chain
            .store
            .get_blinded_block(&block.parent_root())
            .unwrap()
            .is_none()
    );
    assert!(matches!(
        chain.get_inclusion_list_transactions(block.parent_root(), slot.saturating_sub(1u64), true),
        Err(BeaconChainError::MissingBeaconBlock(_))
    ));
    let mut bad_signature = envelope.clone();
    bad_signature.signature = Signature::empty();
    assert!(matches!(
        chain
            .verify_envelope_for_gossip(Arc::new(bad_signature), EnvelopeSource::Rpc)
            .await,
        Err(EnvelopeError::BadSignature)
    ));
    let mut wrong_bid = envelope.clone();
    wrong_bid.message.payload.block_hash = types::ExecutionBlockHash::zero();
    assert!(matches!(
        chain
            .verify_envelope_for_gossip(Arc::new(wrong_bid), EnvelopeSource::Rpc)
            .await,
        Err(EnvelopeError::BlockHashMismatch { .. })
    ));
    let verified = chain
        .verify_envelope_for_gossip(Arc::new(envelope), EnvelopeSource::Rpc)
        .await
        .unwrap();
    let status = chain
        .process_execution_payload_envelope(
            root,
            verified,
            NotifyExecutionLayer::Yes,
            BlockImportSource::RangeSync,
            || Ok(()),
        )
        .await
        .expect("trusted checkpoint envelope must not require its unavailable parent IL view");
    assert!(matches!(
        status,
        beacon_chain::AvailabilityProcessingStatus::MissingComponents(..)
    ));
    let wire = mock.server.take_previous_request().unwrap();
    assert_eq!(wire["method"], "engine_newPayloadV6");
    assert_eq!(wire["params"][4], serde_json::json!([]));
    let columns = generate_data_column_sidecars_from_block(&block, &spec);
    assert!(!columns.is_empty());
    let status = chain.process_rpc_custody_columns(columns).await.unwrap();
    assert!(matches!(
        status,
        beacon_chain::AvailabilityProcessingStatus::Imported(..)
    ));
    let choice = chain.canonical_head.fork_choice_read_lock();
    assert_eq!(
        choice.get_block(&root).unwrap().execution_status,
        proto_array::ExecutionStatus::Valid(payload_hash)
    );
    assert!(
        !choice
            .proto_array()
            .unsatisfied_inclusion_lists()
            .contains(&root)
    );
}

#[tokio::test]
async fn heze_v6_satisfaction_survives_fork_choice_reload() {
    let (unsatisfied_hash, satisfied_hash) = tokio::join!(
        assert_heze_v6_fork_choice_after_reload(false),
        assert_heze_v6_fork_choice_after_reload(true),
    );
    assert_eq!(unsatisfied_hash, satisfied_hash);
}

async fn assert_heze_v6_fork_choice_after_reload(satisfied: bool) -> Hash256 {
    use beacon_chain::payload_envelope_verification::EnvelopeSource;
    use beacon_chain::{BeaconChain, NotifyExecutionLayer};
    use execution_layer::{PayloadStatusV1, PayloadStatusV1Status};
    use fork_choice::{PayloadStatus, ResetPayloadStatuses};
    use types::BlockImportSource;

    let harness = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(
            ForkName::Heze.make_genesis_spec(E::default_spec()),
        ))
        .deterministic_keypairs(VALIDATOR_COUNT)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    harness.extend_to_slot(Slot::new(1)).await;
    let parent = harness.head_block_root();
    let list_slot = Slot::new(1);
    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(parent, list_slot)
        .unwrap();
    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(
                list_slot,
                committee[0],
                dependent_root,
                0xc1,
            ),
            is_timely: true,
        });
    let state = harness.get_current_state();
    harness.advance_slot();
    let slot = Slot::new(2);
    let (block, envelope, _) = harness.make_block_with_envelope(state, slot).await;
    let root = block.0.canonical_root();
    harness.process_block(slot, root, block).await.unwrap();
    let envelope = Arc::new(envelope.unwrap());
    let hash = envelope.message.payload.block_hash;
    let mock = &harness.mock_execution_layer.as_ref().unwrap().server;
    mock.set_payload_statuses(
        hash,
        PayloadStatusV1 {
            status: PayloadStatusV1Status::Valid,
            latest_valid_hash: Some(hash),
            validation_error: None,
            inclusion_list_satisfied: Some(satisfied),
        },
    );
    let verified = harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Gossip)
        .await
        .unwrap();
    harness
        .chain
        .process_execution_payload_envelope(
            root,
            verified,
            NotifyExecutionLayer::Yes,
            BlockImportSource::Lookup,
            || Ok(()),
        )
        .await
        .unwrap();

    let wire = mock.take_previous_request().unwrap();
    assert_eq!(wire["method"], "engine_newPayloadV6");
    assert_eq!(wire["params"][4], serde_json::json!(["0xc1"]));

    {
        let choice = harness.chain.canonical_head.fork_choice_read_lock();
        assert_eq!(
            choice.get_block(&root).unwrap().execution_status,
            proto_array::ExecutionStatus::Valid(hash)
        );
        assert_eq!(
            choice
                .proto_array()
                .unsatisfied_inclusion_lists()
                .contains(&root),
            !satisfied,
        );
        assert_eq!(
            choice
                .should_build_on_full(&root, PayloadStatus::Full, slot + 1)
                .unwrap(),
            satisfied,
        );
    }
    harness.chain.persist_fork_choice().unwrap();
    let restored = BeaconChain::<EphemeralHarnessType<E>>::load_fork_choice(
        harness.chain.store.clone(),
        ResetPayloadStatuses::OnlyWithInvalidPayload,
        &harness.spec,
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        restored.get_block(&root).unwrap().execution_status,
        proto_array::ExecutionStatus::Valid(hash)
    );
    assert_eq!(
        restored
            .should_build_on_full(&root, PayloadStatus::Full, slot + 1)
            .unwrap(),
        satisfied,
    );
    hash.into_root()
}

#[tokio::test]
async fn heze_fcu_resolves_deferred_verdict_from_empty_head() {
    assert_heze_fcu_resolves_deferred_verdict_from_empty_head(false).await;
}

#[tokio::test]
async fn heze_fcu_resolves_deferred_verdict_with_proof_relay() {
    assert_heze_fcu_resolves_deferred_verdict_from_empty_head(true).await;
}

async fn assert_heze_fcu_resolves_deferred_verdict_from_empty_head(relay_proofs: bool) {
    use beacon_chain::payload_envelope_verification::EnvelopeSource;
    use beacon_chain::{BeaconChain, NotifyExecutionLayer, OverrideForkchoiceUpdate};
    use execution_layer::{PayloadStatusV1, PayloadStatusV1Status};
    use fork_choice::{PayloadStatus, ResetPayloadStatuses};
    use proto_array::ExecutionStatus;
    use types::BlockImportSource;

    let builder = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(
            ForkName::Heze.make_genesis_spec(E::default_spec()),
        ))
        .deterministic_keypairs(VALIDATOR_COUNT)
        .fresh_ephemeral_store()
        .mock_execution_layer();
    let harness = if relay_proofs {
        builder.proof_engine().build()
    } else {
        builder.build()
    };
    harness.extend_to_slot(Slot::new(1)).await;

    let ancestor = harness.head_block_root();
    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(ancestor, Slot::new(1))
        .unwrap();
    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(
                Slot::new(1),
                committee[0],
                dependent_root,
                0xc1,
            ),
            is_timely: true,
        });

    let state = harness.get_current_state();
    harness.advance_slot();
    let slot = Slot::new(2);
    let (contents, envelope, state_after_parent) =
        harness.make_block_with_envelope(state, slot).await;
    let parent = contents.0.canonical_root();
    harness.process_block(slot, parent, contents).await.unwrap();
    let envelope = Arc::new(envelope.unwrap());
    let execution_hash = envelope.message.payload.block_hash;
    let mock = &harness.mock_execution_layer.as_ref().unwrap().server;
    mock.all_payloads_syncing_on_forkchoice_updated();
    mock.all_payloads_accepted_on_new_payload(true);
    let verified = harness
        .chain
        .verify_envelope_for_gossip(envelope, EnvelopeSource::Gossip)
        .await
        .unwrap();
    harness
        .chain
        .process_execution_payload_envelope(
            parent,
            verified,
            NotifyExecutionLayer::Yes,
            BlockImportSource::Lookup,
            || Ok(()),
        )
        .await
        .unwrap();
    {
        let choice = harness.chain.canonical_head.fork_choice_read_lock();
        assert_eq!(
            choice.get_block(&parent).unwrap().execution_status,
            ExecutionStatus::Optimistic(execution_hash)
        );
        assert!(
            !choice
                .proto_array()
                .unsatisfied_inclusion_lists()
                .contains(&parent)
        );
    }

    harness.advance_slot();
    let child_slot = slot + 1;
    // Construct the child before resuming the syncing consumer response. A producer
    // cannot build through a SYNCING FCU; the import below must still stay deferred.
    mock.all_payloads_valid_on_forkchoice_updated();
    let (contents, _, _) = harness
        .make_block_with_envelope_on(state_after_parent, child_slot, PayloadStatus::Full)
        .await;
    mock.all_payloads_syncing_on_forkchoice_updated();
    let child = contents.0.canonical_root();
    harness
        .process_block(child_slot, child, contents)
        .await
        .unwrap();
    harness.chain.recompute_head_at_current_slot().await;
    let head = harness.chain.canonical_head.cached_head();
    assert_eq!(head.head_block_root(), child);
    assert_eq!(head.head_payload_status(), PayloadStatus::Empty);
    let params = head.forkchoice_update_parameters();
    assert_eq!(params.head_hash, Some(execution_hash));

    mock.take_previous_request();
    mock.set_fcu_payload_status(
        execution_hash,
        PayloadStatusV1 {
            status: PayloadStatusV1Status::Valid,
            latest_valid_hash: Some(execution_hash),
            validation_error: None,
            inclusion_list_satisfied: Some(false),
        },
    );
    harness
        .chain
        .update_execution_engine_forkchoice(
            child_slot,
            params,
            PayloadStatus::Empty,
            OverrideForkchoiceUpdate::AlreadyApplied,
        )
        .await
        .unwrap();
    let wire = mock.take_previous_request().unwrap();
    assert_eq!(wire["method"], "engine_forkchoiceUpdatedV5");
    assert_eq!(
        wire["params"][0]["headBlockHash"],
        serde_json::json!(execution_hash)
    );

    {
        let choice = harness.chain.canonical_head.fork_choice_read_lock();
        assert_eq!(
            choice.get_block(&parent).unwrap().execution_status,
            ExecutionStatus::Valid(execution_hash)
        );
        let failures = choice.proto_array().unsatisfied_inclusion_lists();
        assert!(failures.contains(&parent));
        assert!(!failures.contains(&child));
        assert!(!failures.contains(&ancestor));
    }
    harness.chain.persist_fork_choice().unwrap();
    let restored = BeaconChain::<EphemeralHarnessType<E>>::load_fork_choice(
        harness.chain.store.clone(),
        ResetPayloadStatuses::OnlyWithInvalidPayload,
        &harness.spec,
    )
    .unwrap()
    .unwrap();
    assert!(
        !restored
            .should_build_on_full(&parent, PayloadStatus::Full, child_slot)
            .unwrap()
    );
}

fn get_harness() -> BeaconChainHarness<EphemeralHarnessType<E>> {
    BeaconChainHarness::builder(E::default())
        .default_spec()
        .deterministic_keypairs(VALIDATOR_COUNT)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build()
}

fn transaction(byte: u8) -> ProgressiveVariableList<u8> {
    ProgressiveVariableList::new(vec![byte]).unwrap()
}

fn signed_inclusion_list(
    slot: Slot,
    validator_index: u64,
    dependent_root: Hash256,
    tx_byte: u8,
) -> SignedInclusionList {
    SignedInclusionList {
        message: InclusionList {
            slot,
            validator_index,
            dependent_root,
            transactions: ProgressiveVariableList::new(vec![transaction(tx_byte)]).unwrap(),
        },
        signature: Signature::empty(),
    }
}

#[tokio::test]
async fn committee_matches_the_state_level_helper() {
    let harness = get_harness();
    let slot = Slot::new(1);

    let block_root = harness.head_block_root();
    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, slot)
        .unwrap();

    assert_eq!(committee.len(), E::inclusion_list_committee_size());

    let mut state = harness.get_current_state();
    state
        .build_committee_cache(RelativeEpoch::Current, &harness.chain.spec)
        .unwrap();
    let expected = state.get_inclusion_list_committee(slot).unwrap();

    assert_eq!(committee, expected);

    // The store is keyed on the attester shuffling decision root, which is what a producer puts on
    // the inclusion list as its `dependent_root`.
    assert_eq!(
        dependent_root,
        state
            .attester_shuffling_decision_root(block_root, RelativeEpoch::Current)
            .unwrap()
    );
}

/// Anchoring on a block root from the slot's own epoch resolves, where the head root does not.
#[tokio::test]
async fn committee_resolves_for_a_slot_in_the_previous_epoch() {
    let harness = get_harness();
    harness
        .extend_slots(E::slots_per_epoch() as usize + 1)
        .await;

    let slot = Slot::new(E::slots_per_epoch() - 1);
    assert!(harness.chain.canonical_head.cached_head().head_slot() > slot);

    let block_root = harness
        .chain
        .block_root_at_slot(slot, WhenSlotSkipped::Prev)
        .unwrap()
        .unwrap();

    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, slot)
        .unwrap();

    let mut state = harness.get_current_state();
    state
        .build_committee_cache(RelativeEpoch::Previous, &harness.chain.spec)
        .unwrap();
    let expected = state.get_inclusion_list_committee(slot).unwrap();
    assert_eq!(committee, expected);

    assert!(matches!(
        harness
            .chain
            .inclusion_list_committee(harness.head_block_root(), slot),
        Err(BeaconChainError::InvalidShufflingId { .. })
    ));

    // `get_inclusion_list_transactions` resolves the dependent root itself, so cover it here too.
    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(slot, committee[0], dependent_root, 0xaa),
            is_timely: true,
        });

    assert_eq!(
        harness
            .chain
            .get_inclusion_list_transactions(block_root, slot, true)
            .unwrap(),
        vec![transaction(0xaa)]
    );
    assert!(matches!(
        harness
            .chain
            .get_inclusion_list_transactions(harness.head_block_root(), slot, true),
        Err(BeaconChainError::InvalidShufflingId { .. })
    ));
}

#[tokio::test]
async fn bits_and_transactions_read_back_through_the_chain() {
    let harness = get_harness();
    let slot = Slot::new(1);
    let block_root = harness.head_block_root();

    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, slot)
        .unwrap();

    let timely_submitter = committee[0];
    let late_submitter = *committee
        .iter()
        .find(|index| **index != timely_submitter)
        .unwrap();

    let mut store = harness.chain.inclusion_list_store.write();
    store.process_inclusion_list(GossipVerifiedInclusionList {
        signed_inclusion_list: signed_inclusion_list(slot, timely_submitter, dependent_root, 0xaa),
        is_timely: true,
    });
    store.process_inclusion_list(GossipVerifiedInclusionList {
        signed_inclusion_list: signed_inclusion_list(slot, late_submitter, dependent_root, 0xbb),
        is_timely: false,
    });
    drop(store);

    // A validator holding two positions has both bits set.
    let bits = harness
        .chain
        .get_inclusion_list_bits(block_root, slot, false)
        .unwrap();
    for (position, validator_index) in committee.iter().enumerate() {
        let expected = *validator_index == timely_submitter || *validator_index == late_submitter;
        assert_eq!(bits.get(position).unwrap(), expected);
    }

    let timely_bits = harness
        .chain
        .get_inclusion_list_bits(block_root, slot, true)
        .unwrap();
    for (position, validator_index) in committee.iter().enumerate() {
        assert_eq!(
            timely_bits.get(position).unwrap(),
            *validator_index == timely_submitter
        );
    }

    assert!(
        harness
            .chain
            .is_inclusion_list_bits_inclusive(block_root, slot, &bits, false)
            .unwrap()
    );
    assert!(
        !harness
            .chain
            .is_inclusion_list_bits_inclusive(block_root, slot, &timely_bits, false)
            .unwrap()
    );
    assert!(
        harness
            .chain
            .is_inclusion_list_bits_inclusive(block_root, slot, &timely_bits, true)
            .unwrap()
    );

    // The spec does not require transaction order to be preserved.
    let transactions = harness
        .chain
        .get_inclusion_list_transactions(block_root, slot, false)
        .unwrap();
    assert_eq!(transactions.len(), 2);
    assert!(transactions.contains(&transaction(0xaa)));
    assert!(transactions.contains(&transaction(0xbb)));

    assert_eq!(
        harness
            .chain
            .get_inclusion_list_transactions(block_root, slot, true)
            .unwrap(),
        vec![transaction(0xaa)]
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn per_slot_task_prunes_the_store() {
    let harness = get_harness();
    let slot = Slot::new(1);
    let block_root = harness.head_block_root();

    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, slot)
        .unwrap();

    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(slot, committee[0], dependent_root, 0xaa),
            is_timely: true,
        });

    // The store retains the two slots behind the current one.
    while harness.chain.slot().unwrap() < slot + 2 {
        harness.advance_slot();
    }
    harness.chain.per_slot_task().await;
    assert!(
        !harness
            .chain
            .get_inclusion_list_transactions(block_root, slot, false)
            .unwrap()
            .is_empty()
    );

    harness.advance_slot();
    harness.chain.per_slot_task().await;
    assert!(
        harness
            .chain
            .get_inclusion_list_transactions(block_root, slot, false)
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn proposer_inclusion_list_transactions_returns_the_stored_inclusion_list() {
    let harness = get_harness();
    let il_slot = Slot::new(1);
    let block_root = harness.head_block_root();

    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, il_slot)
        .unwrap();

    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(
                il_slot,
                committee[0],
                dependent_root,
                0xaa,
            ),
            is_timely: true,
        });

    let ProposerInclusionLists { transactions, bits } = harness
        .chain
        .proposer_inclusion_lists(block_root, il_slot + 1)
        .unwrap();
    assert_eq!(
        transactions,
        ProgressiveTransactions::new(vec![transaction(0xaa)]).unwrap()
    );
    for (position, validator) in committee.iter().enumerate() {
        assert_eq!(bits.get(position).unwrap(), *validator == committee[0]);
    }
}

#[tokio::test]
async fn proposer_inclusion_list_transactions_includes_untimely_lists() {
    let harness = get_harness();
    let il_slot = Slot::new(1);
    let block_root = harness.head_block_root();

    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(block_root, il_slot)
        .unwrap();

    let timely_submitter = committee[0];
    let late_submitter = *committee
        .iter()
        .find(|index| **index != timely_submitter)
        .unwrap();

    let mut store = harness.chain.inclusion_list_store.write();
    store.process_inclusion_list(GossipVerifiedInclusionList {
        signed_inclusion_list: signed_inclusion_list(
            il_slot,
            timely_submitter,
            dependent_root,
            0xaa,
        ),
        is_timely: true,
    });
    store.process_inclusion_list(GossipVerifiedInclusionList {
        signed_inclusion_list: signed_inclusion_list(il_slot, late_submitter, dependent_root, 0xbb),
        is_timely: false,
    });
    drop(store);

    let ProposerInclusionLists { transactions, bits } = harness
        .chain
        .proposer_inclusion_lists(block_root, il_slot + 1)
        .unwrap();
    assert_eq!(transactions.len(), 2);
    assert!(transactions.contains(&transaction(0xaa)));
    assert!(transactions.contains(&transaction(0xbb)));
    for (position, validator) in committee.iter().enumerate() {
        assert_eq!(
            bits.get(position).unwrap(),
            [timely_submitter, late_submitter].contains(validator)
        );
    }
}

#[tokio::test]
async fn proposer_inclusion_lists_returns_an_error_for_an_unknown_branch() {
    let harness = get_harness();
    let il_slot = Slot::new(1);
    let unknown_root = Hash256::ZERO;

    assert!(
        harness
            .chain
            .get_inclusion_list_transactions(unknown_root, il_slot, false)
            .is_err()
    );

    assert!(matches!(
        harness
            .chain
            .proposer_inclusion_lists(unknown_root, il_slot + 1),
        Err(BeaconChainError::MissingBeaconBlock(root)) if root == unknown_root
    ));
}

#[tokio::test]
async fn heze_self_build_commits_the_same_inclusion_lists_it_sends_to_the_engine() {
    let spec = ForkName::Heze.make_genesis_spec(E::default_spec());
    let harness = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(spec))
        .deterministic_keypairs(VALIDATOR_COUNT)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    let il_slot = Slot::new(1);
    let proposal_slot = il_slot + 1;
    harness.extend_to_slot(il_slot).await;
    let state = harness.get_current_state();
    let (committee, dependent_root) = harness
        .chain
        .inclusion_list_committee(harness.head_block_root(), il_slot)
        .unwrap();
    let submitter = committee[0];
    harness
        .chain
        .inclusion_list_store
        .write()
        .process_inclusion_list(GossipVerifiedInclusionList {
            signed_inclusion_list: signed_inclusion_list(il_slot, submitter, dependent_root, 0xaa),
            is_timely: true,
        });
    let captured_transactions = Arc::new(Mutex::new(None));
    let engine_transactions = captured_transactions.clone();
    let chain = Arc::downgrade(&harness.chain);
    harness
        .mock_execution_layer
        .as_ref()
        .unwrap()
        .server
        .ctx
        .hook
        .lock()
        .set_forkchoice_updated_hook(Box::new(move |_, attributes| {
            if let Some(JsonPayloadAttributes::V5(attributes)) = attributes
                && attributes.slot_number == proposal_slot.as_u64()
            {
                *engine_transactions.lock().unwrap() = Some(attributes.inclusion_list_transactions);
                // The committee member equivocates after the payload build already has its inputs.
                chain
                    .upgrade()
                    .unwrap()
                    .inclusion_list_store
                    .write()
                    .process_inclusion_list(GossipVerifiedInclusionList {
                        signed_inclusion_list: signed_inclusion_list(
                            il_slot,
                            submitter,
                            dependent_root,
                            0xbb,
                        ),
                        is_timely: false,
                    });
            }
            None
        }));
    harness.advance_slot();
    let (contents, envelope, _) = harness.make_block_with_envelope(state, proposal_slot).await;
    assert!(envelope.is_some(), "proposal should self-build an envelope");
    let body = contents.0.message().body();
    let SignedExecutionPayloadBidRef::Heze(bid) = body.signed_execution_payload_bid().unwrap()
    else {
        panic!("expected a Heze bid");
    };

    assert_eq!(
        captured_transactions.lock().unwrap().take().unwrap(),
        ProgressiveTransactions::new(vec![transaction(0xaa)]).unwrap()
    );
    for (position, validator) in committee.iter().enumerate() {
        assert_eq!(
            bid.message.inclusion_list_bits.get(position).unwrap(),
            *validator == submitter,
        );
    }
    assert!(
        harness
            .chain
            .proposer_inclusion_lists(harness.head_block_root(), proposal_slot)
            .unwrap()
            .transactions
            .is_empty(),
        "fresh reads should exclude the equivocator",
    );
}
