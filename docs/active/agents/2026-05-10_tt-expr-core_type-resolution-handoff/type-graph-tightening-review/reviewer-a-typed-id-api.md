# Reviewer A: Typed-ID/API Boundary Review

- Date: 2026-05-10
- Task title: Type graph tightening review, reviewer A
- Task description: Review typed-ID/API-boundary correctness in recent `typed_type_graph` and type-resolution work, focusing on lax APIs that accept broad IDs where the semantic contract is narrower.
- Related planning files: `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/README.md`, `docs/active/agents/2026-05-10_tt-expr-core_type-resolution-handoff/type-graph-tightening-review/README.md`

## Findings

### High: resolution context erases the lexical/generic scope owner set into `AnyNodeId`

- Evidence: `ResolutionContext` stores `resolution_context_owner: AnyNodeId` in `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:72`.
- Evidence: that broad value is then used for generic lookup in `resolve_ordinary_source` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:150`, `resolve_type_generic_param` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:487`, and `generic_params_for_owner` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:555`.
- Evidence: the ID layer already defines the narrower generic-owner set at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs:62` and implements `GenericParamOwnerId` at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs:1249`.

The weakened invariant is that generic-parameter lookup is valid only in a lexical/generic scope owner, but the resolver accepts any graph node and relies on `match` fallthrough to return `None`. That makes invalid caller state representable: a `FieldNodeId`, `ParamNodeId`, `ImportNodeId`, or `UnresolvedNodeId` can be threaded through a type-use site and will silently suppress generic-param resolution rather than fail at the construction boundary.

Recommended tightening tasks:

- Introduce a narrow resolver-context owner type instead of `AnyNodeId`. The existing `GenericParamOwnerId` is close but does not include top-level `ConstNodeId` and `StaticNodeId`, which are valid type-use resolution scopes with no generics; a dedicated `TypeResolutionScopeOwnerId = GenericParamOwnerId | ConstNodeId | StaticNodeId` would match current roots.
- Change `ResolutionContext::resolution_context_owner`, `ordinary_type_use_site`, `trait_type_use_site`, and resolver iteration helpers to carry that narrow type.
- Keep explicit widening to `AnyNodeId` only at topology/index lookups such as `containing_module`.
- Make failed construction of a resolver context an internal error at the boundary, not a generic lookup miss.

### High: generic-owner helpers accept `AnyNodeId` and convert wrong-owner calls into ordinary misses

- Evidence: `resolve_type_generic_param(owner: AnyNodeId, ...)` accepts any node at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:487`.
- Evidence: `resolve_type_generic_param_in_owner(owner: AnyNodeId, ...)` accepts any node at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:509`.
- Evidence: `generic_params_for_owner(owner: AnyNodeId)` encodes the real accepted set by a `match` over function, method, struct, enum, union, type alias, trait, and impl, then returns `None` for all other variants at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:555`.

The weakened invariant is that "this owner can declare generic parameters" is left to caller discipline even though `GenericParamOwnerId` already models that set. The current API cannot distinguish "owner has no matching `T`" from "caller passed a node that cannot own generic params". That matters for tightening because a wrong owner produces under-resolution, not a hard internal-state signal.

Recommended tightening tasks:

- Change `generic_params_for_owner` and `resolve_type_generic_param_in_owner` to take `GenericParamOwnerId`.
- Let the broader resolver context explicitly try/refine into `GenericParamOwnerId` before generic lookup; if the context is `Const` or `Static`, skip lookup because those scopes are known non-generic, not because an arbitrary node failed a match.
- Return an error, or at least use a separate result state, when a type-use site expected a generic-owner context but cannot refine to one.

### Medium: associated-item owner recovery loses the existing `AssociatedItemOwnerId` proof

- Evidence: `associated_owner_for_method` returns `Option<AnyNodeId>` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:526`.
- Evidence: the only successful relation arms are `ImplAssociatedItem` and `TraitAssociatedItem`, returning `source.as_any()` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:531` and `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:535`.
- Evidence: the ID layer already models this exact owner set as `AssociatedItemOwnerId = TraitNodeId | ImplNodeId` at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs:90` and implements it at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/mod.rs:1298`.

