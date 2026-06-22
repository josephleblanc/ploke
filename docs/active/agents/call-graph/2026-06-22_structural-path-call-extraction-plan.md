# Structural path-call extraction plan

Date: 2026-06-22
Status: implemented; focused path-call fixture test green
Scope: broaden structural call-site coverage from `self.method()` calls to path-style call expressions.

## Goal

Record path-style calls from original `syn` bodies without adding new semantic resolution behavior.

First target fixture row:

```rust
pub fn demonstrate_new_module() -> (NewTestStruct, NewTestEnum) {
    let test_struct = NewTestStruct::new(
        format!("Test item #{}", counter),
        75
    );
    // ...
}
```

Expected first behavior:

- `NewTestStruct::new(...)` is observed as `syn::ExprCall` with an `ExprPath` callee.
- The parser emits one `CallNode::PathCall` owned by `CallBodyOwnerId::Function(demonstrate_new_module_id)`.
- The path call records:
  - path `NewTestStruct::new`;
  - arg count `2`;
  - generic arg count `0`;
  - deterministic `PathCallSiteId` from owner + path discriminator + span + cfgs;
  - one `BodyContainsCall` relation.
- No `CallRelation::Function` / associated-function edge is emitted in this slice.

## Implementation steps

1. Add a focused RED test in `uuid_phase3_resolution/call_sites.rs`.
2. Add parser-internal `generate_path_call_site_id(...)` over `CallId`.
3. Extend `call_extraction.rs` with `visit_expr_call` handling where the callee is `syn::Expr::Path`.
4. Record `PathCallNode` and `BodyContainsCall`.
5. Leave dynamic call and macro call extraction for later slices.
6. Re-run focused call-site tests and existing type-relation baseline.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
```

All commands passed. The `call_sites` filter ran three focused tests and all passed.

## Non-goals

- No path-call semantic resolution.
- No `Self::new()` / associated function target edge yet.
- No free-function resolver yet.
- No macro extraction for `format!` yet.
- No dynamic callee extraction yet.
