---
date: 2026-03-27
task title: Fixture-to-Variant Coverage Mapping
task description: Create fixture-to-variant coverage mapping and identify highest-value missing runs/tests.
related planning files:
  - /home/brasides/code/ploke/.cursor/plans/ingestion-pipeline-comparison-overview_88f15a9b.plan.md
---

## Pipeline Variants (for reference)

- `V1` Classic crate ingest: `syn_parser::try_run_phases_and_merge` -> `ploke_transform::transform_parsed_graph`
- `V2` Union+CU crate ingest: `syn_parser::try_run_phases_union_for_crate_with_dimensions` -> `ploke_transform::transform_union_crate_and_structural_masks`
- `V3` Workspace ingest: `syn_parser::parse_workspace(_with_config)` -> `ploke_transform::transform_parsed_workspace`
- `V4` TUI integrated indexing path: index_workspace(parse/transform + embed)
- `V5` TUI workspace-update path: embed refresh with parse skipped
- `V6` Xtask parser-only: `cargo xtask parse ...`
- `V7` Xtask profile-ingest: `cargo xtask profile-ingest ...`

## Fixture Map (fixture -> intended stress area -> covered variants -> highest-value missing variant runs)

### `fixture_nodes` (baseline crate ingest; module tree + merged graph)

- Intended stress area: baseline crate structure, merge/module-tree correctness, and “happy path” for classic ingest/transform.
- Covered variants:
  - `V1` Classic crate ingest: `crates/ingest/ploke-transform/src/transform/mod.rs` test `test_insert_all` merges module tree + runs `transform_parsed_graph` after `test_run_phases_and_collect("fixture_nodes")`.
  - `V4` TUI integrated indexing path (parse skipped): `crates/ploke-tui/tests/indexing_non_blocking.rs` indexes `tests/fixture_crates/fixture_nodes` with `needs_parse = false`.
  - `V4` TUI integrated indexing path (parse included): `crates/ploke-tui/tests/indexing_freeze_repro.rs` indexes `tests/fixture_crates/fixture_nodes` with `needs_parse = true` (used to validate state manager responsiveness during indexing).
  - `V6` Xtask parser-only: `xtask/tests/parse_debug_commands.rs` uses `tests/fixture_crates/fixture_nodes` for parse-debug commands.
- Missing / highest-value additional runs:
  - `V4` parse-skip contract verification: add an assertion that graph/transform writes do not change when `needs_parse = false` (e.g., anchor relation row counts stable) while embedding refresh still occurs.
  - `V1` resolve contract coverage: add direct assertions around `try_run_phases_and_resolve` invariants (currently mostly covered indirectly through merge/tree + transform tests).

### `fixture_cfg_cu` (union+CU happy path; cfg refinement over structural CU slices)

- Intended stress area: `V2` union pipeline completion on a crate layout that currently works, plus persistence of cfg-refined CU enablement.
- Covered variants:
  - `V2` Union+CU crate ingest (union parse + CU enumeration):
    - `crates/ingest/syn_parser/tests/processing/union_phases.rs` uses `fixture_cfg_cu` and calls:
      - `try_run_phases_union_for_crate_with_dimensions` (baseline + env-dimension variants)
      - compares `parsed_graphs_for_masks` to `try_run_phases_and_resolve` output.
    - Evidence: the file explicitly documents that `fixture_multi_target_cu` is currently avoided because union merge errors on lib+bin roots.
  - `V2` Union+CU crate ingest (cfg-refined CU mask persistence):
    - `crates/ingest/ploke-transform/src/transform/union_crate_masks.rs` test `persists_cfg_refined_masks_per_compilation_unit` runs `transform_union_crate_and_structural_masks(&db, parsed_graphs, Some(keys))` using parsed graphs from `test_run_phases_and_collect("fixture_cfg_cu")`.
    - `crates/ingest/ploke-transform/src/transform/compilation_unit.rs` test `cfg_filter_feature_gated_item_respects_compilation_unit_key` checks filtering behavior driven by compilation unit keys derived from `fixture_cfg_cu`.
- Missing / highest-value additional runs:
  - `V2` idempotency/atomicity: run `transform_union_crate_and_structural_masks` (or full `index_workspace` parse+transform union path) multiple times and compare CU-enabled anchor tables for no-diff; also add a partial-failure test that ensures atomicity of CU mask writes.
  - `V4` end-to-end in union mode: run TUI indexing with `PLOKE_COMPILATION_UNION=1` against `fixture_cfg_cu` and assert parse/transform outputs and embed results line up with CU mask expectations (currently most tests are unit-/parser-level).

### `fixture_multi_target_cu` (distinct lib/bin roots; current union merge is blocked)

- Intended stress area: correct union+CU behavior for a crate with both a library root and a binary root (ensuring distinct CU membership for lib vs bin).
- Covered variants (partial, via structural CU slices persistence rather than full union-merge):
  - `V2` Union+CU crate ingest (structural CU slice membership + persistence):
    - `crates/ingest/syn_parser/tests/processing/compilation_units.rs` test `structural_slices_diverge_for_lib_and_bin_targets` uses `run_phases_and_collect("fixture_multi_target_cu")` and asserts enabled-node divergence for lib vs bin slices built from compilation unit keys.
    - `crates/ingest/ploke-transform/src/transform/compilation_unit.rs` test `fixture_multi_target_persists_distinct_lib_and_bin_membership_rows` persists CU slice enablement for lib vs bin and asserts membership sets differ.
- Known gap blocking true end-to-end `V2`:
  - `crates/ingest/syn_parser/tests/processing/union_phases.rs` states union merge currently errors on crates with both library and binary roots (`fixture_multi_target_cu`), so union-phase tests intentionally use `fixture_cfg_cu` instead.
