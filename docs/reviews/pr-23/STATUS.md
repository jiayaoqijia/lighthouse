# Current Gloas source-vector admission

The complete clean composition **c93c64e5ae71c771133e48f0b40c5326f9e19f45**
passes all eight local release-qualification stages:12195/12195 current and
historical compatibility cases,403 beacon-chain/4 client/69 state-processing
library tests, the actual encrypted Rust/JS independent peer, its delivered-byte
real BLS/KZG consumer,24 complete native network integration tests, affected
Clippy and the native production node build. The two external tests omitted
from ordinary unit runs are each explicitly executed and pass.
`COMPOSED_QUALIFICATION.json` retains the source, crypto-mode/binary identities,
wire sizes and hashes, all stage receipts and the exact finite scope.

Hosted C93 qualification also passes input preparation, pinned dependency/spec
generation, every regenerated SSZ byte check, both clean consumers and the full
12195-case native replay. Its last artifact-upload step fails because the action
forbids literal `..` path segments. The follow-up exports canonical runner-temp
paths through GITHUB_ENV and uses those same values in upload inputs; actual
shell/environment probes and actionlint pass. The complete new hosted run,
including successful upload and all aggregate checks, remains the live merge
gate. This workflow/documentation follow-up leaves native code and tests
identical to the qualified C93 composition.

The four light-client client/store-role vectors remain explicitly unrun and
outside full-node serving, never counted as passed. Original-04 attribution is
unconfirmed.48h/scale and Lodestar upstream approval remain excluded, not passed.
The following sections retain earlier source-specific diagnostic checkpoints.

## Earlier diagnostic checkpoints

In progress against `eth2030` source `983f62e9b08e3ee655df951909df099e80e5ad7b`. Independent Python vectors are generated from unchanged consensus-specs `d2021d2719235f0df4726a1d330502cb15aa8ea3` (package version 1.7.0-beta.5; no corresponding release tag at the audit). Both mainnet and minimal presets include Gloas tests and inherited tests. Reference generator skips are reported separately from emitted client cases.

A separate explicit-path runner admits every registered emitted case through the existing native EF handler, emits per-case results and fails unsupported handlers, loading failures and known/BLS skips. Existing default release fixture roots and full-suite behavior are preserved. The Gloas gossip adapter is being extended to native bid, signed envelope, proposer preferences, payload attestation and data-column processing. Ordered cross-topic setup messages retain their independent bytes. Final acceptance and test counts are not yet established.

Initial real-BLS execution passed the generated fork, transition, operations and epoch samples already consumed. Fast-confirmation fixtures explicitly marked `bls_setting: 2` contain synthetic invalid signatures and require a separately labelled fake-crypto mathematical lane; they cannot be reported as real-BLS acceptance. Current converted-type static SSZ consumption and strict count reconciliation remain required.

### Complete inherited corpus and strict admission

The independent current reference produced 953 direct Gloas vectors (950 real-BLS cases, three synthetic-signature FCR cases). The first inherited generation failed from disk exhaustion; its partial result is not reported as a passed generation. The source-unchanged resume completed with 394 passed and 12,127 upstream-skipped pytest items, while explicitly accounting for 12,661 completed/reused items. The complete artifact union has 12,065 distinct manifested cases with authenticated per-file identities, no duplicate-path disagreements, and no manifest-only cases. Two distinct source generator functions share the same voluntary-exit output destination in each preset; the source-node accounting retains them separately from unique runtime case counts.

The first clean source-stamped replay passed 9,344 SSZ cases and three new FCR cases; expanded FCR replay exposed exact committee-weight changes and synthetic-signature aggregate-key state-root incompatibility. A fixture-only public-key aggregation mode preserves real public-key bytes while explicitly retaining fake-signature status; it is prohibited from the real state/gossip acceptance lane. Historical known-failure waivers execute in the explicit current snapshot runner and remain failures until fixed.

The clean 3b5ae8d36 real replay improved from 820/950 to 896/950 after timing, failed-root and source-observation repairs. The remaining 54 cases are not accepted: they isolate actual EL-status guards and bid-clock endpoints, plus adapter omissions for the reference's explicit synthetic builder-finality override and activate-at-genesis/future-activation preference contexts. These synthetic context adapters must remain distinct from full chain import/transition qualification. All raw earlier results remain archived.

