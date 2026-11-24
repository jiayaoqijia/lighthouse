use std::sync::Arc;

use crate::{
    BeaconChain, BeaconChainError, BeaconChainTypes, BeaconStore, CanonicalHead,
    beacon_proposer_cache::{BeaconProposerCache, with_proposer_cache},
    proposer_preferences_verification::{
        ProposerPreferencesError, proposer_preference_cache::GossipVerifiedProposerPreferenceCache,
    },
    validator_pubkey_cache::ValidatorPubkeyCache,
};
use eth2::types::{EventKind, ForkVersionedResponse};
use parking_lot::{Mutex, RwLock};
use slot_clock::SlotClock;
use state_processing::builder_deposits_cache::OnboardBuildersCache;
use tracing::debug;
use types::{ChainSpec, EthSpec, Hash256, ProposerPreferences, SignedProposerPreferences};

/// Verify that proposer preferences are consistent with the current chain state
pub(crate) fn verify_preferences_consistency<E: EthSpec>(
    preferences: &ProposerPreferences,
    slot_clock: &impl SlotClock,
    spec: &ChainSpec,
) -> Result<(), ProposerPreferencesError> {
    let proposal_slot = preferences.proposal_slot;
    let current_slot = slot_clock
        .now()
        .ok_or(ProposerPreferencesError::UnableToReadSlot)?;
    let now_ms = slot_clock
        .now_duration()
        .ok_or(ProposerPreferencesError::UnableToReadSlot)?
        .as_millis();
    let disparity_ms = spec.maximum_gossip_clock_disparity().as_millis();
    let proposal_epoch = proposal_slot.epoch(E::slots_per_epoch());

    if let Some(gloas_fork_epoch) = spec.gloas_fork_epoch
        && proposal_epoch < gloas_fork_epoch
    {
        return Err(ProposerPreferencesError::ProposalEpochPreGloas { proposal_epoch });
    }

    // Gloas is_past_slot and is_future_slot compare milliseconds inclusively.
    // Rounding to epochs/slots drops the valid clock-disparity boundary.
    let proposal_start_ms = slot_clock
        .start_of(proposal_slot)
        .ok_or(ProposerPreferencesError::UnableToReadSlot)?
        .as_millis();
    if now_ms > proposal_start_ms + disparity_ms {
        return Err(ProposerPreferencesError::ProposalSlotAlreadyPassed {
            proposal_slot,
            current_slot,
        });
    }
    let lookahead_start = proposal_epoch
        .saturating_sub(spec.min_seed_lookahead)
        .start_slot(E::slots_per_epoch());
    let lookahead_start_ms = slot_clock
        .start_of(lookahead_start)
        .ok_or(ProposerPreferencesError::UnableToReadSlot)?
        .as_millis();
    if now_ms + disparity_ms < lookahead_start_ms {
        return Err(ProposerPreferencesError::InvalidProposalEpoch { proposal_epoch });
    }

    Ok(())
}

pub struct GossipVerificationContext<'a, T: BeaconChainTypes> {
    pub canonical_head: &'a CanonicalHead<T>,
    pub gossip_verified_proposer_preferences_cache: &'a GossipVerifiedProposerPreferenceCache,
    pub slot_clock: &'a T::SlotClock,
    pub spec: &'a ChainSpec,
    pub store: &'a BeaconStore<T>,
    pub beacon_proposer_cache: &'a Mutex<BeaconProposerCache>,
    pub validator_pubkey_cache: &'a RwLock<ValidatorPubkeyCache<T>>,
    pub builder_onboarding_cache: Option<&'a OnboardBuildersCache>,
    pub genesis_validators_root: Hash256,
}

/// A wrapper around `SignedProposerPreferences` that has been verified for gossip propagation.
#[derive(Debug, Clone)]
pub struct GossipVerifiedProposerPreferences {
    pub signed_preferences: Arc<SignedProposerPreferences>,
}

