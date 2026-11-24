# Progressive partial-column cell bitmaps

- [x] Read current d2021d27 Gloas partial-column schema and EIP-7916 conversion; reproduce independent root mismatches in both presets.
- [ ] Use fork-correct native ProgressiveBitList for Gloas partial sidecars, metadata, constructors and consumers, preserving Fulu bounded semantics.
- [ ] Replay all independent partial sidecar/metadata SSZ bytes, values and roots, including empty and multi-chunk bitmaps.
- [ ] Verify native partial/transport/actual consumer tests, cargo check/build and publish an independent eth2030-target PR.
