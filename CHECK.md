# Development checklist

## EIP-8282 actual CL wire consumer (2026-11-18)

- [x] Pin the same real-BLS SSZ records produced by the native EL Engine regression.
- [x] Decode and verify builder-specific domains and actual registration/top-up/exit processing.
- [x] Verify request-list bytes, authorization, pending-payment gates and unchanged validator registry.
- [x] Run targeted tests, formatting, cargo check and full lint-fix; publish the separate consumer review.
