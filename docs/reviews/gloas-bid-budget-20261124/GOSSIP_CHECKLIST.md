# Gloas gossip clause checklist

Authority: current specs/gloas/p2p-interface.md at d2021d27. This groups the normative validator steps by shared production consumer; cache-fixture tests are not substituted for signed tests.

| Check | Current implementation / evidence |
| --- | --- |
| Highest value | Best bid remains keyed by slot and both parent hashes; lower/equal bids are ignored. |
| Clock eligibility | Current/next plus maximum gossip clock disparity at both edges; signed within/outside controls. |
| Payment and payload identity | Execution payment zero and block hash different from parent. |
| Blob limit | Proposal-epoch blob schedule limits the commitment count. |
| Known parent and descendant | Parent root/state must exist and bid slot follows its parent. |
| Lookahead | Proposal epoch is within the parent state proposer lookahead before state advance. |
| Preferences | Dependency root and fee recipient are resolved on the same parent branch. |
| Execution parent and gas target | Observed execution payload, compatible target and current head branch are checked. |
| Parent RANDAO | Expected mix is read from parent state before processing skipped slots. |
| Slot processing | Initialize caches, clone authoritative parent state and apply process_slots; no mutation of canonical state. |
| Builder identity | Index, payload-builder version and active/finalized status are checked at the proposal state. |
| Bid budget | Whole ETH of balance minus both pending withdrawal/payment collections; saturation at zero. |
| Collateral and exit | Bid coverage and matching parent execution-address/pubkey exit requests remain enforced. |
| Signature | Correct state/fork/domain/registered key; failure creates no cache credit. |
| Atomic publication | Post-signature count/highest-value admission is atomic; independent parent/slot/builder keys and pruning. |
| Proposer/direct route | Directly requested proposer bids retain independent verification and candidate selection; gossip excess is not forwarded. |

Current/next integer-slot checks alone were insufficient: a full signed bid at time 23.75s for slot 3 (12-second slots) was rejected inside the allowed next-slot tolerance. The new clock-aware entrypoint preserves the ordinary fast path and applies the standard SlotClock past/future tolerance helpers for edge cases. No wider arbitrary future window is introduced.
