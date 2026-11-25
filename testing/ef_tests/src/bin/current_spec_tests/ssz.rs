use ef_tests::*;
use std::path::Path;
use types::*;
pub fn consume<E: EthSpec + TypeName>(name: &str, path: &Path) -> Result<Vec<CaseResult>, String> {
    macro_rules! run {
        ($ty:ty) => {
            Ok(SszStaticHandler::<$ty, E>::default().snapshot_results(path, ForkName::Gloas))
        };
    }
    match name {
        "AttestationData" => run!(AttestationData),
        "BLSToExecutionChange" => run!(BlsToExecutionChange),
        "BeaconBlockHeader" => run!(BeaconBlockHeader),
        "Builder" => run!(Builder),
        "BuilderDepositRequest" => run!(BuilderDepositRequest),
        "BuilderExitRequest" => run!(BuilderExitRequest),
        "BuilderPendingPayment" => run!(BuilderPendingPayment),
        "BuilderPendingWithdrawal" => run!(BuilderPendingWithdrawal),
        "Checkpoint" => run!(Checkpoint),
        "ConsolidationRequest" => run!(ConsolidationRequest),
        "Deposit" => run!(Deposit),
        "DepositData" => run!(DepositData),
        "DepositMessage" => run!(DepositMessage),
        "DepositRequest" => run!(DepositRequest),
        "Eth1Data" => run!(Eth1Data),
        "Fork" => run!(Fork),
        "ForkData" => run!(ForkData),
        "PayloadAttestationData" => run!(PayloadAttestationData),
        "PayloadAttestationMessage" => run!(PayloadAttestationMessage),
        "PendingConsolidation" => run!(PendingConsolidation),
        "PendingDeposit" => run!(PendingDeposit),
        "PendingPartialWithdrawal" => run!(PendingPartialWithdrawal),
        "ProposerPreferences" => run!(ProposerPreferences),
        "ProposerSlashing" => run!(ProposerSlashing),
        "SignedBLSToExecutionChange" => run!(SignedBlsToExecutionChange),
        "SignedBeaconBlockHeader" => run!(SignedBeaconBlockHeader),
        "SignedProposerPreferences" => run!(SignedProposerPreferences),
        "SignedVoluntaryExit" => run!(SignedVoluntaryExit),
        "SigningData" => run!(SigningData),
        "SingleAttestation" => run!(SingleAttestation),
        "SyncAggregatorSelectionData" => run!(SyncAggregatorSelectionData),
        "SyncCommitteeMessage" => run!(SyncCommitteeMessage),
        "Validator" => run!(Validator),
        "VoluntaryExit" => run!(VoluntaryExit),
        "Withdrawal" => run!(Withdrawal),
        "WithdrawalRequest" => run!(WithdrawalRequest),
        "AggregateAndProof" => run!(AggregateAndProofGloas<E>),
        "Attestation" => run!(AttestationGloas<E>),
        "AttesterSlashing" => run!(AttesterSlashingGloas<E>),
        "BeaconBlockBody" => run!(BeaconBlockBodyGloas<E>),
        "ContributionAndProof" => run!(ContributionAndProof<E>),
        "DataColumnSidecar" => run!(DataColumnSidecarGloas<E>),
        "ExecutionPayload" => run!(ExecutionPayloadGloas<E>),
        "ExecutionPayloadBid" => run!(ExecutionPayloadBidGloas<E>),
        "ExecutionPayloadEnvelope" => run!(ExecutionPayloadEnvelope<E>),
        "ExecutionRequests" => run!(ExecutionRequestsGloas<E>),
        "HistoricalSummary" => run!(types::state::HistoricalSummary),
        "IndexedAttestation" => run!(IndexedAttestationGloas<E>),
        "IndexedPayloadAttestation" => run!(IndexedPayloadAttestation<E>),
        "LightClientBootstrap" => run!(LightClientBootstrapGloas<E>),
        "LightClientFinalityUpdate" => run!(LightClientFinalityUpdateGloas<E>),
        "LightClientHeader" => run!(LightClientHeaderGloas<E>),
        "LightClientOptimisticUpdate" => run!(LightClientOptimisticUpdateGloas<E>),
        "LightClientUpdate" => run!(LightClientUpdateGloas<E>),
        "NewPayloadRequest" => run!(NewPayloadRequest<E>),
        "PayloadAttestation" => run!(PayloadAttestation<E>),
        "SignedAggregateAndProof" => run!(SignedAggregateAndProofGloas<E>),
        "SignedContributionAndProof" => run!(SignedContributionAndProof<E>),
        "SignedExecutionPayloadBid" => run!(SignedExecutionPayloadBidGloas<E>),
        "SignedExecutionPayloadEnvelope" => run!(SignedExecutionPayloadEnvelope<E>),
        "SyncAggregate" => run!(SyncAggregate<E>),
        "SyncCommittee" => run!(SyncCommittee<E>),
        "SyncCommitteeContribution" => run!(SyncCommitteeContribution<E>),
        "BeaconBlock" => Ok(SszStaticWithSpecHandler::<BeaconBlock<E>, E>::default()
            .snapshot_results(path, ForkName::Gloas)),
        "SignedBeaconBlock" => Ok(
            SszStaticWithSpecHandler::<SignedBeaconBlock<E>, E>::default()
                .snapshot_results(path, ForkName::Gloas),
        ),
        "BeaconState" => Ok(SszStaticTHCHandler::<BeaconState<E>, E>::default()
            .snapshot_results(path, ForkName::Gloas)),
        "DataColumnsByRootIdentifier" => Ok(SszStaticWithSpecHandler::<
            DataColumnsByRootIdentifier<E>,
            E,
        >::default()
        .snapshot_results(path, ForkName::Gloas)),
        _ => helpers::<E>(name, path),
    }
}

