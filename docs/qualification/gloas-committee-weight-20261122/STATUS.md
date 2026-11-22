# Frozen fork-choice boundary correction

The selected Gloas is_head_weak sums raw justified effective balances of equivocators in all head-slot committees, regardless of their latest-message destination. Current ProtoArray explicitly approximates that with head-node equivocating_attestation_score, which misses committee members voting on competing branches. The proposer-head path also uses plain head weight. A source-grounded independent threshold counterexample and exact store-to-proto context are being prepared; no corrected result is claimed yet.

The suspected threshold-policy issue was disproved: frozen phase0 calculate_committee_fraction explicitly excludes slashed active validators. Existing threshold totals must remain unchanged. This review targets committee-equivocation contribution only and preserves pre-Gloas behavior, real errors and bounded resource use.
