# Ingestion Pipeline Variant Catalog

- date: 2026-03-27
- task title: Catalog ingestion pipeline variants
- task description: Document current ingestion variants with trigger, stage order, and outputs.
- related planning files:
  - `/home/brasides/code/ploke/.cursor/plans/ingestion-pipeline-comparison-overview_88f15a9b.plan.md`
  - `/home/brasides/code/ploke/.cursor/plans/compilation-unit-slices-and-db-masks_874f1391.plan.md`

## Variant Catalog

### 1) Classic crate ingest (single selected root)

- Trigger:
  - `syn_parser::try_run_phases_and_merge` (or `try_run_phases_and_merge_with_target`) from crate ingest paths.
  - Used by TUI crate indexing when `PLOKE_COMPILATION_UNION` is disabled.
- Stage order:
  1. Parse+resolve: `try_run_phases_and_resolve_with_target`
  2. Root partition/select: `ParsedCodeGraph::partition_by_selected_roots` -> `select_default_root_path`
  3. Merge selected root: `merge_for_root`
  4. Build/prune module tree for selected root: `build_tree_and_prune_for_root_path`
  5. Transform merged graph: `ploke_transform::transform_parsed_graph`
- Outputs:
  - Parser output:
    - `merged_graph: Some(ParsedCodeGraph)`
    - `module_tree: Some(ModuleTree)`
    - `parsed_graphs_for_masks: None`
    - `compilation_units: None`
  - DB artifacts:
    - Core graph-derived node/edge relations and `crate_context` rows (via `transform_parsed_graph`).

### 2) Union+CU crate ingest (all target roots + structural masks)

- Trigger:
  - `syn_parser::try_run_phases_union_for_crate_with_dimensions` (or `_for_crate` default dims).
  - Followed by `ploke_transform::transform_union_crate_and_structural_masks`.
  - Used by TUI crate indexing when `PLOKE_COMPILATION_UNION` is enabled and by `xtask profile-ingest --compilation-unions`.
- Stage order:
  1. Parse+resolve all discovered roots: `try_run_phases_and_resolve_with_target(..., None)`
  2. Enumerate CU keys (requested dimensions): `enumerate_compilation_unit_keys`
  3. Union-merge and tree prune: `merge_union_graph_and_prune_tree`
  4. Transform merged union graph once: `transform_parsed_graph`
  5. Build structural CU slices: `build_structural_compilation_unit_slices`
  6. Per-CU root merge + cfg refinement + persist:
     - `partition_by_selected_roots` -> `merge_for_root`
     - `filter_structural_slice_by_cfg`
     - `insert_structural_compilation_unit_slice`
- Outputs:
  - Parser output:
    - `merged_graph: Some(ParsedCodeGraph)` (union merged)
    - `module_tree: Some(ModuleTree)` (union-pruned tree)
    - `parsed_graphs_for_masks: Some(Vec<ParsedCodeGraph>)`
    - `compilation_units: Some(Vec<CompilationUnitKey>)`
  - DB artifacts:
    - Normal graph relations from one merged transform.
    - Structural CU mask relations (`compilation_unit_*`) per enumerated compilation unit.

### 3) Workspace ingest (workspace metadata + per-member crate transforms)

- Trigger:
  - `syn_parser::parse_workspace` (or `parse_workspace_with_config`) -> `ploke_transform::transform_parsed_workspace`.
  - Used by TUI when the resolved target is a workspace; also available from xtask parse workspace commands.
- Stage order:
  1. Read workspace manifest: `try_parse_manifest`
  2. Validate selected members (optional selection/config path)
  3. For each selected member: `try_run_phases_and_merge_with_target`
  4. Build `ParsedWorkspace { workspace, crates }`
  5. Transform workspace metadata row: `transform_workspace_metadata`
  6. For each parsed crate: extract merged graph + module tree and run `transform_parsed_graph`
