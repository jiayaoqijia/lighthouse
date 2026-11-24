# Complete current Gloas reference admission

- [x] Isolate merged eth2030 983f62e9 and read contribution/spec-test guidance.
- [x] Pin current consensus source d2021d27 and provision an independent real-BLS generator environment.
- [ ] Generate the complete current Gloas suites for both presets, including inherited applicable transition/operation/SSZ cases; retain all skips/failures.
- [ ] Replay every supported emitted vector through actual Lighthouse handlers; add missing compatible handlers without deriving expected states from the client.
- [ ] Exercise negative bid/envelope/request admission and independent mixed-peer consumers.
- [ ] Verify cargo check, full affected tests and CI; submit a separate eth2030-target PR.

Heze research is separate from the scheduled Glamsterdam contract. Unsupported vectors are reported, never silently admitted. A stale published release is not substituted for the selected current contract.
