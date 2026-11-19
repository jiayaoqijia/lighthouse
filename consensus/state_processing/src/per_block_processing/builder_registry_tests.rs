use super::*;
use crate::per_block_processing::withdrawals::gloas::process_withdrawals;
use beacon_chain::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use ssz::Encode;
use std::sync::Arc;
use types::{
    DepositMessage, Domain, MinimalEthSpec, SignedRoot, consts::gloas::BUILDER_INDEX_FLAG,
};

type E = MinimalEthSpec;
type Harness = BeaconChainHarness<EphemeralHarnessType<E>>;
const AMOUNT: u64 = 2_000_000_000;

async fn fixture() -> (Harness, BeaconState<E>) {
    let spec = ForkName::Gloas.make_genesis_spec(E::default_spec());
    let harness = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(spec))
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    harness.extend_to_slot(Slot::new(1)).await;
    let mut state = harness.get_current_state();
    // Apply the real signed head bid as a FULL parent before exercising withdrawals.
    crate::per_block_processing::apply_parent_execution_payload(
        &mut state,
        &types::ExecutionRequestsGloas::default(),
        &harness.spec,
    )
    .unwrap();
    assert_eq!(state.fork_name_unchecked(), ForkName::Gloas);
    assert_eq!(
        *state.latest_block_hash().unwrap(),
        state.latest_execution_payload_bid().unwrap().block_hash()
    );
    (harness, state)
}

fn signed_deposit(harness: &Harness, key: usize, address: u8) -> BuilderDepositRequest {
    let mut credentials = Hash256::ZERO;
    credentials[0] = harness.spec.builder_withdrawal_prefix_byte;
    credentials.as_mut_slice()[12..].fill(address);
    let message = DepositMessage {
        pubkey: (&harness.validator_keypairs[key].pk).into(),
        withdrawal_credentials: credentials,
        amount: AMOUNT,
    };
    let domain = harness.spec.compute_domain(
        Domain::BuilderDeposit,
        harness.spec.genesis_fork_version,
        Hash256::ZERO,
    );
    let signature = harness.validator_keypairs[key]
        .sk
        .sign(message.signing_root(domain));
    let request = BuilderDepositRequest {
        pubkey: message.pubkey,
        withdrawal_credentials: credentials,
        amount: AMOUNT,
        signature: signature.into(),
    };
    assert!(request.is_valid_builder_deposit_signature(&harness.spec));
    request
}

fn recover(state: &mut BeaconState<E>, spec: &ChainSpec) {
    let expected = state.canonical_root().unwrap();
    let bytes = state.as_ssz_bytes();
    *state = BeaconState::from_ssz_bytes(&bytes, spec).unwrap();
    assert_eq!(state.canonical_root().unwrap(), expected);
}

fn sweep(state: &mut BeaconState<E>, harness: &Harness) -> Vec<(u64, Address, u64)> {
    let first = state.next_withdrawal_index().unwrap();
    process_withdrawals(state, &harness.spec).unwrap();
    let all = state.payload_expected_withdrawals().unwrap();
    for (offset, withdrawal) in all.iter().enumerate() {
        assert_eq!(withdrawal.index, first + offset as u64);
    }
    assert_eq!(
        state.next_withdrawal_index().unwrap(),
        first + all.len() as u64
    );
    let builders = all
        .iter()
        .filter(|withdrawal| withdrawal.validator_index & BUILDER_INDEX_FLAG != 0)
        .map(|withdrawal| {
            (
                withdrawal.validator_index,
                withdrawal.address,
                withdrawal.amount,
            )
        })
        .collect();
    recover(state, &harness.spec);
    builders
}

fn activate(state: &mut BeaconState<E>) {
    *state.slot_mut() = Epoch::new(2).start_slot(E::slots_per_epoch());
    state.finalized_checkpoint_mut().epoch = Epoch::new(1);
}

fn exit(state: &mut BeaconState<E>, harness: &Harness, index: usize) {
    let builder = state.get_builder(index as u64).unwrap();
    let request = BuilderExitRequest {
        pubkey: builder.pubkey,
        source_address: builder.execution_address,
    };
    assert!(
        state
            .is_active_builder(index as u64, &harness.spec)
            .unwrap()
    );
    process_builder_exit_requests(state, &[request], &harness.spec).unwrap();
    assert_eq!(
        state.get_builder(index as u64).unwrap().withdrawable_epoch,
        state
            .current_epoch()
            .safe_add(harness.spec.min_builder_withdrawability_delay)
            .unwrap()
    );
}

