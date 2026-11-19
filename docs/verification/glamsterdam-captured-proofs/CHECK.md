# Actual nondefault progressive-state proof parity

- [x] Capture the same actual 384-validator Gloas SSZ state from three running CLs and independently verify the full state root.
- [x] Generate current/next/finalized proof branches with the unchanged selected Python SSZ/spec implementation and reject mutated siblings.
- [ ] Compare Rust decoding, full state root, exact SSZ readback and all three complete branches with the captured independent vectors; reject changed leaves/siblings/indices.
- [ ] Run affected release type tests, workspace check and qualified byte checks; integrate the source and public bounded inputs into eth2030.
