# Verified checkpoint-proof cache import

- [x] Retain the independently authenticated original checkpoint and peer bootstrap; reproduce unavailable local immediate post-state without aliasing by slot.
- [x] Add an offline database command that verifies a Gloas bootstrap against the exact local block/header, local period committee and Merkle state commitment before writing only its proof cache entry.
- [x] Test valid/idempotent import and wrong root/header/committee/proof rejection without cache mutation; run workspace check, affected release tests, CLI documentation generation and release build.
- [x] Repair the owned original cache through the command, verify the same original request with the unchanged independent reference consumer, and integrate into eth2030.
- [x] Document local-state backfill versus verified-peer recovery and preserve the original failure boundaries.