#[tokio::test]
async fn exited_topups_sweep_once_and_reused_index_requires_new_registration() {
    let (harness, mut state) = fixture().await;
    let old = signed_deposit(&harness, 0, 0x44);
    process_builder_deposit_requests(&mut state, std::slice::from_ref(&old), &harness.spec)
        .unwrap();
    activate(&mut state);
    exit(&mut state, &harness, 0);
    let due = state.get_builder(0).unwrap().withdrawable_epoch;
    let registered = state.get_builder(0).unwrap().deposit_epoch;
    // Existing identity top-ups do not verify another possession proof or change its recipient.
    let mut topup = old.clone();
    topup.signature = bls::SignatureBytes::empty();
    topup.withdrawal_credentials.as_mut_slice()[12..].fill(0x55);
    process_builder_deposit_requests(&mut state, &[topup.clone()], &harness.spec).unwrap();
    assert_eq!(state.get_builder(0).unwrap().withdrawable_epoch, due);
    assert_eq!(state.get_builder(0).unwrap().balance, 2 * AMOUNT);
    assert_eq!(
        state.get_builder(0).unwrap().execution_address,
        Address::repeat_byte(0x44)
    );
    *state.slot_mut() = due.saturating_sub(1u64).start_slot(E::slots_per_epoch());
    assert!(sweep(&mut state, &harness).is_empty());
    assert_eq!(state.get_index_for_new_builder().unwrap(), 1);
    *state.slot_mut() = due.start_slot(E::slots_per_epoch());
    assert_eq!(
        sweep(&mut state, &harness),
        vec![(BUILDER_INDEX_FLAG, Address::repeat_byte(0x44), 2 * AMOUNT)]
    );
    assert_eq!(state.get_builder(0).unwrap().balance, 0);
    assert!(sweep(&mut state, &harness).is_empty());
    assert_eq!(state.get_index_for_new_builder().unwrap(), 0);
    // A swept identity still in the registry is exited, never reactivated by a top-up.
    process_builder_deposit_requests(&mut state, &[topup], &harness.spec).unwrap();
    let reset_due = due
        .safe_add(harness.spec.min_builder_withdrawability_delay)
        .unwrap();
    assert_eq!(state.get_builder(0).unwrap().withdrawable_epoch, reset_due);
    assert_eq!(state.get_builder(0).unwrap().deposit_epoch, registered);
    assert!(!state.is_active_builder(0, &harness.spec).unwrap());
    assert_eq!(state.get_index_for_new_builder().unwrap(), 1);
    assert!(sweep(&mut state, &harness).is_empty());
    *state.slot_mut() = reset_due.start_slot(E::slots_per_epoch());
    assert_eq!(
        sweep(&mut state, &harness),
        vec![(BUILDER_INDEX_FLAG, Address::repeat_byte(0x44), AMOUNT)]
    );
    let replacement = signed_deposit(&harness, 1, 0x66);
    process_builder_deposit_requests(&mut state, &[replacement.clone()], &harness.spec).unwrap();
    assert_eq!(state.builders().unwrap().len(), 1);
    let builder = state.get_builder(0).unwrap();
    assert_eq!(builder.pubkey, replacement.pubkey);
    assert_eq!(builder.execution_address, Address::repeat_byte(0x66));
    assert_eq!(builder.balance, AMOUNT);
    assert_eq!(builder.deposit_epoch, reset_due);
    assert_eq!(builder.withdrawable_epoch, harness.spec.far_future_epoch);
    assert_eq!(
        state
            .get_pending_balance_to_withdraw_for_builder(0)
            .unwrap(),
        0
    );
    recover(&mut state, &harness.spec);
    let before = state.canonical_root().unwrap();
    let mut invalid_old = old.clone();
    invalid_old.signature = bls::SignatureBytes::empty();
    process_builder_deposit_requests(&mut state, &[invalid_old], &harness.spec).unwrap();
    assert_eq!(
        state.canonical_root().unwrap(),
        before,
        "replaced identity must authenticate a new registration"
    );
    process_builder_deposit_requests(&mut state, &[old], &harness.spec).unwrap();
    assert_eq!(state.builders().unwrap().len(), 2);
    assert_eq!(state.get_builder(0).unwrap().pubkey, replacement.pubkey);
    assert_eq!(
        state.get_builder(1).unwrap().pubkey,
        (&harness.validator_keypairs[0].pk).into()
    );
    assert_eq!(state.get_builder(1).unwrap().balance, AMOUNT);
    assert_eq!(state.get_builder(1).unwrap().deposit_epoch, reset_due);
    assert_eq!(
        state.get_builder(1).unwrap().execution_address,
        Address::repeat_byte(0x44)
    );
    assert_eq!(
        state.get_builder(1).unwrap().withdrawable_epoch,
        harness.spec.far_future_epoch
    );
    recover(&mut state, &harness.spec);
}

