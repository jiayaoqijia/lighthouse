# Frozen fork-choice boundary correction

The selected Gloas is_head_weak sums raw justified effective balances of equivocators in all head-slot committees, regardless of their latest-message destination. Current ProtoArray explicitly approximates that with head-node equivocating_attestation_score, which misses committee members voting on competing branches. The proposer-head path also uses plain head weight. A source-grounded independent threshold counterexample and exact store-to-proto context are being prepared; no corrected result is claimed yet.

The suspected threshold-policy issue was disproved: frozen phase0 calculate_committee_fraction explicitly excludes slashed active validators. Existing threshold totals must remain unchanged. This review targets committee-equivocation contribution only and preserves pre-Gloas behavior, real errors and bounded resource use.

## Implemented and component checks

Exact committee credit is computed from the authenticated head-slot state and unfiltered current justified-state effective balances. Ordinary voting balances and the unslashed-active threshold are unchanged. Memberships alone use a four-entry derived cache; state reads/shuffling run outside its lock. Every request re-sums current equivocation and checkpoint data. Missing/invalid state propagates an error rather than zero credit. No persisted schema is added. The credit is forwarded through proposer boost, canonical payload status, definitive/preliminary proposer-head checks and EF viable-tree checks.

The independent unchanged reference produces actual BLS-valid attester slashings with no latest head message. Both presets were regenerated again byte-for-byte with assertions/BLS explicitly active. The old head-weak rule fails its compiled counterexample. All 41 proto-array tests passed; the actual reference-SSZ Store test passed both presets, cold restoration, current slashing/balance changes, nonmember exclusion, bounded eviction and missing/mismatched-state errors. The release beacon-chain check and EF test-target check passed. Setup-only Arc/anchor initialization errors were retained and are not cryptographic negative controls.

Final qualification remains pending: shared Cargo output across divergent worktrees later exposed incompatible dependency metadata in another branch. All affected workspace crates must be rebuilt and the full component/real-node/simulator suite rerun in one combined checkout. Existing passing component logs do not substitute for that exact-source gate or the fresh 48-hour campaign.
