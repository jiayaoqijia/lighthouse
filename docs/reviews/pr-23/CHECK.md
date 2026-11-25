# Complete current Gloas reference admission

- [x] Isolate merged eth2030 983f62e9 and read contribution/spec-test guidance.
- [x] Pin current consensus source d2021d27 and provision an independent real-BLS generator environment.
- [x] Generate the complete current Gloas suites for both presets, including inherited applicable transition/operation/SSZ cases; retain all skips/failures.
- [x] Replay every supported emitted vector through actual Lighthouse handlers; add missing compatible handlers without deriving expected states from the client.
- [x] Exercise negative bid/envelope/request admission; independent mixed-peer consumers are separately qualified in PR21.
- [ ] Verify cargo check, full affected tests and CI; submit a separate eth2030-target PR.

Heze research is separate from the scheduled Glamsterdam contract. Unsupported vectors are reported, never silently admitted. A stale published release is not substituted for the selected current contract.

- [x] Package the unchanged dynamic corpus and current historical FCR compatibility inputs with per-file identities.
- [x] Reproduce all SSZ goldens independently and confirm byte identity; implement the same complete hosted qualification workflow.
- [x] Complete canonical light-client collection replay after the separate native cache/store repair and require all 12195 full-node/compatibility receipts.
