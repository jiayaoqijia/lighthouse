# Authorized Glamsterdam integration into eth2030

The user authorized direct integration into `eth2030`. The candidate retains target `29d66c23805f01cd8441825f7e803b394db50244`, the qualified source candidate `4432411764d89be3fad6af98b42cdd4e995d8a82`, and all three original reviewed heads as ancestors. Existing Heze/Frame/checkpoint paths remain present. Original reviews: [1](https://github.com/jiayaoqijia/lighthouse/pull/1), [2](https://github.com/jiayaoqijia/lighthouse/pull/2), [3](https://github.com/jiayaoqijia/lighthouse/pull/3).

Source `50c8be1a92ed4fd517d30ba5ee16ce3873732008` passes 62 release `state_processing` library tests (including Gloas fork slot/ring-wrap, credential-first independent builder/validator onboarding, cache/full verification, and real-BLS native EL request consumption), 12 release inclusion-list/Heze integration tests, full workspace `cargo check` and release Lighthouse build. Git formatting hooks remain enabled. The binary reports `Lighthouse v8.4.0-rc.0-50c8be1`.

The initial 13 minimal-spec envelope test entries returned at fork guards and are excluded from evidence. Corrected release runs enable the existing `fork_from_env` test feature and explicitly select `FORK_NAME=gloas` and `FORK_NAME=fulu`. Together they execute 11 distinct Gloas bodies and two Fulu bodies successfully, including request-root rejection, duplicate/modified-slot cache controls, optimistic ancestor promotion and envelope lookup after restart. [Exact qualification and remaining scope](docs/verification/glamsterdam-envelope-fork-controls-20261119.md).

The first debug workspace check encountered a reset downloading a fixed deposit-contract ABI. It was rerun successfully after restoring existing build-cache inputs whose four SHA-256 values match the build script's required values. Release tests initially exposed two identical test definitions imported through the original review ancestry; those exact duplicate bodies were removed before the successful final run.

This closes integration/build qualification of the three existing fixes (R05). It does not claim completion of R06 authentication/lifecycle, R07 payment invariants, R08 independent progressive proofs, R09 real-peer recovery, or R10–R13 full-client/resource/release campaigns. Those remain active in the EL program tracker. The original unstable-target PRs remain open for their original review lineage.

## Preserved prior task records

# Lighthouse implementation status

## Heze checkpoint envelope parent repair (2026-11-18)

- Based on ETH2030 `a05a56de`, in isolated branch `fix/heze-checkpoint-history`.
  Consensus specification `14e6ce5a5334910b0d0575d782e468305ad92373`,
  `specs/heze/fork-choice.md#get_forkchoice_store`, initializes the trusted
  anchor's inclusion-list satisfaction to true.
- The actual checkpoint-envelope regression failed before the fix with
  `MissingBeaconBlock` for its unavailable pre-anchor parent. Payload notification
  now recognizes only the root-bound trusted anchor at/before the store anchor
  slot; ordinary parent/list lookups retain their errors.
- The regression uses real signed blocks and an empty checkpoint store, forces
  Engine notification, rejects bad signatures and mismatched bids, and waits for
  verified custody columns before importing. Engine V6 wire assertions require
  an explicit empty anchor IL and retain the nonempty observed IL on live payloads.
- Five Heze regressions pass, including true/false verdict persistence, empty-head
  deferred FCU, proof relay and self-build list parity. `cargo fmt --all`, full
  `make lint-fix` and scoped `cargo check` pass. Release build and paired live
  history qualification remain pending.
- Compared upstream experimental Heze `f66bd5ac`: its list helper returns empty
  on all lookup errors. This patch keeps the strict live-view behavior and scopes
  the bootstrap treatment to the specification's trusted anchor.

## Worktree build version cache repair (2026-11-18)

- The first release build contained the checkpoint fix but reported the earlier
  `952464b` commit: the inherited version build script treated a worktree's
  `.git` file as a directory, so Cargo did not track its HEAD or common refs.
- Git plumbing now resolves the actual HEAD, loose branch ref and packed refs.
  Packed-to-loose updates watch the existing ref directory. Source archives
  without Git retain the existing version fallback.
- A real cached Cargo probe failed before the fix, then passed through ordinary
  checkout commits, worktree commits, packed refs, detached HEAD and paths with
  spaces. Fixture Git state is isolated from parent Git environment and hooks.
- Five version tests, cargo sort, cargo fmt, scoped cargo check and full
  make lint-fix pass. The release must be rebuilt and checked before using it
  in live evidence; the mismatched-version image has not run a devnet.

## Qualified paired history control (2026-11-18)

- Runtime `a4ceb86e` reports its actual commit and passed with native EL
  `e0451092`: coded sponsorship finalized through signed FOCIL at slot 66 and
  the blob Frame at slot 165. A new validator-free CL joined at slot 256,
  checkpoint-synced and returned both original finalized historical envelopes.
  Raw transaction identities, Beacon/execution roots and commitments matched.
- Actual EL restart preserved both workloads' receipts, historical storage and
  paymaster settlement. The enclave, all labeled containers/volumes and its
  engine were removed. The new CL shares the synchronized second EL; independent
  EL late join is a separate gate.
- The checkpoint and Git-version regressions, cargo fmt/sort, scoped cargo check,
  full lint-fix and release build passed. Code is published on `eth2030`.
- [Paired evidence](https://github.com/fyInALT/eth2030/blob/feat/frame-8141-execution/docs/verification/frame-8141-20261118-paymaster-history-final.json)
