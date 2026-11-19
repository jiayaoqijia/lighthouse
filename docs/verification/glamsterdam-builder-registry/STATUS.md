# Builder registry lifecycle in progress

The selected authority is consensus-specs 2ea9d96a420927eb1a35d7c9399c8b3691c69f6a, Gloas get_index_for_new_builder, add_builder_to_registry, process_builder_deposit_request, process_builder_exit_request and builder withdrawal sweep. Existing payment tests cover current/previous/evicted payment windows, quorum edges, cancellation and actual debits. This separate qualification adds registry identity and exit/sweep/reuse boundaries using authentic signed deposits and actual state-processing consumers. No new runtime configuration is introduced.
