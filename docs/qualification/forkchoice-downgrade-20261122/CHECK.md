# Fork-choice snapshot downgrade compatibility

- [x] Retain four actual hosted pre-Gloas round-trip failures: v29-to-v28 decoder reports OffsetSkipsVariableBytes(12).
- [x] Trace current live PersistedForkChoiceV30 snapshots versus migration's strict V29-only decoder.
- [ ] Accept current/legacy supported compressed snapshot encodings before lossless conversion.
- [ ] Reject post-Gloas nodes and any nonempty inclusion-list obligations before producing migration writes; preserve malformed/IO errors.
- [ ] Run all four pre-Gloas disk downgrade/upgrade/readback controls, post-Gloas rejection and atomic unchanged-state guards.
- [ ] Build/lint and submit an independent eth2030-targeted review.
