# Compression dependency audit correction

Hosted project audit fails for libz-sys1.1.31 under RUSTSEC-2026-0544. The primary RustSec advisory specifies1.1.33 as patched. This review updates that transitive patch only; it does not add an advisory ignore, change the protocol target, or upgrade unrelated libraries. Actual audit/build outcomes remain pending. Reference: https://rustsec.org/advisories/RUSTSEC-2026-0544.html .

The lockfile changes only libz-sys version/checksum. Required --release --locked cargo check passes; actual cargo-audit0.22.2 exits0 against a freshly CLI-fetched, recorded advisory DB revision, retaining10 existing allowed warnings and the existing single crypto-bigint advisory policy. The first make-audit attempt fails in libgit2's SSH transport and remains failed. No new ignore or disabled check is introduced. Cumulative release build and fork integration remain pending.
