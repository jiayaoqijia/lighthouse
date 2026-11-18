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
