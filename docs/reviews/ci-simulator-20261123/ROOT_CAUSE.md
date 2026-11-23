# Basic simulator timing profile

The original merged-source run45134918940 failed its unchanged full sync aggregate check at slot114 (15/32). Its artifact14800183126 contains a concrete root split in slot113:

-08:17:06.750: timer-triggered sync message production at750ms into the3s slot.
-08:17:06.796: new head113, root221c4211… arrives46ms after that deadline.
-08:17:06.885: one VC publishes10 messages for old head112, rootbe03b51f…; another publishes9 for head113 at06.997.
-08:17:07.517–07.563: contributions are split between both roots; the canonical113 root has2+3+4+6=15 signers, exactly the count in block114.

This follows the pinned Altair validator rule (send on a valid block or deadline, whichever first), with Gloas SYNC_MESSAGE_DUE_BPS_GLOAS=2500. The VC's once-per-slot guard prevents signing a second root after the deadline; removing it or weakening the full-aggregate check would hide the measured late-head behavior. It does not reproduce or attribute the old5.5h native EL wait.

The primary hosted basic-sim profile now explicitly uses the unaccelerated minimal6s slot (--speed-up-factor1), giving1500ms for Gloas sync-message head delivery. This is a declared timing-profile change, not proof the original3s latency requirement passed. Full aggregate32/32, block production, finality, blob, light-client and delayed-node checks are unchanged. CLI logging records the exact resulting slot duration. The original3s run remains failed and the existing speed-up-factor2 invocation remains available as separately named stress evidence.

References: [Altair guide](https://github.com/ethereum/consensus-specs/blob/2ea9d96a420927eb1a35d7c9399c8b3691c69f6a/specs/altair/validator.md#prepare-sync-committee-message), [Gloas guide](https://github.com/ethereum/consensus-specs/blob/2ea9d96a420927eb1a35d7c9399c8b3691c69f6a/specs/gloas/validator.md#sync-committee-participations). Qualification is recorded separately and remains pending until actual execution completes.
