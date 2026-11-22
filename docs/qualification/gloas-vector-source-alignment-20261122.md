# Gloas reference-vector source alignment

## Checklist

- [x] Retain hosted EF failures and compare mismatches against the selected consensus source2ea9d96a.
- [x] Confirm v1.7.0-beta.4 fork vectors predate the selected availability/onboarding contract; do not revert conforming production code.
- [x] Generate replacement Gloas fork vectors directly with the unchanged selected reference, preserving all cases and real BLS.
- [x] Freeze source/archive/vector manifests and verify actual minimal/mainnet Rust fork consumers.
- [x] Wire exact-source fixture admission into CI, preserve older vectors for unaffected suites, and reject missing/corrupt provenance.
- [ ] Verify all hosted tests, release build and independent fork integration order before closing.

The currently running48h observer uses functional57ccb466 and is not modified by test-only source alignment. It covers steady operation after the already-completed fork; a fork-transition vector run is an independent necessary acceptance gate. The original EF CI failures remain retained. This review does not rewrite expected output from Lighthouse results or skip fork_upgrade.

## Simulator listener collision

The retained hosted fallback-simulator log proves the boot node HTTP API binds127.0.0.1:42429 before the sixth node attempts fixed P2P0.0.0.0:42429 and fails EADDRINUSE. This is an actual internal listener collision, not a consensus failure or a retry-worthy network flake. The simulator now requests port0 for all TCP/discovery/QUIC P2P listeners and uses the network service's actual ENRs, matching the existing node-test-rig policy. The OS reserves the sockets while they are in use; no free-port probe or fixed alternative range is introduced. Actual simulator regression/build remains pending.

## Source generation and admission result

Both independent unchanged-reference generator runs pass86 tests (44 minimal,42 mainnet; two mainnet-only large-validator preset exclusions declared upstream). Fresh source modules are regenerated from the original archive; all344 generated files exactly match the initial run. The public3,890,744-byte vector archive is07bd5e170824851ff0af5a33efd24a8cd99021e72d32eeb754fdf0d7868304a9. The original source archive remains1fc9482c214443d708b4ae4b225dce47246331e530361de56e0277400517bc94. The six admission/rollback controls pass. Archives are intentionally tracked despite the generic tarball ignore rule.

The old simulator fails deterministically in16.84s with actual libp2p EADDRINUSE while an owned socket reserves42429; this reproduces the hosted collision. Actual Rust full-state consumers now pass all44 minimal and42 mainnet cases with real BLS; no case returns at a fork or fake-crypto guard. The fixed simulator's complete fallback run remains pending. Identical pre-existing dependency-order and equivalent test/malloc lint cleanups from runtime review4 are included for independently reviewable current-compiler CI; they merge identically in either order.

## Pinned analyzer parity

The exact fail-closed modern Lockbud toolchain, real compiler smoke, raw-log preservation and individually reviewed diagnostic policy from runtime review4 are shared identically here. This prevents a standalone qualification review from inheriting the old analyzer's false success. A subsequent hosted minimal-spec run also exposes rpc_tests::test_tcp_blocks_by_range_over_limit timing out; its actual transport/fixture behavior is being reproduced and remains an open CI gate.
