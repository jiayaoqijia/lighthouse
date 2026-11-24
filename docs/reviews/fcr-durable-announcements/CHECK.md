# FCR durable announcements

- [x] Read current phase0/Gloas FCR and restart contracts at consensus-specs d2021d27.
- [x] Reproduce ignored crash test: announced slot 43 restores slot 39 after epoch-snapshot-only recovery.
- [ ] Persist announced roots with a compatible fork-choice snapshot before publishing them, outside fork-choice/FCR locks.
- [ ] Preserve consensus progress and conservative finalized fallback on persistence failure.
- [ ] Enable the real crash regression and rerun all FCR/restart cases with real BLS, cargo check and project lint.
- [ ] Submit separately to eth2030 and verify hosted CI before the already-authorized companion merge.
