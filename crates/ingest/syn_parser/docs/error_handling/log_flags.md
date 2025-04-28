# Log Targets for `syn_parser`

This document lists the `target` strings used with the `log` crate macros (`debug!`, `info!`, `warn!`, `error!`) within the `syn_parser` crate. Using specific targets allows for fine-grained control over log output during debugging and development.

## Targets

*   **`mod_tree_vis`**: Logs visibility checking logic within `ModuleTree`, particularly the `is_accessible_from` function and related helpers. Useful for debugging why an item is considered visible or not from a specific module.
    *   Example: `RUST_LOG=mod_tree_vis=debug cargo test -p syn_parser`
*   **`mod_tree_build`**: Logs events during the initial construction and population of the `ModuleTree`, such as inserting modules and handling duplicates.
    *   Example: `RUST_LOG=mod_tree_build=debug cargo test -p syn_parser`
*   **`mod_tree_path`**: Logs the processing and resolution of `#[path]` attributes on module declarations. Essential for debugging issues related to custom module file locations.
    *   Example: `RUST_LOG=mod_tree_path=debug cargo test -p syn_parser`
*   **`mod_tree_cfgs`**: Logs the handling of conditional compilation flags (`cfg`) associated with modules, especially when relevant to path resolution or visibility (though currently less used).
    *   Example: `RUST_LOG=mod_tree_cfgs=debug cargo test -p syn_parser`
*   **`mod_tree_bfs`**: Logs steps during the Breadth-First Search (BFS) used in `shortest_public_path` calculations within `ModuleTree`. Helps trace how the shortest path is found.
    *   Example: `RUST_LOG=mod_tree_bfs=debug cargo test -p syn_parser -- --nocapture` (Use `--nocapture` to see logs during tests)

## Usage

To enable logging for specific targets and levels, set the `RUST_LOG` environment variable before running your command (e.g., `cargo test`).

The format is generally `target=level`, or `crate::module=level`. You can specify multiple targets separated by commas.

**Log Levels:** `error`, `warn`, `info`, `debug`, `trace` (most verbose).

**Examples:**

*   Enable `debug` level logs specifically for visibility checks:
    ```bash
    RUST_LOG=mod_tree_vis=debug cargo test -p syn_parser
    ```
*   Enable `trace` level logs (very verbose) for path attribute resolution:
    ```bash
    RUST_LOG=mod_tree_path=trace cargo test -p syn_parser
    ```
*   Enable `debug` logs for both building the tree and BFS path finding:
    ```bash
    RUST_LOG=mod_tree_build=debug,mod_tree_bfs=debug cargo test -p syn_parser
    ```
*   Enable `debug` level logs for all `syn_parser` targets:
    ```bash
    RUST_LOG=syn_parser=debug cargo test -p syn_parser
    ```
*   Enable `debug` for `mod_tree_path` and `trace` for `mod_tree_vis`:
    ```bash
    RUST_LOG=mod_tree_path=debug,mod_tree_vis=trace cargo test -p syn_parser
    ```

**Note:** You need a logging implementation initialized in your application or test setup (like `env_logger` or `tracing-subscriber`) for the logs to appear. If logs aren't showing up, ensure a logger is initialized somewhere (often in `main.rs` or test setup functions). For tests, using `env_logger::init()` or `env_logger::try_init()` in a test setup function or directly within a test is common. Remember to run tests with `-- --nocapture` to see the output.
