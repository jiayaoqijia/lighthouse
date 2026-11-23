# Payload preparation CI repair status

Original selected failures reproduced. After repair31 beacon invalidation/deferred-verdict tests and both optimistic-head API tests pass with original outcome assertions retained. Locked workspace/all-target cargo check passes. Full Fulu API and hosted profiles are pending. Production protocol code is unchanged; original04/48h boundaries remain open.

The complete local Fulu mainnet API/library invocation now passes216/216,0 skipped, including the105 cases previously left unrun by fail-fast. This used source59778dc9 before merging the separately verified url correction; generated binary identity is not relabelled to the new merge. Locked workspace/all-target recheck and Gloas controls on the combined source are being verified.

Merged target URL correction without conflicts; the combined locked workspace/all-target check also passes. The next PR head starts a fresh hosted matrix on that composition. Earlier source-specific216-pass/API and31-pass/beacon receipts are retained without substituting for the new hosted result.
