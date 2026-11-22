## Gloas reference-vector source alignment (2026-11-22)

The inherited v1.7.0-beta.4 fork vectors conflict with selected consensus reference2ea9d96a on the fork-slot availability and credential-first independent builder/validator onboarding updates. Production behavior matches the selected source. A separate review is generating source-pinned reference vectors and retaining actual BLS/full-state comparisons rather than reverting or skipping the normative behavior. [Checklist](docs/qualification/gloas-vector-source-alignment-20261122.md). Generation and consumer CI remain pending.

## Glamsterdam adversarial and cross-client qualification (2026-11-19)

- [x] Reproduce and close signed-but-state-inconsistent envelope gossip/cache admission; the remaining R06 lifecycle matrix stays open.
- [ ] R06: Read the selected signed-envelope contract and execute fork-pinned authentication/cache, optimistic FULL/EMPTY/PENDING and late/reorg/restart controls; exclude conditional early returns from evidence.
- [x] Qualify real signed builder bid recording, current/previous/evicted payment windows, quorum edges, slashed-proposer cancellation, exit gates and actual withdrawal debit conservation; registry eviction and full-process supply remain open.
- [ ] R07: Assert actual builder settlement exactly once across late/slashed/no-quorum/exit/eviction cases, including supply and withdrawals.
- [ ] R08: Compare nondefault progressive fork migration and actual proof consumers against independent roots/proofs.
- [ ] R09: Exercise real partial-column group/version/custody peers through loss, duplication and reconnect.
- [ ] R10-R13: Qualify independent full-process clients, builder/fork/recovery/resource campaigns, then publish the compatible artifact manifest and runbook.

## Authorized Glamsterdam integration into eth2030 (2026-11-19)

- [x] Freeze origin/eth2030 and the qualified paired candidate; inspect the actual merge tree for source deletions rather than comparing unrelated heads.
- [x] Merge first-slot/onboarding/real-BLS request changes while retaining existing Heze/Frame paths and all task records.
- [x] Run cargo fmt, cargo check, targeted actual state/chain/history tests and release build; never skip hooks.
- [x] Push a non-forced update directly to origin/eth2030 as authorized, and record exact paired revisions and remaining campaign results.

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
