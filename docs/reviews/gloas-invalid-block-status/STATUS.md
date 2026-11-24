# Gloas dependent-message admission

Two independently reproduced runtime gaps are repaired against `eth2030` base `983f62e9b08e3ee655df951909df099e80e5ad7b`.

A bounded set of 1024 authenticated consensus-failed unsigned block roots distinguishes an unknown block (ignore) from a known failed block (reject). Roots are recorded only after the proposer signature is verified: committed invalid body signatures, or definitive transition/state-root failures after all signatures pass. A bad outer signature, unknown parent and transient engine/store errors do not authenticate invalidity. Failed pending blocks, bids and columns are removed together. Full/partial-column, envelope, payload-attestation and block-parent admission consult the result; gossip and sync scoring handle the explicit rejection.

Proposer preferences can still depend on a validated canonical ancestor before finality. Fork choice deliberately hides these roots. The verifier now authenticates the canonical dependent root against the head state's block-root history and resolves its stored block and hot/cold state, while preserving live noncanonical branch support and dependent-slot/child checks. Envelope finality reads the current fork-choice checkpoint rather than a possibly older cached head checkpoint.

Independent unchanged consensus-specs `d2021d2719235f0df4726a1d330502cb15aa8ea3` fixtures retain their original source filenames and compressed/raw hashes under `beacon_node/beacon_chain/tests/testdata`. Three executed real-BLS regressions pass: a correctly signed wrong-state-root block produces a real state-root failure and dependent rejections; replacing its outer signature cannot poison the root; unknown envelopes remain unknown; cache capacity is bounded; and the same valid signed proposer preferences are accepted both before and after finality hides their dependency. The latter originally failed in the current upstream gossip vectors with `DependentRootUnknown`.

Targeted cargo check (including test code), formatting and the three regression tests pass. Full affected Gloas tests, complete node build, snapshot replay and remote CI remain in progress. No merge is performed before qualification.