impl GossipVerifiedProposerPreferences {
    pub fn new<T: BeaconChainTypes>(
        signed_preferences: Arc<SignedProposerPreferences>,
        ctx: &GossipVerificationContext<'_, T>,
    ) -> Result<Self, ProposerPreferencesError> {
        let proposal_slot = signed_preferences.message.proposal_slot;
        let dependent_root = signed_preferences.message.dependent_root;
        let validator_index = signed_preferences.message.validator_index;

        if ctx
            .gossip_verified_proposer_preferences_cache
            .get_seen_validator(&proposal_slot, dependent_root, validator_index)
        {
            return Err(ProposerPreferencesError::AlreadySeen {
                validator_index,
                proposal_slot,
            });
        }

        verify_preferences_consistency::<T::EthSpec>(
            &signed_preferences.message,
            ctx.slot_clock,
            ctx.spec,
        )?;

        let proposal_epoch = proposal_slot.epoch(T::EthSpec::slots_per_epoch());

        let lookahead_epoch = proposal_epoch.saturating_sub(ctx.spec.min_seed_lookahead);
        let head = ctx.canonical_head.cached_head();
        let head_block_root = head.head_block_root();
        let lookahead_start_slot = lookahead_epoch.start_slot(T::EthSpec::slots_per_epoch());
        let dependent_slot = lookahead_start_slot.saturating_sub(1_u64);
        let fork_choice_read = ctx.canonical_head.fork_choice_read_lock();
        let live_block = fork_choice_read.get_block(&dependent_root);
        let has_qualifying_child = fork_choice_read
            .get_children(&dependent_root)
            .iter()
            .any(|child| child.slot > dependent_slot);
        drop(fork_choice_read);

        // Fork choice intentionally hides blocks before finality. An upcoming
        // lookahead can still depend on such a canonical block, whose validated
        // block/state remain in hot or cold storage.
        let (dependent_block_slot, dependent_state_root) = if let Some(block) = live_block {
            (block.slot, block.state_root)
        } else {
            let block = ctx
                .store
                .get_blinded_block(&dependent_root)
                .map_err(BeaconChainError::DBError)?
                .ok_or(ProposerPreferencesError::DependentRootUnknown { dependent_root })?;
            (block.slot(), block.state_root())
        };
        if dependent_block_slot > dependent_slot {
            return Err(ProposerPreferencesError::DependentRootTooRecent {
                dependent_root,
                block_slot: dependent_block_slot,
                epoch_start_slot: lookahead_start_slot,
            });
        }
        let canonical_dependent = head
            .snapshot
            .beacon_state
            .get_block_root(dependent_slot)
            .is_ok_and(|root| *root == dependent_root);
        if !has_qualifying_child && dependent_root != head_block_root && !canonical_dependent {
            return Err(ProposerPreferencesError::InvalidDependentRoot { dependent_root });
        }

        // Resolve the proposer for `proposal_slot` via the proposer shuffling cache.
        let proposer = with_proposer_cache(
            ctx.beacon_proposer_cache,
            dependent_root,
            proposal_epoch,
            |proposers| proposers.get_slot::<T::EthSpec>(proposal_slot),
            || {
                debug!(
                    ?dependent_root,
                    %proposal_slot,
                    "Proposer shuffling cache miss for proposer preferences verification"
                );

                // Load a state at `target_slot` so it has the correct proposer lookahead.
                let target_slot = proposal_epoch
                    .saturating_sub(ctx.spec.min_seed_lookahead)
                    .start_slot(T::EthSpec::slots_per_epoch());

                let (state_root, state) = match ctx
                    .store
                    .get_advanced_hot_state(dependent_root, target_slot, dependent_state_root)
                    .map_err(BeaconChainError::DBError)?
                {
                    Some(state) => state,
                    None => {
                        let state = ctx
                            .store
                            .get_state(&dependent_state_root, Some(dependent_block_slot), true)
                            .map_err(BeaconChainError::DBError)?
                            .ok_or(ProposerPreferencesError::DependentRootUnknown {
                                dependent_root,
                            })?;
                        (dependent_state_root, state)
                    }
                };

                Ok::<_, ProposerPreferencesError>((state_root, state))
            },
            ctx.builder_onboarding_cache,
            ctx.spec,
        )?;

        if proposer.index as u64 != validator_index {
            return Err(ProposerPreferencesError::InvalidProposalSlot {
                validator_index,
                proposal_slot,
            });
        }

        // Verify the signature using the proposer's pubkey from the validator pubkey cache
        let signature_is_valid = {
            let pubkey_cache = ctx.validator_pubkey_cache.read();
            let pubkey = pubkey_cache
                .get(validator_index as usize)
                .ok_or(ProposerPreferencesError::BadSignature)?;
            signed_preferences.verify_signature::<T::EthSpec>(
                pubkey,
                &proposer.fork,
                ctx.genesis_validators_root,
                ctx.spec,
            )
        };
        if !signature_is_valid {
            return Err(ProposerPreferencesError::BadSignature);
        }

        let gossip_verified = GossipVerifiedProposerPreferences { signed_preferences };

        ctx.gossip_verified_proposer_preferences_cache
            .insert_seen_validator(&gossip_verified);

        ctx.gossip_verified_proposer_preferences_cache
            .insert_preferences(gossip_verified.clone());

        Ok(gossip_verified)
    }
}