#[tokio::test]
async fn registry_reuses_first_eligible_zero_balance_index_after_actual_sweep() {
    let (harness, mut state) = fixture().await;
    for key in 0..3 {
        process_builder_deposit_requests(
            &mut state,
            &[signed_deposit(&harness, key, 0x40 + key as u8)],
            &harness.spec,
        )
        .unwrap();
    }
    activate(&mut state);
    for index in 0..3 {
        exit(&mut state, &harness, index);
    }
    let due = state.get_builder(0).unwrap().withdrawable_epoch;
    *state.slot_mut() = due.saturating_sub(1u64).start_slot(E::slots_per_epoch());
    assert_eq!(state.get_index_for_new_builder().unwrap(), 3);
    assert!(sweep(&mut state, &harness).is_empty());
    *state.slot_mut() = due.start_slot(E::slots_per_epoch());
    let paid = sweep(&mut state, &harness);
    assert_eq!(
        paid,
        (0..3u64)
            .map(|index| (
                BUILDER_INDEX_FLAG | index,
                Address::repeat_byte(0x40 + index as u8),
                AMOUNT
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(paid.iter().map(|x| x.2).sum::<u64>(), 3 * AMOUNT);
    assert!(sweep(&mut state, &harness).is_empty());
    // A real same-key deposit fills index 0 again and delays its sweep, so select 1 then 2.
    process_builder_deposit_requests(
        &mut state,
        &[signed_deposit(&harness, 0, 0x99)],
        &harness.spec,
    )
    .unwrap();
    for (key, expected_index) in [(3, 1), (4, 2)] {
        assert_eq!(state.get_index_for_new_builder().unwrap(), expected_index);
        let deposit = signed_deposit(&harness, key, 0x70 + key as u8);
        process_builder_deposit_requests(&mut state, &[deposit.clone()], &harness.spec).unwrap();
        assert_eq!(
            state.get_builder(expected_index).unwrap().pubkey,
            deposit.pubkey
        );
        assert_eq!(state.builders().unwrap().len(), 3);
        recover(&mut state, &harness.spec);
    }
    let reset_due = state.get_builder(0).unwrap().withdrawable_epoch;
    *state.slot_mut() = reset_due.start_slot(E::slots_per_epoch());
    assert_eq!(
        state.get_index_for_new_builder().unwrap(),
        3,
        "balance must be swept before reuse"
    );
    assert_eq!(
        sweep(&mut state, &harness),
        vec![(BUILDER_INDEX_FLAG, Address::repeat_byte(0x40), AMOUNT)]
    );
    assert_eq!(state.get_index_for_new_builder().unwrap(), 0);
    let replacement = signed_deposit(&harness, 5, 0x88);
    process_builder_deposit_requests(&mut state, &[replacement.clone()], &harness.spec).unwrap();
    assert_eq!(state.builders().unwrap().len(), 3);
    assert_eq!(state.get_builder(0).unwrap().pubkey, replacement.pubkey);
    assert_eq!(
        state.get_builder(0).unwrap().execution_address,
        Address::repeat_byte(0x88)
    );
    assert_eq!(state.get_builder(0).unwrap().balance, AMOUNT);
    recover(&mut state, &harness.spec);
}
