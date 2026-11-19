use super::{StateContext, process_builder_pending_payments};
use crate::{
    ConsensusContext, VerifySignatures,
    common::slash_validator,
    per_block_processing::{
        apply_parent_execution_payload, process_execution_payload_bid,
        process_operations::process_builder_exit_requests, settle_builder_payment,
        withdrawals::gloas::process_withdrawals,
    },
};
use beacon_chain::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use safe_arith::SafeArith;
use std::sync::Arc;
use tree_hash::TreeHash;
use types::{
    Address, BeaconState, Builder, BuilderExitRequest, BuilderPendingPayment,
    BuilderPendingWithdrawal, Epoch, EthSpec, ExecutionBlockHash, ExecutionPayloadBidGloas,
    ExecutionRequestsGloas, ForkName, MinimalEthSpec, Slot,
    consts::gloas::{BUILDER_INDEX_FLAG, PAYLOAD_BUILDER_VERSION},
};

type E = MinimalEthSpec;
type Harness = BeaconChainHarness<EphemeralHarnessType<E>>;
const INITIAL: u64 = 10_000_000_000;
const AMOUNT: u64 = 1_000_000_000;

async fn fixture() -> (Harness, BeaconState<E>) {
    let spec = ForkName::Gloas.make_genesis_spec(E::default_spec());
    let harness = BeaconChainHarness::builder(E::default())
        .spec(Arc::new(spec))
        .deterministic_keypairs(64)
        .fresh_ephemeral_store()
        .mock_execution_layer()
        .build();
    harness.extend_to_slot(Slot::new(17)).await;
    let mut state = harness.get_current_state();
    state.finalized_checkpoint_mut().epoch = Epoch::new(1);
    state
        .builders_mut()
        .unwrap()
        .push(Builder {
            pubkey: (&harness.validator_keypairs[0].pk).into(),
            version: PAYLOAD_BUILDER_VERSION,
            execution_address: Address::repeat_byte(0x44),
            balance: INITIAL,
            deposit_epoch: Epoch::new(0),
            withdrawable_epoch: harness.spec.far_future_epoch,
        })
        .unwrap();
    let bid = ExecutionPayloadBidGloas {
        parent_block_hash: *state.latest_block_hash().unwrap(),
        parent_block_root: *state
            .get_block_root(state.slot().saturating_sub(1u64))
            .unwrap(),
        block_hash: ExecutionBlockHash::repeat_byte(0x42),
        prev_randao: *state.get_randao_mix(state.current_epoch()).unwrap(),
        slot: state.slot(),
        builder_index: 0,
        value: AMOUNT,
        fee_recipient: Address::repeat_byte(0x33),
        gas_limit: 30_000_000,
        execution_requests_root: ExecutionRequestsGloas::<E>::default().tree_hash_root(),
        ..ExecutionPayloadBidGloas::default()
    };
    let signed = harness.sign_payload_bid(bid, &state);
    process_execution_payload_bid(
        &mut state,
        signed.to_ref(),
        VerifySignatures::True,
        &harness.spec,
    )
    .unwrap();
    assert_eq!(
        state
            .get_pending_balance_to_withdraw_for_builder(0)
            .unwrap(),
        AMOUNT
    );
    assert_eq!(state.get_builder(0).unwrap().balance, INITIAL);
    (harness, state)
}

fn roll_payments(state: &mut BeaconState<E>, harness: &Harness) {
    let spec = &harness.spec;
    let context = StateContext {
        current_epoch: state.current_epoch(),
        next_epoch: state.current_epoch().safe_add(1).unwrap(),
        finalized_checkpoint: state.finalized_checkpoint(),
        is_in_inactivity_leak: false,
        total_active_balance: state.get_total_active_balance().unwrap(),
        churn_limit: state.get_validator_churn_limit(spec).unwrap(),
        fork_name: state.fork_name_unchecked(),
    };
    process_builder_pending_payments(state, &context, spec).unwrap();
}

fn assert_paid_once(state: &mut BeaconState<E>, harness: &Harness) {
    assert_eq!(state.builder_pending_withdrawals().unwrap().len(), 1);
    let queued = state.builder_pending_withdrawals().unwrap().get(0).unwrap();
    assert_eq!(queued.amount, AMOUNT);
    assert_eq!(queued.fee_recipient, Address::repeat_byte(0x33));
    assert_eq!(
        state.get_builder(0).unwrap().balance,
        INITIAL,
        "queueing is not a debit"
    );
    let exit = BuilderExitRequest {
        source_address: Address::repeat_byte(0x44),
        pubkey: state.get_builder(0).unwrap().pubkey,
    };
    process_builder_exit_requests(state, std::slice::from_ref(&exit), &harness.spec).unwrap();
    assert_eq!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        harness.spec.far_future_epoch,
        "pending payment must block exit"
    );
    process_withdrawals(state, &harness.spec).unwrap();
    let withdrawals = state.payload_expected_withdrawals().unwrap();
    let paid = withdrawals
        .iter()
        .filter(|w| w.address == Address::repeat_byte(0x33))
        .collect::<Vec<_>>();
    assert_eq!(paid.len(), 1);
    assert_eq!(paid[0].validator_index, BUILDER_INDEX_FLAG);
    assert_eq!(paid[0].amount, AMOUNT);
    assert_eq!(
        state.get_builder(0).unwrap().balance + paid[0].amount,
        INITIAL
    );
    assert!(state.builder_pending_withdrawals().unwrap().is_empty());
    process_withdrawals(state, &harness.spec).unwrap();
    assert_eq!(state.get_builder(0).unwrap().balance, INITIAL - AMOUNT);
    assert!(
        !state
            .payload_expected_withdrawals()
            .unwrap()
            .iter()
            .any(|w| w.address == Address::repeat_byte(0x33))
    );
    process_builder_exit_requests(state, &[exit], &harness.spec).unwrap();
    assert_ne!(
        state.get_builder(0).unwrap().withdrawable_epoch,
        harness.spec.far_future_epoch
    );
}