### Expanded inherited results and reproducibility

Clean source 93b8e984d replays 2,521 supported dynamic cases: 2,512 pass; six operation vectors explicitly require ignored BLS and pass separately in the restricted synthetic-signature operation lane; two genesis reward *component* cases exposed a harness mismatch with the consensus genesis epoch no-op; one full-node light-client collection case confirmed active server-cache defects now isolated in the light-client repair branch. All 645 current/inherited gossip cases pass, including native dedicated partial-validation failure callbacks. Four emitted light-client sync-*client* vectors are explicitly inventoried as a different role from the full-node server; they are not counted as passed.

The reward adapter now extracts deltas from the same native per-validator arithmetic used by consensus, without applying them or bypassing the consensus genesis no-op. Two unchanged independent native regressions pass: supplied state bytes remain unchanged, genesis component penalties are nonzero, and consensus epoch processing still leaves genesis balances untouched. The complete 92-case reward replay passes at clean source a79ddcfb978a835b66be71f4b54201c75f7fb895.

Fresh independent regeneration emits all 9,344 SSZ cases in 72.75 seconds (11,392 upstream skips outside the selected emitted suite). Every emitted source file matches the earlier frozen input by SHA-256: zero missing files and zero differences. CI reproduces these goldens from the pinned source rather than commit the oversized aggregate archive. All 134 current-source compatibility vectors for the two changed pre-Gloas FCR handlers across Altair through Fulu pass native replay at the same clean a79ddcfb source; stale beta.4 expected roots are not waived.

The reproducible input package retains all 2525 dynamic cases in a 6835436-byte XZ archive without changing their files. `current_reference.py` requires exactly 12195 passed full-node/compatibility receipts, verifies every installed/regenerated input, stamps and preserves both clean-source executables, partitions the six explicitly ignored-BLS operation fixtures, and fails missing, duplicate, skipped or mismatched-profile results. Four light-client client/store cases remain explicitly unrun, outside the full-node role. Full composed qualification is pending the separate canonical light-client cache/store repair; preliminary results are not substituted for that final run.

The complete new inventory driver at the prior 2a1496789 snapshot reports
12194 passing cases and exactly one failure, the independently reproduced
unknown-bootstrap semantic mismatch. The complete collection case passes in
the repaired clean 7c38f3d0f client. Final all-case replay uses immutable real
and fixture-crypto executables both stamped with that same clean source; raw
earlier failures remain retained. The pinned Python environment deliberately
uses upstream's ruamel.yaml, not an undeclared PyYAML dependency.

The initial hosted current-reference workflow was rejected before job creation
because runner.temp is not allowed in job-level env expressions. Artifact paths
now use the allowed github.workspace context outside the source checkout; the
corrected workflow passes actionlint. This workflow/documentation follow-up
does not change the native source qualified by the 7c38f3d0f executables. Final
hosted acceptance must include the actual named current-reference job, not only
the pre-existing test-suite checks.

## Complete local current-reference result

All **12195/12195** full-node and historical compatibility cases now pass in
320 strict consumer jobs at the clean, immutable 7c38f3d0f source executables.
There are no native failures, missing receipts, duplicate receipts or waived
cases. `LOCAL_REFERENCE_QUALIFICATION.json` preserves each job's counts, exact
source/crypto identities and binary receipts. The four client/store-role cases
remain explicitly unrun, never counted as passed. The real-profile dynamic
lane has2515 cases; six explicitly ignored-BLS operations,330 synthetic-signature
FCR cases and9344 SSZ cases retain their separate crypto labels.

The complete collection replay now covers genesis finality, canonical reorgs,
committee-period transitions, repeated bootstrap reads and the unknown-root
case through real signed-block import and the native cache/store. The extended
Clippy check includes all affected packages **and** the ef_tests feature; it
passes after equivalent fixture-condition/unit-handler cleanups and checked
reward assertion arithmetic. Both workflow files pass actionlint. Full affected
runtime tests/node build and latest hosted qualification are still in progress.
