# 2026-05-15 ploke-tui create_file focused-root path drift

Status: active; root cause suspected, not fixed.

## Summary

During Prototype 1 broad headless-TUI campaign
`p1-broad-harness-grok4fast-20260515-1`, slot
`node-8ca60d9fab09f48e-r2` applied one valid edit and then attempted to create
`crates/ploke-protocol/src/performance.rs`.

The model supplied a workspace-relative path, but `ploke-tui` resolved it under
the focused crate root:

```text
<workspace>/crates/ploke-protocol/crates/ploke-protocol/src/performance.rs
```

The nested path did not exist, so the create-file proposal failed. A following
non-semantic patch to `crates/ploke-protocol/src/lib.rs` failed with the same
nested-root shape.

## Impact

- Workspace-relative file paths can become invalid after an earlier edit in the
  focused crate.
- Tool output can mask the wrong base by displaying a shorter relative path,
  leaving the model with inconsistent path semantics.
- A broad harness slot can report or reason about a candidate path that was not
  actually staged.
- Slots that already applied a useful edit can keep tool-calling into failures
  or timeouts, reducing admitted child count.

The one admitted child from the campaign, commit `d8f61fcd`, only contains the
actual git diff in `crates/ploke-protocol/src/core.rs`: it adds
`EvidencePolicy::new()` and `EvidencePolicy::is_empty()`. The failed
`performance.rs` path did not enter the committed patch.

## Evidence

The harness setup did load the workspace root and focused crate separately:

```text
workspace root:
/home/brasides/.ploke-eval/campaigns/p1-broad-harness-grok4fast-20260515-1/prototype1/workspaces/edit-harness/node-8ca60d9fab09f48e-r2

focused root:
/home/brasides/.ploke-eval/campaigns/p1-broad-harness-grok4fast-20260515-1/prototype1/workspaces/edit-harness/node-8ca60d9fab09f48e-r2/crates/ploke-protocol
```

The slot first applied a semantic edit to `crates/ploke-protocol/src/core.rs`.
After that applied edit, the model called `create_file` with:

```text
crates/ploke-protocol/src/performance.rs
```

The proposal path became:

```text
crates/ploke-protocol/crates/ploke-protocol/src/performance.rs
```

relative to the workspace root, which means the file tool was effectively
joining the model-supplied workspace-relative path against the focused crate.

## Suspected Chain

1. `ploke-eval` prepares the sparse workspace and calls
   `set_loaded_workspace(workspace_root, member_roots, Some(focused_root))`.
2. The model applies an edit inside the focused crate.
3. The edit/update path emits a `ReIndex` for the loaded crate.
4. The reindex/index-target path may refresh loaded state with the member root
   as the active environment root.
5. `create_file` asks `tool_path_context()` for the primary root and resolves a
   workspace-qualified relative path under the focused crate.

This chain is not yet proven by a focused test, but it matches the recorded tool
artifact shape.

## Current Policy Decision

Filesystem tools should interpret model-supplied relative paths against the
loaded workspace root whenever a workspace is loaded.

Focused crate state may be used for semantic search, graph ranking, future Rust
visibility analysis, or similar context-sensitive lookup. It must not be the
implicit base for filesystem path joining or model-facing path display.

If crate-local path semantics are needed later, they should be explicit, for
example through a separate crate identifier plus path field. Bare relative paths
shown to the model should remain workspace-root-relative.

## Code Anchors

- `crates/ploke-eval/src/runner.rs`: broad harness workspace preparation and
  `set_loaded_workspace(...)`.
- `crates/ploke-tui/src/app_state/core.rs`: `tool_path_context()`,
  `loaded_workspace_root()`, and `focused_crate_root()`.
- `crates/ploke-tui/src/utils/path_scoping.rs`: relative path resolution against
  a primary root.
- `crates/ploke-tui/src/tools/create_file.rs`: create-file path resolution and
  display-path construction.
- `crates/ploke-tui/src/app_state/database.rs`: post-edit `ReIndex` emission.
- `crates/ploke-tui/src/app/events.rs`: `ReIndex` to index-target command
  mapping.
- `crates/ploke-tui/src/app_state/handlers/indexing.rs`: indexing handler that
  refreshes loaded workspace state.

## Repro Test Shape

Add a canary that constructs a loaded workspace with a focused member, then:

1. resolves `crates/<member>/src/new.rs` through the file-tool path context;
2. applies or simulates an edit in the focused member;
3. runs the same reindex path used after an applied edit;
4. resolves `crates/<member>/src/new.rs` again;
5. asserts both resolutions land under the workspace root, not under
   `<workspace>/crates/<member>/crates/<member>/...`.

The same test family should cover `create_file`, patch tools, and displayed
paths so the model sees the same path vocabulary it can safely reuse.

## Related Boundary

This bug is separate from the adapter admission boundary observed in the same
campaign: if an edit is applied but the model keeps tool-calling until timeout,
the headless adapter can fail to write a submitted broad-harness result, leaving
the dirty workspace unadmitted. Both issues reduce admitted child count, but this
report is specifically about path-root drift between workspace and focused crate.
