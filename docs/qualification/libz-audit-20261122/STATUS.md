# Compression dependency audit correction

Hosted project audit fails for libz-sys1.1.31 under RUSTSEC-2026-0544. The primary RustSec advisory specifies1.1.33 as patched. This review updates that transitive patch only; it does not add an advisory ignore, change the protocol target, or upgrade unrelated libraries. Actual audit/build outcomes remain pending. Reference: https://rustsec.org/advisories/RUSTSEC-2026-0544.html .
