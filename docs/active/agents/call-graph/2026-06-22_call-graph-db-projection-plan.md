# Call graph database projection plan

Date: 2026-06-22
Status: implemented; first transform projection test green
Scope: persist parser-owned structural call sites, call containment edges, semantic call edges, and resolution statuses into Cozo via `ploke-transform`.

## Goal

Make the typed call graph available to downstream database queries instead of keeping it only in parser/resolver memory.

The first vertical database surface should persist four fact families:

1. `call_site`: structural call expression occurrence.
2. `call_site_edge`: body-owner containment edge to a call site.
3. `call_relation`: proven semantic call target edge.
4. `call_resolution_status`: fail-closed status for each considered call site.

This is intentionally parallel to the existing type-relation transform boundary:

```rust
let type_relation_report = resolve_type_relations_after_tree(&parsed_graph, tree)?;
transform_type_relations(db, &type_relation_report)?;
```

Call graph projection should use the same shape:

```rust
let call_resolution_report = resolve_call_relations_after_tree(&parsed_graph, tree)?;
transform_call_sites(db, code_graph.call_sites())?;
transform_call_site_relations(db, code_graph.call_site_relations())?;
transform_call_resolution_report(db, &call_resolution_report)?;
```

## Schema target

### `call_site`

Occurrence facts:

- `id: Uuid`
- `owner_id: Uuid`
- `call_kind: String` (`Path`, `Method`, `Dynamic`, `Macro`)
- `span: [Int; 2]`
- `cfgs: [String]`
- `path: [String]?`
- `method_name: String?`
- `macro_name: String?`
- `receiver_kind: String?`
- `receiver_path: [String]?`
- `arg_count: Int?`
- `generic_arg_count: Int?`

### `call_site_edge`

Structural body-containment facts:

- `source_id: Uuid`
- `target_id: Uuid`
- `relation_kind: String` (`BodyContainsCall`)
- `source_kind: String` (`Function`, `Method`)
- `target_kind: String` (`Path`, `Method`, `Dynamic`, `Macro`)

### `call_relation`

Resolved semantic call target facts:

- `source_id: Uuid`
- `target_id: Uuid`
- `relation_kind: String` (`Function`, `Method`)
- `source_kind: String` (`Path`, `Method`)
- `target_kind: String` (`Function`, `Method`)

### `call_resolution_status`

Resolution outcome facts:

- `source_id: Uuid`
- `source_kind: String`
- `status_kind: String` (`Resolved`, `Unresolved`, `Ambiguous`, `External`, `Unsupported`)
- `resolution_kind: String?` (`LocalExact` for current resolved rows)

## Test target

Use `fixture_path_resolution::restricted_vis_mod::inner::call_restricted` because it already has a resolved local path-call edge:

```rust
super::restricted_func();
```

The transform test should assert that the database contains:

- one `call_site` row for the path call;
- one `call_site_edge` row from `call_restricted` to that call site;
- one `call_relation` row from the call site to `restricted_func`;
- one `call_resolution_status` row with `Resolved` / `LocalExact`.

Also keep the parser/resolver invariant that unsupported rows are statuses, not fake edges; later tests should add explicit unsupported DB checks for `PathBuf::new()` and macro calls.

## Verification result

Completed after implementation:

```bash
cargo check -p ploke-transform
cargo check -p ploke-transform --features typed_type_graph
cargo test -p ploke-transform --features typed_type_graph transform::tests -- --nocapture
cargo test -p syn_parser --features typed_type_graph call_sites -- --nocapture
```

All commands passed. The transform test asserts persisted `call_site`, `call_site_edge`, `call_relation`, and `call_resolution_status` rows for the resolved `super::restricted_func()` path call.

## Non-goals

- No proof-fact projection in this slice.
- No external-summary schema in this slice.
- No cross-crate target resolution in this slice.
- No database query API in `ploke-db` yet; raw Cozo queries in transform tests are sufficient for this projection boundary.