- Outputs:
  - Parser output:
    - `ParsedWorkspace` with workspace metadata section plus per-member `ParsedCrate`.
  - DB artifacts:
    - `workspace_metadata` row (root path, members, resolver, exclude, package_version).
    - Per-member crate graph relations and crate_context rows.

### 4) TUI integrated indexing path (parse/transform + embedding index)

- Trigger:
  - `ploke_tui::app_state::handlers::indexing::index_workspace(..., needs_parse = true)`.
  - Calls `run_parse_resolved`, then launches embed indexer task.
- Stage order:
  1. Resolve target kind/path: `resolve_index_target`
  2. Parse+transform via `run_parse_resolved`:
     - Crate target: classic crate ingest or union+CU ingest (gated by `PLOKE_COMPILATION_UNION`)
     - Workspace target: workspace ingest path
  3. Update loaded workspace/system path policy state
  4. Start async embedding indexer: `IndexerTask::index_workspace`
- Outputs:
  - Parse/transform outputs as in variants 1/2/3.
  - Embedding artifacts:
    - Embedding rows/updates for indexable nodes, progress events, and index completion/cancel/fail status routing through the event bus.

### 5) TUI workspace-update path (embed refresh with parse skipped)

- Trigger:
  - `index_workspace(..., needs_parse = false)` in the app-state handler flow.
- Stage order:
  1. Resolve/anchor target path against loaded state
  2. Skip parser and DB graph transform stages entirely
  3. Reuse loaded workspace state and launch embed indexer over that workspace root
- Outputs:
  - No new parser output object for this invocation.
  - No graph transform writes from parser output in this invocation.
  - Embedding artifacts refreshed from current DB/workspace state via indexer pipeline.

### 6) Xtask parser-only exposure (`cargo xtask parse ...`)

- Trigger:
  - `xtask` parse subcommands in `xtask/src/commands/parse.rs`.
- Stage order (entrypoint dependent):
  - `parse discovery`: discovery only (`run_discovery_phase`)
  - `parse phases-resolve`: parse+resolve only (`try_run_phases_and_resolve`)
  - `parse phases-merge`: parse+resolve+merge (`try_run_phases_and_merge`)
  - `parse workspace` / `parse workspace-config`: workspace parse (`parse_workspace` / `parse_workspace_with_config`)
- Outputs:
  - CLI `ParseOutput` summaries (counts, duration, etc.) for inspection/profiling.
  - No transform/DB writes in parser-only command path.

### 7) Xtask profile ingest (`cargo xtask profile-ingest`)

- Trigger:
  - `xtask::profile_ingest` with `--target`, optional `--stages`, optional `--compilation-unions`.
- Stage order (target and flags dependent):
  1. Parse stage:
     - Workspace target: `parse_workspace`
     - Crate target: classic `try_run_phases_and_merge` or union `try_run_phases_union_for_crate_with_dimensions`
  2. Transform stage:
     - Workspace target: `transform_parsed_workspace`
     - Crate target classic: `transform_parsed_graph`
     - Crate target union: `transform_union_crate_and_structural_masks`
  3. Embed stage (optional): embedding runtime/indexer execution
- Outputs:
  - Timing/perf aggregates for parse/transform/embed stages (text or JSON).
  - Optional DB and embedding side effects when corresponding stages are enabled.

## Standardized Comparison Dimensions & Metrics

These dimensions define what the comparison harness should record per `(variant x target)` run, and how to interpret results consistently.

### Shared evaluation rubric

- `PASS`: all required metrics for the dimension meet the invariant expectations for the target.
- `PARTIAL`: some metrics pass, but at least one required invariant is missing, divergent, or weakly validated.
- `UNKNOWN`: no direct/automated evidence collected for the required metrics (fallback inference only).

### 1) Functional correctness

- Required metrics: `success_shape`, `core_graph_contract`, `module_tree_contract`, `cu_membership_contract`, `cfg_refined_delta`
- How to measure: reuse existing stage-level assertions where available; otherwise record stable counts/sets for anchor tables (avoid blob equality).

