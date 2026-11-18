# Development checklist

## Gloas first-slot payload availability (2026-11-18)

- [x] Compare actual fork/slot call order with consensus-specs 2ea9d96a.
- [x] Reproduce the fork-slot availability mismatch through the real slot transition.
- [x] Clear only the fork slot and verify normal and wrapped ring positions.
- [x] Run all 23 library tests, formatting, full lint-fix and cargo check.
- [ ] Publish the isolated first-slot PR.

## Gloas builder/validator onboarding independence (2026-11-18)

- [x] Read the selected consensus fork specification and compare actual registry/queue routing.
- [x] Reproduce shared-key registration, non-builder queue retention and invalid-signature prefix behavior.
- [x] Implement exact credential-first routing while retaining cached/full verification equivalence.
- [x] Run all 23 library tests, formatting, full workspace lint-fix and cargo check.
- [ ] Submit the separate onboarding review.
