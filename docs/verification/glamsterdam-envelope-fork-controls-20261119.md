# Explicit-fork envelope controls

The first `envelope_verification` command omitted `fork_from_env`. All 13 entries returned at their fork guards and are excluded from qualification. This correction retains the original record and counts executed bodies, not nominal green test entries.

Both release repeats pass with `-p beacon_chain --features spec-minimal,fork_from_env --test beacon_chain_spec_minimal_tests envelope_verification`, the shared target cache and two build jobs. `FORK_NAME=gloas` executes eleven bodies; `FORK_NAME=fulu` executes the two pre-Gloas bodies. Each invocation still reports 13 entries because the other fork's guards return, so the total is thirteen distinct executed controls, not twenty-six independent cases.

The Gloas controls cover genesis parent-payload seeding, lookup after restart, optimistic import and later-valid ancestor promotion, execution-requests-root mismatch, observed gas limits, same-builder and modified-slot deduplication, HTTP/RPC bypass with seen marking, and re-verification through a different source. The Fulu controls check gas-limit caching with/without scheduled Gloas. Assertions run through actual `BeaconChainHarness` consumers; the Engine endpoint is mocked. This is component qualification, not independent EL/CL process interoperability.

The selected signed-envelope authentication format is unchanged. R06 still requires the complete contents/hash/signature/request-binding matrix, payload FULL/EMPTY/PENDING and latestValidHash/reorg/descendant invalidation controls. R07-R13 remain separate obligations. The existing code integration/build evidence is retained; no new runtime configuration switch was introduced.
