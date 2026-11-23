# Source-vector and simulator qualification

## Bound discovery socket follow-up

The first ephemeral-listener correction omitted discovery UDP from the boot ENR. Complete simulator logs reported `ENR has no compatible UDP socket to connect to`. The simulator now binds UDP once, retains that exact socket via discv5 0.12 FromSockets and advertises its nonzero port before startup. TCP/QUIC remain OS-assigned. A real UDP/retained-ownership regression and the full simulator are being rerun; this routing defect is not yet a proven explanation of every canonical miss.

The real UDP ownership/receive test passed, including a refused second bind and successful packet reception on the advertised held socket. `cargo check --release -p simulator` also passed. Complete default fallback simulation before this follow-up (source 7eff0951, including the PTC fix) still failed: 125/129 canonical blocks, missing slots33/35/94/115, head91.00%. Slot21 was present, but this does not by itself prove all cross-run causality. The unchanged full simulator must pass with the bound-discovery source before acceptance.
