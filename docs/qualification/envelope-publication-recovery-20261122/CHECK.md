# Standard envelope publication and recovery checks

- [x] Preserve the complete failed fallback simulator, actual missing canonical slots and all published envelope records.
- [x] Identify independent stateful multi-BN fetch/publish provider mismatch and blob-cache take-before-validation side effects.
- [ ] Read the pinned Gloas validator/Beacon API contract and reproduce cache loss/provider mismatch with actual consumers.
- [ ] Preserve blob availability across rejected/retried submissions; bind stateful publication to the provider that supplied the envelope.
- [ ] Diagnose the simulator's canonical missing slots independently: all six missing-slot envelopes were actually published, so provider/cache defects alone do not establish that failure's root cause.
- [ ] Run real BLS/header/payload/column validation controls, full simulator, required build and regression gates; publish an independent eth2030-targeted PR.