### 2) Target coverage

- Required metrics: `target_type_coverage`, `multi_target_union_success_path`
- How to measure: explicitly track which fixture categories ran per variant (no inference from adjacent tests).

### 3) Data product parity

- Required metrics: `merged_graph_parity`, `workspace_metadata_parity`, `cu_mask_parity`, `db_relation_presence`
- How to measure: record relation presence and row counts for anchor tables keyed by run target; for union/CU prefer per-CU-key metrics.

### 4) Performance

- Required metrics: `parse_ms`, `transform_ms`, `embed_ms`, `wall_ms`
- How to measure: use `cargo xtask profile-ingest` timing aggregates as the standardized source of truth.

### 5) Stability / idempotency

- Required metrics: `repeat_run_no_diff`, `partial_failure_atomicity`, `interruption_consistency`
- How to measure: run the same `variant+target` multiple times and compare only stable identifiers and anchor-table summaries.

### 6) Operational behavior

- Required metrics: `error_surface_quality`, `partial_success_policy`, `resolution_consistency`
- How to measure: assert on structured error variants when available; otherwise record stable error-message/phase tags, and compare resolution artifacts rather than incidental ordering.

## Test Mapping by Variant and Behavior Area

Confidence scale used below:
- **High**: multiple direct assertions over the variant's key contract surface.
- **Medium**: partial/direct coverage or mostly smoke-level checks.
- **Low**: little/no direct coverage; behavior inferred from adjacent tests.

### Variant 1) Classic crate ingest

- Functional correctness: **High**
  - `syn_parser/tests/full/basic.rs`
  - `syn_parser/tests/full/github_clones.rs`
  - `syn_parser/tests/uuid_phase*/...` families (phase-level parser/resolve behavior)
- Target coverage: **Medium**
  - Strong on single-root crates and real-world clones (`github_clones.rs`)
  - Multi-target "classic selected root" behavior is exercised indirectly via CU tests, not a dedicated classic contract test.
- Data product parity (merged graph + module tree): **High**
  - `github_clones.rs` stage-isolation tests (`merge_*`, `build_module_tree_*`) and end-to-end `try_run_phases_and_merge`.
- Stability/idempotency: **Low**
  - No explicit repeated-ingest/idempotency tests for classic parse+transform chain.
- Operational behavior (error surfacing/path handling): **Medium**
  - Some negative/diagnostic tests exist; limited explicit contract checks for error shape from `try_run_phases_and_merge`.

### Variant 2) Union+CU crate ingest

- Functional correctness: **Medium**
  - `syn_parser/tests/processing/union_phases.rs` validates output fields, mask snapshot parity, and CU enumeration.
  - `syn_parser/tests/processing/compilation_units.rs` validates lib/bin structural divergence and unusual-lib roots.
  - `ploke-transform/src/transform/union_crate_masks.rs` test validates cfg-refined CU mask persistence.
- Target coverage: **Medium**
  - `fixture_cfg_cu` (lib-only) is well covered.
  - Known gap for true lib+bin union success path (documented in `union_phases.rs` comments).
- Data product parity (union graph + CU masks): **High**
  - Parser output field presence/shape is checked.
  - DB-side CU enabled-node persistence is directly asserted.
- Stability/idempotency: **Low**
  - No direct repeated-run/atomicity assertions for `transform_union_crate_and_structural_masks`.
- Operational behavior: **Medium**
  - Some known-failure behavior is documented; few explicit assertions for error message/diagnostic contract.

### Variant 3) Workspace ingest

- Functional correctness: **Medium**
  - `syn_parser/tests/full/mock_serde_parse.rs` covers `parse_workspace` across full and selective member runs.
  - `ploke-transform/src/transform/workspace.rs` tests cover workspace metadata persistence from fixture-backed parse output.
