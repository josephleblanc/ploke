# `ploke-transform`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-transform` converts `syn_parser` graph structures into CozoDB relation rows. It is the boundary between Rust syntax/semantic graph construction and database storage/query semantics.

## Responsibilities

- Insert graph node categories into their schema-specific Cozo relations.
- Insert relation edges, imports/reexports, modules, crate context, and type metadata.
- Run late type-use/type-relation resolution against the `ModuleTree` before insertion.
- Provide schema modules used by `ploke-db` and tests to keep relation shape consistent.
- Transform entire parsed workspaces through the `workspace` entry point.

## Key Files

- [`crates/ingest/ploke-transform/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-transform/src/lib.rs) — crate module boundary.
- [`crates/ingest/ploke-transform/src/transform/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-transform/src/transform/mod.rs) — primary graph-to-relation transform orchestration.
- [`crates/ingest/ploke-transform/src/schema/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-transform/src/schema/mod.rs) — schema module boundary for relation definitions.
- [`crates/ingest/ploke-transform/src/transform/workspace.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-transform/src/transform/workspace.rs) — parsed workspace transform entry.
- [`crates/ingest/ploke-transform/src/transform/type_graph.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/ploke-transform/src/transform/type_graph.rs) — typed type graph edge insertion when the feature is enabled.

## Public API

- `transform_parsed_graph(db, parsed_graph, tree)` — main single-crate graph transform. It calls `resolve_type_relations_after_tree` to produce the v2 typed type graph relations.
- `transform_parsed_workspace` — workspace-level transform re-exported from `workspace`.
- `transform_code_graph` — deprecated legacy transform for raw `CodeGraph`.
- `insert_structural_compilation_unit_slice` and `transform_union_crate_and_structural_masks` — special-case structural compilation-unit helpers.

## Internal Structure

`transform_parsed_graph` consumes a `ParsedCodeGraph`, extracts the required `CrateContext`, and inserts relations in a stable order: type graph edges/types, functions, defined types, traits, impls, modules, consts/statics/macros, imports, relations, resolved type uses/type relations, and crate context. Each node category lives in its own small transform module.

## Dependencies

- **Uses:** `syn_parser`, `ploke-core`, `cozo`, `uuid`, schema modules from this crate.
- **Used by:** `ploke-tui` indexing, `ploke-db` schema/query code, fixture generation, and typed graph tests.

## Notable Patterns / Gotchas

- The code assumes every `ParsedCodeGraph` reaching transform has a `CrateContext`; missing context is an invariant violation.
- Imports are now graph facts, not optional metadata. When relation handling changes, import nodes and import-bearing relations must stay in lockstep.
- Transform always inserts the v2 typed type graph relations (`type_use`, `type_contains`, and `type_relation`) produced by `resolve_type_relations_after_tree`.
