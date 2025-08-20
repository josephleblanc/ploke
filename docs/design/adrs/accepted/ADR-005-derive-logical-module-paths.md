# ADR-005: Derive Logical Module Paths in Phase 2 Parsing

## Status
ACCEPTED

## Context
Phase 2 parallel parsing ([`00_overview_batch_processing_model.md`](docs/plans/uuid_refactor/00_overview_batch_processing_model.md#phase-2-parallel-parse--provisional-graph-generation)) initially created root `ModuleNode`s for each file using a hardcoded path `["crate"]`. This led to incorrect item associations within partial graphs, hindering debugging and downstream processing. See `test_module_node_top_pub_mod_paranoid` failure history.

## Decision
1.  Implemented `derive_logical_path` heuristic in [`visitor/mod.rs`](crates/ingest/syn_parser/src/parser/visitor/mod.rs) to estimate a file's module path from its file system location relative to `src/`.
2.  Modified `analyze_file_phase2` in [`visitor/mod.rs`](crates/ingest/syn_parser/src/parser/visitor/mod.rs) to use this derived path for the root `ModuleNode`'s `path`, `name`, and synthetic `NodeId` context.

## Consequences
- **Positive:**
    *   Corrects item-to-module association within Phase 2 partial graphs.
    *   Improves debuggability and accuracy of intermediate `CodeGraph` state.
    *   Provides better structured input for Phase 3 module merging.
- **Negative:**
    *   **Known Limitation:** The `derive_logical_path` heuristic **does not handle `#[path]` attributes**. Full accuracy requires future enhancement (likely in Phase 1 discovery).
    *   Required updates to Phase 2 tests previously assuming `["crate"]`.
- **Neutral:**
    *   Final module path/ID resolution remains a Phase 3 responsibility.
    *   Respects Phase 2 parallel constraints (no cross-worker communication).

## Compliance
- Aligns with multi-phase processing goals ([`PROPOSED_ARCH_V3.md`](PROPOSED_ARCH_V3.md)).
