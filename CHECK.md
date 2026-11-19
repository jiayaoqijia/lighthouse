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

## Prior qualified candidate checklist

# Development checklist

## Gloas first-slot payload availability (2026-11-18)

- [x] Compare actual fork/slot call order with consensus-specs 2ea9d96a.
- [x] Reproduce the fork-slot availability mismatch through the real slot transition.
- [x] Clear only the fork slot and verify normal and wrapped ring positions.
- [x] Run all 23 library tests, formatting, full lint-fix and cargo check.
- [x] Publish the isolated first-slot PR (Lighthouse fork PR 1).

## Gloas builder/validator onboarding independence (2026-11-18)

- [x] Read the selected consensus fork specification and compare actual registry/queue routing.
- [x] Reproduce shared-key registration, non-builder queue retention and invalid-signature prefix behavior.
- [x] Implement exact credential-first routing while retaining cached/full verification equivalence.
- [x] Run all 23 library tests, formatting, full workspace lint-fix and cargo check.
- [x] Submit the separate onboarding review (Lighthouse fork PR 2).

## Combined paired-client qualification (2026-11-18)

- [x] Apply only the two owned functional commits to the existing paired-client base f85fa05b6.
- [x] Preserve both regression functions when resolving the shared test-module insertion.
- [x] Pass all 24 library tests, cargo check, formatting and full workspace lint-fix.
- [x] Publish the isolated integration evidence branch.

## EIP-8282 actual CL wire consumer (2026-11-18)

- [x] Pin the same real-BLS SSZ records produced by the native EL Engine regression.
- [x] Decode and verify builder-specific domains and actual registration/top-up/exit processing.
- [x] Verify request-list bytes, authorization, pending-payment gates and unchanged validator registry.
- [x] Run targeted tests, formatting, cargo check and full lint-fix; publish the separate consumer review.
