# Gloas balance-backed gossip bid allowance

- [x] Read current Gloas gossip bid validation and pending-withdrawal-aware limit.
- [x] Reproduce rejection of the second permitted signed bid from a 2-ETH builder.
- [x] Implement per-slot/parent/builder counters, authoritative available balance and atomic admission.
- [x] Verify pending withdrawals, zero allowance, parent/slot isolation, pruning, bad signatures and concurrent races.
- [ ] Run cargo check, targeted real-BLS Gloas tests and the existing explicit Heze rejection control and formatting hooks.
- [ ] Submit to eth2030, verify CI and merge under the existing Lighthouse authorization.

- [x] Reproduce and repair the missing MAXIMUM_GOSSIP_CLOCK_DISPARITY bid window with signed inside/outside boundary controls.
