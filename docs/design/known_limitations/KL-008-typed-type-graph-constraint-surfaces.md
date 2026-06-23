# KL-008 Some typed type graph constraint surfaces are not emitted as reachable relations

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
const/static types, references, slices, arrays, named generic arguments,
generic declaration bounds, qualified associated type projections, and trait
associated type bounds. Type where-clause predicates are supported for fresh
ingestion, but real-corpus backup contracts for them have not been added yet.

Some Rust constraint surfaces are still parsed or stored only as metadata, or
are not represented precisely enough to become reachable typed graph relations:

- Non-type where-clause predicates, such as lifetime-only predicates.
- Generic defaults and const generic parameter types as queryable type roots.
- Associated const/type items as precise associated-item owners, including
  associated type defaults and impl associated type definitions.

Generic declaration bounds and generic-param-owned bound roots are supported for
fresh parser/transform fixture ingestion and by the regenerated typed corpus
backup fixtures from 2026-05-10. Qualified associated type projections such as
`<Const<N> as IntoArrayLength>::ArrayLength` now preserve and resolve the
`as Trait` qualifier as a trait-position source edge.

Trait associated type bounds such as
`trait TimeZone { type Offset: Offset; }` are now collected on the containing
trait and emitted as `AssociatedTypeBound` type-use roots.

Type where-clause predicates such as `where T: Trait`,
`where Vec<T>: Trait`, and `where <T as Trait>::Assoc: OtherTrait` are now
collected as predicate subjects plus trait-position bounds for fresh ingestion.
The transform emits `WherePredicateSubject` and `WherePredicateBound` roots,
and direct local type-param subjects additionally get `WhereGenericParamBound`
roots on the generic parameter node. Real-corpus backup contracts for these
where-clause surfaces still need to be added/regenerated.

## Symptom

Earlier strict real-corpus DB contracts in
[`corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
failed with `reachable: []` for `TimeZone { type Offset: Offset; } -> Offset`.
That case now passes after regenerating the 2026-05-10 chrono typed backup.

The remaining limitation is not represented by a passing corpus contract yet:
fresh ingestion covers type where-clause predicates, but real-corpus backup
contracts for them have not been added, and precise associated item owners
still do not have complete typed graph surfaces.

## Cause

The current v2 type graph emits roots for ordinary owner/type slots in
[`transform/type_graph.rs`](../../../crates/ingest/ploke-transform/src/transform/type_graph.rs),
including functions, methods, fields, aliases, traits, impls, consts, and
statics. Fresh ingestion now also emits `type_use` roots for generic type
parameter bounds, type where-clause predicates, and trait associated type
bounds. It does not yet emit roots for generic defaults or const generic
parameter types.

Generic parameter bounds are collected as generic-param metadata in
[`visitor/state.rs`](../../../crates/ingest/syn_parser/src/parser/visitor/state.rs)
and persisted as `generic_type.bounds` in
[`transform/secondary_nodes.rs`](../../../crates/ingest/ploke-transform/src/transform/secondary_nodes.rs),
and fresh typed graph ingestion now exposes them through `type_use ->
type_relation` for both containing owners and generic-param owners.

Qualified associated type projections are now represented by preserving the
projection's qualified self type and optional `as Trait` qualifier as structural
children of the named projection node. The resolver walks the qualifier in trait
position and the transform emits `QualifiedSelf` / `QualifiedTrait`
`type_contains` edges.

Trait and impl associated type/const parsing still has explicit TODOs in
[`visitor/code_visitor.rs`](../../../crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs),
so associated type bounds are currently owned by the containing trait rather
than by a precise associated type item node.

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
  - `associated_type_bounds::chrono_backup_timezone_associated_offset_bound_reaches_offset_trait`

Current focused command:

```text
cargo test -p ploke-db type_graph_queries::corpus_contracts -- --nocapture
```

Expected current result: the ordinary, generic-bound, qualified-projection, and
associated-type-bound contracts pass. Fresh where-clause fixtures pass, but
real-corpus where-clause contracts still need to be added/regenerated.

Fresh-ingestion fixture coverage for generic bounds:

- [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`](../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs)
  - `v2_resolves_generic_declaration_bound`
- [`crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs)
  - `generic_bound_roots_are_queryable_from_item_and_param_owners`
- [`crates/ploke-db/tests/unit/type_graph_queries/reachability.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/reachability.rs)
  - `reachable_targets_include_generic_declaration_bound_trait`

Fresh-ingestion fixture coverage for type where-clause predicates:

- [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`](../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs)
  - `v2_resolves_where_direct_type_param_bound`
  - `v2_resolves_where_direct_type_param_subject`
  - `v2_resolves_where_composite_subject_nested_type_param`
  - `v2_resolves_where_projection_subject_trait_qualifier`
- [`crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/direct_roots.rs)
  - `where_predicate_roots_are_queryable_from_item_and_param_owners`
  - `composite_where_predicate_subject_is_not_a_generic_param_bound`
- [`crates/ploke-db/tests/unit/type_graph_queries/reachability.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/reachability.rs)
  - `reachable_targets_include_where_direct_type_param_bound_trait`
  - `reachable_targets_include_where_composite_subject_nested_type_param`

Regenerated real-corpus coverage for generic bounds:

- [`crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - `generic_bounds::generic_array_backup_generic_array_bound_reaches_array_length_trait`
  - `generic_bounds::chrono_backup_date_timezone_bound_reaches_timezone_trait`
  - `generic_bounds::chrono_backup_datetime_timezone_param_bound_source_reaches_timezone_trait`

Fresh-ingestion and regenerated real-corpus coverage for qualified projections:

- [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`](../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs)
  - `v2_resolves_qualified_projection_trait_qualifier`
- [`crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - `qualified_projections::generic_array_backup_const_array_length_projection_reaches_into_array_length_trait`

Fresh-ingestion and regenerated real-corpus coverage for associated type bounds:

- [`crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs`](../../../crates/ingest/syn_parser/tests/uuid_phase3_resolution/type_relations_v2.rs)
  - `v2_resolves_associated_type_bound`
- [`crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs`](../../../crates/ploke-db/tests/unit/type_graph_queries/corpus_contracts.rs)
  - `associated_type_bounds::chrono_backup_timezone_associated_offset_bound_reaches_offset_trait`

## Possible future resolution paths

1. Regenerate typed real-corpus backup DBs so generic declaration-bound corpus
   contracts can move out of the red bucket. Done for the 2026-05-10 typed
   corpus backups.
2. Emit typed graph roots for generic defaults and const generic parameter
   types, with a clear owner policy for containing items vs generic parameter
   nodes.
3. Add and, if needed, regenerate real-corpus backup contracts for type
   where-clause predicates.
4. Parse and persist associated type/const items as precise owners, then emit
   type-use roots for associated type defaults and impl associated type
   definitions.
5. Add strict contracts for remaining surfaces, then update this KL entry until
   it can be resolved.

## Further reading / evidence

- [`docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/`](../../active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/)
- [`docs/active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/`](../../active/agents/2026-05-02_tt-expr-core_type-resolution-test-review/)
