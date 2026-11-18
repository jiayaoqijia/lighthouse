# Gloas first-slot payload availability

[PR 1](https://github.com/jiayaoqijia/lighthouse/pull/1), branch `fix/glamsterdam-first-slot`, based on upstream `unstable` `8104a2467`. The same missing initialization is present in the local paired-client checkout `f85fa05b6`.

The selected normative input is [consensus-specs 2ea9d96a, Gloas fork logic](https://github.com/ethereum/consensus-specs/blob/2ea9d96a420927eb1a35d7c9399c8b3691c69f6a/specs/gloas/fork.md#upgrading-the-state), including merged consensus-specs PR 5808. The fork's current payload-availability bit must be false after state migration. The preceding slot ran Fulu processing, so its normal Gloas next-slot clearing did not run.

`upgrade_state_to_gloas` previously initialized every availability bit to true. The change clears the fork slot's ring index after constructing the new state. It uses checked modulo and preserves all other history bits.

The regression uses the real `per_slot_processing` path and a real minimal-preset beacon-chain harness, with genuine cryptography. It crosses the scheduled fork at epoch 1 and beyond one historical-ring wrap. It checks that the fork slot is unavailable, its predecessor remains available, and the next slot is cleared by ordinary Gloas processing without changing either preceding result.

The baseline test compiled and failed with the fork slot still true. The local log is `/tmp/glamsterdam-cl-first-slot-baseline.log`. After the change, all 23 `state_processing` library tests and `cargo check -p state_processing` pass. `cargo fmt --all` and the repository's complete `make lint-fix` command also pass (full workspace, benches and tests; 3m 51s).

Commands use the existing shared target cache:

```sh
CARGO_TARGET_DIR=/projects/eth2030/lighthouse/target cargo test -p state_processing --lib
CARGO_TARGET_DIR=/projects/eth2030/lighthouse/target cargo check -p state_processing
cargo fmt --all
CARGO_TARGET_DIR=/projects/eth2030/lighthouse/target make lint-fix
```

This qualifies the fork-state/slot transition, not complete EL/CL devnet behavior, payload authentication alternatives or the full Gloas release matrix. No existing shared checkout is switched or modified.
