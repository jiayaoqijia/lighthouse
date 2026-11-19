use super::*;
use beacon_chain::test_utils::BeaconChainHarness;
use ssz::{Decode, Encode};
use std::sync::Arc;
use types::{BuilderPendingWithdrawal, Epoch, ExecutionRequestsGloas, MinimalEthSpec};

const DEPOSIT: &[u8] = include_bytes!("../../testdata/eip8282/builder-deposit.ssz");
const WRONG_DOMAIN: &[u8] = include_bytes!("../../testdata/eip8282/wrong-domain-deposit.ssz");
const EXIT: &[u8] = include_bytes!("../../testdata/eip8282/builder-exit.ssz");

/// These exact records are also emitted by the native EL HTTP Engine regression.
/// Real BLS verification independently checks its domain and SSZ signing root.
#[tokio::test]
async fn native_el_builder_requests_reach_actual_cl_state_processing() {
    type E = MinimalEthSpec;
    let mut spec = ForkName::Gloas.make_genesis_spec(E::default_spec());
    spec.genesis_fork_version = [0, 0, 0, 1];
    let spec = Arc::new(spec);
    let harness = BeaconChainHarness::builder(E::default())
        .spec(spec.clone())
        .deterministic_keypairs(E::slots_per_epoch() as usize)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    let mut state = harness.get_current_state();
    let validators = state.validators().iter().cloned().collect::<Vec<_>>();
    let balances = state.balances().iter().copied().collect::<Vec<_>>();
    let pending_deposits = state
        .pending_deposits()
        .unwrap()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    assert!(state.builders().unwrap().is_empty());

    let deposit = BuilderDepositRequest::from_ssz_bytes(DEPOSIT).unwrap();
    let wrong_domain = BuilderDepositRequest::from_ssz_bytes(WRONG_DOMAIN).unwrap();
    let exit = BuilderExitRequest::from_ssz_bytes(EXIT).unwrap();
    assert_eq!(deposit.amount, 1_000_000_000);
    assert_eq!(deposit.as_ssz_bytes(), DEPOSIT);
    assert_eq!(exit.as_ssz_bytes(), EXIT);
    assert!(deposit.is_valid_builder_deposit_signature(&spec));
    assert!(!wrong_domain.is_valid_builder_deposit_signature(&spec));
    process_builder_deposit_requests(&mut state, &[wrong_domain], &spec).unwrap();
    assert!(state.builders().unwrap().is_empty());
    process_builder_deposit_requests(&mut state, std::slice::from_ref(&deposit), &spec).unwrap();
    assert_eq!(state.builders().unwrap().len(), 1);
    assert_eq!(state.get_builder(0).unwrap().balance, deposit.amount);
    assert_eq!(
        state.get_builder(0).unwrap().execution_address,
        exit.source_address
    );

    let mut requests = ExecutionRequestsGloas::<E>::default();
    requests.builder_deposits.push(deposit.clone()).unwrap();
    requests.builder_exits.push(exit.clone()).unwrap();
    let wire = requests.get_execution_requests_list();
    assert_eq!(wire.len(), 2);
    assert_eq!(wire[0].as_ref(), [vec![3], DEPOSIT.to_vec()].concat());
    assert_eq!(wire[1].as_ref(), [vec![4], EXIT.to_vec()].concat());

    // An existing builder top-up preserves its execution address and does not
    // require another proof of possession. Keep the required B0 prefix.
    let mut top_up = deposit.clone();
    top_up.amount = 2_000_000_000;
    top_up.signature = bls::SignatureBytes::empty();
    top_up.withdrawal_credentials[31] ^= 1;
    process_builder_deposit_requests(&mut state, &[top_up], &spec).unwrap();
    assert_eq!(state.get_builder(0).unwrap().balance, 3_000_000_000);
    assert_eq!(
        state.get_builder(0).unwrap().execution_address,
        exit.source_address
    );

    // Inactive builders cannot exit. Then make every other condition valid
    // before testing source-address and pending-withdrawal rejection.
    process_builder_exit_requests(&mut state, std::slice::from_ref(&exit), &spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        spec.far_future_epoch
    );
    *state.slot_mut() = Epoch::new(2).start_slot(E::slots_per_epoch());
    state.finalized_checkpoint_mut().epoch = Epoch::new(1);
    assert!(state.is_active_builder(0, &spec).unwrap());
    let mut unauthorized = exit.clone();
    unauthorized.source_address = types::Address::ZERO;
    process_builder_exit_requests(&mut state, &[unauthorized], &spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        spec.far_future_epoch
    );
    state
        .builder_pending_withdrawals_mut()
        .unwrap()
        .push(BuilderPendingWithdrawal {
            fee_recipient: exit.source_address,
            amount: 1,
            builder_index: 0,
        })
        .unwrap();
    assert_eq!(
        state
            .get_pending_balance_to_withdraw_for_builder(0)
            .unwrap(),
        1
    );
    process_builder_exit_requests(&mut state, std::slice::from_ref(&exit), &spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        spec.far_future_epoch
    );
    state
        .builder_pending_withdrawals_mut()
        .unwrap()
        .pop_front(1)
        .unwrap();
    // A pending bid payment also blocks exit before it reaches withdrawals.
    state
        .builder_pending_payments_mut()
        .unwrap()
        .get_mut(0)
        .unwrap()
        .withdrawal = BuilderPendingWithdrawal {
        fee_recipient: exit.source_address,
        amount: 2,
        builder_index: 0,
    };
    assert_eq!(
        state
            .get_pending_balance_to_withdraw_for_builder(0)
            .unwrap(),
        2
    );
    process_builder_exit_requests(&mut state, std::slice::from_ref(&exit), &spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        spec.far_future_epoch
    );
    *state
        .builder_pending_payments_mut()
        .unwrap()
        .get_mut(0)
        .unwrap() = types::BuilderPendingPayment::default();
    process_builder_exit_requests(&mut state, &[exit], &spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        Epoch::new(2)
            .safe_add(spec.min_builder_withdrawability_delay)
            .unwrap()
    );
    assert_eq!(
        state.validators().iter().cloned().collect::<Vec<_>>(),
        validators
    );
    assert_eq!(
        state.balances().iter().copied().collect::<Vec<_>>(),
        balances
    );
    assert_eq!(
        state
            .pending_deposits()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        pending_deposits
    );
}
