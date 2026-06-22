# External-root path-call classification plan

Date: 2026-06-22
Status: implemented; paranoid fixture test green
Scope: distinguish direct external-root path calls from unsupported parser shapes.

## Goal

Move path calls with explicit external roots from `Unsupported` to `External` without emitting local semantic edges.

Initial fixture target:

```rust
std::path::Path::new("")
```

inside `fixture_path_resolution::root_func`.

Expected behavior:

- Structural extraction records `CallNode::PathCall` with path `std::path::Path::new`.
- The call resolver emits exactly one `CallResolutionStatus::External` for that call site.
- The resolver emits no `CallRelation::Function` or other local edge.

## Classification rule for this slice

A path call is `External` when its first segment is one of:

- `std`
- `core`
- `alloc`
- a declared dependency name from the parsed graph

This deliberately does **not** attempt import-alias external classification yet. Calls such as `PathBuf::new()` and `HashMap::new()` remain `Unsupported` until the resolver follows imports/re-exports or an external-summary catalog exists.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
cargo test -p ploke-transform --features typed_type_graph transform::tests -- --nocapture
```

All commands passed. The `call_sites` filter ran thirteen paranoid fixture tests and all passed.

## Non-goals

- No external summary IDs yet.
- No import alias classification yet.
- No associated-function target families yet.
- No local edge for external calls.
- No proof-fact blocker projection yet.
