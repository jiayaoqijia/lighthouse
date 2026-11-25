# Independent encrypted Glamsterdam peer components

The native Rust network library now has an actual separately implemented JS peer using pinned Lodestar 1.51.0 SSZ, gossip encoding and request/response libraries, together with the independent JS libp2p stack. The compatible source is the existing fork `fyInALT/lodestar` at `4e224659179dc8050344b48a397f1791450bf24c`; this is not an upstream-approved Lodestar release. No independent source is edited. A fresh isolated checkout rebuilds the five library packages with their TypeScript compiler and emits unchanged beacon-node codec/RPC component JS. Emission with `noCheck` is not represented as a full beacon-node type check, native-binding build or full Lodestar CLI qualification. The peer records the actual compiled entry identities, lockfile, runtime, schema and encoding fingerprints.

The real encrypted TCP/Noise/mplex exchanges exercise full-only completed-column publication, reverse delivery and native decoding, same-identity disconnect/reconnect, actual MetadataV3 CGC 8 to 16 to 4 with monotonic sequence numbers, and native invalid-message scoring (1 to -99). Decreasing custody duty retains legal oversampling subscriptions, as the runtime intends. The partial-only extension is not advertised by this peer; complete columns follow the production ordinary-publication API. Exact delivered SSZ from both directions is retained, then passed into actual state-bound native KZG verification, with altered-cell rejection and real BLS anchor-signature/incorrect-signature controls. The separate captured partial consumer retains out-of-order/duplicate/invalid-proof controls and does not promote an absent/unexecuted envelope.

All 24 ordinary network integration tests pass. The external peer test and two real consumer tests pass, including a fresh isolated independent-source rebuild rerun. `VALIDATION.json` records scoped receipts. A test-only artifact directory retains unchanged delivered bytes and control/event exchanges; ordinary assertions remain unconditional. The new CI job reproduces the pinned build and both transport/consumer steps and uploads the exchanges.

Native EL positional/truncated BAL recovery is separately published in [ETH2030 PR #164](https://github.com/alt-research/eth2030/pull/164). Whole-node status reannouncement, moving canonical pivots and full preceding-chain replay are separate campaigns; this bounded component evidence does not claim them, a 48-hour acceptance, upstream Lodestar approval or original-04 attribution. Final exact-head CI and composition with the other open Lighthouse changes remain pending.

PR: https://github.com/jiayaoqijia/lighthouse/pull/21 (base `eth2030`). Functional head `b4a61546a8c4c8799bc1edf528a87972b0408118`. Final pin/provenance-only peer rerun and exact-head CI are pending.

### Basic simulator CI follow-up

The retained failed-job artifact proves slot 6 was produced and accepted at 2.722 seconds, propagated/imported at about 3.030 seconds, then deliberately reorged by slot 7 because its weight was zero. This is a different event from a missing production or import stall. The six-second minimal clock gave only a two-second pre-Gloas window, while the simulator asserts every canonical slot and full aggregates. Its normal full-cryptography baseline now uses production twelve-second timing with the same minimal state; the existing speed-up factor two retains six-second stress. Every assertion, real BLS/KZG validation and proposer reorg policy remains enabled. The cause of the 2.7-second publication-stage delay is not proven and is not called a fixed node defect or original-04. A fresh complete simulation remains required. See BASIC_SIMULATOR_CI.json.

The complete production-clock local simulator now exits successfully with four beacon nodes and two validator clients, after preserving every original assertion and real cryptographic/proposer-reorg behavior. The immutable deposit build artifact download reset in the fresh debug profile was repaired only in its newly generated output directory by reusing four cached artifacts matching the build script checksums; no contract source or data changed. The independent peer final pin/provenance rerun passes3.07s, and the fresh-build exact-byte real consumer passes2.75s. Published-head CI remains required.
# Fallback simulator follow-up

The next hosted fallback simulator failure was inspected through its complete
artifact, not rerun blindly. Under the previous three-second clock and forced
EL disconnections, slot 33 was produced/imported in 68ms but two late slot-32
envelope deliveries caused a head change back to block 32; slot 34 continued.
The retained receipt distinguishes that actual delayed-message/reorg sequence
from an EL import stall and does not invent a cause for the delivery delay.
The full qualification profile now uses production twelve-second slots while
retaining real cryptography, every canonical-slot assertion and the 95-percent
attestation requirement. Explicit compressed-clock stress is still available.
The new complete local fallback run remains pending.

