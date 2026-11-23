# Lighthouse eth2030 integration

The user explicitly authorized sequential direct integration to eth2030, then clarified Lighthouse only. Existing EL master/PR state was read but not changed. No upstream unstable integration was performed.

Starting target42f6fc26e5c48c4c1ec16368a0a7df705bcc3e69, PRs5→6→8→11→10→7→9→12→4 were merged with ordinary merge commits. Each head was frozen, each resulting parent list matched the prior target/head, and each resulting tree matched the preflight. The final code-merge head is572fe1ffb1df2e24de3989ea774ad07cd2179a31, tree0577e7c2f8d8657028f48873d79ca785744f02da. See MERGES.json.

PR1–3 heads were already ancestors of eth2030 before this sequence. GitHub rejected retargeting their empty eth2030 comparison (no new commits). Their redundant reviews were closed with inclusion evidence; they are not falsely reported as three new merges. No commit was merged into unstable. See ALREADY_INTEGRATED.json.

Actual merged Rust/protocol code, test inputs, Cargo.lock, Makefile and workflows equal qualified3782df1 when documentation/shared trackers are excluded. cargo fmt --all -- --check and locked workspace/all-target compilation pass. Fresh release library suites pass379 beacon-chain,4 fork-choice,41 proto-array and67 state-processing tests. The actual Gloas envelope_api path passes with FORK_NAME=gloas and fork_from_env enabled; four minimum-version downgrade cases pass with each of Fulu and Gloas. Total500 scoped tests pass; no signature, state-root, fixture or failure assertion was weakened. See VALIDATION.json.

Previously red standalone PR workflows remain historical failed results; user-authorized merging does not relabel them green. The existing hosted suite only triggers pushes to stable/staging/trying/pr/*, not eth2030. A CI-only ref pr/eth2030-merged-20261123-572fe1ff will point at the final documentation commit to run the unchanged complete workflow on this integrated source. Its hosted result is a separate pending gate until the run actually completes. No workflow rule or skip-ci label is changed.

These integration/build/regression results do not create a new operating measurement, prove original04's exact historical holder/physical storage cause, or grant48h/public-release admission. Original failures and the completed bounded source-specific/BLS/stopped-head evidence remain retained.
