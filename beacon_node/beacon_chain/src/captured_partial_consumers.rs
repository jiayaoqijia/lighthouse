//! Actual mainnet-preset process artifacts, also consumed by the unchanged spec.
//! The captured block/post-state are an authenticated test anchor, not a replay
//! of the entire preceding chain or an assertion of network execution validity.
use crate::custody_context::NodeCustodyType;
use crate::data_column_verification::{
    GossipDataColumnError, GossipPartialDataColumnError, GossipVerifiedPartialDataColumn,
    KzgVerifiedCustodyPartialDataColumnGloas, PartialColumnVerificationResult,
    validate_partial_data_column_sidecar_for_gossip,
};
use crate::pending_payload_cache::Availability;
use crate::test_utils::BeaconChainHarness;
use flate2::read::GzDecoder;
use serde::Deserialize;
use slot_clock::{SlotClock, TestingSlotClock};
use ssz::{Decode, Encode};
use std::{collections::BTreeMap, io::Read, sync::Arc, time::Duration};
use types::{
    BeaconState, BlockImportSource, ChainSpec, DataColumnSidecarGloas, EthSpec, Hash256,
    MainnetEthSpec, PartialDataColumnGloas, PartialDataColumnSidecarGloas, SignedBeaconBlock, Slot,
};

type E = MainnetEthSpec;

#[derive(Deserialize)]
struct Vectors {
    slot: u64,
    beacon_root: Hash256,
    column_index: u64,
    full_column_ssz: String,
    partials: BTreeMap<String, String>,
}

fn bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex.strip_prefix("0x").unwrap()).unwrap()
}

