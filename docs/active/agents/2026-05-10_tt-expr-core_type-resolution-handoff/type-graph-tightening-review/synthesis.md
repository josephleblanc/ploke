# Type Graph Tightening Review Synthesis

- Date: 2026-05-10
- Inputs:
  - [`reviewer-a-typed-id-api.md`](./reviewer-a-typed-id-api.md)
  - [`reviewer-b-transform-db.md`](./reviewer-b-transform-db.md)
  - [`reviewer-c-tests-fixtures.md`](./reviewer-c-tests-fixtures.md)

## Summary

The reviewers agree that the recent typed graph work is directionally correct at the parser terminal-relation layer, but several surrounding APIs and DB projections are weaker than the semantic model. The main risk is not current green tests; it is future work compiling through broad `AnyNodeId`, raw string roles, or under-specified slot coordinates while violating invariants that the repo already has types to express.

The strongest existing part is the parser-side terminal edge model: `TypeRelation::Ordinary` and `TypeRelation::Trait` preserve distinct typed source/target families. The weakest parts are owner/context typing before relation construction, stringly transform emission, DB enum/schema drift, and tests that assert reachability without proving the exact root/role/source path.

## Findings

### 1. Resolver owner context is too broad

Reviewer A found that `ResolutionContext` and the direct type-use helpers accept `AnyNodeId` even when the actual owner set is narrower.

Impacted surfaces:

- `ResolutionContext::resolution_context_owner`
- `next_callable_type_use`
- `next_where_predicate_type_use`
- `next_generic_bound_type_use`
- `ordinary_type_use_site`
- `trait_type_use_site`
- `resolve_type_generic_param`
- `generic_params_for_owner`
- `associated_owner_for_method`

Why this matters: invalid owner state is representable and can degrade into silent under-resolution instead of failing at the construction boundary. This is exactly the future-footgun pattern we want to prevent.

Tightening task: introduce or reuse typed owner sets for resolver context. `GenericParamOwnerId` should own generic bounds and type where predicates. A broader resolver scope enum may be needed for non-generic const/static roots. `AssociatedItemOwnerId` should be preserved when recovering method owners.

### 2. Transform and DB disagree on `type_contains.kind`

Reviewer B found a concrete contract mismatch: transform emits `QualifiedSelf` and `QualifiedTrait`, while DB `TypeContainmentKind` does not decode those variants.

Why this matters: raw reachability can pass because it treats containment kind as an opaque string, while the public structural API can fail or reject rows from the same fresh transform output.

Tightening task: add typed DB support for qualified projection containment or split qualifier edges into their own typed relation. Add a direct `direct_type_contains` test for qualified projection roots.

### 3. `type_use.slot_index` is overloaded

Reviewer B found that one optional integer currently means different things by role: parameter index, field ordinal, flattened generic-bound counter, predicate index, flattened where-bound index, or generic-param-local bound index.

Why this matters: reachability does not care, but exact slot reconstruction and future UI/provenance work will. Some shapes, such as repeated where predicates on the same generic parameter, can reuse the same slot coordinate.

Tightening task: define and test the role-by-role slot contract. Prefer a `slot_path` or typed slot metadata over one ambiguous `slot_index`. At minimum, add multi-predicate/multi-bound tests that expose current ambiguity before broadening the model further.

### 4. DB projection erases parser endpoint families

Reviewer B found that parser endpoint families are strongly typed, but DB rows store UUID/string fields and public queries do not validate `source_kind` / `target_kind`.

Why this matters: fresh transform output is currently typed because the parser hands it typed inputs, but backup fixtures, manual inserts, migrations, or future transform call sites can introduce invalid family crossings that queries may treat as valid.

Tightening task: add typed transform wrappers for relation kind, type-use role, and containment kind. Add DB invariant tests for invalid ordinary/trait endpoint-family rows, either rejecting them on insert or excluding/surfacing them in public query APIs.

### 5. Associated type bounds are intentionally imprecise

Reviewer B noted that `AssociatedTypeBound` roots are owned by the containing trait, not by a precise associated type item.

Why this matters: this is useful as a GraphRAG convenience edge, but it is not the true syntactic owner. It will become fragile when associated type defaults and impl associated type definitions are added.

Tightening task: keep the trait-owned root as an explicit derived/convenience surface if needed, but add precise associated-item ownership before expanding associated type/default support.

### 6. Tests prove positives more than exact contracts

Reviewer C found that current parser and DB tests often prove that an expected path exists, but not that the path came from the intended root, role, slot, depth, or source set.

Impacted patterns:

