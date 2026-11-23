# Honest EF execution receipts

- [x] Trace report generation: BLS-skipped cases are included in the old passed count; inactive handlers produce no case receipt.
- [x] Inspect the152 selected source-generated Gloas cases: no bls_setting=2 entries (mainnet42+29, minimal44+37).
- [x] Report actual pass/failure/BLS-skip/known-skip counts and crypto mode; label inactive handlers without changing selection or failure policy.
- [x] Add mixed/all-skipped/empty/failing controls; test in the single composed checkout after the current qualification jobs finish.
- [ ] Execute real-crypto EF targets and preserve applicable/unsupported boundaries; submit independent PR targeting eth2030.
