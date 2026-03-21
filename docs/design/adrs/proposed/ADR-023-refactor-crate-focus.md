# Draft ADR: Replace `crate_focus`-centric runtime state with explicit loaded crate/workspace state

Evaluated against git `a4f139ba` on 2026-03-21.

## Status
Proposed

## Context
`ploke-tui` historically used `SystemState.crate_focus` as a central runtime field. In the current codebase, that concept has already partially shifted: `SystemStatus` now stores both `loaded_workspace` and `crate_focus`, where `crate_focus` is an `Option<CrateId>` and the focused root path is derived through workspace membership rather than stored directly as a path.

Even with that improvement, the focused-crate concept still carries multiple meanings in practice:

- anchor path for command and indexing resolution
- representation of what project/crate is loaded
- focus for search and graph interpretation
- backup/save target context

These meanings are not equivalent.

This ambiguity became more problematic as workspace-aware parsing and indexing were introduced. A single focused-crate field does not faithfully represent a loaded workspace with multiple member crates. It also encourages incorrect behavior where relative command resolution may drift to process cwd or other incidental context instead of using explicit application state.

The database already serves as the primary source of truth for loaded semantic content:
- parsed code items
- relations
- persisted workspace metadata
- parsed Cargo manifest data

Any runtime state that overlaps with persisted semantic content should be auxiliary and validated against the database rather than treated as an independent authority.

## Decision
`SystemState.crate_focus` will be retired as the primary representation of loaded project state.

Runtime state will instead distinguish between:

- loaded workspace context
- loaded crate runtime/cache state
- ephemeral analysis focus passed as function parameters, not stored as durable system state

The durable application runtime model will move toward:

- `loaded_workspace: Option<LoadedWorkspaceState>`
- `loaded_crates: BTreeMap<PathBuf, LoadedCrateState>`

Where:

- `LoadedWorkspaceState` represents the currently loaded workspace environment, if any.
- `LoadedCrateState` represents runtime-only crate data used for operational efficiency and incremental update support.
- Cozo remains the source of truth for semantic crate/workspace contents.
- Any overlap between runtime state and persisted DB state must be treated as derived/cache data and validated against DB truth where appropriate.

`LoadedCrateState` is explicitly not a second semantic source of truth. It is intended for runtime-adjacent data such as:

- file hash state
- per-file / future per-item update support structures
- parser-to-database bridge artifacts
- transient mappings needed for efficient updates or reconciliation

Analysis focus will not be stored as a field in `SystemState`. Functions that need a focal crate for search, visibility interpretation, or graph traversal must receive that focus explicitly as a parameter.

This ADR does not treat the current `crate_focus: Option<CrateId>` shape as the end state. It is an intermediate representation that is already better than a raw `Option<PathBuf>`, but it still overloads focused-crate identity with other runtime concerns that should be modeled separately.

## Command/API direction
Indexing commands should use strong target typing rather than a generic path-only target.

The command model should move toward distinct operations such as:

- `IndexCrate`
- `IndexWorkspace`

These commands should carry strongly typed target descriptors rather than raw `PathBuf` values wherever possible, so that developer intent is explicit at the API boundary. User input may still begin as a path or command with no explicit path, but resolution from that input should produce a typed indexing target before the core indexing workflow executes.

Resolution from user input into a typed indexing target may still involve path handling, but path interpretation should happen before the core indexing command is formed.

## Consequences
### Positive
- Eliminates ambiguity around `crate_focus`
- Makes workspace-aware state first-class
- Aligns runtime state with actual application operating context
- Keeps semantic authority in Cozo
- Improves safety of indexing and restore flows by using explicit target kinds
- Prevents search/analysis semantics from being conflated with app-global state
- Aligns the runtime model with the current direction already visible in `SystemStatus`, where focused root is derived through workspace membership rather than stored directly as a raw path

### Negative
- Requires refactoring existing `crate_focus` call sites
- Requires auditing indexing, save/restore, and path resolution behavior
- Introduces additional runtime structures that must be kept coherent with DB state

### Constraints / invariants
- Cozo is authoritative for loaded semantic data
- `LoadedCrateState` must not become a shadow semantic store
- If `loaded_workspace` is `Some`, all loaded crates in runtime state must be coherent with that workspace
- Search/retrieval code must not infer analysis focus from global runtime state
- Any runtime overlap with persisted crate/workspace metadata must be treated as cache or operational state and validated against DB state when reused across operations

## Near-term implementation guidance
1. Audit current `crate_focus` usages.
2. Classify each usage as one of:
   - loaded environment state
   - command/path anchor
   - analysis/search focus
   - backup/save context
3. Introduce `loaded_crates` and keep `loaded_workspace` as the environment-level state.
4. Update indexing commands toward explicit crate/workspace targets.
5. Move search focus to explicit parameters.
6. Remove or rename `crate_focus` only after its remaining usages are eliminated or reclassified.

## Open questions
- Should relative command resolution use a dedicated persisted runtime anchor, or should it always be derived from loaded workspace / loaded crate state?
- Should runtime state permit multiple standalone loaded crates outside a workspace, or remain single-environment for now?
- What exact typed target model should be used for `IndexCrate` and `IndexWorkspace`?

## Current code notes
- `SystemStatus` is currently defined in [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs). At git `a4f139ba`, it contains `loaded_workspace: Option<LoadedWorkspaceState>` and `crate_focus: Option<CrateId>`, along with versioning, dependency, and parse-failure tracking.
- `focused_crate_root()` and `focused_crate_name()` are derived accessors in [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs), not direct stored fields.
- `set_loaded_workspace(...)` and `set_focus_from_root(...)` in [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs) already treat loaded workspace membership as the underlying authority for focus.
- DB restore currently writes runtime state through [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs), using `workspace_metadata` when present and otherwise falling back to `set_focus_from_root(...)`.
- Indexing currently still enters through `StateCommand::IndexWorkspace` and `handlers::indexing::index_workspace(...)` in [indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs), which is part of why target-kind semantics remain blurry.
- `crate_focus` usage is still spread across tooling and DB flows:
  - tool path scoping in [create_file.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/create_file.rs), [code_edit.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs), and [rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs)
  - incremental scan and save flows in [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs)
  - stale-state checks and path policy derivation in [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs)

## References
- [core.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/core.rs)
- [database.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/database.rs)
- [indexing.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app_state/handlers/indexing.rs)
- [create_file.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/create_file.rs)
- [code_edit.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_edit.rs)
- [rag/tools.rs](/home/brasides/code/ploke/crates/ploke-tui/src/rag/tools.rs)
