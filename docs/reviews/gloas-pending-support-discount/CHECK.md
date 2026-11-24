# Pending-vote empty-slot support discount

- [x] Compare current Gloas fast-confirmation source and independently generated EMPTY/FULL-parent vectors.
- [x] Count the selected parent branch and PENDING node as a set; exclude the opposite branch and preserve validator filters.
- [x] Add weighted native regression controls and pass all 13 FCR library tests.
- [ ] Replay all three current is_one_confirmed vectors from the final composed executable.
- [ ] Run cargo check, complete FCR tests and composed qualification; publish an independent eth2030-target PR.

- [x] Replace old pro-rata maximum committee weight with the current exact distinct participant calculation and retain heterogeneous/cross-epoch/inactive/slashed controls.
- [x] Pass all 196 unchanged current-source FCR vectors and 9,344 SSZ vectors in explicit fixture mode.
- [x] Package source/vector provenance and four installer integrity/publication-rollback checks; keep the real-BLS lane separate.
- [ ] Verify final exact-head CI and composed runtime before merge.
