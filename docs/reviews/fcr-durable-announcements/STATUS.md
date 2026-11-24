# Status

Local FCR crash durability work is complete. An announced root is persisted atomically with a compatible fork-choice snapshot before cached-head and Engine publication. The write runs on the existing blocking worker after releasing fork-choice/FCR locks. Persistence failure conservatively announces finalized without poisoning consensus; the same head/slot can retry after the failure.

Both Fulu and Gloas complete restart matrices pass 22/22 tests with real blst/supranational and no fake_crypto. They cover crash/graceful shutdown, stalls, reduced participation, age/reorg rejection, long downtime, Engine safe publication and a one-shot checkpoint-write failure with atomic rejection and continued block import. Actual lighthouse binary cargo check and project-policy Clippy pass. VALIDATION.json pins the source blobs and exact runs.

The baseline crash still fails with the corrected signed-block lookup fixture (slot 43 drops to 39), proving the original invariant was not waived. An intermediate fixture failed MissingCustodyColumns because a range-sync helper combined an already-stored crash envelope with absent columns; the test now replays the signed lookup block and explicitly verifies/imports its supplied envelope. Earlier 20-pass/1-ignore and intermediate failures remain distinct from the final 22-pass/no-ignore runs.

The companion PR targets eth2030 under the user's instruction. Hosted CI and integration are pending at this local snapshot; no EL master merge is performed. This newly reproduced durability gap does not resolve the unrelated historical original-04 waiter attribution. No actual 48h/larger-scale pass or upstream Lodestar approval is claimed.

Final failure-path follow-up also retains the previously sent high-water root and resets the slot/delay gauges to the selected finalized fallback. Root-change metrics are counted after persistence/fallback selection, not for an unsent candidate. The explicit high-water regression and both 22-case matrices, actual binary check and project-policy lint pass again. Each changed announcement adds one atomic fork-choice checkpoint; capacity beyond the bounded acceptance remains separately qualified.
