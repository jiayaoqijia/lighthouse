//! Branch-local light-client data. Only the selected head publishes serving data.
use super::*;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(super) struct BranchUpdates<E: types::EthSpec> {
    pub latest_finality: Option<Arc<LightClientUpdate<E>>>,
    pub latest_optimistic: Option<Arc<LightClientUpdate<E>>>,
    pub best: BTreeMap<u64, Arc<LightClientUpdate<E>>>,
}
impl<E: types::EthSpec> Default for BranchUpdates<E> {
    fn default() -> Self {
        Self {
            latest_finality: None,
            latest_optimistic: None,
            best: BTreeMap::new(),
        }
    }
}
pub(super) struct Update<E: types::EthSpec> {
    pub update: Arc<LightClientUpdate<E>>,
    pub finality_available: bool,
    pub period: Option<u64>,
}
impl<T: BeaconChainTypes> LightClientServerCache<T> {
    /// Rebuild a missing branch from actual stored ancestors. The steady-state
    /// LRU is bounded; a cold start or deep reorg reconstructs from retained
    /// blocks rather than borrowing another branch's latest/best values.
    fn branch_updates(
        &self,
        store: BeaconStore<T>,
        root: Hash256,
        spec: &ChainSpec,
    ) -> Result<Arc<BranchUpdates<T::EthSpec>>, BeaconChainError> {
        if let Some(data) = self.branches.lock().get(&root).cloned() {
            return Ok(data);
        }
        let mut missing = Vec::new();
        let mut cursor = root;
        let mut data = loop {
            if let Some(data) = self.branches.lock().get(&cursor).cloned() {
                break data;
            }
            match store.get_blinded_block(&cursor)? {
                Some(block) if block.slot() == spec.genesis_slot => {
                    let data = Arc::new(BranchUpdates::default());
                    self.branches.lock().insert(cursor, data.clone());
                    break data;
                }
                Some(block) => {
                    missing.push(cursor);
                    cursor = block.parent_root();
                }
                None if cursor == root => {
                    return Err(BeaconChainError::DBInconsistent(format!(
                        "selected light-client head missing: {root}"
                    )));
                }
                // Checkpoint starts may not retain the anchor's ancestors. Do
                // not fabricate historical updates from an unrelated branch.
                None => {
                    let data = Arc::new(BranchUpdates::default());
                    if let Some(anchor) = missing.pop() {
                        self.branches.lock().insert(anchor, data.clone());
                    }
                    break data;
                }
            }
        };
        for root in missing.into_iter().rev() {
            let block = store.get_blinded_block(&root)?.ok_or_else(|| {
                BeaconChainError::DBInconsistent(format!(
                    "light-client branch block disappeared: {root}"
                ))
            })?;
            let mut next = (*data).clone();
            if !spec
                .fork_name_at_slot::<T::EthSpec>(block.slot())
                .altair_enabled()
            {
                data = Arc::new(next);
                self.branches.lock().insert(root, data.clone());
                continue;
            }
            let aggregate = block.message().body().sync_aggregate()?;
            if let Some(built) = self.build_update(
                store.clone(),
                block.slot(),
                &block.parent_root(),
                aggregate,
                spec,
            )? {
                next.latest_optimistic = Some(built.update.clone());
                if built.finality_available {
                    next.latest_finality = Some(built.update.clone());
                }
                if let Some(period) = built.period {
                    let better = match next.best.get(&period) {
                        Some(previous) => {
                            previous.is_better_light_client_update(&built.update, spec)?
                        }
                        None => true,
                    };
                    if better {
                        next.best.insert(period, built.update);
                    }
                }
            }
            data = Arc::new(next);
            self.branches.lock().insert(root, data.clone());
        }
        Ok(data)
    }

    /// Apply the full-node reference's selected-head view, including a reorg to
    /// a branch with no eligible signature. Persist period changes atomically
    /// before publishing in-memory latest values; a failed batch leaves the
    /// previous serving view available for retry.
    pub fn recompute_for_head(
        &self,
        store: BeaconStore<T>,
        root: Hash256,
        spec: &ChainSpec,
    ) -> Result<(), BeaconChainError> {
        let mut selected = self.selected_head.lock();
        if selected
            .as_ref()
            .is_some_and(|(previous, _)| *previous == root)
        {
            return Ok(());
        }
        let data = self.branch_updates(store.clone(), root, spec)?;
        let mut changes = Vec::new();
        let mut periods: BTreeSet<_> = data.best.keys().copied().collect();
        if let Some((_, previous)) = selected.as_ref() {
            periods.extend(previous.best.keys().copied());
        } else {
            // Upgrade/restart: reconcile durable entries too, including legacy
            // zero-participant or noncanonical period values.
            periods.extend(store.light_client_update_periods()?);
        }
        for period in periods {
            let new = data.best.get(&period);
            let old = selected
                .as_ref()
                .and_then(|(_, previous)| previous.best.get(&period));
            if new.zip(old).is_some_and(|(new, old)| Arc::ptr_eq(new, old)) {
                continue;
            }
            changes.push((period, new.map(|update| (**update).clone())));
        }
        store.apply_light_client_updates(changes)?;
        *self.latest_finality_update.write() = data
            .latest_finality
            .as_ref()
            .map(|update| (**update).clone().into());
        *self.latest_optimistic_update.write() = data
            .latest_optimistic
            .as_ref()
            .map(|update| (**update).clone().into());
        *selected = Some((root, data));
        Ok(())
    }
}
