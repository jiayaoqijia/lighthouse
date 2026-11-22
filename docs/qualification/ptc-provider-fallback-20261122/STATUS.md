# PTC provider fallback investigation

The deadline path stops at the first successful HTTP response, including None or negative availability, without querying a configured provider that may already have the authenticated payload/data. Full-simulator traces show one VC publishing a negative payload vote at slot20 while other providers have the timely payload, followed by a canonical slot21 miss. This is a concrete fallback behavior to reproduce; it is not yet a proven explanation of every simulator miss.

Ordinary Gloas attestation timing was checked against the frozen specification: early head events are allowed and same-slot data.index=0 means PENDING, not EMPTY. The service also guards already-initiated slots. Those behaviors must not be disabled merely to make the simulation green.

## Local component qualification

The actual HTTP-client/service counterexample compiled and failed on the previous first-success path. The corrected deadline path tries all configured providers for one complete timely view, then returns a coherent negative response only if none is complete. It never combines flags or roots between providers. Wrong-slot data is rejected, and all-none/all-error results remain distinct. All 21 payload-attestation-service tests passed in 1.90s, including existing signing, SSZ/JSON, early-event and deadline controls. Full simulator and final frozen-source qualification remain pending.