#[tokio::test]
async fn captured_gloas_rows_reach_real_state_validation_and_pending_availability() {
    let vectors: Vectors = serde_json::from_slice(include_bytes!(
        "../../lighthouse_network/tests/testdata/gloas-multi-row-partials.json"
    ))
    .unwrap();
    let config: types::Config = yaml_serde::from_slice(include_bytes!(
        "../../lighthouse_network/tests/testdata/gloas-private-network.yaml"
    ))
    .unwrap();
    let spec = Arc::new(ChainSpec::from_config::<E>(&config).unwrap());
    let mut raw_state = vec![];
    GzDecoder::new(&include_bytes!("../tests/testdata/gloas-two-row-state.ssz.gz")[..])
        .read_to_end(&mut raw_state)
        .unwrap();
    let mut state = BeaconState::<E>::from_ssz_bytes(&raw_state, &spec).unwrap();
    let block = SignedBeaconBlock::<E>::from_ssz_bytes(
        include_bytes!("../tests/testdata/gloas-two-row-block.ssz"),
        &spec,
    )
    .unwrap();
    assert_eq!(state.canonical_root().unwrap(), block.state_root());
    assert_eq!(block.canonical_root(), vectors.beacon_root);
    assert_eq!(state.validators().len(), 384);
    let clock = TestingSlotClock::new(
        Slot::new(0),
        Duration::from_secs(state.genesis_time()),
        spec.get_slot_duration(),
    );
    clock.set_slot(vectors.slot);
    let block = Arc::new(block);
    let harness = BeaconChainHarness::builder(E::default())
        .spec(spec)
        .deterministic_keypairs(384)
        .node_custody_type(NodeCustodyType::Supernode)
        .testing_slot_clock(clock)
        .initial_state_ephemeral_store(state, (*block).clone(), None)
        .mock_execution_layer()
        .build();
    harness.set_current_slot(Slot::new(vectors.slot));
    harness.chain.pending_payload_cache.insert_block(
        vectors.beacon_root,
        block,
        BlockImportSource::Gossip,
    );
    assert!(
        harness
            .chain
            .pending_payload_cache
            .custody_context()
            .sampling_columns_for_epoch(Slot::new(vectors.slot).epoch(E::slots_per_epoch()))
            .contains(&vectors.column_index)
    );
    let column = |name: &str| PartialDataColumnGloas::<E> {
        block_root: vectors.beacon_root,
        slot: Slot::new(vectors.slot),
        index: vectors.column_index,
        sidecar: PartialDataColumnSidecarGloas::from_ssz_bytes(&bytes(&vectors.partials[name]))
            .unwrap(),
    };
    let verify = |partial: PartialDataColumnGloas<E>| {
        validate_partial_data_column_sidecar_for_gossip(
            Box::new(partial.into()),
            &harness.chain,
            Duration::from_secs(1),
        )
    };
    // Re-run genuine cryptographic and context mutations through the state-bound
    // validator, without a testing-only "verified" constructor or fake proofs.
    let mut bad_cell = column("row_0");
    bad_cell.sidecar.column[0][0] ^= 1;
    let mut bad_proof = column("row_0");
    bad_proof.sidecar.kzg_proofs[0] = column("row_1").sidecar.kzg_proofs[0];
    let mut bad_index = column("row_0");
    bad_index.index = 1;
    let mut bad_slot = column("row_0");
    bad_slot.slot = Slot::new(vectors.slot - 1);
    let mut unknown = column("row_0");
    unknown.block_root = Hash256::repeat_byte(0x75);
    let mut swapped_cells = column("full");
    swapped_cells.sidecar.column.swap(0, 1);
    let mut swapped_proofs = column("full");
    swapped_proofs.sidecar.kzg_proofs.swap(0, 1);
    let mut wrong_bitmap = column("row_1");
    wrong_bitmap.sidecar.cells_present_bitmap = types::CellBitmap::<E>::with_capacity(2).unwrap();
    wrong_bitmap
        .sidecar
        .cells_present_bitmap
        .set(0, true)
        .unwrap();
    let mut wrong_length = column("row_0");
    wrong_length.sidecar.cells_present_bitmap = types::CellBitmap::<E>::with_capacity(3).unwrap();
    wrong_length
        .sidecar
        .cells_present_bitmap
        .set(0, true)
        .unwrap();
    let mut missing_cell = column("row_0");
    missing_cell.sidecar.column = Default::default();
    let mut missing_proof = column("row_0");
    missing_proof.sidecar.kzg_proofs = Default::default();
    for partial in [
        bad_cell,
        bad_proof,
        bad_index,
        bad_slot,
        unknown,
        swapped_cells,
        swapped_proofs,
        wrong_bitmap,
        wrong_length,
        missing_cell,
        missing_proof,
    ] {
        assert!(!matches!(
            verify(partial),
            PartialColumnVerificationResult::Ok(_)
        ));
    }
    for (name, partial, expected_added, expected_complete) in [
        ("row_1", column("row_1"), 1, false),
        ("row_1_duplicate", column("row_1"), 0, false),
        ("row_0", column("row_0"), 1, true),
        ("full_duplicate", column("full"), 0, true),
    ] {
        let result = verify(partial);
        if expected_added == 0 {
            assert!(
                matches!(
                    result,
                    PartialColumnVerificationResult::Err(
                        GossipPartialDataColumnError::GossipDataColumnError(
                            GossipDataColumnError::PriorKnownUnpublished
                        )
                    )
                ),
                "duplicate gossip did not use the verified-cache ignore classification"
            );
            assert_eq!(
                harness
                    .chain
                    .pending_payload_cache
                    .is_column_complete(&vectors.beacon_root, vectors.column_index),
                expected_complete
            );
            continue;
        }
        let PartialColumnVerificationResult::Ok(GossipVerifiedPartialDataColumn::PostGloas {
            column,
            bid,
        }) = result
        else {
            panic!("actual independent-verified {name} failed native state/KZG gossip validation")
        };
        // Custody was established from the actual native supernode context above.
        let custody = KzgVerifiedCustodyPartialDataColumnGloas::from_asserted_custody(column);
        let (availability, merged) = harness
            .chain
            .pending_payload_cache
            .merge_partial_data_columns(vectors.beacon_root, &[custody], &bid)
            .unwrap();
        assert_eq!(merged.added_cells, expected_added, "{name}");
        assert_eq!(
            harness
                .chain
                .pending_payload_cache
                .is_column_complete(&vectors.beacon_root, vectors.column_index),
            expected_complete,
            "{name}"
        );
        assert!(
            matches!(availability, Availability::MissingComponents(root) if root == vectors.beacon_root),
            "partial data promoted an unexecuted/missing envelope"
        );
        assert!(
            !harness
                .chain
                .pending_payload_cache
                .is_blob_data_available(&vectors.beacon_root)
        );
    }
    let columns = harness
        .chain
        .pending_payload_cache
        .get_data_columns(vectors.beacon_root)
        .unwrap();
    assert_eq!(columns.len(), 1);
    let actual = columns[0].as_gloas().unwrap();
    let independent =
        DataColumnSidecarGloas::<E>::from_ssz_bytes(&bytes(&vectors.full_column_ssz)).unwrap();
    assert_eq!(actual.as_ssz_bytes(), independent.as_ssz_bytes());
}
