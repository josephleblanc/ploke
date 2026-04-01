# ADR-025: Stage scan-based file module duplicates in `ModuleTree` until declaration and `#[path]` linking

## Status

ACCEPTED (2026-03-30)

## Context

`syn_parser` discovers many `*.rs` files under a crate `src/` tree and parses each file independently. For every file it creates a **file-root** `ModuleNode` whose logical path is derived from the file’s location under `src/` (see [ADR-005](ADR-005-derive-logical-module-paths.md)).

That model is fast and parallel-friendly, but it can introduce **extra file-root module definitions** that rustc would never treat as reachable module definitions for the same canonical path. A concrete case:

- A cfg-gated **inline** `mod foo {}` and a **separate** `foo.rs` on disk can both produce **two definition** `ModuleNode`s whose `NodePath` is `crate::...::foo` (e.g. the `member_quantized_metal_repro` fixture and `huggingface/candle`’s `quantized` / `metal` pattern).

`ModuleTree::add_module` indexes **definitions** in `path_index: HashMap<NodePath, AnyNodeId>` and, before staging, errored on `DuplicatePath` as soon as a second definition was inserted. Pruning and linkage that could drop **unlinked** file roots run **after** that step, so the pipeline failed before recovery was possible.

A naive local rule such as **“inline always wins over file-based at `add_module` time”** is also unsafe:

- File roots are keyed by **filesystem-derived** logical paths **before** `#[path]` resolution.
- A declaration like `#[path = "path/to/dup.rs"] mod not_dup;` must eventually associate that file with the canonical module path `crate::...::not_dup`, not `crate::...::dup`. Dropping a colliding file root too early can remove a **valid** definition that a later `#[path]` / declaration linking pass would reindex.

We therefore use an explicit place to hold **candidate** file-based definitions that collide on `NodePath` until **module declarations** and **`#[path]`** processing have run, then either **drop** the unlinked scan root or **surface a real error** if both definitions remain legitimately incompatible.

## Decision

1. A dedicated field on `ModuleTree`, **`staged_file_definition_collisions`**, records **file-based** module definitions that could not be inserted into `path_index` because the key was already occupied by another **definition** (typically inline, or another file-root in pathological cases).

2. **Do not** resolve these collisions at first insertion with a fixed “winner” heuristic. Instead:
   - after **module declaration linking** and **`update_path_index_for_custom_paths`** (and any related path-index reconciliation), and
   - before or as part of existing **prune** / finalization,
   run a **finalize staged file duplicates** step that:
   - removes or reindexes **unlinked** scan-based file roots that were only colliding due to discovery order;
   - clears the staging structure so no downstream code sees stale state;
   - preserves **hard errors** when two incompatible definitions would still remain after linkage (invalid Rust or inconsistent graph).

3. Treat **`path_index` as authoritative only after** this finalization (or document any interim reads as explicitly unsupported).

4. **Explicitly out of scope for this ADR:** a **union-of-all-cfgs** graph where `NodePath` maps to multiple simultaneously-valid definitions. Staging is a **single-winner, single-configuration-era** mechanism to unblock merge and pruning; future union semantics would likely require a richer key (e.g. `NodePath` + cfg predicate or `CompilationUnitKey`) and may **replace** this field rather than extend it ad hoc.

   Documented user-visible cases (duplicate **inline** modules under disjoint cfgs) live in [**syn_parser known limitations**](../../syn_parser_known_limitations.md) (L1) and [**KL-003**](../../known_limitations/KL-003-cfg-disjoint-duplicate-inline-mod.md), with repro tests linked there.

## Consequences

- **Positive:**
  - Unblocks `ModuleTree` construction for valid crates that combine cfg-gated inline modules with scan-discovered sibling files (candle-style patterns).
  - Avoids incorrect early drops that ignore `#[path]`-driven canonical path reassignment.
  - Keeps the current contract that `path_index` maps **one canonical definition path → one id** after finalization, so existing consumers (`path_resolver`, `CanonIdResolver`, etc.) do not need immediate wide refactors.

