# Local free-function path-call resolution plan

Date: 2026-06-22
Status: implemented; paranoid fixture test green
Scope: add the first path-call semantic resolver slice for explicit local module-qualified function calls.

## Goal

Resolve a parser-owned structural `PathCall` to a local standalone `FunctionNodeId` when the callee path is explicitly local and module-qualified.

Initial fixture target:

```rust
// tests/fixture_crates/fixture_path_resolution/src/lib.rs
pub mod restricted_vis_mod {
    pub(in crate::restricted_vis_mod) fn restricted_func() {}

    pub mod inner {
        pub fn call_restricted() {
            super::restricted_func();
        }
    }
}
```

Expected call-site behavior:

- `super::restricted_func()` is structurally recorded as `CallNode::PathCall`.
- The resolver starts from the owner function's containing module, interprets `super`, and proves the terminal segment names a local `FunctionNodeId`.
- It emits exactly one typed semantic edge:

```rust
CallRelation::Function {
    source: super_restricted_func_call_site_id,
    target: restricted_func_function_node_id,
}
```

- It emits exactly one resolved status:

```rust
CallResolutionStatus::Resolved {
    source: AnyCallSiteId::Path(super_restricted_func_call_site_id),
    kind: CallResolutionKind::LocalExact,
}
```

## Type-resolution pattern to reuse carefully

`resolve/type_resolution_v2.rs` is the useful model, especially:

1. `Report` + `Summary` output shape.
2. Resolver object over `ParsedCodeGraph + ModuleTree`.
3. Path traversal from a containing module.
4. Handling of `crate`, `self`, and `super` prefixes.
5. Terminal target proof after structural path lookup.
6. Sorted/deduped typed relation output.

But call resolution must not copy type resolution's silent `None` behavior. Every considered call site must receive exactly one status:

- `Resolved(LocalExact)` when a typed local target is proven.
- `Unresolved` when this path-call shape is supported but no local function target is found.
- `Ambiguous` when supported local lookup finds multiple plausible local targets.
- `Unsupported` when the resolver intentionally does not support the path-call shape yet.

## Deliberate narrowness

This slice only supports explicit local path prefixes:

- `crate::...`
- `self::...`
- `super::...`

Unqualified calls such as `foo()` and path-shaped closure calls such as `alias_checker(...)` stay out of scope until local value-binding policy is decided.

External/std/associated-function/constructor-shaped calls remain `Unsupported` for now, including current green rows such as:

- `PathBuf::new()`
- `HashMap::<String, i32>::new()`
- `fs::read_to_string(...)`
- `TupleStruct(1, 2)`

## Implementation steps

1. Add a paranoid call-site test for `super::restricted_func()` in `fixture_path_resolution::restricted_vis_mod::inner::call_restricted`.
2. Extend `CallRelationResolver` with `resolve_path_call(...)`.
3. Add call-specific local path traversal helpers:
   - owner containing-module lookup;
   - `crate`/`self`/`super` prefix handling;
   - intermediate module segment lookup;
   - terminal `FunctionNodeId` proof.
4. Preserve fail-closed behavior for all unsupported path-call shapes.
5. Re-run:

```bash
cargo check -p syn_parser
cargo check -p syn_parser
cargo test -p syn_parser call_sites -- --nocapture
cargo test -p syn_parser type_relations_v2 -- --nocapture
```

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser
cargo test -p syn_parser call_sites -- --nocapture
cargo test -p syn_parser type_relations_v2 -- --nocapture
```

All commands passed. The `call_sites` filter ran twelve paranoid fixture tests and all passed.

## Non-goals

- No unqualified function-call resolution yet.
- No path-to-local-binding / closure call policy yet.
- No associated-function or constructor target family design yet.
- No external call summaries yet.
- No trait dispatch or method receiver typing expansion.
- No shared generic name-resolution utility refactor yet.
