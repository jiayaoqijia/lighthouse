# url dependency audit correction

The merged34cecfc4 hosted check-code job failed audit-CI on url2.5.8 / RUSTSEC-2026-0592. Raise the existing workspace2.x lower bound to2.5.9 and refresh only url/form_urlencoded lock entries. No Engine API, consensus settings, signature or validation behavior changes; no ignore is added.

[Official advisory](https://rustsec.org/advisories/RUSTSEC-2026-0592.html) and [release](https://github.com/servo/rust-url/releases/tag/v2.5.9) confirm the patched range. The unchanged make audit-CI command encountered a local advisory transport failure. A fresh official advisory-db clone at8b75c83c3c9142dd18e93d75b8d895a13d285861 gives a causal comparison with the existing ignore only: original lock reports one vulnerability; patched lock reports zero. Both use no-fetch against that fresh snapshot, without stale/yanked suppression. Warnings remain visible.

cargo check --locked -p sensitive_url passes against the patched graph. An attempted cargo test -p sensitive_url was rejected because this external dependency is not a workspace member; that is not a passing test. Full hosted check-code/build and composed regressions are still to be checked. This dependency correction does not close the other CI failures or historical EL incident.
