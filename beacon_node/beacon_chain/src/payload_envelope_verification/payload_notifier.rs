use std::sync::Arc;

use execution_layer::{NewPayloadRequest, NewPayloadRequestGloas, NewPayloadRequestHeze};
use fork_choice::PayloadVerificationStatus;
use state_processing::per_block_processing::deneb::kzg_commitment_to_versioned_hash;
use tracing::warn;
use types::{ProgressiveTransactions, SignedBeaconBlock, SignedExecutionPayloadEnvelope};

use crate::{
    BeaconChain, BeaconChainError, BeaconChainTypes, NotifyExecutionLayer,
    PayloadVerificationError, execution_payload::notify_new_payload,
    payload_envelope_verification::EnvelopeError,
};

/// Used to await the result of executing payload with a remote EE.
pub struct PayloadNotifier<T: BeaconChainTypes> {
    pub chain: Arc<BeaconChain<T>>,
    envelope: Arc<SignedExecutionPayloadEnvelope<T::EthSpec>>,
    block: Arc<SignedBeaconBlock<T::EthSpec>>,
    payload_verification_status: Option<PayloadVerificationStatus>,
}

impl<T: BeaconChainTypes> PayloadNotifier<T> {
    pub fn new(
        chain: Arc<BeaconChain<T>>,
        envelope: Arc<SignedExecutionPayloadEnvelope<T::EthSpec>>,
        block: Arc<SignedBeaconBlock<T::EthSpec>>,
        notify_execution_layer: NotifyExecutionLayer,
    ) -> Result<Self, EnvelopeError> {
        let payload_verification_status = {
            let payload_message = &envelope.message;

            match notify_execution_layer {
                NotifyExecutionLayer::No if chain.config.optimistic_finalized_sync => {
                    let new_payload_request = Self::build_new_payload_request(&envelope, &block)?;
                    // TODO(gloas): check and test RLP block hash calculation post-Gloas
                    if let Err(e) = new_payload_request
                        .perform_optimistic_sync_verifications(block.fork_name_unchecked())
                    {
                        warn!(
                            block_number = ?payload_message.payload.block_number,
                            info = "you can silence this warning with --disable-optimistic-finalized-sync",
                            error = ?e,
                            "Falling back to slow block hash verification"
                        );
                        None
                    } else {
                        Some(PayloadVerificationStatus::Optimistic)
                    }
                }
                _ => None,
            }
        };

        Ok(Self {
            chain,
            envelope,
            block,
            payload_verification_status,
        })
    }

    pub async fn notify_new_payload(
        self,
    ) -> Result<PayloadVerificationStatus, PayloadVerificationError> {
        if let Some(precomputed_status) = self.payload_verification_status {
            Ok(precomputed_status)
        } else {
            let request = Self::build_new_payload_request(&self.envelope, &self.block)?;
            let request = if self.block.fork_name_unchecked().heze_enabled() {
                let NewPayloadRequest::Gloas(base) = request else {
                    return Err(BeaconChainError::BeaconStateError(
                        types::BeaconStateError::IncorrectStateVariant,
                    )
                    .into());
                };
                let chain = self.chain.clone();
                let parent_root = self.block.parent_root();
                let list_slot = self.block.slot().saturating_sub(1u64);
                // Heze get_forkchoice_store initializes the trusted anchor as IL-satisfied.
                // Its parent predates the checkpoint store and may never be present in fork
                // choice. Keep live payload lookups strict; all other envelope and EL checks
                // still run for the anchor.
                let is_trusted_anchor = self.block.slot()
                    <= self.chain.store.get_anchor_info().anchor_slot
                    && self.envelope.message.beacon_block_root
                        == self
                            .chain
                            .canonical_head
                            .cached_head()
                            .finalized_checkpoint()
                            .root;
                let list = if is_trusted_anchor {
                    Vec::new()
                } else {
                    self.chain
                        .spawn_blocking_handle(
                            move || {
                                chain.get_inclusion_list_transactions(parent_root, list_slot, true)
                            },
                            "heze_envelope_inclusion_list_transactions",
                        )
                        .await??
                };
                NewPayloadRequest::Heze(NewPayloadRequestHeze {
                    execution_payload: base.execution_payload,
                    versioned_hashes: base.versioned_hashes,
                    parent_beacon_block_root: base.parent_beacon_block_root,
                    execution_requests: base.execution_requests,
                    inclusion_list_transactions: ProgressiveTransactions::new(list)
                        .map_err(BeaconChainError::from)?,
                })
            } else {
                request
            };
            notify_new_payload(&self.chain, self.envelope.slot(), request).await
        }
    }

    fn build_new_payload_request<'a>(
        envelope: &'a SignedExecutionPayloadEnvelope<T::EthSpec>,
        block: &'a SignedBeaconBlock<T::EthSpec>,
    ) -> Result<NewPayloadRequest<'a, T::EthSpec>, PayloadVerificationError> {
        let bid = block
            .message()
            .body()
            .signed_execution_payload_bid()
            .map_err(|e| PayloadVerificationError::BeaconChainError(Box::new(e.into())))?
            .message();

        let versioned_hashes = bid
            .blob_kzg_commitments()
            .iter()
            .map(kzg_commitment_to_versioned_hash)
            .collect();

        Ok(NewPayloadRequest::Gloas(NewPayloadRequestGloas {
            execution_payload: &envelope.message.payload,
            versioned_hashes,
            parent_beacon_block_root: envelope.message.parent_beacon_block_root,
            execution_requests: &envelope.message.execution_requests,
        }))
    }
}
