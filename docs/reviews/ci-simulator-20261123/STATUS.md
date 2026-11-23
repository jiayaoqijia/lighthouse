# Simulator profile status

Original45134918940 failed: slot113 head arrived at796ms after750ms message deadline, splitting sync signatures between two roots; block114 included15/32. Investigation and declared-profile qualification are in progress. No original5.5h EL root attribution follows.

The primary CI command explicitly selects unaccelerated minimal6s slots. Exact computed duration is logged, and every original full aggregate/finality/blob/light-client/delayed-node assertion stays unchanged. cargo fmt and cargo check --locked -p simulator pass. A real local full-slot simulation is building/running; it is not yet a passed result.

The actual local full6s minimal simulation completed successfully through slot128 with4 beacon nodes and2 VCs, including the delayed join. All original checks passed. Three attestation-publication endpoint timeout lines remain in the log; this is not a warning-free claim. The original3s failure remains failed. LOCAL_SIMULATION.json retains exact source/command/log identities; hosted final-source CI remains separate.

The hosted basic-simulator job137589755049 passes the explicit full6s profile. Merged dependency and payload-fixture prerequisites are incorporated without conflicts and the combined locked workspace/all-target check passes. This branch targets eth2030; final-head hosted results stay separate from older component runs.

The same CI correction now supplies the standard GITHUB_TOKEN for cargo-binstall to avoid the measured anonymous release-API retry delay. This reuses the existing ephemeral job token and changes no validation/resource budget. Full final-head CI is checked after push.
