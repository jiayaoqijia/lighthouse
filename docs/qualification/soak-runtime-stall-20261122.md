# Frozen Gloas observer runtime stall

## Checklist

- [x] Retain the failed6h32m run, original HTTP10s timeout and30s stop budget. NoOOM was observed.
- [x] Freeze stopped database/log/runtime evidence before diagnostic restart; never relabel this run as passed.
- [ ] Reproduce with the same CL/EL binaries and CPU/memory caps, capture thread stacks and identify the actual dependency cycle.
- [ ] Add a deterministic regression, fix the cause without changing protocol/hash/finality checks, and run required cargo check/tests/lint.
- [ ] Publish the CL correction independently and restart a newly declared48h qualification under the strict EL evidence gate.

## Observed failure

Source d547ba9844fc9598adefdfb5c056e126bdf03e14, actual CL executable56143d366dee843d1a9410752588ce7c8125ae0b6f9984e1fdad7783588ba3f1,2CPUs and4,096,000,000-byte cap. The node stops logging during slot71912 after a state-advance-overloaded warning and a preceding peer-count drop. Its syncing endpoint times out at2026-11-22T03:44:38Z; the failed observer is stopped and CL exits137 after failing graceful shutdown, with OOMKilled=false. The EL exits0 and its prior canonical commitments/MPT consumers agree with independent Geth. A lock/scheduler stall is a hypothesis until thread evidence confirms it.

Private diagnostic records: /projects/eth2030/qualification/soak-audit-20261122/failure-03. Original producer/validators are preserved. The existing user authorization permits the Lighthouse fix to land on the fork eth2030 branch; these are separate from EL master review.
