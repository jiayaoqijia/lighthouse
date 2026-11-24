# Gloas consensus-failed block status

- [x] Read current Gloas envelope/column/payload-attestation networking validity rules and native verification stages.
- [ ] Retain a bounded set of definitive signature-verified consensus failures, excluding transient/unknown errors and unauthenticated signature failures.
- [ ] Reject dependent gossip for known-failed roots while unknown roots remain ignored.
- [ ] Preserve pending/transient recovery and prevent cached failed blocks/bids from being served.
- [ ] Add real imported-block regressions, negative controls, bounded-cache checks and cargo check/build/tests.
- [ ] Publish an independent eth2030-target PR and verify exact-head CI before authorized merge.
