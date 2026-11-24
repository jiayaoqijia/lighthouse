//! Independent current-spec regression: a correctly signed block has a wrong state root.
use crate::block_verification_types::LookupBlock;
use crate::data_column_verification::GossipDataColumnError;
use crate::payload_attestation_verification::Error as PayloadAttestationError;
use crate::payload_envelope_verification::{EnvelopeError, EnvelopeSource};
use crate::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use crate::{BlockError, NotifyExecutionLayer};
use bls::Signature;
use ssz::Decode;
use std::sync::Arc;
use types::*;

type E = MinimalEthSpec;
fn raw(bytes: &[u8]) -> Vec<u8> {
    bytes.to_vec()
}
fn harness() -> (
    BeaconChainHarness<EphemeralHarnessType<E>>,
    SignedBeaconBlock<E>,
    SignedExecutionPayloadEnvelope<E>,
) {
    let config: Config = yaml_serde::from_slice(include_bytes!(
        "../tests/testdata/gloas-consensus-failed/config.yaml"
    ))
    .unwrap();
    let spec = Arc::new(ChainSpec::from_config::<E>(&config).unwrap());
    let state = BeaconState::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-consensus-failed/state.ssz"
        )),
        &spec,
    )
    .unwrap();
    let anchor = SignedBeaconBlock::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-consensus-failed/anchor.ssz"
        )),
        &spec,
    )
    .unwrap();
    let failed = SignedBeaconBlock::<E>::from_ssz_bytes(
        &raw(include_bytes!(
            "../tests/testdata/gloas-consensus-failed/failed.ssz"
        )),
        &spec,
    )
    .unwrap();
    let envelope = SignedExecutionPayloadEnvelope::<E>::from_ssz_bytes(&raw(include_bytes!(
        "../tests/testdata/gloas-consensus-failed/envelope.ssz"
    )))
    .unwrap();
    let genesis_time = state.genesis_time();
    let h = BeaconChainHarness::builder(E::default())
        .spec(spec)
        .keypairs(vec![])
        .mock_execution_layer()
        .mock_execution_layer_all_payloads_valid()
        .recalculate_fork_times_with_genesis(genesis_time)
        .initial_state_ephemeral_store(state, anchor, None)
        .build();
    h.set_current_slot(failed.slot());
    (h, failed, envelope)
}

#[tokio::test]
async fn known_invalid_consensus_root_rejects_dependents_without_signature_poisoning() {
    let (h, failed, envelope) = harness();
    let root = failed.canonical_root();
    let mut unauthenticated = failed.clone();
    *unauthenticated.signature_mut() = Signature::empty();
    assert!(
        h.chain
            .process_block(
                root,
                LookupBlock::new(root, Arc::new(unauthenticated)),
                NotifyExecutionLayer::Yes,
                BlockImportSource::Lookup,
                || Ok(())
            )
            .await
            .is_err()
    );
    assert!(
        !h.chain.canonical_head.block_failed_consensus(&root),
        "bad outer signature poisoned unsigned root"
    );
    h.chain.pending_payload_cache.insert_block(
        root,
        Arc::new(failed.clone()),
        BlockImportSource::Lookup,
    );
    let result = h
        .chain
        .process_block(
            root,
            LookupBlock::new(root, Arc::new(failed.clone())),
            NotifyExecutionLayer::Yes,
            BlockImportSource::Lookup,
            || Ok(()),
        )
        .await;
    assert!(
        matches!(result, Err(BlockError::StateRootMismatch { .. })),
        "{result:?}"
    );
    assert!(h.chain.canonical_head.block_failed_consensus(&root));
    assert!(h.chain.pending_payload_cache.get_bid(&root).is_none());
    assert!(h.chain.pending_payload_cache.get_block(&root).is_none());
    let rejection = h
        .chain
        .verify_envelope_for_gossip(Arc::new(envelope.clone()), EnvelopeSource::Gossip)
        .await;
    assert!(
        matches!(rejection, Err(EnvelopeError::BlockFailedConsensus { .. })),
        "{rejection:?}"
    );
    let unknown = Hash256::repeat_byte(0x77);
    let mut unknown_envelope = envelope;
    unknown_envelope.message.beacon_block_root = unknown;
    assert!(matches!(
        h.chain
            .verify_envelope_for_gossip(Arc::new(unknown_envelope), EnvelopeSource::Gossip)
            .await,
        Err(EnvelopeError::BlockRootUnknown { .. })
    ));
    let column = DataColumnSidecar::Gloas(DataColumnSidecarGloas::<E> {
        index: 0,
        column: Default::default(),
        kzg_proofs: Default::default(),
        slot: failed.slot(),
        beacon_block_root: root,
    });
    assert!(matches!(
        h.chain
            .verify_data_column_sidecar_for_gossip(Arc::new(column), DataColumnSubnetId::new(0))
            .await,
        Err(GossipDataColumnError::BlockFailedConsensus { .. })
    ));
    let message = PayloadAttestationMessage {
        validator_index: 0,
        data: PayloadAttestationData {
            beacon_block_root: root,
            slot: failed.slot(),
            payload_present: true,
            blob_data_available: true,
        },
        signature: Signature::empty(),
    };
    assert!(matches!(
        h.chain
            .verify_payload_attestation_message_for_gossip(message),
        Err(PayloadAttestationError::BlockFailedConsensus { .. })
    ));
}

