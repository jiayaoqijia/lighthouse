# Gloas consensus-failed block status

- [x] Read current Gloas envelope/column/payload-attestation networking validity rules and native verification stages.
- [x] Retain a bounded set of definitive signature-verified consensus failures, excluding transient/unknown errors and unauthenticated signature failures.
- [x] Reject dependent gossip for known-failed roots while unknown roots remain ignored.
- [x] Preserve pending/transient recovery and prevent cached failed blocks/bids from being served.
- [x] Add real imported-block regressions, negative controls, bounded-cache checks and targeted cargo check/tests.
- [x] Resolve validated historical preference dependencies hidden before finality.
- [ ] Complete full affected Gloas tests, node build and current independent snapshot replay.
- [ ] Publish an independent eth2030-target PR and verify exact-head CI before authorized merge.

## Current-reference admission follow-up

- [x] Compare Gloas gossip timing line by line with d2021d27 `is_past_slot`, `is_future_slot` and inclusive `is_within_slot_range`.
- [x] Repair proposer-preference millisecond boundaries and PTC inclusive upper endpoint; add inside/one-millisecond-outside tests.
- [x] Move known consensus-failed full columns into the network REJECT/scoring branch so a validation receipt is emitted.
- [x] Keep non-gossip import availability separate from gossip Seen; RPC/HTTP still perform full signature/content checks and cannot consume the first gossip observation.
- [ ] Re-run full native Gloas library/integration, network profiles and node build after the follow-up.
- [ ] Replay the unchanged complete current-reference gossip corpus in the composed runner.
