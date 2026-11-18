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
