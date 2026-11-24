//! Consume every case in an independently generated Gloas snapshot.
use ef_tests::*;
use std::path::Path;
use types::*;

fn consume<E: EthSpec + TypeName>(
    runner: &str,
    handler: &str,
    path: &Path,
) -> Result<Vec<CaseResult>, String> {
    macro_rules! run {
        ($handler:expr) => {
            Ok($handler.snapshot_results(path, ForkName::Gloas))
        };
    }
    macro_rules! operation {
        ($ty:ty) => {
            run!(OperationsHandler::<E, $ty>::default())
        };
    }
    macro_rules! epoch {
        ($ty:ty) => {
            run!(EpochProcessingHandler::<E, $ty>::default())
        };
    }
    match (runner, handler) {
        ("operations", "attestation") => operation!(Attestation<E>),
        ("operations", "attester_slashing") => operation!(AttesterSlashing<E>),
        ("operations", "block_header") => operation!(BeaconBlock<E>),
        ("operations", "builder_deposit_request") => operation!(BuilderDepositRequest),
        ("operations", "builder_exit_request") => operation!(BuilderExitRequest),
        ("operations", "consolidation_request") => operation!(ConsolidationRequest),
        ("operations", "deposit_request") => operation!(DepositRequest),
        ("operations", "execution_payload_bid") => operation!(ExecutionPayloadBidBlock<E>),
        ("operations", "parent_execution_payload") => operation!(ParentExecutionPayloadBlock<E>),
        ("operations", "payload_attestation") => operation!(PayloadAttestation<E>),
        ("operations", "proposer_slashing") => operation!(ProposerSlashing),
        ("operations", "sync_aggregate") => operation!(SyncAggregate<E>),
        ("operations", "voluntary_exit") => operation!(SignedVoluntaryExit),
        ("operations", "voluntary_exit_churn") => operation!(VoluntaryExitChurn),
        ("operations", "withdrawal_request") => operation!(WithdrawalRequest),
        ("operations", "withdrawals") => operation!(WithdrawalsPayload<E>),
        ("epoch_processing", "builder_pending_payments") => epoch!(BuilderPendingPayments),
        ("epoch_processing", "effective_balance_updates") => epoch!(EffectiveBalanceUpdates),
        ("epoch_processing", "eth1_data_reset") => epoch!(Eth1DataReset),
        ("epoch_processing", "historical_summaries_update") => epoch!(HistoricalSummariesUpdate),
        ("epoch_processing", "inactivity_updates") => epoch!(InactivityUpdates),
        ("epoch_processing", "participation_flag_updates") => epoch!(ParticipationFlagUpdates),
        ("epoch_processing", "pending_consolidations") => epoch!(PendingConsolidations),
        ("epoch_processing", "pending_deposits") => epoch!(PendingBalanceDeposits),
        ("epoch_processing", "pending_deposits_churn") => epoch!(PendingDepositsChurn),
        ("epoch_processing", "proposer_lookahead") => epoch!(ProposerLookahead),
        ("epoch_processing", "ptc_window") => epoch!(PtcWindow),
        ("epoch_processing", "registry_updates") => epoch!(RegistryUpdates),
        ("fork", "fork") => run!(ForkHandler::<E>::default()),
        ("transition", "core") => run!(TransitionHandler::<E>::default()),
        ("sanity", "blocks") => run!(SanityBlocksHandler::<E>::default()),
        ("sanity", "slots") => run!(SanitySlotsHandler::<E>::default()),
        ("random", "random") => run!(RandomHandler::<E>::default()),
        ("light_client", "single_merkle_proof") => run!(MerkleProofValidityHandler::<E>::default()),
        ("fork_choice", _) => {
            let name: &'static str = match handler {
                "get_head" => "get_head",
                "get_parent_payload_status" => "get_parent_payload_status",
                "on_attestation" => "on_attestation",
                "on_block" => "on_block",
                "on_execution_payload_envelope" => "on_execution_payload_envelope",
                "on_payload_attestation_message" => "on_payload_attestation_message",
                "payload_data_availability" => "payload_data_availability",
                "payload_timeliness" => "payload_timeliness",
                "should_apply_proposer_boost" => "should_apply_proposer_boost",
                "deposit_with_reorg" => "deposit_with_reorg",
                "filter_node_tree_variants" => "filter_node_tree_variants",
                _ => return Err(format!("unsupported fork-choice handler {handler}")),
            };
            run!(ForkChoiceHandler::<E>::new(name))
        }
        ("networking", _) => {
            let name = match handler {
                "gossip_beacon_aggregate_and_proof" => "gossip_beacon_aggregate_and_proof",
                "gossip_beacon_attestation" => "gossip_beacon_attestation",
                "gossip_beacon_block" => "gossip_beacon_block",
                "gossip_bls_to_execution_change" => "gossip_bls_to_execution_change",
                "gossip_voluntary_exit" => "gossip_voluntary_exit",
                "gossip_execution_payload_bid" => "gossip_execution_payload_bid",
                "gossip_execution_payload_envelope" => "gossip_execution_payload_envelope",
                "gossip_payload_attestation_message" => "gossip_payload_attestation_message",
                "gossip_proposer_preferences" => "gossip_proposer_preferences",
                "gossip_data_column_sidecar" => "gossip_data_column_sidecar",
                _ => return Err(format!("unsupported networking handler {handler}")),
            };
            run!(GossipValidationHandler::<E>::new(name))
        }
        ("fast_confirmation", _) => {
            let name = match handler {
                "is_one_confirmed" => "is_one_confirmed",
                "previous_epoch" => "previous_epoch",
                "reconfirmation" => "reconfirmation",
                "restart_gu" => "restart_gu",
                _ => return Err(format!("unsupported fast-confirmation handler {handler}")),
            };
            run!(FastConfirmationHandler::<E>::new(name))
        }
        _ => Err(format!("unsupported snapshot consumer {runner}/{handler}")),
    }
}

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert_eq!(
        args.len(),
        3,
        "usage: current_spec_tests mainnet|minimal RUNNER HANDLER_DIRECTORY"
    );
    assert!(
        !cfg!(feature = "fake_crypto"),
        "state/gossip acceptance requires real BLS"
    );
    let path = Path::new(&args[2]);
    let handler = path
        .file_name()
        .expect("handler path")
        .to_str()
        .expect("UTF-8 handler");
    let results = match args[0].as_str() {
        "mainnet" => consume::<MainnetEthSpec>(&args[1], handler, path),
        "minimal" => consume::<MinimalEthSpec>(&args[1], handler, path),
        _ => Err("unsupported preset".into()),
    }
    .unwrap_or_else(|error| panic!("{error}"));
    let mut failed = 0;
    for case in results {
        if case.result.is_err() {
            failed += 1;
        }
        println!(
            "{}",
            serde_json::json!({"path":case.path,"passed":case.result.is_ok(),"error":case.result.err().map(|error|format!("{}: {}",error.name(),error.message())),"crypto":"real"})
        );
    }
    assert_eq!(failed, 0, "snapshot cases failed or were skipped");
}
