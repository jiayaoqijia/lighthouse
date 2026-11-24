//! Replay independent full-node light-client collection vectors through the native store/cache.
use super::common::{load_config, testing_spec_with_config};
use super::*;
use crate::decode::{ssz_decode_file_with, ssz_decode_state, yaml_decode_file};
use beacon_chain::block_verification_types::LookupBlock;
use beacon_chain::custody_context::NodeCustodyType;
use beacon_chain::test_utils::{BeaconChainHarness, EphemeralHarnessType};
use beacon_chain::{ChainConfig, NotifyExecutionLayer};
use serde::Deserialize;
use ssz::Encode;
use std::sync::Arc;
use types::{
    BeaconState, BlockImportSource, Checkpoint, EthSpec, ForkName, ForkVersionDecode, Hash256,
    SignedBeaconBlock,
};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Object {
    fork_digest: String,
    data: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bootstrap {
    block_root: Hash256,
    bootstrap: Option<Object>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct BestUpdate {
    period: u64,
    update: Option<Object>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checks {
    latest_finalized_checkpoint: Checkpoint,
    bootstraps: Vec<Bootstrap>,
    best_updates: Vec<BestUpdate>,
    latest_finality_update: Option<Object>,
    latest_optimistic_update: Option<Object>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Head {
    head_block_root: Hash256,
    checks: Checks,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Step {
    Block { new_block: Object },
    Head { new_head: Head },
}
#[derive(Debug, Clone)]
pub struct LightClientDataCollection<E: EthSpec> {
    path: PathBuf,
    config: Option<types::Config>,
    state: BeaconState<E>,
    steps: Vec<Step>,
}
impl<E: EthSpec> LoadCase for LightClientDataCollection<E> {
    fn load_from_dir(path: &Path, fork: ForkName) -> Result<Self, Error> {
        let config = load_config(path)?;
        let spec = testing_spec_with_config::<E>(fork, config.as_ref())?;
        Ok(Self {
            path: path.to_owned(),
            state: ssz_decode_state(&path.join("initial_state.ssz_snappy"), &spec)?,
            steps: yaml_decode_file(&path.join("steps.yaml"))?,
            config,
        })
    }
}
fn internal(error: impl std::fmt::Debug) -> Error {
    Error::InternalError(format!("native light-client collection: {error:?}"))
}
impl<E: EthSpec> LightClientDataCollection<E> {
    fn compare<T: Encode>(
        &self,
        actual: Option<T>,
        expected: Option<&Object>,
        label: &str,
        context_slot: types::Slot,
    ) -> Result<(), Error> {
        match (actual, expected) {
            (None, None) => Ok(()),
            (Some(actual), Some(expected)) => {
                let spec = testing_spec_with_config::<E>(ForkName::Gloas, self.config.as_ref())?;
                let digest = spec.compute_fork_digest(
                    self.state.genesis_validators_root(),
                    context_slot.epoch(E::slots_per_epoch()),
                );
                if expected.fork_digest != format!("0x{}", hex::encode(digest)) {
                    return Err(Error::FailedToParseTest(
                        "light-client object context fork digest differs from configured source"
                            .into(),
                    ));
                }
                let bytes: Vec<u8> = ssz_decode_file_with(
                    &self.path.join(format!("{}.ssz_snappy", expected.data)),
                    |bytes| Ok::<_, ssz::DecodeError>(bytes.to_vec()),
                )?;
                if actual.as_ssz_bytes() == bytes {
                    Ok(())
                } else {
                    Err(Error::NotEqual(format!(
                        "{label}: native SSZ differs from unchanged independent {}",
                        expected.data
                    )))
                }
            }
            (actual, expected) => Err(Error::NotEqual(format!(
                "{label}: native present={}, reference present={}",
                actual.is_some(),
                expected.is_some()
            ))),
        }
    }
}
impl<E: EthSpec> Case for LightClientDataCollection<E> {
    fn description(&self) -> String {
        self.path.display().to_string()
    }
    fn result(&self, _: usize, fork: ForkName) -> Result<(), Error> {
        let spec = Arc::new(testing_spec_with_config::<E>(fork, self.config.as_ref())?);
        let harness = BeaconChainHarness::<EphemeralHarnessType<E>>::builder(E::default())
            .spec(spec.clone())
            .keypairs(vec![])
            .chain_config(ChainConfig {
                archive: true,
                ..Default::default()
            })
            .genesis_state_ephemeral_store(self.state.clone())
            .mock_execution_layer()
            .mock_execution_layer_all_payloads_valid()
            .recalculate_fork_times_with_genesis(self.state.genesis_time())
            .node_custody_type(NodeCustodyType::Supernode)
            .build();
        for step in &self.steps {
            match step {
                Step::Block { new_block } => {
                    let block = ssz_decode_file_with(
                        &self.path.join(format!("{}.ssz_snappy", new_block.data)),
                        |bytes| SignedBeaconBlock::<E>::from_ssz_bytes_by_fork(bytes, fork),
                    )?;
                    if new_block.fork_digest
                        != format!(
                            "0x{}",
                            hex::encode(spec.compute_fork_digest(
                                self.state.genesis_validators_root(),
                                block.epoch()
                            ))
                        )
                    {
                        return Err(Error::FailedToParseTest(
                            "new block context fork digest mismatch".into(),
                        ));
                    }
                    harness.set_current_slot(block.slot());
                    let root = block.canonical_root();
                    let result = harness
                        .chain
                        .task_executor
                        .clone()
                        .block_on_dangerous(
                            harness.chain.process_block(
                                root,
                                LookupBlock::new(root, Arc::new(block)),
                                NotifyExecutionLayer::Yes,
                                BlockImportSource::Lookup,
                                || Ok(()),
                            ),
                            "current_light_client_collection",
                        )
                        .ok_or_else(|| internal("runtime shutdown"))?;
                    result.map_err(internal)?;
                }
                Step::Head { new_head } => {
                    let block = harness
                        .chain
                        .store
                        .get_blinded_block(&new_head.head_block_root)
                        .map_err(internal)?
                        .ok_or_else(|| internal("selected head block missing"))?;
                    // This fixture explicitly selects a stored canonical branch.
                    // Use the production projection with that head; native
                    // fork-choice selection/notification is separately covered
                    // by real signed-block reorg regressions in beacon_chain.
                    harness
                        .chain
                        .light_client_server_cache
                        .recompute_for_head(
                            harness.chain.store.clone(),
                            new_head.head_block_root,
                            &spec,
                        )
                        .map_err(internal)?;
                    let state = harness
                        .chain
                        .store
                        .get_state(&block.state_root(), Some(block.slot()), true)
                        .map_err(internal)?
                        .ok_or_else(|| internal("selected head state missing"))?;
                    if state.finalized_checkpoint() != new_head.checks.latest_finalized_checkpoint {
                        return Err(Error::NotEqual(
                            "selected native state finalized checkpoint differs".into(),
                        ));
                    }
                    let period = state
                        .finalized_checkpoint()
                        .epoch
                        .sync_committee_period(&spec)
                        .map_err(internal)?;
                    for expected in &new_head.checks.bootstraps {
                        let actual = harness
                            .chain
                            .light_client_server_cache
                            .get_light_client_bootstrap(
                                &harness.chain.store,
                                &expected.block_root,
                                period,
                                &spec,
                            )
                            .map_err(internal)?
                            .map(|(bootstrap, _)| bootstrap);
                        let slot = actual
                            .as_ref()
                            .map(|value| value.get_slot())
                            .unwrap_or_default();
                        self.compare(actual, expected.bootstrap.as_ref(), "bootstrap", slot)?;
                    }
                    for expected in &new_head.checks.best_updates {
                        let mut actual = harness
                            .chain
                            .get_light_client_updates(expected.period, 1)
                            .map_err(internal)?;
                        if actual.len() > 1 {
                            return Err(internal("too many period updates"));
                        }
                        let actual = actual.pop();
                        let slot = actual
                            .as_ref()
                            .map(|value| value.attested_header_slot())
                            .unwrap_or_default();
                        self.compare(actual, expected.update.as_ref(), "best update", slot)?;
                    }
                    let finality = harness
                        .chain
                        .light_client_server_cache
                        .get_latest_finality_update();
                    let slot = finality
                        .as_ref()
                        .map(|value| value.get_attested_header_slot())
                        .unwrap_or_default();
                    self.compare(
                        finality,
                        new_head.checks.latest_finality_update.as_ref(),
                        "latest finality",
                        slot,
                    )?;
                    let optimistic = harness
                        .chain
                        .light_client_server_cache
                        .get_latest_optimistic_update();
                    let slot = optimistic
                        .as_ref()
                        .map(|value| value.get_slot())
                        .unwrap_or_default();
                    self.compare(
                        optimistic,
                        new_head.checks.latest_optimistic_update.as_ref(),
                        "latest optimistic",
                        slot,
                    )?;
                }
            }
        }
        Ok(())
    }
}
