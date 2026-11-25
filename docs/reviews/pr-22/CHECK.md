# Current-source full-node light-client collection conformance

- [x] Isolate the merged progressive-bitmap baseline and retain the independent current-source failure.
- [x] Trace native producer/store/cache and read current full-node light-client rules.
- [x] Repair insufficient-participation update publication while retaining bootstrap data.
- [ ] Compare genesis finality, committee-period transitions and selected-head/reorg results against unchanged independent collection vectors; fix actual mismatches.
- [ ] Add source-pinned native real-BLS regressions, complete affected tests and node build.
- [ ] Publish a separate eth2030-target PR; verify CI and composition before the authorized merge.

## Canonical collection and recovery follow-up

- [x] Reproduce an actual native fork-choice reorg which leaves the old branch period update visible.
- [x] Qualify selected-head branch data, atomic period replacement/removal and retry after persistence failure.
- [ ] Qualify committee-period boundary normalization and identical-content committee persistence across periods.
- [x] Preserve schema identity when reading/writing updates across a fork inside a sync period; retain legacy compatibility.
- [x] Qualify repeated bootstrap reads against a cached branch and a mismatched global period committee.
- [x] Qualify head-change background notifications and run ancestry reconstruction off the async reactor.
