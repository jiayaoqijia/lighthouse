# Status

Two unchanged current consensus-specs d2021d27 FCR vectors fail: pending votes assigned again in empty slots are omitted from the parent-support discount, so confirmation remains behind the independent expected root. The sibling mixed-payload-vote case passes. Repair the set of supported parent nodes without mixing FULL and EMPTY branch votes. FCR fixtures explicitly declare synthetic BLS mode and are not real-BLS acceptance.

The native weighted regression now passes for FULL+PENDING, EMPTY+PENDING and the single pre-Gloas node. It excludes the opposite branch, unrelated roots, inactive/slashed validators and equivocators while counting each validator once. All 13 FCR library tests, targeted cargo check, formatting and dependency sorting pass. The complete current three-case independent replay and final composed qualification remain pending.
