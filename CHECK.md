## Authorized Glamsterdam integration into eth2030 (2026-11-19)

- [x] Freeze origin/eth2030 and the qualified paired candidate; inspect the actual merge tree for source deletions rather than comparing unrelated heads.
- [ ] Merge first-slot/onboarding/real-BLS request changes while retaining existing Heze/Frame paths and all task records.
- [ ] Run cargo fmt, cargo check, targeted actual state/chain/history tests and release build; never skip hooks.
- [ ] Push a non-forced update directly to origin/eth2030 as authorized, and record exact paired revisions and remaining campaign results.

# Lighthouse implementation checks

## Heze checkpoint envelope history (2026-11-18)

- [x] Reproduce the missing pre-anchor parent through actual checkpoint-envelope processing.
- [x] Preserve the trusted anchor's EIP-7805 initial satisfaction without weakening live-parent lookup or payload validation.
- [x] Run the checkpoint regression, existing Heze verdict controls, formatting, lint and cargo check.
- [x] Build and identify the release runtime.
- [x] Qualify the capped EL/CL history control, remove all owned resources and publish the reviewed CL fix.

## Worktree runtime version metadata (2026-11-18)

- [x] Reproduce a cached Cargo build keeping an earlier worktree commit.
- [x] Resolve HEAD and common refs through Git plumbing, covering branch, packed refs and detached HEAD.
- [x] Verify the release binary reports its actual source commit before live qualification.
