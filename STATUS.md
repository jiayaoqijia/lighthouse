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
