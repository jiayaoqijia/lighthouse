# Gloas bid-budget and parent-state validation

Authority: consensus-specs d2021d2719235f0df4726a1d330502cb15aa8ea3, specs/gloas/p2p-interface.md, including merged one-bid-per-ETH PR 5805 and pending-withdrawal subtraction PR 5843. EIP-7732 remains on the unchanged signed Gloas beacon-chain contract.

Behavioral negatives: a 2-ETH builder's second increasing signed bid was rejected by the original one-bid boolean. After repairing counts, a third bid on a 2-ETH parent was incorrectly accepted using a 3-ETH head. Actual real-BLS tests reproduce both before their respective fixes. The initial zero-test exact filter and a storage-anchor fixture error are not counted as behavioral controls.

New tests cover pending withdrawals/payments, saturated zero allowance, missing indices, count/parent/slot ownership, pruning and atomic concurrent admission. Existing invalid-signature controls confirm failed signatures do not mark the builder or highest bid. Direct proposer verification remains independent of gossip allowance, and over-limit bids are not regossiped.

The first complete counter-only suite passed 57 tests. The full state-advance suite then retained a 57/58 failure caused by an old synthetic epoch-0 state finalized in epoch 1. No state-advance check was bypassed: epoch-crossing tests now initialize caches and use an epoch-2 anchor whose latest block header matches its actual block. The corrected targeted cross-epoch case passes. Final check/test/hosted results are attached separately at their exact heads; no prior live CL binary is renamed as this source.
