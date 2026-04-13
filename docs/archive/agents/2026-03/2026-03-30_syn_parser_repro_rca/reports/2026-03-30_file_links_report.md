# date: 2026-03-30
# task title: syn_parser RCA for repro::fail::file_links
# task description: root-cause analysis for module-tree duplicate-path expected-failure repros in crates/ingest/syn_parser/tests/repro/fail/file_links.rs, with suggested fixes (no code edits)
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md

This file covers the following expected-failure repros:

- `repro_duplicate_inline_protos_module_merge_error`
- `repro_duplicate_cli_binary_module_merge_error`
- `repro_duplicate_scheduler_queue_mod_merge_error`
- `repro_duplicate_logging_inline_file_mod_merge_error`
- `repro_duplicate_image_inline_file_mod_merge_error`

## Shared Background (What Is Failing)

All five tests are asserting the same internal failure mode: module-tree construction errors out with a duplicate canonical path (e.g. `crate::cli`, `crate::scheduler::queue`, etc.).

Mechanically this comes from:

1. `ParsedCodeGraph::build_module_tree_from_root_module` inserting *all* `ModuleNode`s into a new `ModuleTree` via `tree.add_module(module.clone())` before doing any linking/pruning. See:
   - /home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs
2. `ModuleTree::add_module` inserting each module definition into `path_index` keyed by its `NodePath`, and returning `ModuleTreeError::DuplicatePath` on collision. See:
   - /home/brasides/code/ploke/crates/ingest/syn_parser/src/resolve/module_tree.rs
3. Unlinked file modules are intended to be handled later (`link_mods_syntactic` warns, and `prune_unlinked_file_modules` removes them), but the duplicate-path error happens before we reach the prune. See:
   - /home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs
   - /home/brasides/code/ploke/crates/ingest/syn_parser/src/resolve/module_tree.rs

There are two distinct root causes across these five repros:

- A. **Incorrect logical module path derivation for nested `main.rs`** (affects `cli` and `scheduler::queue`).
- B. **Inline module definitions colliding with unlinked file modules that should be pruned** (affects `logging`, `image`, `protos::default_index`).

## repro_duplicate_cli_binary_module_merge_error

**Root Cause**

`syn_parser::parser::visitor::logical_module_path_for_file` treats any filename `main.rs` as a “module root” (same as `mod.rs`), regardless of location under `src/`. That means a binary root at `src/cli/main.rs` is assigned the logical module path `crate::cli`, which collides with the library module `cli` defined by `src/cli/mod.rs` (also `crate::cli`).

This is a path-derivation bug: `main.rs` should only be special when it is the root file of a compilation unit (typically `src/main.rs` for the default bin target), not for nested `.../src/**/main.rs`.

**Evidence**

- Fixture shape (valid Rust):
  - `[[bin]] path = "src/cli/main.rs"`
  - `src/lib.rs` contains `pub mod cli;`
  - library module file at `src/cli/mod.rs`
  - /home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_03_cli_collision/member_cli_collision
- Path derivation logic pops `main.rs` unconditionally:
  - /home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs
  - `if last == "mod.rs" || last == "lib.rs" || last == "main.rs" { components.pop(); }`

**Suggested Fix / Mitigation (No Edits Made)**

1. Fix logical-path derivation:
   - Only strip `main.rs` / `lib.rs` when the file is actually the compilation-unit root (e.g., equals `TargetSpec.root`, or equals `crate_src_dir.join(\"main.rs\")` / `lib.rs`).
   - For nested `.../main.rs`, treat it like any other file and use its stem: `main` (so `src/cli/main.rs` would become `crate::cli::main` if treated as a normal module file), or better, compute paths relative to the *target root* instead of always `crate_src_dir`.
2. Longer-term: avoid merging multiple compilation units (lib + bins) into one `crate::...` namespace without a disambiguator.
   - Either build a separate `ModuleTree` per compilation unit, or namespace by target kind/name (`lib::...`, `bin::cli-collision::...`) before merging.

**Confidence**: high.

## repro_duplicate_scheduler_queue_mod_merge_error

**Root Cause**

Same underlying `main.rs` handling bug as above, but in a nested module directory: `src/scheduler/queue/main.rs` is being treated as the module root for `queue` (path `crate::scheduler::queue`) instead of the submodule `main` (path `crate::scheduler::queue::main`).

That collides with the real `queue` module file at `src/scheduler/queue/mod.rs`, which is also `crate::scheduler::queue`.

**Evidence**

