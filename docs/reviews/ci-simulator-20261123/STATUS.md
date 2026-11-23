# Simulator profile status

Original45134918940 failed: slot113 head arrived at796ms after750ms message deadline, splitting sync signatures between two roots; block114 included15/32. Investigation and declared-profile qualification are in progress. No original5.5h EL root attribution follows.

The primary CI command explicitly selects unaccelerated minimal6s slots. Exact computed duration is logged, and every original full aggregate/finality/blob/light-client/delayed-node assertion stays unchanged. cargo fmt and cargo check --locked -p simulator pass. A real local full-slot simulation is building/running; it is not yet a passed result.
