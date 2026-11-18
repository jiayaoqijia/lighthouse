# EIP-8282 native EL request consumer

Branch `test/glamsterdam-builder-request-wire`, based on upstream `unstable`
`8104a2467`. This adds an independent real-crypto consumer regression without
changing production transition semantics.

The selected normative input is [Gloas at consensus-specs 2ea9d96a](https://github.com/ethereum/consensus-specs/blob/2ea9d96a420927eb1a35d7c9399c8b3691c69f6a/specs/gloas/beacon-chain.md)
and [EIP-8282 at EIPs 3db38a37](https://github.com/ethereum/EIPs/blob/3db38a3725105cb67b8804ac3e147967113bc3ca/EIPS/eip-8282.md).
The binary records and their provenance are committed under
`consensus/state_processing/testdata/eip8282`. They are the same exact 184/68-byte
SSZ records checked against ETH2030's actual node-backed HTTP Engine output.
The EL fixture generator independently constructs the signing root and real BLS
signature from public synthetic material.

The regression instantiates a real Gloas beacon-chain state, decodes the records
with Lighthouse SSZ, verifies their PoP with Lighthouse's real BLS path and calls
the production builder deposit/exit processing functions. It checks:

- Builder-specific domain accepted; identical deposit fields signed under the
  validator deposit domain ignored without registration.
- Immediate registration, exact balance and withdrawal address, followed by a
  top-up with no repeated signature and no withdrawal-address redirection.
- Exact type-3/type-4 Engine request-list serialization.
- Inactive and wrong-source exits ignored, then pending withdrawals and pending
  bid payments independently prevent exit while other conditions are valid.
- Clearing those obligations allows exit at the exact specified epoch.
- Validator registry, balances and pending deposits remain unchanged.

The selected detailed CL spec still requires builder credentials on this path;
the top-up preserves that prefix. This does not reinterpret the EIP's abbreviated
deprecation wording or combine pending authentication alternatives.

All 23 `state_processing` library tests, `cargo check -p state_processing`,
`cargo fmt --all` and full workspace `make lint-fix` pass (lint: 3m 55s).
Controlled mutations bypassing PoP, source-address authorization or pending
balance checks each compile and fail this regression. Sources are restored and
the suite passes again. Logs are `/tmp/glamsterdam-cl-builder-wire-*.log`.

This qualifies the actual state-processing component with independently produced
Engine bytes. Full live CL/EL devnet transport, parent-payload processing order,
restart, fork choice and independent-client release qualification remain open.
