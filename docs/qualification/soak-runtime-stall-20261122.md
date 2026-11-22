# Frozen Gloas observer runtime stall

## Checklist

- [x] Retain the failed6h32m run, original HTTP10s timeout and30s stop budget. NoOOM was observed.
- [x] Freeze stopped database/log/runtime evidence before diagnostic restart; never relabel this run as passed.
- [ ] Reproduce with the same CL/EL binaries and CPU/memory caps, capture thread stacks and identify the actual dependency cycle.
- [x] Reproduce definite core-executor blocking in both observed dispatch paths; move them to monitored blocking work with bounded simulation admission.
- [x] Pass both regressions,60 repeats, real captured Gloas consumer, client library tests and required cargo check.
- [ ] Finish project lint/release and publish; keep original incident causality subject to full-duration requalification.
- [ ] Publish the CL correction independently and restart a newly declared48h qualification under the strict EL evidence gate.

## Observed failure

Source d547ba9844fc9598adefdfb5c056e126bdf03e14, actual CL executable56143d366dee843d1a9410752588ce7c8125ae0b6f9984e1fdad7783588ba3f1,2CPUs and4,096,000,000-byte cap. The node stops logging during slot71912 after a state-advance-overloaded warning and a preceding peer-count drop. Its syncing endpoint times out at2026-11-22T03:44:38Z; the failed observer is stopped and CL exits137 after failing graceful shutdown, with OOMKilled=false. The EL exits0 and its prior canonical commitments/MPT consumers agree with independent Geth. A lock/scheduler stall is a hypothesis until thread evidence confirms it.

Private diagnostic records: /projects/eth2030/qualification/soak-audit-20261122/failure-03. Original producer/validators are preserved. The existing user authorization permits the Lighthouse fix to land on the fork eth2030 branch; these are separate from EL master review.

## Confirmed scheduling defects and regression

The attestation simulator executes its synchronous state/fork-choice work inside an async task, and notifier reads head execution status synchronously on the core executor. A one-worker runtime with an independently released contended reader reproduces loss of unrelated async progress in each production dispatch boundary. Both red tests compile and fail for that logical ordering, not for a shorter latency threshold. The original incident's blocked thread stack was not captured before the forced stop, so these regressions establish definite scheduling defects without claiming that every dependency of the original hang is known.

The fix moves both reads through TaskExecutor's monitored blocking API. The simulator has a single in-flight RAII permit so a blocked read cannot create an unbounded backlog; slot-deadline and syncing checks still execute inside the admitted task. Notifier preserves the actual execution verdict/error and fatal panic monitoring. Consensus, BLS, hashes, fork gates,2CPU/4.096GB caps and10s/30s operating deadlines are unchanged. A new full-duration run is still required; failed03 is immutable.

The same frozen diagnostic pair recovered a45s P2P-only loss and4096 concurrent head/status/state-root reads without reproducing the original hang. This is a bounded diagnostic observation, not48-hour acceptance. The complete7872-block failed-prefix native/Geth scan agrees on commitments/ancestry;5 ordinary transactions are present, so no load-capacity claim is made. Its full reported beacon prefix64059–71906 is separately authenticated by the unchanged reference and overlapping finality extension, while the operating failure remains failed.

## Validation scope

The two runtime tests fail with the original inline/async dispatch after extracting the production boundary, then pass after moving the same reader/work to the blocking executor. Both are repeated30times. Existing client serde and actual captured Gloas state/BLS/partial consumers pass. The production state/fork-choice/attestation computations are unchanged. One pre-existing Rust1.99 collapsible-if lint in malloc threshold setup is expressed equivalently; project lint allowances for question-mark/enum naming remain unchanged. An initial make command-line variable leaked into jemalloc MAKEFLAGS; the corrected invocation supplies EXTRA_CLIPPY_OPTS through the environment. These original tooling failures are retained, not treated as green runs.
