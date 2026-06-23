# Dynamic call extraction plan

Date: 2026-06-22
Status: implemented; paranoid fixture tests green
Scope: record non-path `ExprCall` callees as parser-owned `DynamicCall` sites.

## Goal

Keep dynamic/Fn-like call syntax visible in the parser call graph instead of dropping it because it cannot yet be semantically resolved.

New focused fixture:

```rust
// tests/fixture_crates/fixture_call_graph/src/lib.rs
pub fn dynamic_calls() -> i32 {
    let closure = || 7;
    let from_binding = (closure)();
    let from_literal = (|| 11)();
    from_binding + from_literal
}
```

Expected behavior:

- `(closure)()` is recorded as `CallNode::DynamicCall`.
- `(|| 11)()` is recorded as `CallNode::DynamicCall`.
- Both call sites get deterministic `DynamicCallSiteId`s from owner + kind + span + cfgs.
- Both get `BodyContainsCall` structural edges.
- The resolver emits `Unsupported` and no semantic `CallRelation` for both.

## ID policy

Dynamic call-site IDs use the existing `CallId` universe and `DynamicCallSiteId` wrapper. The identity discriminator is the constant string `dynamic`; owner + span + cfgs distinguish occurrences.

This keeps production ID construction parser-internal while allowing tests to regenerate IDs through the explicit test helper.

## Non-goals

- No closure binding resolution.
- No function pointer resolution.
- No `Fn`/`FnMut`/`FnOnce` trait dispatch.
- No reclassification of path-shaped calls such as `alias_checker(...)`; those remain structural `PathCall` until binding-aware policy exists.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser
cargo test -p syn_parser call_sites -- --nocapture
cargo test -p ploke-transform transform::tests -- --nocapture
```

All commands passed. The `call_sites` filter ran seventeen paranoid fixture tests and all passed.
