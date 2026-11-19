use crate::cli::ImportGloasBootstrap;
use beacon_node::ClientConfig;
use environment::RuntimeContext;
use serde::Deserialize;
use store::{HotColdDB, ItemStore, database::interface::BeaconNodeBackend, errors::Error};
use tree_hash::TreeHash;
use types::{
    ChainSpec, EthSpec, ForkName, Hash256, LightClientBootstrapGloas, LightClientHeaderGloas,
    light_client::consts::CURRENT_SYNC_COMMITTEE_INDEX_GLOAS,
};

#[derive(Deserialize)]
#[serde(bound = "E: EthSpec")]
struct BootstrapFile<E: EthSpec> {
    version: ForkName,
    data: LightClientBootstrapGloas<E>,
}

// The exact local block authenticates the header; its state root authenticates
// the committee proof. No state-root or slot-based state substitution is needed.
fn import_verified<E: EthSpec, Hot: ItemStore, Cold: ItemStore>(
    database: &HotColdDB<E, Hot, Cold>,
    block_root: Hash256,
    bootstrap: &LightClientBootstrapGloas<E>,
    spec: &ChainSpec,
) -> Result<bool, String> {
    if bootstrap.header.beacon.canonical_root() != block_root {
        return Err("bootstrap header does not match the requested block root".into());
    }
    let block = database
        .get_blinded_block(&block_root)
        .map_err(|error| format!("unable to read local block: {error:?}"))?
        .ok_or("requested block is not retained locally")?;
    if block.message().tree_hash_root() != block_root
        || block
            .fork_name(spec)
            .map_err(|error| format!("local block fork: {error:?}"))?
            != ForkName::Gloas
    {
        return Err("local block commitment or Gloas fork context is invalid".into());
    }
    let expected_header = LightClientHeaderGloas::block_to_light_client_header(&block)
        .map_err(|error| format!("local light-client header: {error:?}"))?;
    if bootstrap.header != expected_header {
        return Err("bootstrap header/execution proof differs from the local block".into());
    }
    let period = block
        .slot()
        .epoch(E::slots_per_epoch())
        .sync_committee_period(spec)
        .map_err(|error| format!("sync committee period: {error:?}"))?;
    let committee = database.get_sync_committee(period)
        .map_err(|error| format!("local committee cache: {error:?}"))?
        .ok_or("local period committee is unavailable; restore that authenticated state/committee first")?;
    if committee.tree_hash_root() != bootstrap.current_sync_committee.tree_hash_root() {
        return Err(
            "bootstrap committee conflicts with the locally stored period committee".into(),
        );
    }
    let index = CURRENT_SYNC_COMMITTEE_INDEX_GLOAS;
    let depth = index.ilog2() as usize;
    if !merkle_proof::verify_merkle_proof(
        bootstrap.current_sync_committee.tree_hash_root(),
        &bootstrap.current_sync_committee_branch,
        depth,
        index - (1 << depth),
        block.state_root(),
    ) {
        return Err("bootstrap committee proof does not match the local state commitment".into());
    }
    let branch = bootstrap.current_sync_committee_branch.to_vec();
    if database
        .get_sync_committee_branch(&block_root)
        .map_err(|error| format!("existing proof cache: {error:?}"))?
        .as_ref()
        == Some(&branch)
    {
        return Ok(false);
    }
    database
        .store_sync_committee_branch(block_root, &branch)
        .map_err(|error| format!("unable to store authenticated proof: {error:?}"))?;
    Ok(true)
}

