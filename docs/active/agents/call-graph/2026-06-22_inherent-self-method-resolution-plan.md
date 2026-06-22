# Inherent `self.method()` call-resolution plan

Date: 2026-06-22
Status: implemented; focused resolver test green
Scope: add the first semantic call edge for `fixture_nodes::SimpleStruct::public_method` after structural call-site extraction.

## Goal

Green the focused resolver test:

```bash
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_resolves_self_private_method_edge -- --nocapture
```

Expected behavior:

- The structural `MethodCallSiteId` for `self.private_method()` remains owned by `public_method`.
- The resolver proves the owner method belongs to an inherent impl via `ImplAssociatedItem`.
- The resolver searches methods in the same impl by exact method name.
- It emits exactly one typed semantic edge:

```rust
CallRelation::Method {
    source: self_private_method_call_site_id,
    target: private_method_method_node_id,
}
```

- It emits exactly one resolved status:

```rust
CallResolutionStatus::Resolved {
    source: AnyCallSiteId::Method(self_private_method_call_site_id),
    kind: CallResolutionKind::LocalExact,
}
```

- It does not emit unresolved, ambiguous, external, or unsupported status for that same call site.

## Implementation steps

1. Add typed call-resolution relation/status types in `parser::relations`.
2. Add graph storage/accessors for future persisted call-resolution facts:
   - `CodeGraph.call_relations`
   - `CodeGraph.call_resolution_statuses`
3. Add a resolver module under `resolve::call_resolution` with:
   - `CallResolutionReport`
   - `CallResolutionSummary`
   - `CallRelationResolver`
   - `resolve_call_relations_after_tree(...)`
4. Implement only the first semantic proof:
   - method call;
   - literal `self` receiver;
   - owner is a method;
   - owner method belongs to an inherent impl;
   - target is the unique method in the same impl with the same name.
5. Fail closed for everything else in this slice.

## Non-goals

- No trait dispatch.
- No non-`self` receiver typing.
- No path/free-function call resolution.
- No associated function resolution.
- No external summaries.
- No macro expansion or dynamic-call target inference.
- No transform/database/proof projection yet.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
```

All commands passed. The `call_sites` filter ran both structural and resolver tests and both passed.

## Next slice

Broaden structural coverage before broad semantic coverage. Recommended next RED tests:

1. structural path call extraction for a simple local `helper()` or `Type::new()` fixture;
2. local free-function path-call resolution only after path-call structure is green;
3. associated function path-call resolution (`Self::new()` / `SimpleStruct::new()`) after a dedicated structural path-call test.
