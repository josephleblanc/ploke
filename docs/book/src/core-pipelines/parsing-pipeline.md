# Parsing Pipeline

Status: Draft

## Purpose

This chapter documents the `syn_parser` parsing pipeline as it exists today, with enough detail to:
- explain how source files become a merged `ParsedCodeGraph` and `ModuleTree`;
- define key invariants for IDs, graph shape, and phase boundaries;
- provide the forward path to replace archived UUID-refactor planning docs.

## Scope

- In scope:
  - Discovery, parallel parsing, graph merge, module-tree construction, pruning, and canonical ID resolution.
  - The concrete `syn_parser` entry points and data structures used by downstream crates.
  - Known limitations that affect correctness or interpretation of parser output.
- Out of scope:
  - Embedding generation and DB transform/insert internals (covered in other pipeline chapters).
  - Full rustc-level semantic/type resolution.

## Pipeline Overview

Primary crate and modules:
- `crates/ingest/syn_parser/src/lib.rs`
- `crates/ingest/syn_parser/src/discovery/single_crate.rs`
- `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
- `crates/ingest/syn_parser/src/parser/visitor/state.rs`
- `crates/ingest/syn_parser/src/parser/visitor/type_processing.rs`
- `crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs`
- `crates/ingest/syn_parser/src/resolve/mod.rs`
- `crates/ingest/syn_parser/src/resolve/id_resolver.rs`

High-level execution:
1. Discovery (`run_discovery_phase`) collects crate metadata and `.rs` files.
2. Parallel parse (`analyze_files_parallel`) parses each file to a `ParsedCodeGraph`.
3. Merge (`ParsedCodeGraph::merge_new`) combines per-file graphs.
4. Resolve/build tree (`build_tree_and_prune` path) constructs `ModuleTree`, links modules/imports, prunes unlinked file modules.
5. Optional canonicalization (`CanonIdResolver`) maps synthetic IDs to stable canonical IDs.

## Entry Points

- `run_phases_and_collect(fixture_name)`:
  - discovery + parallel parse;
  - returns file-level parse outputs (or partial-success error envelope).
- `run_phases_and_merge(fixture_name)`:
  - discovery + parse + merge + tree build;
  - returns `ParserOutput { merged_graph, module_tree }`.
- `try_run_phases_and_merge(target_crate: &Path)`:
  - same shape for non-fixture crate paths.

## Phase 1: Discovery

Discovery produces `DiscoveryOutput { crate_contexts, warnings }`, where each `CrateContext` contains:
- crate name/version;
- derived crate namespace UUID;
- root path;
- discovered `.rs` files;
- parsed features/dependencies/dev-dependencies.

Current behavior to note:
- file discovery is `src/**.rs` via `walkdir`;
- if both `lib.rs` and `main.rs` exist, `main.rs` is currently excluded (documented limitation);
- namespace derivation currently hashes crate name (version argument is accepted but intentionally ignored in implementation).

## Phase 2: Parallel Parse (Per-File Graph Generation)

`analyze_files_parallel`:
- iterates each discovered crate context;
- uses rayon parallel iteration across files;
- derives a logical module path from filesystem location;
- calls `analyze_file_phase2(...)` per file.

`analyze_file_phase2`:
- parses file with `syn`;
- initializes `VisitorState` with crate namespace + current file path (+ cfg context where enabled);
- creates a root file module node;
- traverses AST using `CodeVisitor`;
- emits a `ParsedCodeGraph` containing:
  - primary nodes (functions, types, impls, traits, modules, consts/statics/macros/imports),
  - secondary/type graph data,
  - syntactic relations,
  - synthetic IDs and tracking hashes.

ID and hash generation context:
- `NodeId::Synthetic`: generated from stable context (namespace/file/module/scope/kind/name and cfg hash context where applicable).
- `TypeId`: generated through structural `TypeKind` + related-type recursion (`get_or_create_type` / `process_type`).
- `TrackingHash`: generated from token stream content via `VisitorState::generate_tracking_hash`.

## Phase 3: Merge, ModuleTree, and Pruning

After per-file parsing:
- graphs are merged into one `ParsedCodeGraph`;
- module-tree build links declarations/definitions and processes `#[path]`-based module routing;
- internal definition-import links are added;
- unlinked file modules are pruned from tree state, then corresponding graph pruning is applied.

Important caveat:
- pruning intentionally does not fully verify all secondary-node granularity (tracked as known limitation).

## Canonical ID Resolution (Post-Tree)

`CanonIdResolver` resolves graph item IDs against module-tree paths:
- input: merged graph + resolved module tree + crate namespace;
- output: iterator of `(AnyNodeId, CanonId)` mappings;
- canonical ID generation uses resolved item path/kind/cfg context.

This phase is currently modeled as downstream from parse+tree, not part of Phase 2 parsing.

## Data Products and Contracts

By stage:
1. Discovery:
  - contract: crate contexts and source file inventory are complete enough to parse.
2. Parallel parse:
  - contract: each parseable file yields a valid per-file graph with deterministic synthetic IDs.
3. Merge + tree:
  - contract: merged graph and module tree are mutually consistent after pruning.
4. Canonical resolution:
  - contract: stable resolved IDs can be produced for items with resolvable defining paths.

## Known Limitations (Current Snapshot)

This list is intentionally short; details remain in archived docs until migrated:
- Associated const/type items in `impl`/`trait` blocks have incomplete coverage.
- Some cfg behaviors are intentionally partial (evaluation atom support and fallback bias).
- Duplicate unnamed impl handling is imperfect and currently patched.
- Discovery `lib.rs`/`main.rs` dual-root handling is intentionally constrained.
- Some legacy docs describe outdated behavior (for example old TypeId conflation notes now marked solved).

## Migration Plan From Archived UUID Docs

The archived UUID planning set contains valuable rationale but mixes outdated and current details.
This chapter is the replacement target for the parser-specific parts of:
- `docs/archive/plans/uuid_refactor/00_overview_batch_processing_model.md`
- `docs/archive/plans/uuid_refactor/90a_type_processing_overview.md`
- `docs/archive/plans/uuid_refactor/02c_phase2_known_limitations.md`
- `docs/archive/plans/uuid_refactor/03a_phase3_known_limitations.md`
- `docs/archive/plans/uuid_refactor/03b_canonical_path.md`

Migration approach:
1. Treat this chapter as canonical for current behavior.
2. Move still-valid design rationale into "Design Patterns and Decisions" chapters.
3. Keep only unresolved limitation tracking in dedicated active docs.
4. Archive historical implementation plans as superseded.

## Notes

Cross-references to keep updated as this chapter matures:
- `../crate-reference/syn-parser.md`
- `../design-patterns-and-decisions/adr-index.md`
- `../testing-and-invariants/pipeline-invariants.md`