- Corpus reachability helpers use `depth >= min_depth`.
- Parser relation tests assert one expected row exists exactly once, but do not reject extra rows for the same source.
- Direct-root DB coverage does not yet cover every `TypeUseRole`.
- Backup corpus tests can remain green while fresh parser/transform ingestion regresses.

Tightening task: add source-scoped exact-set parser assertions, role-aware DB reachability assertions, exact-depth corpus assertions where stable, and one direct-root contract per `TypeUseRole`.

### 7. KL-008 status is misleading

Reviewer C found that KL-008 says type where-clause predicates pass while also saying real-corpus backup contracts still need to be added/regenerated.

Why this matters: future work may treat missing real-corpus where-clause coverage as already complete.

Tightening task: update KL-008 to distinguish fresh-fixture where-clause support from real-corpus backup coverage.

## Task List

### Immediate contract fixes

1. Replace resolver-context `AnyNodeId` with typed owner context.
2. Change generic-bound and where-predicate helper APIs to take `GenericParamOwnerId`.
3. Preserve `AssociatedItemOwnerId` from associated method owner lookup.
4. Add `QualifiedSelf` and `QualifiedTrait` to the DB containment API, or split them into a typed qualifier relation.
5. Replace transform string literals for type graph roles/kinds with typed wrappers at the insertion boundary.

### Test hardening

1. Add parser source-scoped exact-set assertions for type relations.
2. Add negative ordinary/trait family-crossing tests.
3. Add DB direct-root tests for every `TypeUseRole`.
4. Add role-aware reachability helpers that select the expected root before checking targets.
5. Convert stable corpus direct cases from `depth >= min_depth` to exact depth.
6. Add multi-bound and multi-predicate fixtures for where-clause slot/owner separation.
7. Add direct `type_contains` tests for qualified projection containment.
8. Add DB invariant tests for invalid endpoint-family rows.

### Modeling follow-ups

1. Define a durable slot-coordinate model for `type_use`.
2. Add precise associated type item ownership before associated type defaults and impl associated definitions.
3. Add fresh-ingestion coverage for at least one reduced real-corpus-style crate.
4. Correct KL-008 so fresh-fixture support and corpus-backup support are tracked separately.

## Suggested Next Slice

The first implementation slice should be the resolver owner-context tightening, because it directly addresses the root concern: helper APIs should not accept broader IDs than their semantic contract permits. This can be done without changing DB fixture files.

The second slice should fix the `QualifiedSelf` / `QualifiedTrait` DB enum mismatch, because it is a concrete transform/API contract break that tests can expose cleanly.

The third slice should add exact-set and role-aware tests before changing broader slot semantics, so the next implementation changes have sharper failure signals.

## Progress

### Addressed on 2026-05-10

- Resolver context no longer stores a raw `AnyNodeId`; it uses a typed resolver-scope owner that admits generic declaration owners plus const/static roots.
- Generic-bound and where-predicate iterator helpers now take `GenericParamOwnerId` instead of `AnyNodeId`.
- Method associated-owner lookup now preserves `AssociatedItemOwnerId` instead of erasing trait/impl ownership into `AnyNodeId`.
- DB containment decoding now includes `QualifiedSelf` and `QualifiedTrait`.
- A direct DB containment test now pins qualified projection self/trait qualifier rows.
- Transform-side type graph emission now uses private typed role/kind enums at the insertion boundary instead of passing raw string literals from every call site.
- Parser relation tests now have a reusable source-scoped exact-set assertion helper, with initial coverage for qualified projection, associated type bound, where-predicate, generic-supertrait, and nested generic field sources.
- DB reachability tests now have a role/slot-aware helper that first selects the intended `type_use` root and then asserts the reachable target path from that exact root, with explicit exact-vs-minimum depth expectations.
- DB type graph query APIs now validate persisted `type_relation` endpoint families before surfacing paths, with focused tests that inject invalid ordinary/trait family crossings and assert owner/target queries exclude them.

### Still Open

- `type_use.slot_index` still has role-dependent coordinate semantics and needs a durable slot model or stricter role-by-role contract tests.
- Parser relation exact-source coverage should be expanded across more roles; the helper exists, but coverage is not yet comprehensive.
- DB reachability role-aware coverage should be expanded across more roles; the helper exists, but coverage is not yet comprehensive.
- DB persistence still stores endpoint families as UUID/string rows; query-time invariant tests exist, but insert-time rejection or a stronger persisted schema is still open.
- Associated type bounds are still owned by the containing trait as a convenience surface, not by precise associated type item nodes.
- KL-008 still needs a cleanup pass to distinguish fresh-fixture where-clause support from real-corpus backup coverage.
