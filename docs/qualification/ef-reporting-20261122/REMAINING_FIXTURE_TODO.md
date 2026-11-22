# Remaining EF fixture boundaries

The real-crypto run executed/passed5366 mainnet and5925 minimal vector bodies after deduplicating repeated handler paths within each preset. Each preset also has24 incompatible-BLS-mode skips and43 known/harness skips; these are not passes. All152 source-generated Gloas fork/transition cases execute/pass without either kind of skip. See SKIP_INVENTORY.json for actual handler receipts.

The43 existing known/harness skips per preset are:

| Count | Source boundary | Required follow-up |
| --- | --- | --- |
|5|Pre-Fulu epoch-boundary proposer re-org policy; Fulu/Gloas are not excluded|Keep the intentional policy/spec distinction explicit; confirm the linked proposer-lookahead change before altering legacy production behavior.|
|18|Fulu attestation/aggregate gossip fixtures requiring synthetic history, unknown-finalized roots or sub-slot timing the production-backed harness cannot construct|Add a precise fixture adapter/model with equivalent validation; do not insert consensus-invalid setup blocks through normal production import merely to make a test pass.|
|13|Phase0-through-Fulu gossip-block fixtures retaining a consensus-invalid parent in their setup model|Model the invalid-parent classification separately from valid-chain import, then execute the expected ignore/reject outcomes.|
|7|Post-Altair genesis-epoch empty rewards fixtures, one per fork including Gloas|Reconcile the pure rewards-delta fixture with the epoch-zero no-rewards processing adapter; execute an appropriate delta consumer before claiming coverage.|

Additional inactive targets are also explicit: the dedicated execution-envelope operation handler has no selected published `operations/execution_payload` fixtures, model-only FCR/compliance is not real-BLS receipt coverage, and Heze is outside the frozen Gloas target. Real signed HTTP retry/validation tests, source-generated signed transitions and independent live finality replay remain separately evidenced consumers; do not relabel inactive handlers as executed vectors.

- [ ] Resolve the above fixture adapters/policy boundaries with source-grounded expected outcomes and actual consumers.
- [ ] Keep all skipped/unexecuted cases visible until independently resolved; no blanket waiver or "all cases passed" claim.
- [ ] Enable future published envelope-operation vectors only with verified nonempty input and real validation, retaining the existing signed positive/negative consumers.
- [ ] Keep model-only/fake-crypto fixture results separate from native full-signature release admission.

This inventory records existing acceptance debt rather than asserting a production protocol defect from a skipped test alone. The failed48-hour operating gate and repeated runtime stall remain independently unresolved and cannot be waived by EF success.
