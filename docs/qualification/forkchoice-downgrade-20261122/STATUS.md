# Snapshot downgrade compatibility correction

Live persistence writes PersistedForkChoiceV30 with a third variable field for unsatisfied inclusion lists. The v29-to-v28 migration decodes only V29 (two variable fields), so all four legitimate pre-Gloas downgrade/readback campaigns fail with OffsetSkipsVariableBytes(12). The current live decoder already supports V30 and legacy V29. Migration must use that decoder and explicitly reject nonempty obligations or Gloas nodes before conversion, instead of dropping fields or weakening the post-fork downgrade restriction. Original failures remain retained.

## Local validation

The original hosted pre-Gloas disk failures decoded a current three-field V30 snapshot using the strict two-field V29 decoder (`OffsetSkipsVariableBytes(12)`). The downgrade now uses the existing current/legacy reader, rejects nonempty unsatisfied inclusion lists before producing writes, and retains the existing V29 proto-node rejection. The unused strict V29 byte reader was removed.

All four Fulu disk configurations passed (3.42s). Each now round-trips both current and legacy snapshot encodings through downgrade/upgrade and resumes the chain, checking finalization and historical reads. A separate snapshot/version-key control passed for outstanding obligations and compressed malformed bytes; neither failure changes either key. Gloas-specific rejection and release cargo check are still being checked.

The complete four Gloas disk cases also passed (3.27s), preserving the pre-existing earlier v31 payload-summary rejection where applicable. A direct v29 Gloas-node atomic rejection control has been added as well. Its compile attempt exposed shared-Cargo cross-worktree dependency metadata from the separate committee branch, not a source trait requirement on this branch. Final composition will clean all workspace package artifacts and rerun both guards, all eight disk configurations and release checks in one checkout; this gate remains pending.
