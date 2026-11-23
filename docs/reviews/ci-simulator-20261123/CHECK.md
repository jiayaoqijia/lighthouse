# Simulator timing qualification

- [x] Retain original hosted simulator artifact and identify actual root split at slot113.
- [x] Read pinned Altair/Gloas message deadlines; never allow non-VALID Engine builds.
- [x] Qualify an explicit full-slot CI profile with unchanged full sync aggregate, finality, blobs, light-client and delayed-join checks.
- [x] Retain accelerated3s profile as separately named stress evidence, not a passed budget.
- [x] Verify cargo check/local simulator and submit a separate eth2030 PR.

The actual old3s profile remains failed; local and hosted full6s checks pass. The branch is updated with merged13/14 prerequisites; locked workspace/all-target compilation passes on the composition. Final composed hosted CI is required before claiming that gate complete.
