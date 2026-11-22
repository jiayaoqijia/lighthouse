# Exact Gloas committee equivocation weight

- [x] Read frozen2ea9d96a Gloas is_head_weak and distinguish committee membership from latest-vote destination.
- [x] Verify that phase0 calculate_committee_fraction intentionally excludes slashed active balances; preserve that behavior.
- [ ] Publish an unchanged-reference threshold counterexample: equivocator belongs to the head committee but voted on another fork.
- [ ] Compute exact head-slot committee contribution using raw justified-state effective balances, including inactive/slashed entries where required by the selected function.
- [ ] Feed exact context into proposer-boost and proposer-head consumers; preserve error propagation and avoid permanent hot-path state reads.
- [ ] Verify bounded cache cold/warm/restart behavior and unchanged pre-Gloas behavior with actual consumers and independent cases.
- [ ] Run native build/lint/fork-choice regressions and publish the independent eth2030-targeted review before a new release tuple is qualified.
