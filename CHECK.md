# Lighthouse implementation checks

## Heze checkpoint envelope history (2026-11-18)

- [x] Reproduce the missing pre-anchor parent through actual checkpoint-envelope processing.
- [x] Preserve the trusted anchor's EIP-7805 initial satisfaction without weakening live-parent lookup or payload validation.
- [x] Run the checkpoint regression, existing Heze verdict controls, formatting, lint and cargo check.
- [ ] Build and identify the release runtime.
- [ ] Qualify the capped EL/CL history control, remove all owned resources and publish the reviewed CL fix.