pub(super) fn import_file<E: EthSpec>(
    import: &ImportGloasBootstrap,
    config: ClientConfig,
    context: &RuntimeContext<E>,
) -> Result<(), String> {
    let metadata =
        std::fs::metadata(&import.file).map_err(|error| format!("bootstrap file: {error}"))?;
    if metadata.len() > 1024 * 1024 {
        return Err("bootstrap JSON exceeds the 1 MiB import limit".into());
    }
    let bytes = std::fs::read(&import.file).map_err(|error| format!("bootstrap file: {error}"))?;
    let input: BootstrapFile<E> =
        serde_json::from_slice(&bytes).map_err(|error| format!("bootstrap JSON: {error}"))?;
    if input.version != ForkName::Gloas {
        return Err("this import requires a Gloas bootstrap".into());
    }
    let hot = config.get_db_path();
    if !hot.is_dir() {
        return Err("the existing beacon database directory is unavailable".into());
    }
    let database = HotColdDB::<E, BeaconNodeBackend, BeaconNodeBackend>::open(
        &hot,
        &config.get_freezer_db_path(),
        &config.get_blobs_db_path(),
        |_, from, to| {
            if from == to {
                Ok(())
            } else {
                Err(Error::MigrationError(
                    "bootstrap import requires the current schema; migrate separately first".into(),
                ))
            }
        },
        config.store,
        context.eth2_config.spec.clone(),
    )
    .map_err(|error| format!("unable to exclusively open beacon database: {error:?}"))?;
    let changed = import_verified(
        &database,
        import.block_root,
        &input.data,
        &context.eth2_config.spec,
    )?;
    tracing::info!(block_root = ?import.block_root, changed, "Imported authenticated Gloas bootstrap proof cache");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use beacon_chain::test_utils::{
        AttestationStrategy, BeaconChainHarness, BlockStrategy, EphemeralHarnessType,
    };
    use std::sync::Arc;
    use types::{BeaconState, LightClientBootstrap, MinimalEthSpec};

    #[tokio::test]
    async fn verified_import_rejects_unbound_data_and_repairs_only_the_proof_cache() {
        type E = MinimalEthSpec;
        let spec = ForkName::Gloas.make_genesis_spec(E::default_spec());
        let harness = BeaconChainHarness::<EphemeralHarnessType<E>>::builder(E::default())
            .spec(spec.clone().into())
            .deterministic_keypairs(64)
            .fresh_ephemeral_store()
            .mock_execution_layer()
            .build();
        harness.advance_slot();
        let block_root = harness
            .extend_chain(
                2,
                BlockStrategy::OnCanonicalHead,
                AttestationStrategy::AllValidators,
            )
            .await;
        let database = &harness.chain.store;
        let block = database.get_blinded_block(&block_root).unwrap().unwrap();
        let mut state: BeaconState<E> = database
            .get_state(&block.state_root(), Some(block.slot()), false)
            .unwrap()
            .unwrap();
        let LightClientBootstrap::Gloas(bootstrap) =
            LightClientBootstrap::from_beacon_state(&mut state, &block, &spec).unwrap()
        else {
            panic!("expected explicit Gloas bootstrap")
        };
        let period = block
            .slot()
            .epoch(E::slots_per_epoch())
            .sync_committee_period(&spec)
            .unwrap();
        assert!(import_verified(database, block_root, &bootstrap, &spec).is_err());
        assert!(
            database
                .get_sync_committee_branch(&block_root)
                .unwrap()
                .is_none()
        );
        database
            .store_sync_committee(period, &bootstrap.current_sync_committee)
            .unwrap();
        let mask = Hash256::from([1u8; 32]);
        assert!(import_verified(database, block_root ^ mask, &bootstrap, &spec).is_err());
        let mut unknown_header = bootstrap.clone();
        unknown_header.header.beacon.proposer_index += 1;
        let unknown_root = unknown_header.header.beacon.canonical_root();
        assert!(import_verified(database, unknown_root, &unknown_header, &spec).is_err());
        assert!(
            database
                .get_sync_committee_branch(&unknown_root)
                .unwrap()
                .is_none()
        );

        let mut bad_header = bootstrap.clone();
        bad_header.header.execution_branch[0] ^= mask;
        assert!(import_verified(database, block_root, &bad_header, &spec).is_err());
        let mut bad_committee = bootstrap.clone();
        let committee = Arc::make_mut(&mut bad_committee.current_sync_committee);
        let other = committee
            .pubkeys
            .iter()
            .position(|key| key != &committee.pubkeys[0])
            .unwrap();
        committee.pubkeys.swap(0, other);
        assert!(import_verified(database, block_root, &bad_committee, &spec).is_err());
        let mut bad_proof = bootstrap.clone();
        bad_proof.current_sync_committee_branch[0] ^= mask;
        assert!(import_verified(database, block_root, &bad_proof, &spec).is_err());
        assert!(
            database
                .get_sync_committee_branch(&block_root)
                .unwrap()
                .is_none()
        );
        assert!(import_verified(database, block_root, &bootstrap, &spec).unwrap());
        assert!(!import_verified(database, block_root, &bootstrap, &spec).unwrap());
        assert_eq!(
            database
                .get_sync_committee_branch(&block_root)
                .unwrap()
                .unwrap(),
            bootstrap.current_sync_committee_branch.to_vec()
        );
        assert_eq!(state.canonical_root().unwrap(), block.state_root());
    }
}
