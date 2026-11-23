# Mock Engine validated-payload lifecycle

- [x] Retain both complete fallback failures, all published missing-slot envelopes and head rollback traces.
- [x] Read frozen Engine API a9e9095c Paris payload-validation/fork-choice contract and EIP-3675 canonicality rules.
- [x] Reproduce VALID parent followed by child-before-FCU returning SYNCING in the actual execution block generator.
- [x] Retain validated payloads independently of canonical head; both lifecycle controls and all three actual KZG fixture tests pass.
- [ ] Verify canonical head changes only via FCU, then rerun the full unchanged simulator and hosted checks.
- [ ] Publish an independent eth2030-targeted PR, preserve source/merge provenance and remaining full48h boundary.
