# Actual progressive checkpoint proof consumers

- [x] Read selected light-client/EIP-7688 rules and trace real finalized-bootstrap failures through production pruning and proof consumers.
- [x] Reproduce checkpoint proof loss when the epoch-boundary block is skipped, preserving the actual failed HTTP evidence.
- [ ] Retain needed proof branches using canonical block intervals; verify actual finalized bootstrap and independent nondefault progressive state/header proofs.
- [ ] Run meaningful controls, affected release tests, cargo check and production release build, then integrate directly into eth2030 as authorized.
- [ ] Roll the exact qualified CL artifact into the owned campaign and verify actual cross-client consumers and recovery.
