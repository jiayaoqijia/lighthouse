# Status

Implementing a confirmed Gloas admission gap from the current independent consensus vectors. The existing envelope and payload-attestation verification explicitly conflate an unknown block with a known consensus-failed block. The fix must record only authenticated definitive failures; a corrupt outer signature, unavailable parent or transient engine/store error must never poison the valid unsigned block root. This is independent of the new EF snapshot runner and does not change the official expected outcomes.