use serde::Deserialize;
use ssz::{Decode, Encode};
use std::fmt::Debug;
use tree_hash::TreeHash;

#[derive(
    Debug,
    Clone,
    PartialEq,
    ssz_derive::Encode,
    ssz_derive::Decode,
    tree_hash_derive::TreeHash,
    Deserialize,
)]
#[serde(deny_unknown_fields)]
struct Eth1Block {
    timestamp: u64,
    deposit_root: Hash256,
    deposit_count: u64,
}
#[derive(
    Debug,
    Clone,
    PartialEq,
    ssz_derive::Encode,
    ssz_derive::Decode,
    tree_hash_derive::TreeHash,
    Deserialize,
)]
#[serde(deny_unknown_fields)]
struct PowBlock {
    block_hash: ExecutionBlockHash,
    parent_hash: ExecutionBlockHash,
    total_difficulty: Uint256,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Group {
    beacon_block_root: Hash256,
    slot: Slot,
}
#[derive(Deserialize)]
#[serde(bound = "E:EthSpec", deny_unknown_fields)]
struct Parts<E: EthSpec> {
    available: CellBitmap<E>,
    requests: CellBitmap<E>,
}
#[derive(Deserialize)]
#[serde(bound = "E:EthSpec", deny_unknown_fields)]
struct Partial<E: EthSpec> {
    cells_present_bitmap: CellBitmap<E>,
    #[serde(with = "ssz_types::serde_utils::prog_list_of_hex_fixed_vec")]
    partial_column: ssz_types::ProgressiveVariableList<Cell<E>, E::MaxBlobCommitmentsPerBlock>,
    kzg_proofs: ssz_types::ProgressiveVariableList<kzg::KzgProof, E::MaxBlobCommitmentsPerBlock>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Matrix {
    cell: String,
    kzg_proof: kzg::KzgProof,
    column_index: u64,
    row_index: u64,
}
fn merkle(fields: &[Hash256]) -> Hash256 {
    let bytes: Vec<u8> = fields
        .iter()
        .flat_map(|field| field.as_slice().iter().copied())
        .collect();
    tree_hash::merkle_root(&bytes, fields.len())
}
fn yaml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    yaml_serde::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
fn check<T: Encode + Decode + PartialEq + Debug>(
    value: T,
    root: Hash256,
    want: &[u8],
    expected: Hash256,
) -> Result<(), String> {
    if value.as_ssz_bytes() != want {
        return Err("native SSZ encoding differs".into());
    }
    let decoded = T::from_ssz_bytes(want).map_err(|e| format!("native SSZ decode: {e:?}"))?;
    if decoded != value {
        return Err("native SSZ decoded value differs".into());
    }
    if root != expected {
        return Err(format!(
            "tree root differs: actual {root} expected {expected}"
        ));
    }
    Ok(())
}
fn helper_case<E: EthSpec>(name: &str, path: &Path) -> Result<(), String> {
    let compressed =
        std::fs::read(path.join("serialized.ssz_snappy")).map_err(|e| e.to_string())?;
    let want = snap::raw::Decoder::new()
        .decompress_vec(&compressed)
        .map_err(|e| e.to_string())?;
    #[derive(Deserialize)]
    struct Roots {
        root: Hash256,
    }
    let roots: Roots = yaml(&path.join("roots.yaml"))?;
    let value = path.join("value.yaml");
    match name {
        "Eth1Block" => {
            let v: Eth1Block = yaml(&value)?;
            let root = v.tree_hash_root();
            check(v, root, &want, roots.root)
        }
        "PowBlock" => {
            let v: PowBlock = yaml(&value)?;
            let root = v.tree_hash_root();
            check(v, root, &want, roots.root)
        }
        "PartialDataColumnGroupID" => {
            let v: Group = yaml(&value)?;
            let root = merkle(&[v.beacon_block_root, v.slot.tree_hash_root()]);
            check(
                PartialDataColumnGroupId {
                    beacon_block_root: v.beacon_block_root,
                    slot: v.slot,
                },
                root,
                &want,
                roots.root,
            )
        }
        "PartialDataColumnPartsMetadata" => {
            let v: Parts<E> = yaml(&value)?;
            let root = merkle(&[v.available.tree_hash_root(), v.requests.tree_hash_root()]);
            check(
                PartialDataColumnPartsMetadata::<E> {
                    available: v.available,
                    requests: v.requests,
                },
                root,
                &want,
                roots.root,
            )
        }
        "PartialDataColumnSidecar" => {
            let v: Partial<E> = yaml(&value)?;
            let native = PartialDataColumnSidecarGloas::<E> {
                cells_present_bitmap: v.cells_present_bitmap,
                column: v.partial_column,
                kzg_proofs: v.kzg_proofs,
            };
            let root = native.tree_hash_root();
            check(native, root, &want, roots.root)
        }
        "MatrixEntry" => {
            let v: Matrix = yaml(&value)?;
            let cell = Cell::<E>::new(
                hex::decode(v.cell.trim_start_matches("0x")).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("cell width: {e:?}"))?;
            let root = merkle(&[
                cell.tree_hash_root(),
                v.kzg_proof.tree_hash_root(),
                v.column_index.tree_hash_root(),
                v.row_index.tree_hash_root(),
            ]);
            let mut encoded = cell.as_ssz_bytes();
            encoded.extend(v.kzg_proof.as_ssz_bytes());
            encoded.extend(v.column_index.as_ssz_bytes());
            encoded.extend(v.row_index.as_ssz_bytes());
            if encoded != want || root != roots.root {
                Err("reference MatrixEntry helper codec/root differs".into())
            } else {
                Ok(())
            }
        }
        _ => Err(format!("unsupported current SSZ type {name}")),
    }
}
fn helpers<E: EthSpec>(name: &str, path: &Path) -> Result<Vec<CaseResult>, String> {
    if !matches!(
        name,
        "Eth1Block"
            | "PowBlock"
            | "PartialDataColumnGroupID"
            | "PartialDataColumnPartsMetadata"
            | "PartialDataColumnSidecar"
            | "MatrixEntry"
    ) {
        return Err(format!("unsupported current SSZ type {name}"));
    }
    let mut paths = Vec::new();
    for suite in std::fs::read_dir(path).map_err(|e| e.to_string())? {
        let suite = suite.map_err(|e| e.to_string())?;
        if !suite.file_type().map_err(|e| e.to_string())?.is_dir() {
            continue;
        }
        for case in std::fs::read_dir(suite.path()).map_err(|e| e.to_string())? {
            let case = case.map_err(|e| e.to_string())?;
            if case.file_type().map_err(|e| e.to_string())?.is_dir() {
                paths.push(case.path());
            }
        }
    }
    paths.sort();
    if paths.is_empty() {
        return Err("empty SSZ handler".into());
    }
    Ok(paths
        .into_iter()
        .enumerate()
        .map(|(case_index, path)| {
            let result = helper_case::<E>(name, &path).map_err(Error::FailedToParseTest);
            CaseResult {
                case_index,
                desc: name.into(),
                path,
                result,
            }
        })
        .collect())
}
