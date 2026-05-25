# date: 2026-03-30
# task title: syn_parser repro RCA (cfg_gates)
# task description: root-cause analysis for cfg-gated duplicate module-path merge failures exercised by `crates/ingest/syn_parser/tests/repro/fail/cfg_gates.rs`
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md

## Failure: `repro_duplicate_quantized_metal_mod_merge_error`

**Root Cause**

- `syn_parser` builds a `ModuleTree` by inserting every parsed `ModuleNode` into a canonical-path index that requires uniqueness for module *definitions* (inline + file-based). See `ModuleTree::add_module` in [module_tree.rs](../../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs).
- In this repro, there are *two distinct module definitions* for `crate::quantized::metal`:
  - an inline `mod metal {}` in `quantized/mod.rs` under `#[cfg(not(feature = "metal"))]`
  - a file-based module definition created from parsing `quantized/metal.rs` (because discovery/parsing treats every discovered `.rs` file as a file-based module root, even if it is unreachable from the crate root)
- That yields two definition `ModuleNode`s with the same `NodePath` (`crate::quantized::metal`), and `ModuleTree::add_module` errors early with `ModuleTreeError::DuplicatePath` on the second insertion (definition `path_index` collision). The prune phase that would normally drop unlinked file-based modules never runs because the build fails first.

**Evidence**

- Fixture shape:
  - `quantized/mod.rs` defines `metal` both as a cfg-gated file module decl and an inline fallback: [mod.rs](../../../../../tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_quantized_metal_repro/src/quantized/mod.rs)
  - The corresponding file module exists and is parsed as a module definition: [metal.rs](../../../../../tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_quantized_metal_repro/src/quantized/metal.rs)
- The duplicate-path error is thrown during `ModuleTree::add_module` when inserting into `path_index` for non-declaration modules. See [module_tree.rs](../../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs) around the `self.path_index.entry(node_path.clone())` occupied-branch.
- `parse_workspace` ultimately surfaces this as `SynParserError::InternalState("Failed to build module tree: ...")` after `build_tree_and_prune_for_root_path` fails. See [lib.rs](../../../../../crates/ingest/syn_parser/src/lib.rs).

**Suggested Fix (No Edits Made)**

1. Make file-module ingestion reachability-based, not scan-based.
Build the module graph by starting from crate root and following `mod` declarations / `#[path]` attributes to discover which module files should be parsed. This matches Rust semantics and prevents unreachable sibling `metal.rs` from producing a conflicting file-based module definition when an inline `mod metal {}` is used instead.
2. If keeping scan-based discovery for parallelism, defer indexing file-based module roots until after linkage.
Treat file-based module roots as “candidates” until a corresponding declaration links them into the tree; only then insert into `path_index`. If an inline definition exists at the same canonical path and no declaration links the file module, drop the file module as unlinked before it can trip duplicate detection.
3. Cfg evaluation alone is insufficient unless it also gates file parsing.
Even with the existing `cfg_eval` hooks in the visitor, `quantized/metal.rs` is still parsed today because discovery enumerates files first and `analyze_files_parallel` parses them unconditionally. If cfg evaluation is pursued, it needs to participate in reachability/file selection (or in early “unlinked file module” pruning) to prevent the file-based root module from being inserted alongside an inline definition.

**Confidence:** High.

## Failure: `repro_duplicate_cfg_gated_module_merge_error`

**Root Cause**

- The repro declares two sibling modules with the same name (`readwrite_pv64v2`) under mutually exclusive `#[cfg(...)]` guards in a single source file. See [c.rs](../../../../../tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_cfg_duplicate_mods_repro/src/backend/libc/c.rs).
- `syn_parser` currently builds a single unconditional module tree from the parsed syntax and enforces canonical-path uniqueness for module declarations/definitions, but it does not model cfg-disjoint alternatives. As a result, both `mod readwrite_pv64v2 { ... }` blocks are treated as active, producing two module definitions at the same `NodePath`.
- `ModuleTree::add_module` then fails with `ModuleTreeError::DuplicatePath` because the `path_index` (for definitions) already contains `crate::backend::libc::c::readwrite_pv64v2` when the second module is inserted. See [module_tree.rs](../../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs).

**Evidence**

- Fixture uses `target_os` and `target_env` atoms (and `all/any/not`): [c.rs](../../../../../tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids/member_cfg_duplicate_mods_repro/src/backend/libc/c.rs)
- Duplicate-path detection is unconditional and does not consider `ModuleNode.cfgs` at all during indexing. `ModuleNode` does store `cfgs`, but `ModuleTree::add_module` keys solely on `NodePath`. See [module.rs](../../../../../crates/ingest/syn_parser/src/parser/nodes/module.rs) and [module_tree.rs](../../../../../crates/ingest/syn_parser/src/resolve/module_tree.rs).
- There is a `cfg_eval` feature-gated attempt to skip items in the visitor (`visit_item_mod` checks `should_include_item`), but:
  - it is feature-gated and likely off in normal builds
  - the evaluator is explicitly work-in-progress and currently does not support important atoms like `target_env` (used by this repro), so it cannot reliably disambiguate these modules even when enabled. See [cfg_evaluator.rs](../../../../../crates/ingest/syn_parser/src/parser/visitor/cfg_evaluator.rs) and [attribute_processing.rs](../../../../../crates/ingest/syn_parser/src/parser/visitor/attribute_processing.rs).

**Suggested Fix (No Edits Made)**

1. Decide the intended ingestion semantics for cfg:
“Single active configuration” (drop inactive branches): enable robust cfg evaluation and filter AST items before module-tree build.
“Union of all configurations” (keep everything): represent cfg-disjoint alternatives explicitly in the graph/tree instead of enforcing uniqueness at the canonical path level.
2. If choosing cfg evaluation:
Extend cfg parsing/evaluation to support at least: `target_env`, bare flags like `unix/windows/test/debug_assertions`, and common name-value atoms (`target_pointer_width`, `target_endian`, etc.).
Ensure unknown atoms are handled consistently (either conservative drop, or keep-with-unknown), and avoid “weakening” cfg expressions by silently dropping unsupported atoms from `all/any/not` (currently possible because unsupported metas return `None` during parsing and are filter-mapped away).
Derive the active feature set from enabled features for the target build, not from feature definitions in `Cargo.toml`.
3. If choosing union semantics:
Change the `ModuleTree` indices from `NodePath -> Id` to `NodePath -> Vec<Id>` (or a richer structure) and require downstream resolution to be cfg-aware (e.g., disambiguate by compilation-unit key or by cfg predicates).
This is larger work, but avoids dropping code and matches the reality that `crate::...::readwrite_pv64v2` can legitimately refer to different definitions under different cfgs.

**Confidence:** Medium-High (high on “cfg-disjoint duplicates are not modeled”; medium on which fix direction is preferred for the project).
