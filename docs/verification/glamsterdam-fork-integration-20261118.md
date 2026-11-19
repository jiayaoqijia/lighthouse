# Combined Gloas fork fixes on the paired client

Base: `f85fa05b6`, the existing paired-client checkout. Functional source checkpoint: `e6ff51d52` (with first-slot commit `71100a6c0`). Only the owned fixes from [PR 1](https://github.com/jiayaoqijia/lighthouse/pull/1) and [PR 2](https://github.com/jiayaoqijia/lighthouse/pull/2) were cherry-picked; the shared test insertion retains both regressions. The original checkout remains unchanged.

All 24 state_processing library tests pass on this combined source, including the real slot/fork transition, ordered builder/validator routing, complete post-state root cache equivalence and existing signature-cache checks. `cargo check -p state_processing`, `cargo fmt --all`, and the full workspace `make lint-fix` also pass. The latter took 3m 57s. Logs are `/tmp/glamsterdam-cl-integrated-{tests,check,lint}.log`.

This is component/library compilation and transition evidence for the actual paired-client source. It is not a launched multi-client devnet, final payload-authentication choice, EIP-8282 end-to-end pipeline or a complete Gloas release gate. The branch is retained independently for integration review.