- Target coverage: **Medium**
  - Good coverage for fixture workspaces and member selection.
  - Limited coverage for larger real-world workspaces (`parse_workspace_serde_github_clone` is ignored in `github_clones.rs`).
- Data product parity (workspace metadata + per-member transforms): **High**
  - `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture` verifies metadata row content.
- Stability/idempotency: **Low**
  - No repeated workspace ingest assertions for consistency across runs.
- Operational behavior: **Medium**
  - Known-issue tolerant branches in `mock_serde_parse.rs` reduce strict failure signaling in some scenarios.

### Variant 4) TUI integrated indexing path (parse/transform + embed)

- Functional correctness: **Low**
  - Existing `ploke-tui` tests (`crates/ploke-tui/src/app_state/tests.rs`) focus chat/event/concurrency behavior, not direct indexing pipeline contracts.
- Target coverage: **Low**
  - No direct variant tests that assert crate-vs-workspace target resolution through `index_workspace` ingest path.
- Data product parity: **Low**
  - No direct assertions linking parse/transform outputs with embedding side effects in this integrated path.
- Stability/idempotency: **Low**
  - No repeated indexing consistency tests for this path.
- Operational behavior: **Low**
  - Limited direct cancellation/interruption assertions for indexing task lifecycle in this integrated pipeline.

### Variant 5) TUI workspace-update path (parse skipped, embed refresh)

- Functional correctness: **Low**
  - No dedicated tests asserting parse-skip behavior (`needs_parse = false`) while still updating embeddings.
- Target coverage: **Low**
  - No explicit workspace-update matrix over fixture targets.
- Data product parity: **Low**
  - No direct assertions that graph writes are skipped while embedding updates proceed.
- Stability/idempotency: **Low**
  - No repeated update-run consistency tests.
- Operational behavior: **Low**
  - No explicit cancellation/resume/error-surfacing tests for update-only indexing path.

### Variant 6) Xtask parser-only exposure (`cargo xtask parse ...`)

- Functional correctness: **Medium**
  - `xtask/tests/cli_invariant_tests.rs` validates command surface/help invariants.
  - Parser correctness itself is largely inherited from `syn_parser` tests; xtask tests are mostly CLI-level.
- Target coverage: **Low**
  - Limited direct test coverage for parse subcommand behavior across all target modes.
- Data product parity: **Medium**
  - Appropriate for parser-only path (no DB writes expected), but explicit assertions for "no transform side-effects" in xtask tests are sparse.
- Stability/idempotency: **Low**
  - No repeated command-run consistency tests.
- Operational behavior: **Medium**
  - CLI error/help handling is covered; deeper runtime error contract checks are limited.

### Variant 7) Xtask profile ingest (`cargo xtask profile-ingest`)

- Functional correctness: **Low**
  - No direct tests for `profile-ingest` stage combinations in `xtask/tests`.
- Target coverage: **Low**
  - No explicit fixture matrix for crate/workspace + union mode under profile-ingest.
- Data product parity: **Low**
  - No test assertions for parse/transform/embed output parity in profile mode.
- Stability/idempotency: **Low**
  - No repeated profile-run consistency checks.
- Operational behavior: **Low**
  - No direct checks for stage failure surfacing/partial-stage handling contract.

## Cross-Variant Confidence Snapshot

- **High confidence areas**
  - Classic parser+merge module-tree construction on standard fixtures and selected real-world crates.
  - Union parser field shape and CU structural slice construction/refinement.
  - Workspace metadata transform persistence for fixture-backed workspace parse output.
- **Medium confidence areas**
  - Workspace parse behavior under selective member parsing.
  - Xtask parse CLI surface and argument/help invariants.
- **Lowest-confidence areas (largest gaps)**
  - TUI integrated indexing and workspace-update paths as end-to-end ingest variants.
  - `profile-ingest` behavior matrix.
  - Idempotency/atomicity checks across transform-heavy variants (especially union CU persistence and workspace/update re-runs).
