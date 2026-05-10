# KL-008 Typed type graph constraint surfaces are not emitted as reachable relations

## Lifecycle

- **discovered:** 2026-05-10
- **reproduced:** 2026-05-10 ([`corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs))
- **deferred:** N/A
- **resolved:** N/A

## Description

The `typed_type_graph` pipeline currently resolves ordinary type-use surfaces
well when they are represented as roots in `type_use` and terminal nodes in
`type_relation`. Real-corpus backup contracts now pass for function params and
returns, method params and returns, fields, type aliases, impl self/trait roots,
const/static types, references, slices, arrays, and named generic arguments.

Several Rust constraint surfaces are still parsed or stored only as metadata, or
are not represented precisely enough to become reachable typed graph relations:

- Qualified associated type projections such as
  `<Const<N> as IntoArrayLength>::ArrayLength`.
- Associated type bounds such as `trait TimeZone { type Offset: Offset; }`.
- Where-clause predicates and associated const/type items more generally.

Generic declaration bounds and generic-param-owned bound roots are supported for
fresh parser/transform fixture ingestion and by the regenerated typed corpus
backup fixtures from 2026-05-10.

## Symptom

The strict real-corpus DB contracts in
[`corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
fail with `reachable: []` for the `constraint_surfaces_red` module:

- `ConstArrayLength = <Const<N> as IntoArrayLength>::ArrayLength` does not reach
  `IntoArrayLength`.
- `TimeZone { type Offset: Offset; }` does not reach `Offset`.

The owner rows and target rows exist in the backup DBs. The missing piece is the
typed graph path between them.

## Cause

The current v2 type graph emits roots for ordinary owner/type slots in
[`transform/type_graph.rs`](../../../crates/ingest/ploke-transform/src/transform/type_graph.rs),
including functions, methods, fields, aliases, traits, impls, consts, and
statics. Fresh ingestion now also emits `type_use` roots for generic type
parameter bounds. It does not yet emit roots for generic defaults, const generic
parameter types, or where-clause predicates.

Generic parameter bounds are collected as generic-param metadata in
[`visitor/state.rs`](../../../crates/ingest/syn_parser/src/parser/visitor/state.rs)
and persisted as `generic_type.bounds` in
[`transform/secondary_nodes.rs`](../../../crates/ingest/ploke-transform/src/transform/secondary_nodes.rs),
and fresh typed graph ingestion now exposes them through `type_use ->
type_relation` for both containing owners and generic-param owners.

Qualified associated type projection support is also incomplete. The type
processing path records nested associated type values, but does not preserve and
resolve the `as Trait` qualifier as a trait-position source edge for the typed
graph.

Trait and impl associated type/const parsing still has explicit TODOs in
[`visitor/code_visitor.rs`](../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs),
so associated type bounds do not have precise associated-item owners yet.

## Relationship to other work

This limitation is specific to the `typed_type_graph` v2 relation model. It does
not invalidate the ordinary `type_use`, `type_contains`, and `type_relation`
paths that already pass over real backup fixtures.

The current restart context is tracked in
[`2026-05-10_current-type-resolution-state.md`](../../active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/2026-05-10_current-type-resolution-state.md).

## Current policy

- Keep strict tests red for missing constraint-surface behavior; do not weaken
  assertions to accept absent graph paths.
- Do not silently synthesize projection or associated-item edges without a
  representation decision for their owners and roles.
- Prefer adding exact corpus contracts before implementing each missing surface,
  so limitations can be removed one by one when the graph emits real paths.

## Repro tests / fixtures

- [`crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - `constraint_surfaces_red::generic_array_backup_const_array_length_projection_reaches_into_array_length_trait`
  - `constraint_surfaces_red::chrono_backup_timezone_associated_offset_bound_reaches_offset_trait`

Current focused command:

```text
cargo test -p ploke-db --features typed_type_graph type_graph_queries::corpus_contracts -- --nocapture
```

Expected current result: the non-constraint contracts and generic-bound
contracts pass, and the two remaining `constraint_surfaces_red::*` contracts
fail with `reachable: []`.

Fresh-ingestion fixture coverage for generic bounds:

- [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`](../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs)
  - `v2_resolves_generic_declaration_bound`
- [`crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs)
  - `generic_bound_roots_are_queryable_from_item_and_param_owners`
- [`crates/ploke-db/tests/unit/type_graph_queries/reachability.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/reachability.rs)
  - `reachable_targets_include_generic_declaration_bound_trait`

Regenerated real-corpus coverage for generic bounds:

- [`crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - `generic_bounds::generic_array_backup_generic_array_bound_reaches_array_length_trait`
  - `generic_bounds::chrono_backup_date_timezone_bound_reaches_timezone_trait`
  - `generic_bounds::chrono_backup_datetime_timezone_param_bound_source_reaches_timezone_trait`

## Possible future resolution paths

1. Regenerate typed real-corpus backup DBs so generic declaration-bound corpus
   contracts can move out of the red bucket. Done for the 2026-05-10 typed
   corpus backups.
2. Emit typed graph roots for generic defaults and const generic parameter
   types, with a clear owner policy for containing items vs generic parameter
   nodes.
3. Model where-clause predicates as first-class bound surfaces rather than
   treating them as unstructured metadata.
4. Preserve qualified associated type projection qualifiers and resolve
   `<T as Trait>::Assoc` to the `Trait` target in trait position.
5. Parse and persist associated type/const items as precise owners, then emit
   type-use roots for associated type bounds and defaults.
6. Promote passing red contracts out of `constraint_surfaces_red` as each
   surface becomes supported, and update this KL entry until it can be resolved.

## Further reading / evidence

- [`docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/`](../../active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/)
- [`docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/`](../../active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/)
