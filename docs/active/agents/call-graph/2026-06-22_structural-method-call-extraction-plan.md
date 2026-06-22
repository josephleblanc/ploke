# Structural method-call extraction plan

Date: 2026-06-22
Status: implemented; focused fixture test green
Scope: make `syn_parser` emit the first parser-owned structural call-site fact for `fixture_nodes::SimpleStruct::public_method`.

## Goal

Green the focused structural call-site test without adding semantic resolution. This row is now covered by the paranoid call-site harness:

```bash
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_and_resolves_self_private_method_call_site -- --nocapture
```

Expected green behavior:

- `self.private_method()` is observed from the original `syn` method body as `ExprMethodCall`.
- The parser emits exactly one `CallNode::MethodCall` owned by `CallBodyOwnerId::Method(public_method_id)`.
- The method call records:
  - `method_name = "private_method"`;
  - `receiver = MethodCallReceiver::SelfValue`;
  - `arg_count = 0`;
  - `generic_arg_count = 0`;
  - byte span `(721, 742)`;
  - empty cfgs for this fixture row.
- The parser emits exactly one `CallSiteRelation::BodyContainsCall` from the body owner to the call site.
- No `CallRelation` or resolution status is introduced in this slice.

## Implementation steps

1. Add a small body-call visitor under `parser::visitor` that walks `syn::Block` values.
2. In this first slice, record only `ExprMethodCall` expressions whose receiver is the literal `self` value.
3. Use parser-internal `generate_method_call_site_id(...)`; do not expose a production call-site constructor.
4. Wire extraction into standalone function bodies and impl/trait method bodies after their typed owner IDs are known.
5. Extend `CodeGraph.call_sites` and `CodeGraph.call_site_relations` with the extracted facts.
6. Re-run the focused call-site test and typed parser checks.

## Non-goals

- No local/free-function call extraction yet.
- No path call, dynamic call, or macro call extraction yet.
- No semantic target resolution yet.
- No transform/database/proof projection yet.

## Verification result

Completed after implementation:

```bash
cargo check -p syn_parser
cargo check -p syn_parser --features typed_type_graph
cargo test -p syn_parser --features typed_type_graph type_relations_v2 -- --nocapture
cargo test -p syn_parser --features typed_type_graph fixture_nodes_public_method_records_and_resolves_self_private_method_call_site -- --nocapture
```

All commands passed. This row is now covered by the paranoid call-site harness under `fixture_nodes_public_method_records_and_resolves_self_private_method_call_site`.

## Next slice after green

Add typed call-resolution storage/statuses and a RED resolver test for exact inherent `self.method()` resolution before implementing the resolver.
