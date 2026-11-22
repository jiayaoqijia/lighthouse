# PTC availability across configured providers

- [x] Trace failed full-simulator slot20: healthy providers report timely payloads, another returns payload_present=false at the deadline; slot21 later disappears from the canonical chain.
- [x] Verify frozen validator contract: payload_present records reception before payload_due; data.slot must match the assigned slot and message flags must come from a coherent provider response.
- [x] Reproduce first-provider None/negative response suppressing a ready second provider with actual PTC service/HTTP clients.
- [x] Prefer a single complete positive provider response; retain a real negative response only after no configured provider reports complete availability. Do not synthesize flags/root across providers.
- [x] Preserve all-none/all-error behavior, reject wrong-slot responses, and retain existing early-event/deadline signing guards.
- [ ] Run full PTC service controls, complete simulator, native build/lint and submit an independent eth2030-targeted PR.
