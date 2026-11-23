# Engine fault injection after payload preparation

Merged-source full CI run45134918940 failed18 payload-invalidation tests, two Heze deferred-verdict tests, and two Fulu optimistic-head API tests. Local unchanged-source selected baselines reproduce PayloadIdUnavailable and NoViablePayloadBid (3/3 selected beacon failures and2/2 API failures).

These cases asked the same mock Engine to build a new payload while its retained FCU response was SYNCING/INVALID. The merged mock correctly returns no payload ID for those statuses. Paris Engine API forkchoiceUpdated rules4,7,8 require a valid head before a build and a null payloadId for SYNCING/INVALID. Reverting that guard would reintroduce the false recovery behavior fixed earlier.

Preparation now uses a working producer before each import's fault response is applied. Invalid-parent and invalid-head recovery fixtures explicitly restore the consumer's SYNCING response after preparation. Heze's child fixture restores SYNCING before import; the original deferred-execution/EMPTY-head, inclusion-satisfaction and proof-relay assertions remain. API fixtures build first, then import with SYNCING newPayload/FCU, assert actual optimistic fork-choice state, and allow the local producer to build the fallback proposal while the CL still classifies its head as optimistic. Original fee recipient, local payload cache and extra-data assertions remain.

No production Engine, fork-choice validation, signature, gas or deadline code changes.Mainnet/minimal presets and complete recent-fork hosted runs remain required; a selected pass is not full CI.

Validation so far: full locked workspace/all-target cargo check;31 selected beacon tests pass, including all original20 failures; both API cases pass. Full Fulu API and hosted matrices are in progress. Negative mock non-VALID FCU/no-ID and genuine peer recovery controls must stay enabled.

References: refs/execution-apis/src/engine/paris.md#engine_forkchoiceUpdatedV1 and frozen consensus-specs2ea9d96a Gloas and separately retained Heze test contracts. This explains the newly observed CI regressions, not the unrelated historical5.5h native EL waiter.

Expanding the matrix reproduced four additional Gloas mainnet invalidation/cache failures, two Gloas minimal envelope cases and one Fulu minimal health case. They used the same non-VALID producer state before fixture preparation. These are corrected with the same preparation/import boundary, preserving all optimistic ancestry, promotion, EMPTY/FULL, health status and invalid-parent assertions. Both selected mainnet fork invocations pass31 framework tests; all three formerly failing minimal active paths now pass. Guard-return cases are not promoted to executed-path coverage.
