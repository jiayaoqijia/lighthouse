# Gloas reference-vector source alignment

## Checklist

- [x] Retain hosted EF failures and compare mismatches against the selected consensus source2ea9d96a.
- [x] Confirm v1.7.0-beta.4 fork vectors predate the selected availability/onboarding contract; do not revert conforming production code.
- [ ] Generate replacement Gloas fork vectors directly with the unchanged selected reference, preserving all cases and real BLS.
- [ ] Freeze source/archive/vector manifests and verify actual minimal/mainnet Rust fork consumers.
- [ ] Wire exact-source fixture admission into CI, preserve older vectors for unaffected suites, and reject missing/corrupt provenance.
- [ ] Verify all hosted tests, release build and independent fork integration order before closing.

The currently running48h observer uses functional57ccb466 and is not modified by test-only source alignment. It covers steady operation after the already-completed fork; a fork-transition vector run is an independent necessary acceptance gate. The original EF CI failures remain retained. This review does not rewrite expected output from Lighthouse results or skip fork_upgrade.

## Simulator listener collision

The retained hosted fallback-simulator log proves the boot node HTTP API binds127.0.0.1:42429 before the sixth node attempts fixed P2P0.0.0.0:42429 and fails EADDRINUSE. This is an actual internal listener collision, not a consensus failure or a retry-worthy network flake. The simulator now requests port0 for all TCP/discovery/QUIC P2P listeners and uses the network service's actual ENRs, matching the existing node-test-rig policy. The OS reserves the sockets while they are in use; no free-port probe or fixed alternative range is introduced. Actual simulator regression/build remains pending.