- Fixture shape:
  - `src/scheduler/mod.rs` contains `mod queue;`
  - `src/scheduler/queue/mod.rs` contains `mod main;`
  - `src/scheduler/queue/main.rs` exists
  - /home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_scheduler_queue_repro
- `logical_module_path_for_file` strips `main.rs` regardless of location (see same pointer as above):
  - /home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs

**Suggested Fix / Mitigation (No Edits Made)**

- Same as `cli`:
  - Do not treat nested `main.rs` as a module root file.
  - For this fixture specifically, `src/scheduler/queue/main.rs` must map to `crate::scheduler::queue::main`.

**Confidence**: high.

## repro_duplicate_logging_inline_file_mod_merge_error

**Root Cause**

The crate defines `logging` as an inline module in `lib.rs` (`pub mod logging { ... }`), and also has a `src/logging.rs` file present on disk.

Rust semantics: the inline module is the definition; the existence of `src/logging.rs` is irrelevant unless referenced by a `mod logging;` declaration.

Current `syn_parser` behavior: it builds `ModuleNode`s for file-based modules from the filesystem and inserts them into the `ModuleTree` *before* it has a chance to classify and prune unlinked file modules. Since the file-based `logging.rs` produces the same canonical path (`crate::logging`) as the inline module, `ModuleTree::add_module` errors with `DuplicatePath`.

**Evidence**

- Fixture:
  - inline module in `src/lib.rs`
  - empty `src/logging.rs` exists
  - /home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_logging_inline_file_repro
- Build order inserts all modules before linking/pruning:
  - /home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/graph/parsed_graph.rs
- Duplicate-path detection is immediate in `add_module`:
  - /home/brasides/code/ploke/crates/ingest/syn_parser/src/resolve/module_tree.rs
- Unlinked file module pruning exists but happens later:
  - /home/brasides/code/ploke/crates/ingest/syn_parser/src/resolve/module_tree.rs

**Suggested Fix / Mitigation (No Edits Made)**

1. Make module-tree build tolerant of “unlinked file modules” during initial registration:
   - Do not insert file-based modules into `path_index` until after `link_mods_syntactic` + `process_path_attributes` + pruning identifies which file modules are actually linked.
   - Alternatively, store file-based modules in a separate staging map keyed by path and only promote linked ones.
2. If keeping current structure, a narrower mitigation is:
   - When a duplicate path is detected between an inline module and a file-based module, treat the file-based one as “unlinked candidate” and allow it to be pruned, rather than failing early.

This should be done carefully so we still error on real ambiguities (e.g. both `foo.rs` and `foo/mod.rs` present for a declared `mod foo;`).

**Confidence**: high.

## repro_duplicate_image_inline_file_mod_merge_error

**Root Cause**

Same as `logging`, but for the `image` module: inline `pub mod image { ... }` plus an on-disk `src/image.rs` file that is not linked via `mod image;`.

**Evidence**

- /home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_image_inline_file_repro
- Same module-tree build order and `add_module` collision mechanism as `logging`.

**Suggested Fix / Mitigation (No Edits Made)**

- Same as `logging` (defer file-module path indexing or stage/prune unlinked file modules before enforcing uniqueness).

**Confidence**: high.

## repro_duplicate_inline_protos_module_merge_error

**Root Cause**

`protos/mod.rs` defines `pub mod default_index { include!(\"default_index.rs\"); }` inline, and there is also a sibling file `protos/default_index.rs`.

Rust semantics: `default_index.rs` is included as source text into the inline module; it is not a module file unless there is a `mod default_index;` declaration.

Current `syn_parser` behavior: it treats `protos/default_index.rs` as a file-based module anyway (likely due to filesystem enumeration) and tries to register it as a `ModuleNode` with canonical path `crate::protos::default_index`, colliding with the inline module at the same path.

**Evidence**

- Fixture:
  - /home/brasides/code/ploke/tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_protos_default_index_repro
- Same `ModuleTree::add_module` duplicate-path failure mode as `logging/image`.

**Suggested Fix / Mitigation (No Edits Made)**

Same staging/pruning fix as `logging/image` applies here.

If you want to be more semantic (harder but better):

- Teach discovery/build to avoid treating every `*.rs` as a module file; instead build the set of module files by following `mod foo;` declarations (plus `#[path]`) from crate roots, and treat other `*.rs` as “unlinked” and prunable without indexing collisions.

**Confidence**: medium-high (exact file-enumeration behavior wasn’t traced end-to-end, but the failure follows directly from duplicate-path insertion order).