- Missing / highest-value additional runs:
  - Highest priority: `V2` union+CU end-to-end on `fixture_multi_target_cu` once union merge supports lib+bin roots.
    - Scenario: run `try_run_phases_union_for_crate_with_dimensions` on `fixture_multi_target_cu` -> feed `transform_union_crate_and_structural_masks`.
    - Assertions to add: merged graph contains both lib and bin symbols; CU mask persistence produces distinct enabled node sets for lib CU vs bin CU.

### `fixture_unusual_lib` (nonstandard lib path; compilation unit key + slice membership correctness)

- Intended stress area: structural CU slice behavior when the lib root is not in the conventional `src/lib.rs` location.
- Covered variants (partial, via structural CU slice behavior):
  - `V2` Union+CU crate ingest (structural CU slicing correctness):
    - `crates/ingest/syn_parser/tests/processing/compilation_units.rs` test `structural_slices_support_custom_lib_path_targets`:
      - validates compilation unit key `target_root` selection for lib vs bin targets
      - validates enabled-node membership divergence driven by those keys.
- Missing / highest-value additional runs:
  - `V2` end-to-end union+CU ingest on `fixture_unusual_lib` (if/when union merge supports this layout) and validation that persisted CU mask enablement matches the structural slice expectations.

### `tests/fixture_workspace/ws_fixture_01` (workspace ingest + workspace status/update; embed refresh correctness)

- Intended stress area: multi-member workspace parsing, workspace metadata persistence, and correctness of workspace freshness tracking + selective member refresh.
- Covered variants:
  - `V3` Workspace ingest:
    - `crates/ingest/ploke-transform/src/transform/workspace.rs` test `transform_parsed_workspace_persists_workspace_metadata_fields_from_committed_fixture` parses `ws_fixture_01` via `parse_workspace` and runs `transform_parsed_workspace`.
  - `V4` TUI integrated indexing path (workspace parse + transform included):
    - `crates/ploke-tui/tests/workspace_status_update.rs` calls `index_workspace(..., needs_parse = true)` on `ws_fixture_01`.
  - `V5` TUI workspace-update path (embed refresh; graph writes should be skipped):
    - Same test file calls `workspace_update_for_test(&state, &event_bus)` after inducing drift in a workspace member.
    - It asserts member freshness transitions (stale -> fresh) and that embeddings for unchanged members are preserved.
- Missing / highest-value additional runs:
  - Explicit `V5` parse-skip contract verification: add assertions that graph/transform writes are unchanged when the update path is exercised (not just embedding preservation).
  - Cancellation/interruption consistency: add explicit cancellation/resume tests for workspace ingest/update to ensure task lifecycle does not leak and DB invariants hold.

### `tests/fixture_workspace/fixture_mock_serde` (workspace parsing; parser-only debug; complex multi-crate layout)

- Intended stress area: `parse_workspace` correctness for a workspace shaped like a real serde-like crate hierarchy (main + core + derive + internal support crates).
- Covered variants:
  - `V3` Workspace ingest (parse_workspace):
    - `crates/ingest/syn_parser/tests/full/mock_serde_parse.rs` tests `parse_workspace` on `fixture_mock_serde`.
  - `V6` Xtask parser-only:
    - `xtask/tests/parse_debug_commands.rs` uses `fixture_mock_serde` for parser-debug commands (`parse debug cargo-targets`).
- Missing / highest-value additional runs:
  - `V3` end-to-end `transform_parsed_workspace` coverage for this fixture (currently stronger on `ws_fixture_01` for metadata persistence).

## Other Underused Fixtures (candidates for future variant runs)

These are flagged by the plan as potential comparison additions, but they are not currently mapped to the full ingestion variants above (at least not in the variant-level unit tests we inspected):

- `duplicate_name_fixture_3`: present in parser-phase determinism tests (`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/determinism.rs`), but not in variant-level merge/union/workspace pipelines.
- `fixture_impls`: registered in workspace `Cargo.toml` but no direct ingestion variant tests found.
- `fixture_nodes_copy`: used in TUI/RAG code-edit fixture scaffolding (not variant-level ingest correctness).
- `fixture_path_edge`: registered in workspace `Cargo.toml` but no direct ingestion variant tests found.
- `fixture_import_duplicate_leaf`: present in parser phase import-ID disambiguation tests (`crates/ingest/syn_parser/tests/uuid_phase2_partial_graphs/import_id_disambiguation.rs`), but not in variant-level merge/union/workspace pipelines.

## Highest-Value Missing Runs/Tests (ranked)

1. `V2` union+CU end-to-end on `fixture_multi_target_cu` (lib+bin union success path).
   - Why: this is currently blocked/avoided due to known union-merge failure; once fixed, it gives both target-compat coverage and strongest “lib vs bin CU membership divergence” validation.

2. `V5` workspace-update path parse-skip contract.
   - Why: existing tests validate embedding preservation and freshness transitions, but do not explicitly prove that graph/transform writes are skipped when `needs_parse=false`.

3. `V2` CU transform idempotency/atomicity on persisted mask writes.
   - Why: current `transform_union_crate_and_structural_masks` coverage checks correctness of a single execution; repeated-run and partial-failure atomicity are still gaps.

4. `V4` TUI integrated indexing parse/transform + embed parity for a fixture where union mode is enabled.
   - Why: `ploke-tui` parse path switches on `PLOKE_COMPILATION_UNION`; most current indexing tests are not asserting the union mode path end-to-end.

5. `V3` `transform_parsed_workspace` end-to-end for `fixture_mock_serde`.
   - Why: `parse_workspace` is covered, but transform coverage is currently anchored more strongly on `ws_fixture_01`.

