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

All commands passed. Transform tests assert persisted `call_site`, `call_site_edge`, `call_relation`, and `call_resolution_status` rows for the resolved `super::restricted_func()` path call, the resolved `self.private_method()` method call, and the unsupported/no-edge `PathBuf::new()` path call.

## 2026-06-23 workspace checkpoint and feature-gate correction

The focused verification above was insufficient. A later workspace run showed
that adding the persisted call graph relation family made existing backup DB
fixtures stale, especially typed corpus snapshots that predate `call_relation`.
The failure signature is:

```text
Cannot find requested stored relation 'call_relation'
```

Before this database projection slice can be considered fully integrated, the
fixture/workspace checkpoint from
[`README.md#workspace-checkpoint-and-feature-gate-protocol`](README.md#workspace-checkpoint-and-feature-gate-protocol)
must pass or its failures must be explicitly classified. In particular:

```bash
cargo xtask fixtures ensure --snapshots
cargo run -p xtask --features typed_type_graph -- fixtures regenerate --typed
cargo xtask verify-fixtures
cargo xtask verify-backup-dbs
cargo test --workspace --no-fail-fast
```

Do not ignore tests to land the projection early. If downstream crates or
fixtures are not ready for the new persisted relations in the default build,
keep the projection and consuming tests behind the rollout feature
`call_graph`, and propagate that feature through crate dependencies until the
whole plan is complete and the flag can be removed.

Do not make backup import paths silently tolerate missing `call_relation`; that
would weaken the schema contract. Regenerate or refresh fixtures instead, and
record provider-backed typed embedding fixture blockers separately if
credentials are unavailable. Fixture lifecycle details live in
[`docs/testing/BACKUP_DB_FIXTURES.md`](../../../testing/BACKUP_DB_FIXTURES.md)
and [`docs/how-to/recreate-backup-db-fixtures.md`](../../../how-to/recreate-backup-db-fixtures.md).

## Non-goals

- No proof-fact projection in this slice.
- No external-summary schema in this slice.
- No cross-crate target resolution in this slice.
- No database query API in `ploke-db` yet; raw Cozo queries in transform tests are sufficient for this projection boundary.
