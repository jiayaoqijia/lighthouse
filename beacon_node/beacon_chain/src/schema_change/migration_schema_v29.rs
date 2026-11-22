use crate::beacon_chain::{BeaconChainTypes, FORK_CHOICE_DB_KEY};
use crate::persisted_fork_choice::{
    PersistedForkChoice, PersistedForkChoiceV28, PersistedForkChoiceV29,
};
use std::collections::HashMap;
use store::hot_cold_store::HotColdDB;
use store::{DBColumn, Error as StoreError, KeyValueStore, KeyValueStoreOp};
use tracing::warn;
use types::EthSpec;

/// Upgrade from schema v28 to v29.
///
/// - Clears `best_child` and `best_descendant` on all nodes (replaced by
///   virtual tree walk).
/// - Fails if the persisted fork choice contains any V17 (pre-Gloas) proto
///   nodes at or after the Gloas fork slot.
///
/// Returns a list of store ops to be applied atomically with the schema version write.
pub fn upgrade_to_v29<T: BeaconChainTypes>(
    db: &HotColdDB<T::EthSpec, T::HotStore, T::ColdStore>,
) -> Result<Vec<KeyValueStoreOp>, StoreError> {
    let gloas_fork_slot = db
        .spec
        .gloas_fork_epoch
        .map(|epoch| epoch.start_slot(T::EthSpec::slots_per_epoch()));

    // Load the persisted fork choice (v28 format).
    let Some(fc_bytes) = db
        .hot_db
        .get_bytes(DBColumn::ForkChoice, FORK_CHOICE_DB_KEY.as_slice())?
    else {
        return Ok(vec![]);
    };

    let persisted_v28 = PersistedForkChoiceV28::from_bytes(&fc_bytes, db.get_config())?;

    // Check for V17 nodes at/after the Gloas fork slot.
    if let Some(gloas_fork_slot) = gloas_fork_slot {
        let bad_node = persisted_v28
            .fork_choice_v28
            .proto_array_v28
            .nodes
            .iter()
            .find(|node| node.slot >= gloas_fork_slot);

        if let Some(node) = bad_node {
            return Err(StoreError::MigrationError(format!(
                "cannot upgrade from v28 to v29: found V17 proto node at slot {} (root: {:?}) \
                 which is at or after the Gloas fork slot {}. This node has synced a chain with \
                 Gloas disabled and cannot be upgraded. Please resync from scratch.",
                node.slot, node.root, gloas_fork_slot,
            )));
        }
    }

    // Read the previous proposer boost before converting to V29 (V29 no longer stores it).
    let previous_proposer_boost = persisted_v28
        .fork_choice_v28
        .proto_array_v28
        .previous_proposer_boost;

    // Convert to v29.
    let mut persisted_v29 = PersistedForkChoiceV29::from(persisted_v28);

    // Subtract the proposer boost from the boosted node and all its ancestors.
    //
    // In the V28 schema, `apply_score_changes` baked the proposer boost directly into node
    // weights and back-propagated it up the parent chain. In V29, the boost is computed
    // on-the-fly during the virtual tree walk. If we don't subtract the baked-in boost here,
    // it will be double-counted after the upgrade.
    if !previous_proposer_boost.root.is_zero() && previous_proposer_boost.score > 0 {
        let score = previous_proposer_boost.score;
        let indices: HashMap<_, _> = persisted_v29
            .fork_choice
            .proto_array
            .indices
            .iter()
            .cloned()
            .collect();

        if let Some(node_index) = indices.get(&previous_proposer_boost.root).copied() {
            let nodes = &mut persisted_v29.fork_choice.proto_array.nodes;
            let mut current = Some(node_index);
            while let Some(idx) = current {
                if let Some(node) = nodes.get_mut(idx) {
                    *node.weight_mut() = node.weight().saturating_sub(score);
                    current = node.parent();
                } else {
                    break;
                }
            }
        } else {
            warn!(
                root = ?previous_proposer_boost.root,
                "Proposer boost node missing from fork choice"
            );
        }
    }

    Ok(vec![
        persisted_v29.as_kv_store_op(FORK_CHOICE_DB_KEY, db.get_config())?,
    ])
}

