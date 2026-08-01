# `syn_parser`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`syn_parser` discovers Rust crates/workspaces, parses Rust source files, constructs code graphs, and resolves module trees. It is the first stage of Ploke's code-understanding pipeline.

## Responsibilities

- Parse Cargo manifests and discover crate source files, tests, examples, benches, and explicit targets.
- Support workspace-level parsing with selected members and optional target selectors.
- Run phase-2 file analysis in parallel and collect partial successes/errors.
- Produce `ParsedCodeGraph` plus `ModuleTree` outputs for downstream transform/query layers.
- Provide stable node/type IDs and re-export parser graph access traits and core types.

## Key Files

- [`crates/ingest/syn_parser/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/lib.rs) — crate API, workspace parsing entry points, `ParserOutput`, `ParsedWorkspace`, `ParsedCrate`.
- [`crates/ingest/syn_parser/src/discovery/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/discovery/mod.rs) — manifest parsing, target collection, source file discovery, namespace derivation.
- [`crates/ingest/syn_parser/src/parser/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/parser/mod.rs) — parser submodule exports, `CodeGraph`, `ParsedCodeGraph`, parser channel, and `analyze_files_parallel`.
- [`crates/ingest/syn_parser/src/resolve/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/resolve/mod.rs) — module-tree and relation/type-resolution boundary.
- [`crates/ingest/syn_parser/src/compilation_unit/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ingest/syn_parser/src/compilation_unit/mod.rs) — compilation-unit key enumeration for multi-target parsing.

## Public API

- `parse_workspace_with_config(target_workspace_dir, &ParseWorkspaceConfig)` — parses selected workspace members with optional target selector.
- `parse_workspace(target_workspace_dir, selected_crates)` — convenience wrapper without target selector.
- `try_run_phases_and_resolve` / `try_run_phases_and_resolve_with_target` — discovery plus parallel parse for a single crate.
- `run_phases_and_collect` / `run_phases_and_merge` — high-level fixture-style parsing helpers referenced by the crate docs.
- `ParserOutput { merged_graph, module_tree, graph_debug }` — carries merged graph and module tree to downstream consumers.
- `ParsedWorkspace { workspace, crates }` and `ParsedCrate { crate_context, parser_output }` — workspace parse result shapes.

## Internal Structure

Discovery is deliberately single-threaded: it reads `Cargo.toml`, resolves workspace membership, derives crate namespaces, and gathers `.rs` files before parallel parsing starts. Parsing then delegates to parser/visitor code to build per-file graph fragments. Resolve code merges those fragments into crate-level graph and module-tree outputs so later crates can treat module paths and type relationships as graph facts.

## Dependencies

- **Uses:** `syn`, `quote`, `cargo_toml`, `walkdir`, `rayon`, `itertools`, `ploke-core`, `ploke-error`.
- **Used by:** `ploke-tui` parsing/index commands, `ploke-transform`, tests/fixtures, and downstream type-resolution work.

## Notable Patterns / Gotchas

- `ParseWorkspaceConfig.selected_crates` is normalized against the workspace root and rejects members that are not in the manifest.
- Discovery treats invalid crate paths, missing manifests, and missing source roots as critical errors; some walkdir failures are collected as non-fatal warnings.
- `typed_type_graph` changes downstream transform/type-resolution behavior, so parser outputs must preserve enough slot/type data for that feature path.