#[test]
fn known_invalid_consensus_root_memory_is_bounded() {
    let (h, _, _) = harness();
    for index in 0..1025 {
        h.chain.canonical_head.mark_consensus_failed(Hash256::from(
            types::Uint256::from(index).to_be_bytes::<32>(),
        ));
    }
    assert!(
        !h.chain
            .canonical_head
            .block_failed_consensus(&Hash256::ZERO)
    );
    assert!(
        h.chain
            .canonical_head
            .block_failed_consensus(&Hash256::from(
                types::Uint256::from(1024).to_be_bytes::<32>()
            ))
    );
}

#[tokio::test]
#[cfg(feature = "ef_tests")]
async fn finalized_ancestor_remains_a_valid_preferences_dependency() {
    let config: Config = yaml_serde::from_slice(include_bytes!(
        "../tests/testdata/gloas-consensus-failed/config.yaml"
    ))
    .unwrap();
    let spec = Arc::new(ChainSpec::from_config::<E>(&config).unwrap());
    let state = BeaconState::<E>::from_ssz_bytes(
        include_bytes!("../tests/testdata/gloas-finalized-preferences/state.ssz"),
        &spec,
    )
    .unwrap();
    let blocks: Vec<SignedBeaconBlock<E>> = vec![
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-00.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-01.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-02.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-03.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-04.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-05.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-06.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-07.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-08.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-09.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-10.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-11.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-12.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-13.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-14.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-15.ssz"),
            &spec,
        )
        .unwrap(),
        SignedBeaconBlock::from_ssz_bytes(
            include_bytes!("../tests/testdata/gloas-finalized-preferences/block-16.ssz"),
            &spec,
        )
        .unwrap(),
    ];
    let preferences = SignedProposerPreferences::from_ssz_bytes(include_bytes!(
        "../tests/testdata/gloas-finalized-preferences/preferences.ssz"
    ))
    .unwrap();
    for finalize in [false, true] {
        let h = BeaconChainHarness::builder(E::default())
            .spec(spec.clone())
            .keypairs(vec![])
            .mock_execution_layer()
            .mock_execution_layer_all_payloads_valid()
            .recalculate_fork_times_with_genesis(state.genesis_time())
            .initial_state_ephemeral_store(state.clone(), blocks[0].clone(), None)
            .build();
        h.set_current_slot(blocks.last().unwrap().slot());
        for block in blocks.iter().skip(1) {
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
        }
        h.chain.recompute_head_at_current_slot().await;
        if finalize {
            h.chain
                .canonical_head
                .fork_choice_write_lock()
                .testing_set_finalized_checkpoint(Checkpoint {
                    epoch: Epoch::new(1),
                    root: blocks[8].canonical_root(),
                });
            assert!(
                h.chain
                    .canonical_head
                    .fork_choice_read_lock()
                    .get_block(&preferences.message.dependent_root)
                    .is_none()
            );
        }
        let result = h
            .chain
            .verify_proposer_preferences_for_gossip(Arc::new(preferences.clone()));
        assert!(result.is_ok(), "finalized={finalize}: {result:?}");
    }
}
