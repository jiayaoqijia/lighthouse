# Gloas consensus-failed block status

- [x] Read current Gloas envelope/column/payload-attestation networking validity rules and native verification stages.
- [x] Retain a bounded set of definitive signature-verified consensus failures, excluding transient/unknown errors and unauthenticated signature failures.
- [x] Reject dependent gossip for known-failed roots while unknown roots remain ignored.
- [x] Preserve pending/transient recovery and prevent cached failed blocks/bids from being served.
- [x] Add real imported-block regressions, negative controls, bounded-cache checks and targeted cargo check/tests.
- [x] Resolve validated historical preference dependencies hidden before finality.
- [ ] Complete full affected Gloas tests, node build and current independent snapshot replay.
- [ ] Publish an independent eth2030-target PR and verify exact-head CI before authorized merge.
