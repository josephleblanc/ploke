No correctness regressions found in the scoped `ploke-db` type-graph/database/index changes.

I checked the current tree and the typed type-graph test set passes under the feature gate.

Open questions / gaps:
- `crates/ploke-db/src/type_graph/fixed_rules.rs:59-88` deduplicates visited terminals by `terminal_type_id` only. There is no fixture here that exercises a shared-substructure DAG or alternate-depth paths to the same terminal, so that edge is still unproven.
- `crates/ploke-db/src/type_graph.rs:561-679` sorts reachability rows without `type_use_id`. Rows that differ only by `type_use_id` can still tie on ordering.

