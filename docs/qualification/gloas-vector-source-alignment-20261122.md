# Gloas reference-vector source alignment

## Checklist

- [x] Retain hosted EF failures and compare mismatches against the selected consensus source2ea9d96a.
- [x] Confirm v1.7.0-beta.4 fork vectors predate the selected availability/onboarding contract; do not revert conforming production code.
- [ ] Generate replacement Gloas fork vectors directly with the unchanged selected reference, preserving all cases and real BLS.
- [ ] Freeze source/archive/vector manifests and verify actual minimal/mainnet Rust fork consumers.
- [ ] Wire exact-source fixture admission into CI, preserve older vectors for unaffected suites, and reject missing/corrupt provenance.
- [ ] Verify all hosted tests, release build and independent fork integration order before closing.

The currently running48h observer uses functional57ccb466 and is not modified by test-only source alignment. It covers steady operation after the already-completed fork; a fork-transition vector run is an independent necessary acceptance gate. The original EF CI failures remain retained. This review does not rewrite expected output from Lighthouse results or skip fork_upgrade.
