# Patched compression dependency

- [x] Retain actual hosted cargo-audit failure and verify primary advisory RUSTSEC-2026-0544.
- [ ] Update only libz-sys1.1.31 to reviewed patched1.1.33; preserve protocol/runtime dependencies and existing advisory policy.
- [ ] Run the actual project audit, required native build/check and release fingerprint validation.
- [ ] Publish an independent eth2030-targeted review and verify cumulative merge composition.
