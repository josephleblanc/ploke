# Current Type Resolution Workflow

- Date: 2026-05-10
- Task title: Current typed type-graph workflow
- Task description: Durable restart note for how to continue the `typed_type_graph` type-resolution work without losing the current implementation strategy.
- Related planning files: `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/2026-05-10_current-type-resolution-state.md`, `docs/design/known_limitations/KL-008-typed-type-graph-constraint-surfaces.md`

## Working Model

Continue from the v2 typed graph model rather than reviving direct slot mutation as the primary abstraction. The current semantic shape is:

- `type_use`: owner item to root type-use slot.
- `type_contains`: structural nesting from one type node to another.
- `type_relation`: terminal type-use source to resolved target item.

This keeps composite type structure intact while still giving query code durable resolved-target edges. Avoid treating `TypeId::Synthetic -> TypeId::Resolved` rewriting as the main persisted model unless a future design explicitly reintroduces it as a derived cache.

## Workflow

1. Add or preserve strict contracts before implementation work.
2. Prefer real-corpus backup contracts for DB behavior and parser fixture contracts for exact source/target semantics.
3. Let missing behavior stay red with concrete assertions; do not add stub-tolerant or tautological tests.
4. Classify every red bucket as either an implementation target or a documented known limitation.
5. Implement one semantic surface at a time, then rerun the narrow focused tests through a sub-agent.
6. When a red bucket turns green, update the known-limitations doc and the corpus coverage table in the test file.

## Current Red Bucket

The current intentionally red bucket is documented as `KL-008`: typed graph constraint surfaces.

The known failing surfaces are:

- Qualified associated type projections, for example `<Const<N> as IntoArrayLength>::ArrayLength`.
- Associated type bounds, for example `trait TimeZone { type Offset: Offset; }`.
- Where-clause predicates, which are not fully covered by the current red corpus contracts yet.

Generic declaration bounds and generic-param-owned bound roots are supported for
fresh fixture ingestion and by the regenerated 2026-05-10 typed corpus backup
fixtures.

## Current Implementation Status

The generic declaration-bound slice has been implemented for fresh parses:

- The parser-side v2 resolver walks generic type-parameter bounds as trait-position type-use sites.
- The transform layer emits `GenericBound` roots from containing owners and `GenericParamBound` roots from generic parameter owners.
- DB-facing `TypeUseRole` recognizes both new roles.
- Fresh fixture tests cover parser relation emission, direct root rows, and reachability from both owner surfaces.

The generic declaration-bound corpus contracts have moved out of
`constraint_surfaces_red` after regenerating the typed backup DB fixtures. The
remaining red corpus contracts are projection and associated-type-bound
surfaces.

The next implementation bucket should be chosen from the remaining `KL-008` surfaces: where-clause predicates, qualified associated type projections, or associated type bounds.

## Generic-Bound Owner Policy

The current red tests assert two related but different queries:

- Containing-item reachability, for example `GenericArray -> ArrayLength` and `Date -> TimeZone`.
- Generic-param-owned reachability, for example `DateTime::Tz -> TimeZone`.

Do not accidentally satisfy only one of these if the intended graph surface needs both. A plausible first policy is to emit roots for both the containing generic owner and the generic parameter node, pointing at the same bound root. The containing owner is useful for GraphRAG item expansion; the generic parameter owner is the precise declaration site.

Resolution context should still be the containing item or associated owner when resolving the bound path, because that is where module scope, `Self`, and surrounding generic parameters are looked up.

There is already a parser-side `GenericRelation` enum that describes generic declaration structure such as `DeclaresParam`, `TypeBound`, `TypeDefault`, and `ConstParamType`. It is not currently the DB traversal surface. Before introducing a separate persisted generic-relation path, decide whether these facts should remain structural metadata or become first-class graph query relations alongside `type_use`, `type_contains`, and `type_relation`.

## Guardrails

- Do not relax schema import, backup fixture loading, or correctness validation to make stale fixtures pass.
- Do not silently skip unresolved type surfaces; keep missing paths visible through red tests or known-limitations docs.
- Use sub-agents for focused test execution as required by `AGENTS.md`.
- Keep this workflow doc and the handoff README in sync when the active red bucket or implementation strategy changes.