#[tokio::test]
async fn signed_bid_current_previous_and_evicted_parent_settle_once() {
    for epochs in 0..=2 {
        let (harness, mut state) = fixture().await;
        let parent = state.latest_execution_payload_bid().unwrap().slot();
        for _ in 0..epochs {
            roll_payments(&mut state, &harness);
        }
        *state.slot_mut() = Epoch::new(parent.epoch(E::slots_per_epoch()).as_u64() + epochs)
            .start_slot(E::slots_per_epoch())
            .saturating_add(1u64);
        // Current/previous windows retain the reservation, while a no-quorum old
        // parent loses its window entry and settles directly from its signed bid.
        assert_eq!(state.builder_pending_withdrawals().unwrap().len(), 0);
        apply_parent_execution_payload(
            &mut state,
            &ExecutionRequestsGloas::default(),
            &harness.spec,
        )
        .unwrap();
        assert_eq!(
            *state.latest_block_hash().unwrap(),
            state.latest_execution_payload_bid().unwrap().block_hash()
        );
        if epochs < 2 {
            let index = parent.as_usize() % E::slots_per_epoch() as usize
                + if epochs == 0 {
                    E::slots_per_epoch() as usize
                } else {
                    0
                };
            let root = state.canonical_root().unwrap();
            settle_builder_payment(&mut state, index).unwrap();
            assert_eq!(
                state.canonical_root().unwrap(),
                root,
                "cleared payment must not queue again"
            );
        }
        assert_paid_once(&mut state, &harness);
    }
}

#[tokio::test]
async fn payment_quorum_edges_rotate_and_evict_without_debit() {
    let (harness, mut state) = fixture().await;
    let quorum = (64 * 32_000_000_000 / E::slots_per_epoch()) * 6 / 10;
    for (index, weight) in [quorum - 1, quorum, quorum + 1].into_iter().enumerate() {
        *state
            .builder_pending_payments_mut()
            .unwrap()
            .get_mut(index)
            .unwrap() = BuilderPendingPayment {
            weight,
            withdrawal: BuilderPendingWithdrawal {
                fee_recipient: Address::repeat_byte(index as u8 + 1),
                amount: AMOUNT,
                builder_index: 0,
            },
            proposer_index: 1,
        };
    }
    roll_payments(&mut state, &harness);
    let queued = state
        .builder_pending_withdrawals()
        .unwrap()
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(queued.len(), 2);
    assert_eq!(queued[0].fee_recipient, Address::repeat_byte(2));
    assert_eq!(queued[1].fee_recipient, Address::repeat_byte(3));
    assert_eq!(state.get_builder(0).unwrap().balance, INITIAL);
    roll_payments(&mut state, &harness);
    assert_eq!(
        state.builder_pending_withdrawals().unwrap().len(),
        2,
        "no-quorum window must not pay on eviction"
    );
    assert!(
        state
            .builder_pending_payments()
            .unwrap()
            .iter()
            .all(|p| p.withdrawal.amount == 0)
    );
}

#[tokio::test]
async fn slashed_proposer_payment_is_cancelled_before_parent_settlement() {
    let (harness, mut state) = fixture().await;
    let mut context = ConsensusContext::new(state.slot());
    let proposer = context.get_proposer_index(&state, &harness.spec).unwrap() as usize;
    slash_validator(&mut state, proposer, None, &mut context, &harness.spec).unwrap();
    assert!(state.get_validator(proposer).unwrap().slashed);
    assert_eq!(
        state
            .get_pending_balance_to_withdraw_for_builder(0)
            .unwrap(),
        0
    );
    apply_parent_execution_payload(
        &mut state,
        &ExecutionRequestsGloas::default(),
        &harness.spec,
    )
    .unwrap();
    assert!(state.builder_pending_withdrawals().unwrap().is_empty());
    process_withdrawals(&mut state, &harness.spec).unwrap();
    assert_eq!(state.get_builder(0).unwrap().balance, INITIAL);
    assert!(
        !state
            .payload_expected_withdrawals()
            .unwrap()
            .iter()
            .any(|w| w.address == Address::repeat_byte(0x33))
    );
}
