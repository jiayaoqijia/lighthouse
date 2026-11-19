# Signed envelope state admission

The selected [consensus-specs 2ea9d96a gossip contract](https://github.com/ethereum/consensus-specs/blob/2ea9d96a420927eb1a35d7c9399c8b3691c69f6a/specs/gloas/p2p-interface.md#new-execution_payload) requires state consistency before marking an envelope seen. Previously a correctly re-signed envelope with a wrong parent beacon root passed gossip verification and could suppress a later authentic envelope. Other state-dependent checks were likewise deferred until execution-pending processing, after the claimed gas limit had entered the observation cache.

Source `0262e8f90` checks the parent root against the imported block and reuses the complete pure consensus verifier before observation on gossip, RPC and HTTP, retaining the loaded state snapshot for subsequent processing. Empty transactions are rejected by the common verifier. The existing whole-message BLS authentication and chosen signing domain are preserved. This adopts the selected merged contract without choosing open alternative authentication formats.

Two new explicit-Gloas tests have no environment-dependent early return. Real proposer signatures cover seven inconsistent contents: parent beacon root, gas limit, randao, execution parent, timestamp, expected withdrawals and empty transactions. They assert the exact rejection guard, peer rejection, no gas-cache pollution and acceptance of the authentic envelope after failures. Repeated RPC/HTTP verification still rejects those contents after a valid observation. Nine payload-content replacements retaining the original signature and claimed block hash each fail authentication on all sources and on cached recovery paths. Together these execute 108 source/cache negative assertions plus their authentic positives.

Local qualification:

- The original correctly signed parent-root regression failed before the fix (actual admission succeeded).
- Full explicit-Gloas minimal chain suite: 243 passed, zero failed, one ignored, 120.12 seconds. The ignored crash-recovery case is explicitly reproduced separately and remains open; it is not counted as a passing gate.
- Envelope release controls pass under both explicit `FORK_NAME=gloas` and `fulu` with `spec-minimal,fork_from_env`. Gloas executes thirteen bodies; Fulu executes four including the two new explicit-Gloas controls. These are fifteen distinct bodies, not thirty.
- All 62 state-processing release library tests pass; full workspace `cargo check` and formatting pass.
- The release runtime builds in 6m12s and reports `Lighthouse v8.4.0-rc.0-0262e8f`, mainnet=true, minimal=false. This is a built runtime, not a launched cross-client campaign.

Logs remain under `/tmp/eth2030-cl-envelope-{cache-red,cache-qualified,all-gloas,all-fulu,state-processing,check,release}.log`. R06 still requires the complete FULL/EMPTY/PENDING, late, reorg and invalidation campaign. R07-R13 remain independently tracked.

Deposit requests are intentionally unbounded progressive lists in the selected specification; no obsolete per-list deposit cap was added. Network byte limits remain a separate acceptance concern. A theoretical envelope-size helper omits new builder-list contributions, but its consumer also clamps against MAX_PAYLOAD_SIZE; no concrete network-limit bypass is asserted from that arithmetic alone.
