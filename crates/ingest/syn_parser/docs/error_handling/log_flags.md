# Log Targets for `syn_parser`

This document lists the `target` strings used with the `log` crate macros (`debug!`, `info!`, `warn!`, `error!`) within the `syn_parser` crate. Using specific targets allows for fine-grained control over log output during debugging and development.

## Targets

*   **`graph_find`**: Used for logging within search/find methods in `crates/ingest/syn_parser/src/parser/graph.rs`.
*   **`node_id`**: Used for logging within ID generation methods in `crates/ingest/syn_parser/src/parser/visitor/state.rs`.
