# Log Targets for `syn_parser`

This document lists the `target` strings used with the `log` crate macros (`debug!`, `info!`, `warn!`, `error!`) within the `syn_parser` crate. Using specific targets allows for fine-grained control over log output during debugging and development.

## Targets

*   **`graph_find`**: Used for logging within search/find methods in `crates/ingest/syn_parser/src/parser/graph.rs`.
*   **`node_id`**: Used for logging within ID generation methods in `crates/ingest/syn_parser/src/parser/visitor/state.rs`.
*   **`mod_tree_vis`**: Used for logging within visibility checking methods (like `is_accessible`) in `crates/ingest/syn_parser/src/parser/module_tree.rs`.

## Usage

To enable logging for specific targets and levels, set the `RUST_LOG` environment variable before running your command (e.g., `cargo test`).

The format is generally `target=level`, or `crate::module=level`. You can specify multiple targets separated by commas.

**Log Levels:** `error`, `warn`, `info`, `debug`, `trace` (most verbose).

**Examples:**

*   Enable `debug` level logs specifically for the `graph_find` target:
    ```bash
    RUST_LOG=graph_find=debug cargo test -q -p syn_parser
    ```
*   Enable `debug` level logs for all targets within the `syn_parser` crate:
    ```bash
    RUST_LOG=syn_parser=debug cargo test -q -p syn_parser
    ```
*   Enable `trace` level logs for the `node_id` target and `debug` for `graph_find`:
    ```bash
    RUST_LOG=node_id=trace,graph_find=debug cargo test -q -p syn_parser
    ```

**Note:** You need a logging implementation initialized in your application or test setup (like `env_logger` or `tracing-subscriber`) for the logs to appear. If logs aren't showing up, ensure a logger is initialized somewhere (often in `main.rs` or test setup functions).
