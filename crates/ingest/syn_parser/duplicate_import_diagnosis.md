Title: Duplicate Import Nodes Causing Relation Validation Panic when Parsing ploke-tui

Summary
- Running `cargo test -p syn_parser` fails in `full::parse_self::new_parse_tui` with a panic from relation validation.
- Root cause is duplicate Import nodes created for the same item within the same module in `ploke-tui`.
- This is not caused by function-body parsing; `syn_parser` does not recurse into function blocks.

Reproduction Evidence
- Command: `cargo test -p syn_parser --test mod full::parse_self::new_parse_tui -- --nocapture --test-threads=1`.
- With logging (`RUST_LOG=temp=debug,debug_dup=debug`), duplicate relations are reported:
  - `DUPLICATE: Contains(S:255b692b..35abfb27 → Import(S:929a473d..b619dba0))`
  - `DUPLICATE: ModuleImports(S:255b692b..35abfb27 → S:929a473d..b619dba0)`
- Tracing the Import ID shows it maps to `crate::llm::router_only::ApiRoute` in module `manager`:
  - `ImportNode { id: ...929a473d..., source_path: ["crate","llm","router_only","ApiRoute"], visible_name: "ApiRoute" }`
  - `ImportNode { id: ...929a473d..., source_path: ["crate","llm","router_only","ApiRoute"], visible_name: "_", original_name: Some("ApiRoute") }`
- The duplicates originate from two import statements in the same module (file `crates/ploke-tui/src/llm/manager/mod.rs`):
  - `use crate::llm::router_only::{ApiRoute, Router};`
  - `use crate::llm::router_only::ApiRoute as _;`

Why It Panics
- Import node IDs are derived via `NodeId::generate_synthetic` using:
  - crate namespace, file path, module path, item name, item kind, parent scope ID, and cfg bytes.
- We intentionally normalize `use ... as _` by using the original name for ID generation so `_` aliases hash the same as the unaliased import.
- In the same module, both the direct import and the `_` alias generate the same `ImportNodeId`.
- `visit_item_use` unconditionally pushes both Import nodes and both `Contains` and `ModuleImports` relations.
- `GraphAccess::validate_unique_rels` detects the duplicate relations and, when resolving nodes for debugging, hits `find_node_unique` which finds more than one `ImportNode` with the same ID and panics with `Duplicate node found for ID AnyNodeId::Import(...)`.

Clarification on Function-Scope Imports
- The initial hypothesis suggested duplicates from function-block `use` statements.
- In our visitor:
  - `visit_item_fn` does not call `visit::visit_item_fn`, so function bodies are not descended into.
  - `visit_item_use` early-returns unless the current primary scope is a module.
- Therefore, function-scope `use` statements are not parsed and are not the source of this failure.

Fix Options
- Option A (Recommended): Deduplicate Imports at Insertion Time
  - Before appending to `graph.use_statements` and `module.imports`, check for an existing `ImportNodeId` in the current module and skip inserting duplicates.
  - Similarly, avoid pushing duplicate `SyntacticRelation::Contains` and `SyntacticRelation::ModuleImports` entries for the same `(module_id, import_id)` pair.
  - Pros: Minimal change, preserves ID semantics, prevents duplicates early.
  - Cons: Requires small checks in `visit_item_use` (and `visit_item_extern_crate`).

- Option B: Tolerate Duplicate Import Relations in Validation
  - Extend `validate_unique_rels` to treat duplicate `Contains`/`ModuleImports` whose targets are `ImportNodeId` as non-fatal (similar to the impl special-case).
  - Also skip the `find_node_unique` resolution for such cases to avoid duplicate-node panic.
  - Pros: Quick mitigation without touching the visitor.
  - Cons: Duplicates remain in memory and may affect downstream processing; harder to reason about the graph.

- Option C: Do Not Materialize `use ... as _` Imports
  - Skip creating an `ImportNode` for `_`-aliased imports when an equivalent non-aliased import exists in the same module.
  - Pros: Avoids duplicates with minimal surface.
  - Cons: Loses explicit representation of `_` usage; slightly special-cases import handling.

- Option D (Not Recommended): Change Import NodeId to Include Span
  - Making `ImportNodeId` include the statement span would make repeated identical imports produce distinct IDs.
  - Cons: Diverges from current deterministic ID design that intentionally abstracts away spans for syntactic identity; complicates dedup across files/modules.

Suggested Path
- Implement Option A: add module-scoped dedup checks during `visit_item_use` (and `visit_item_extern_crate`) for both nodes and relations.
- Add tests:
  - Duplicate top-level imports (`use X;` + `use X as _;`) produce a single Import node and no duplicate relations.
  - Repeated identical `use X;` lines in the same module do not crash and dedup as above.
  - Assert that function-scope `use` statements are ignored (no Import nodes created) to guard against regressions.

Status
- Assessment complete: The failure is due to duplicate top-level imports in the same module (not function-scope parsing).
- Ready to implement Option A and add targeted tests upon approval.