impl<T: BeaconChainTypes> BeaconChain<T> {
    pub fn proposer_preferences_gossip_verification_context(
        &self,
    ) -> GossipVerificationContext<'_, T> {
        GossipVerificationContext {
            canonical_head: &self.canonical_head,
            gossip_verified_proposer_preferences_cache: &self
                .gossip_verified_proposer_preferences_cache,
            slot_clock: &self.slot_clock,
            spec: &self.spec,
            store: &self.store,
            beacon_proposer_cache: &self.beacon_proposer_cache,
            validator_pubkey_cache: &self.validator_pubkey_cache,
            builder_onboarding_cache: self.builder_onboarding_cache.as_deref(),
            genesis_validators_root: self.genesis_validators_root,
        }
    }

    pub fn verify_proposer_preferences_for_gossip(
        &self,
        signed_preferences: Arc<SignedProposerPreferences>,
    ) -> Result<GossipVerifiedProposerPreferences, ProposerPreferencesError> {
        let proposal_slot = signed_preferences.message.proposal_slot;
        let validator_index = signed_preferences.message.validator_index;

        let ctx = self.proposer_preferences_gossip_verification_context();
        match GossipVerifiedProposerPreferences::new(signed_preferences, &ctx) {
            Ok(verified) => {
                debug!(
                    %proposal_slot,
                    %validator_index,
                    "Successfully verified gossip proposer preferences"
                );

                if let Some(event_handler) = self.event_handler.as_ref()
                    && event_handler.has_proposer_preferences_subscribers()
                {
                    event_handler.register(EventKind::ProposerPreferences(Box::new(
                        ForkVersionedResponse {
                            version: self.spec.fork_name_at_slot::<T::EthSpec>(proposal_slot),
                            metadata: Default::default(),
                            data: (*verified.signed_preferences).clone(),
                        },
                    )));
                }

                Ok(verified)
            }
            Err(e) => {
                debug!(
                    error = e.to_string(),
                    %proposal_slot,
                    %validator_index,
                    "Rejected gossip proposer preferences"
                );
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use types::{Address, ChainSpec, EthSpec, Hash256, ProposerPreferences, Slot, Spec};

    use super::verify_preferences_consistency;
    use crate::proposer_preferences_verification::ProposerPreferencesError;
    use crate::test_utils::{fork_name_from_env, test_spec};
    use slot_clock::{SlotClock, TestingSlotClock};
    use std::time::Duration;

    type E = Spec;

    fn make_preferences(proposal_slot: Slot, validator_index: u64) -> ProposerPreferences {
        ProposerPreferences {
            dependent_root: Hash256::ZERO,
            proposal_slot,
            validator_index,
            fee_recipient: Address::ZERO,
            target_gas_limit: 30_000_000,
        }
    }

    fn spec() -> ChainSpec {
        test_spec::<E>()
    }

    fn clock_at(slot: Slot) -> TestingSlotClock {
        let clock = TestingSlotClock::new(Slot::new(0), Duration::ZERO, spec().get_slot_duration());
        clock.set_slot(slot.as_u64());
        clock
    }

    #[test]
    fn test_invalid_epoch_too_old() {
        if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
            return;
        }
        let current_slot = Slot::new(2 * E::slots_per_epoch());
        let prefs = make_preferences(Slot::new(3), 0);

        let result = verify_preferences_consistency::<E>(&prefs, &clock_at(current_slot), &spec());
        assert!(matches!(
            result,
            Err(ProposerPreferencesError::ProposalSlotAlreadyPassed { .. })
        ));
    }

    #[test]
    fn test_invalid_epoch_too_far_ahead() {
        if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
            return;
        }
        let current_slot = Slot::new(E::slots_per_epoch());
        let prefs = make_preferences(Slot::new(3 * E::slots_per_epoch() + 1), 0);

        let result = verify_preferences_consistency::<E>(&prefs, &clock_at(current_slot), &spec());
        assert!(matches!(
            result,
            Err(ProposerPreferencesError::InvalidProposalEpoch { .. })
        ));
    }

    #[test]
    fn test_proposal_slot_already_passed() {
        if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
            return;
        }
        let current_slot = Slot::new(10);
        let prefs = make_preferences(Slot::new(9), 0);

        let result = verify_preferences_consistency::<E>(&prefs, &clock_at(current_slot), &spec());
        assert!(matches!(
            result,
            Err(ProposerPreferencesError::ProposalSlotAlreadyPassed { .. })
        ));
    }

    #[test]
    fn test_proposal_slot_equal_to_current() {
        if !fork_name_from_env().is_some_and(|f| f.gloas_enabled()) {
            return;
        }
        let current_slot = Slot::new(10);
        let prefs = make_preferences(Slot::new(10), 0);

        let result = verify_preferences_consistency::<E>(&prefs, &clock_at(current_slot), &spec());
        assert!(result.is_ok(), "{result:?}");
    }
    #[test]
    fn inclusive_millisecond_boundaries() {
        fn check<E: EthSpec>() {
            let mut spec = E::default_spec();
            spec.gloas_fork_epoch = Some(types::Epoch::new(0));
            let clock = TestingSlotClock::new(
                Slot::new(0),
                Duration::from_secs(100),
                spec.get_slot_duration(),
            );
            let proposal = Slot::new(3 * E::slots_per_epoch());
            let preferences = make_preferences(proposal, 0);
            let start = clock.start_of(proposal).unwrap();
            let disparity = spec.maximum_gossip_clock_disparity();
            clock.set_current_time(start + disparity);
            assert!(verify_preferences_consistency::<E>(&preferences, &clock, &spec).is_ok());
            clock.set_current_time(start + disparity + Duration::from_millis(1));
            assert!(matches!(
                verify_preferences_consistency::<E>(&preferences, &clock, &spec),
                Err(ProposerPreferencesError::ProposalSlotAlreadyPassed { .. })
            ));
            let lookahead = proposal
                .epoch(E::slots_per_epoch())
                .saturating_sub(spec.min_seed_lookahead)
                .start_slot(E::slots_per_epoch());
            let start = clock.start_of(lookahead).unwrap();
            clock.set_current_time(start - disparity);
            assert!(verify_preferences_consistency::<E>(&preferences, &clock, &spec).is_ok());
            clock.set_current_time(start - disparity - Duration::from_millis(1));
            assert!(matches!(
                verify_preferences_consistency::<E>(&preferences, &clock, &spec),
                Err(ProposerPreferencesError::InvalidProposalEpoch { .. })
            ));
        }
        check::<types::MinimalEthSpec>();
        check::<types::MainnetEthSpec>();
    }
}
