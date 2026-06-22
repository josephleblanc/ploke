# Structural macro-call extraction plan

Date: 2026-06-22
Status: implemented; focused macro-call fixture test green
Scope: broaden structural call-site coverage from method/path calls to macro invocation expressions in function-like bodies.

## Goal

Record macro invocations from original `syn` bodies without attempting expansion or semantic target resolution.

First target fixture row:

```rust
pub fn use_imported_items() {
    let _macro_output = documented_macro!(fixture alias coverage);
}
```

Expected first behavior:

- `documented_macro!(...)` is observed as `syn::ExprMacro`.
- Statement-position macros such as `println!(...)` are observed as `syn::StmtMacro`.
- The parser emits one `CallNode::MacroCall` owned by `CallBodyOwnerId::Function(use_imported_items_id)` for each macro invocation site.
- The macro call records:
  - macro name/path `documented_macro`;
  - source byte span;
  - deterministic `MacroCallSiteId` from owner + macro discriminator + span + cfgs;
  - one `BodyContainsCall` relation.
- No macro expansion, resolved call edge, or proof-effect projection is attempted in this slice.

## Implementation steps

1. Add a focused RED test in `uuid_phase3_resolution/call_sites.rs`.
2. Add parser-internal `generate_macro_call_site_id(...)` over `CallId`.
3. Extend `call_extraction.rs` with `visit_expr_macro` and `visit_stmt_macro` handling.
4. Record `MacroCallNode` and `BodyContainsCall`.
5. Re-run focused call-site tests and existing type-relation baseline.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
```

All commands passed. The `call_sites` filter now runs eleven paranoid fixture tests and all passed.

## Non-goals

- No macro expansion.
- No call extraction from expanded macro output.
- No macro target resolution.
- No build-script/proc-macro backend integration.
