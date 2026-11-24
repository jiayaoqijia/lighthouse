# Progressive partial-column cell bitmaps

- [x] Read current d2021d27 Gloas partial-column schema and EIP-7916 conversion; reproduce independent root mismatches in both presets.
- [x] Use fork-correct native ProgressiveBitList for Gloas partial sidecars, metadata, constructors and consumers, preserving Fulu bounded semantics.
- [x] Pass six independent Fulu/Gloas goldens and all 14 partial-sidecar unit tests, including empty and multi-chunk bitmaps.
- [ ] Replay every final emitted partial sidecar/metadata static case.
- [ ] Verify native partial/transport/actual consumer tests, cargo check/build and publish an independent eth2030-target PR.