- **Negative:**
  - More state and ordering constraints on `ModuleTree` build: staging must be cleared in every exit path (prune, errors, tests).
  - Risk of subtle bugs if a new pass reads `path_index` “too early” or omits clearing staging.
  - Does not fix **wrong cfg inclusion** (e.g. items included when rustc would exclude them); staging only resolves **duplicate path collisions** given whatever graph was built.

- **Neutral:**
  - Small extra memory for staged entries (typically tiny vs parse cost).
  - Serves as an incremental step toward **decl-driven** or **reachability-driven** file discovery; ADR should be revisited if discovery moves to rustc-like module graph expansion.

## Regression tests (recommended)

- **Fixture / corpus:** `member_quantized_metal_repro` (or equivalent): cfg-gated inline `mod metal` + `metal.rs` scan root → merge succeeds after staging + finalize.
- **`#[path]` vs scan-derived stem:** `#[path = ".../dup.rs"] mod not_dup;` + inline `mod dup {}` + `src/.../dup.rs` scanned as a file root → ensure file root is not dropped until after path reindex; final tree matches intended canonical paths.
- **`#[cfg(test)]` / cfg quirks:** guard against resolving collisions in a way that drops the non-test module when `cfg_eval` behavior differs from `cargo build` (see [ADR-021](ADR-021-always-cfg-test.md) for related cfg semantics).

## Compliance

- Aligns with phased parsing and deferred module resolution ([ADR-006](ADR-006-deffered-relation-linking.md)).
- Builds on filesystem-derived logical paths ([ADR-005](ADR-005-derive-logical-module-paths.md)) while acknowledging that **`#[path]`** can change the canonical mapping after Phase 2.

## Related

- [`syn_parser` known limitations](../../syn_parser_known_limitations.md) — L1 (cfg-disjoint duplicate inline `mod` paths); KL-track entry [KL-003](../../known_limitations/KL-003-cfg-disjoint-duplicate-inline-mod.md).
- Root-cause analysis: `docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_cfg_gates_report.md` (historical failure `repro_duplicate_quantized_metal_mod_merge_error`, now covered by a success repro).

## Implementation notes (2026-03-30)

- **`ModuleTree::staged_file_definition_collisions`:** `Vec<ModuleNodeId>` listing file-based definitions that have no `path_index` entry at their filesystem-derived `NodePath` because the other side of the collision was inline (or the inline displaced a file already indexed). Eligibility is strict: two file roots or two inlines still produce `DuplicatePath`.
- **`add_module`:** On `path_index` collision between file and inline, the inline keeps the stem unless the file was indexed first and the inline arrives second (inline replaces `path_index`; the file id is staged). If the file arrives second, it is staged and `path_index` is unchanged.
- **`update_path_index_for_custom_paths`:** When removing the filesystem path before inserting the canonical `#[path]` key, the stem may still be occupied by the other definition (e.g. inline). If the definition module id is still listed in `staged_file_definition_collisions`, that mismatch is treated as expected stem collision rather than `path_index` corruption.
- **`prune_unlinked_file_modules`:** After pruning, staging entries are dropped for modules that were removed from `modules`.
- **`feature = "validate"`:** `debug_validate_staging_after_custom_path_reindex` and `debug_validate_staging_empty_after_prune` (see `ParsedCodeGraph::build_module_tree_from_root_module`) assert staging invariants when the validate feature is enabled.
- **Tests:** Success repros under `crates/ingest/syn_parser/tests/repro/success/` include `member_quantized_metal_repro`, inline+file duplicates (`member_logging_inline_file_repro`, `member_image_inline_file_repro`, `member_protos_default_index_repro`), and `member_path_stem_collision_repro` (`#[path = "dup.rs"] mod not_dup` + inline `mod dup` + scan `dup.rs`). Remaining duplicate-path **failure** repros in `tests/repro/fail/` assert collisions **outside** this ADR (e.g. two inline modules at the same path).