/// Downgrade from schema v29 to v28.
///
/// Converts either current or legacy schema-v29 snapshots back to V28.
/// Refuses to discard outstanding inclusion-list obligations.
/// Fails if the persisted fork choice contains any V29 proto nodes, as these contain
/// payload-specific fields that cannot be losslessly converted back to V17 format.
///
/// Returns a list of store ops to be applied atomically with the schema version write.
pub fn downgrade_from_v29<T: BeaconChainTypes>(
    db: &HotColdDB<T::EthSpec, T::HotStore, T::ColdStore>,
) -> Result<Vec<KeyValueStoreOp>, StoreError> {
    // Schema v29 can contain the current snapshot or its legacy V29 encoding.
    let Some(fc_bytes) = db
        .hot_db
        .get_bytes(DBColumn::ForkChoice, FORK_CHOICE_DB_KEY.as_slice())?
    else {
        return Ok(vec![]);
    };

    let persisted = PersistedForkChoice::from_bytes(&fc_bytes, db.get_config()).map_err(|e| {
        StoreError::MigrationError(format!(
            "cannot downgrade from v29 to v28: failed to decode fork choice: {:?}",
            e
        ))
    })?;

    // V28 cannot retain these obligations. Refuse the downgrade before emitting any writes.
    if !persisted.unsatisfied_inclusion_lists.is_empty() {
        return Err(StoreError::MigrationError(
            "cannot downgrade from v29 to v28: unsatisfied inclusion lists would be lost"
                .to_string(),
        ));
    }
    let persisted_v29 = PersistedForkChoiceV29 {
        fork_choice: persisted.fork_choice,
        fork_choice_store: persisted.fork_choice_store,
    };

    let has_v29_node = persisted_v29
        .fork_choice
        .proto_array
        .nodes
        .iter()
        .any(|node| matches!(node, proto_array::core::ProtoNode::V29(_)));

    if has_v29_node {
        return Err(StoreError::MigrationError(
            "cannot downgrade from v29 to v28: the persisted fork choice contains V29 proto \
             nodes which cannot be losslessly converted to V17 format. The Gloas-specific \
             payload data would be lost."
                .to_string(),
        ));
    }

    // Convert to v28 and encode.
    let persisted_v28 = PersistedForkChoiceV28::from(persisted_v29);

    Ok(vec![
        persisted_v28.as_kv_store_op(FORK_CHOICE_DB_KEY, db.get_config())?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{BeaconChainHarness, EphemeralHarnessType};
    use store::metadata::{SCHEMA_VERSION_KEY, SchemaVersion};
    use types::{Hash256, MinimalEthSpec};

    #[tokio::test]
    async fn pending_or_malformed_snapshot_downgrade_preserves_disk_keys() {
        let harness =
            BeaconChainHarness::<EphemeralHarnessType<MinimalEthSpec>>::builder(MinimalEthSpec)
                .default_spec()
                .deterministic_keypairs(32)
                .fresh_ephemeral_store()
                .mock_execution_layer()
                .build();
        harness.chain.persist_fork_choice().unwrap();
        let db = harness.chain.store.clone();
        let key = FORK_CHOICE_DB_KEY.as_slice();
        let original = db
            .hot_db
            .get_bytes(DBColumn::ForkChoice, key)
            .unwrap()
            .unwrap();
        let mut snapshot = PersistedForkChoice::from_bytes(&original, db.get_config()).unwrap();
        snapshot
            .unsatisfied_inclusion_lists
            .push(Hash256::repeat_byte(0x71));
        db.store_schema_version_atomically(
            SchemaVersion(29),
            vec![
                snapshot
                    .as_kv_store_op(FORK_CHOICE_DB_KEY, db.get_config())
                    .unwrap(),
            ],
        )
        .unwrap();
        let version = db
            .hot_db
            .get_bytes(DBColumn::BeaconMeta, SCHEMA_VERSION_KEY.as_slice())
            .unwrap();
        let pending = db.hot_db.get_bytes(DBColumn::ForkChoice, key).unwrap();
        let error = crate::schema_change::migrate_schema::<EphemeralHarnessType<MinimalEthSpec>>(
            db.clone(),
            SchemaVersion(29),
            SchemaVersion(28),
        )
        .expect_err("must not drop obligations");
        assert!(format!("{error:?}").contains("unsatisfied inclusion lists"));
        assert_eq!(
            db.hot_db.get_bytes(DBColumn::ForkChoice, key).unwrap(),
            pending
        );
        assert_eq!(
            db.hot_db
                .get_bytes(DBColumn::BeaconMeta, SCHEMA_VERSION_KEY.as_slice())
                .unwrap(),
            version
        );

        let malformed = db.get_config().compress_bytes(&[1, 2, 3]).unwrap();
        db.hot_db
            .put_bytes(DBColumn::ForkChoice, key, &malformed)
            .unwrap();
        crate::schema_change::migrate_schema::<EphemeralHarnessType<MinimalEthSpec>>(
            db.clone(),
            SchemaVersion(29),
            SchemaVersion(28),
        )
        .expect_err("must reject malformed snapshot");
        assert_eq!(
            db.hot_db.get_bytes(DBColumn::ForkChoice, key).unwrap(),
            Some(malformed)
        );
        assert_eq!(
            db.hot_db
                .get_bytes(DBColumn::BeaconMeta, SCHEMA_VERSION_KEY.as_slice())
                .unwrap(),
            version
        );
    }
    #[tokio::test]
    async fn gloas_proto_node_downgrade_preserves_snapshot_and_version() {
        use types::{EthSpec, ForkName};
        let harness =
            BeaconChainHarness::<EphemeralHarnessType<MinimalEthSpec>>::builder(MinimalEthSpec)
                .spec(
                    ForkName::Gloas
                        .make_genesis_spec(MinimalEthSpec::default_spec())
                        .into(),
                )
                .deterministic_keypairs(32)
                .fresh_ephemeral_store()
                .mock_execution_layer()
                .build();
        harness.chain.persist_fork_choice().unwrap();
        let db = harness.chain.store.clone();
        db.store_schema_version(SchemaVersion(29)).unwrap();
        let original = db
            .hot_db
            .get_bytes(DBColumn::ForkChoice, FORK_CHOICE_DB_KEY.as_slice())
            .unwrap();
        let version = db
            .hot_db
            .get_bytes(DBColumn::BeaconMeta, SCHEMA_VERSION_KEY.as_slice())
            .unwrap();
        let error = crate::schema_change::migrate_schema::<EphemeralHarnessType<MinimalEthSpec>>(
            db.clone(),
            SchemaVersion(29),
            SchemaVersion(28),
        )
        .expect_err("must not drop Gloas payload state");
        assert!(format!("{error:?}").contains("V29 proto"), "{error:?}");
        assert_eq!(
            db.hot_db
                .get_bytes(DBColumn::ForkChoice, FORK_CHOICE_DB_KEY.as_slice())
                .unwrap(),
            original
        );
        assert_eq!(
            db.hot_db
                .get_bytes(DBColumn::BeaconMeta, SCHEMA_VERSION_KEY.as_slice())
                .unwrap(),
            version
        );
    }
}