The weakened invariant is that a method's associated owner, once found through a typed syntactic relation, is known to be either a trait or impl. Returning `AnyNodeId` discards that proof and forces downstream callers to rediscover or manually match it. `resolve_type_generic_param` then passes the erased owner back into `resolve_type_generic_param_in_owner`, and `impl_for_self_context` matches it again.

Recommended tightening tasks:

- Return `AssociatedItemOwnerId` from `associated_owner_for_method`.
- Add explicit widening/conversion from `AssociatedItemOwnerId` to the resolver-context or generic-owner type where needed.
- Use the typed associated-owner value in `impl_for_self_context` so the `Self` path has a narrow `ImplNodeId` refinement boundary.

### Medium: type-use iteration helpers use `AnyNodeId` for surfaces with narrower owner contracts

- Evidence: `next_callable_type_use` takes `resolution_context_owner: AnyNodeId` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1287`.
- Evidence: `next_where_predicate_type_use` takes `resolution_context_owner: AnyNodeId` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1335`.
- Evidence: `next_generic_bound_type_use` takes `resolution_context_owner: AnyNodeId` at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1361`.
- Evidence: `ordinary_type_use_site` and `trait_type_use_site` preserve that broad owner at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1384` and `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1398`.

The weakened invariant is that callable/generic-bound/where-predicate roots have specific owner surfaces. Generic bounds and where predicates belong to `GenericParamOwnerId`; callable params/returns belong to function or method owners; const/static root types are different non-generic scopes. The helpers currently erase all of those distinctions into one `AnyNodeId` slot before constructing `TypeUseSite`.

Recommended tightening tasks:

- Split helper parameters by semantic surface: callable owner, generic-bound owner, where-predicate owner, and const/static root owner.
- If a single context object remains useful, make it a narrow enum with variants for the real surfaces rather than a raw `AnyNodeId`.
- Keep the current traversal behavior, but move owner admissibility checks to `DirectTypeUseIter::new` and branch construction.

## Current Code That Looks Correct

- `TypeRelation` endpoints are narrow and correct-by-construction: `Ordinary` uses `OrdinaryTypeSourceId × OrdinaryTypeTargetId`, and `Trait` uses `TraitTypeSourceId × TraitTypeTargetId` in `crates/ingest/syn_parser/src/parser/relations.rs:35`.
- `OrdinaryTypeTargetId` and `TraitTypeTargetId` do not provide a general `TryFrom<AnyNodeId>` path. The type-family docs explicitly call out why generic-param targets need payload refinement at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:119`, and the target families are defined narrowly at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:888` and `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:905`.
- `prove_ordinary_target` correctly refines `AnyNodeId::GenericParam` through `generic_param_node(id)` plus `TypeGenericParamNodeId::try_refine(id, &param.kind)` before constructing an ordinary target at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:252`.
- The structural type-use/source split is sound: `OrdinaryTypeUseId` includes composite ordinary type syntax, while `OrdinaryTypeSourceId` is the named terminal subset, documented at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:40` and implemented at `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:646` and `crates/ingest/syn_parser/src/parser/nodes/ids/internal/type_families.rs:744`.
- `TypeTreeRelationIter` maintains typed work items (`OrdinaryTypeUseId` vs `TraitTypeSourceId`) rather than walking bare `TypeId`s, and it errors when those typed work items encounter the wrong structural node kind at `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1466` and `crates/ingest/syn_parser/src/resolve/type_resolution_v2.rs:1536`.

## Residual Risk

I did not run tests because this was a static review-only pass and the requested output was a report. The main residual risk is that some broad `AnyNodeId` use is legitimate for heterogeneous topology lookups in `ModuleTree`; the tightening should target resolver context and generic/associated-owner APIs, not every index lookup that necessarily crosses the heterogeneous graph boundary.
