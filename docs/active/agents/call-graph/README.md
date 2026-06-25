# Ploke call graph restart spine

Date: 2026-06-22
Status: active feature restart spine
Short description: Current source of truth for the parser call-graph feature thread; read this first after compaction or before dispatching implementation work.

## Read order

1. This file.
2. [`2026-06-25_call-graph-quality-recovery-tracker.md`](2026-06-25_call-graph-quality-recovery-tracker.md) — active quality gate after the DB/proof/code-organization review; read before resuming implementation.
3. [`../2026-06-22_call-graph-id-domain-correction.md`](../2026-06-22_call-graph-id-domain-correction.md) — binding design decision for call-site identity.
4. [`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md) — active structural/resolution test matrix for future slices.
5. [`2026-06-22_structural-method-call-extraction-plan.md`](2026-06-22_structural-method-call-extraction-plan.md) — completed structural method-call slice.
6. [`2026-06-22_inherent-self-method-resolution-plan.md`](2026-06-22_inherent-self-method-resolution-plan.md) — completed first semantic resolver slice.
7. [`2026-06-22_structural-path-call-extraction-plan.md`](2026-06-22_structural-path-call-extraction-plan.md) — completed structural path-call slice.
8. [`2026-06-22_local-free-function-path-call-resolution-plan.md`](2026-06-22_local-free-function-path-call-resolution-plan.md) — completed first local path-call resolver slice.
9. [`2026-06-22_call-graph-db-projection-plan.md`](2026-06-22_call-graph-db-projection-plan.md) — completed first database projection slice.
10. [`2026-06-22_external-root-path-call-classification-plan.md`](2026-06-22_external-root-path-call-classification-plan.md) — completed direct external-root classification slice.
11. [`2026-06-22_dynamic-call-extraction-plan.md`](2026-06-22_dynamic-call-extraction-plan.md) — completed dynamic structural call-site slice.
12. [`2026-06-22_structural-macro-call-extraction-plan.md`](2026-06-22_structural-macro-call-extraction-plan.md) — completed structural macro-call slice.
13. [`../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md`](../2026-06-21_call-graph-fixture-nodes-orchestration-plan.md) — task sequence for the first `fixture_nodes` slice.
14. [`../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md`](../../../../.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md) — broader typed call-graph rollout plan.
15. Long-horizon proof context only if needed:
   - [`../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`](../../../workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md)
   - [`../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`](../../../workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md)
   - [`../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md`](../../../workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md)
   - [`../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`](../../../workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md)

## Current implementation state

Current branch/worktree state is active feature work. Treat this as the restart spine for the incremental call-graph implementation.

Implemented/scaffolded:

- `ploke_core::CallId` as a distinct base identity universe for parser-owned call expression occurrences.
- Typed call-site wrappers over `CallId`, not `NodeId`:
  - `PathCallSiteId`
  - `MethodCallSiteId`
  - `DynamicCallSiteId`
  - `MacroCallSiteId`
- Call endpoint families:
  - `AnyCallSiteId`
  - `CallBodyOwnerId = FunctionNodeId ∪ MethodNodeId ∪ ConstNodeId ∪ StaticNodeId`
- Structural graph storage:
  - `CodeGraph.call_sites`
  - `CodeGraph.call_site_relations`
  - `CallSiteRelation::BodyContainsCall { source: CallBodyOwnerId, target: AnyCallSiteId }`
- Structural body extraction for the first narrow method-call slice:
  - `syn::ExprMethodCall` with literal `self` receiver.
  - `CallNode::MethodCall` emission.
  - `CallSiteRelation::BodyContainsCall` emission.
- Structural path-call extraction:
  - `syn::ExprCall` with `syn::Expr::Path` callee.
  - `CallNode::PathCall` emission.
  - deterministic parser-internal `PathCallSiteId` construction from owner + path + span + cfgs.
- Structural dynamic-call extraction:
  - non-path `syn::ExprCall` callees such as `(closure)()` and `(|| 11)()`.
  - `CallNode::DynamicCall` emission.
  - deterministic parser-internal `DynamicCallSiteId` construction from owner + span + cfgs.
- Structural macro-call extraction:
  - `syn::ExprMacro` and statement-position `syn::StmtMacro`.
  - `CallNode::MacroCall` emission.
  - deterministic parser-internal `MacroCallSiteId` construction from owner + macro path + span + cfgs.
- Structural const/static initializer extraction:
  - item initializer expressions in both `syn` and legacy `syn1` visitor paths.
  - calls owned by `CallBodyOwnerId::{Const, Static}`.
  - `BodyContainsCall` projection persists source owner kinds as `Const` / `Static`.
- Structural associated const initializer extraction:
  - trait default and inherent impl associated const initializer expressions.
  - calls owned by the associated const's existing `ConstNodeId` via
    `CallBodyOwnerId::Const`.
- Explicit closure/async extraction boundary:
  - parser-native call extraction does not descend into closure or async block
    bodies until a nested owner model exists, so inner calls are not attributed
    to the enclosing function owner.
- Typed call-resolution storage/accessor scaffold:
  - `CodeGraph.call_relations`
  - `CodeGraph.call_resolution_statuses`
  - `CallRelation::{Function, DynamicFunction, Method, AssociatedFunction, TupleStructConstructor, EnumVariantConstructor}`
  - `CallResolutionStatus::{Resolved, Unresolved, Ambiguous, External, Unsupported}`
- First semantic resolver slices:
  - `resolve::call_resolution::resolve_call_relations_after_tree(...)`
  - exact inherent `self.method()` resolution within the same impl block.
  - explicit local `crate`/`self`/`super` path-call resolution to local standalone functions.
  - explicit local module-qualified `crate::module::function()` and
    `self::module::function()` path-call resolution to local standalone
    functions.
  - explicit local `super::function()` path-call resolution in both
    path-resolution and focused call-graph fixtures.
  - local generic function path-call resolution with explicit turbofish
    arguments, preserving `generic_arg_count`.
  - raw identifier local function calls such as `r#match()` preserve the raw
    spelling and resolve to local function targets.
  - raw identifier local method calls such as `value.r#type()` preserve the raw
    spelling and resolve to exact local method targets when receiver proof is
    exact.
  - unqualified local function path-call resolution inside the containing module.
  - import/re-export/glob-aware local function path-call resolution for direct
    local binding chains, grouped imports, and imported module aliases.
  - conservative inherent associated-function path-call resolution for
    `Self::method()`, directly visible local `Type::method()`, method-as-
    associated syntax such as `Type::method(&value)`, and qualified local
    `<Type>::method()` calls.
  - import/re-export/glob-aware local type associated-function calls such as
    `ImportedAssocAlias::make()`, `ImportedAssoc::make()`, and
    `ReexportedAssoc::make()`.
  - Rust type-alias associated-function calls such as
    `type Alias = LocalType; Alias::method()` and simple alias chains when
    existing type-relation facts prove each alias target exactly, including
    aliases imported into the caller's scope.
  - conservative trait associated-function path-call resolution for fully
    qualified local `<Type as Trait>::method()` calls where the trait item has
    no `self` receiver.
  - shorthand local trait associated-function path-call resolution for
    `Trait::method()` through direct, alias, glob, grouped, and local re-export
    imports where the trait item has no `self` receiver.
  - same-trait `Self::method()` associated-function path-call resolution inside
    trait default method bodies when the owner method belongs to a local trait
    and the callee name has one no-`self` receiver candidate.
  - local value-binding path-call classification that fails closed for
    shadowed calls such as `let local_target = ...; local_target()` instead of
    emitting a fake edge to the module function.
  - generic `F: FnOnce` value-binding path calls are recorded and fail closed as
    `Unsupported`, preserving the boundary before future Fn/FnOnce semantic
    resolution.
  - boxed `dyn Fn` value-binding path calls are recorded and fail closed as
    `Unsupported`; exact unshadowed prelude-shaped `Box::new(...)` setup calls
    are classified as `External`.
  - explicitly typed local receivers whose type path is proven external or
    exact unshadowed prelude `String`/`Vec` classify `.len()` as `External`,
    while local shadowing still resolves to the local method target when proven.
  - exact local function-item binding calls such as
    `let f = local_target; f()` and `let f: fn() -> i32 = local_target; f()`
    when the initializer path resolves to one local function.
  - exact local function-item binding calls whose initializer path is a local
    import alias, such as `let f = imported_alias; f()`, when the import
    backlink resolves to one local function.
  - exact alias propagation for local function-item bindings such as
    `let f = local_target; let g = f; g()`, typed function-pointer aliases such
    as `let f: fn() -> i32 = local_target; let g: fn() -> i32 = f; g()`, and
    parenthesized `(g)()`, limited to aliases of bindings that already carry
    initializer-path proof.
  - tuple struct and tuple enum variant constructor resolution for visible local
    type bindings with matching tuple-field arity.
  - conservative non-`self` method-call resolution for named owner parameters
    plus explicitly typed and path-initialized local bindings whose local type
    and inherent instance method are proven exactly.
  - tuple-field local receiver method resolution when the root binding is
    constructed from a visible local tuple struct and the field type plus
    inherent method target are proven exactly.
  - tuple-field dynamic function calls such as `value.0()` resolve only when
    the root binding was constructed by a direct tuple-constructor call and the
    selected constructor argument path resolves to one local function.
  - borrowed explicitly typed local receivers such as `(&value).method()`
    resolve through the underlying local type when the method target is proven
    exactly.
  - dereferenced local receivers whose binding is initialized as a direct
    reference to a visible local type path, such as `let value = &LocalAssoc;
    (*value).method()`, resolve through that exact referenced type.
  - local receivers whose binding is initialized as a direct reference to a
    visible local type path, such as `let value = &LocalAssoc; value.method()`,
    resolve through that exact referenced type.
  - explicitly typed reference local receivers, such as
    `let value: &LocalAssoc = &LocalAssoc; value.method()`, resolve through one
    explicit reference layer when the local method target is proven exactly.
  - explicit dereferences of borrowed owner parameters, such as
    `value: &LocalAssoc` followed by `(*value).method()`, resolve through the
    referenced local type when the method target is proven exactly.
  - implicit method-call autoderef for borrowed owner parameters, such as
    `value: &LocalAssoc` followed by `value.method()`, resolves through one
    explicit reference layer when the local method target is proven exactly.
  - awaited local path-call result receivers such as
    `make_ready_local_assoc().await.method()` resolve through the local async
    function's declared return type when the method target is proven exactly.
  - try local path-call result receivers such as `try_local_assoc()?.method()`
    resolve through a syntactic `Result<T, E>` success type when the local
    function return type and method target are proven exactly.
  - method-call result receivers such as `value.clone_assoc().method()`
    resolve through the direct inner method call's exact local target return
    type when the nested call occurrence, inner target, and outer method target
    are all proven exactly; `Self` returns are interpreted through the inner
    method's owning impl.
  - parenthesized method receivers such as `(value).instance_value()` reuse the
    same exact local binding proof as `value.instance_value()`.
  - typed local method receivers whose annotation is a Rust type-alias chain
    resolve through existing exact type-relation alias target proof.
  - inherent method precedence over same-name local trait methods for exact
    local receiver types.
  - conservative local trait-impl instance-method resolution for named owner
    parameters plus explicitly typed and path-initialized local bindings when
    the receiver type, local trait target, trait visibility at the call owner,
    and concrete impl method are all proven exactly.
  - imported trait method lookup for exact concrete local trait impls through
    direct `use`, alias `use ... as ...`, glob imports, and local re-exported
    trait imports; missing trait visibility fails closed without a semantic
    edge.
  - ambiguous local trait-impl instance methods fail closed with `Ambiguous`
    and no semantic edge.
  - `self.method()` calls inside trait impl method bodies resolve to same-impl
    method definitions when the target method is present in the exact impl block.
  - generic receiver method calls resolve to exact local trait method
    declarations when an inline or `where` generic bound proves one local trait
    method target.
  - `impl Trait` parameter method calls resolve to exact local trait method
    declarations when the bound proves one local trait method target.
  - trait-object parameter method calls such as `&dyn LocalTrait` resolve to
    exact local trait method declarations when the trait-object bound proves
    one local trait method target.
  - local bindings annotated as one-bound trait objects, such as
    `let value: &dyn LocalTrait = input; value.method()`, resolve to exact local
    trait method declarations when the bound proves one local trait method
    target; concrete runtime dispatch remains future work.
  - local one-bound trait-object bindings initialized from a direct reference
    to a concrete local type, such as
    `let value: &dyn LocalTrait = &LocalType; value.method()`, or from a
    direct reference to a local binding that already carries exact concrete
    initializer proof, or from a local reference binding that already proves
    one concrete local receiver type, including one-step reference aliases,
    resolve to the exact concrete local trait impl method when the initializer
    type, visible trait target, and impl method are all proven exactly.
  - trait default method body calls such as `self.required()` resolve to exact
    same-trait instance method declarations when the owner method belongs to a
    local trait and the callee name has one `self` receiver candidate.
  - blanket impl method calls such as `impl<T> LocalTrait for T`,
    `impl<T: Bound> LocalTrait for T`, and
    `impl<T> LocalTrait for T where T: Bound` resolve to the concrete blanket
    impl method for the exact one-type-parameter shapes after normal local
    trait visibility proof; constrained forms additionally require exact local
    impl evidence that the concrete receiver type satisfies every bound, with
    bounded recursive proof through other exact one-parameter blanket impls.
  - constrained nominal generic self-type impls such as
    `impl<T: Bound> LocalTrait for Wrapper<T>` resolve only when receiver
    generic arguments and every bound are proven exactly, avoiding erased
    `Wrapper<_>` matches.
  - direct and directly imported external-root path-call classification for `std`/`core`/`alloc`/dependency-root calls.
  - parenthesized dynamic callee path resolution for exact local function targets
    such as `(local_target)()`.
  - parenthesized block-expression dynamic callees such as
    `({ local_target })()` resolve when the block contains exactly one
    unshadowed path expression and that path proves one local function.
  - if-expression dynamic callees such as
    `(if flag { local_target } else { local_target })()` resolve when every
    supported branch path proves the same local function; branches proving
    different local function targets fail closed as `Ambiguous` with no edge.
  - match-expression dynamic callees such as
    `(match flag { true => local_target, false => local_target })()` resolve
    when every supported arm path proves the same local function; arms proving
    different local function targets fail closed as `Ambiguous` with no edge.
  - function-pointer cast dynamic callee path resolution for exact unshadowed
    local function targets such as `(local_target as fn() -> i32)()`.
  - function-pointer cast dynamic local binding calls such as
    `let f: fn() -> i32 = local_target; (f as fn() -> i32)()` resolve when the
    binding initializer path proves one local function.
  - function-pointer casts over opaque function-pointer parameters such as
    `(f as fn() -> i32)()` preserve the local binding path for downstream
    context, but still fail closed with `Unsupported` and no semantic edge.
  - dereferenced function-pointer local binding calls such as
    `let f: fn() -> i32 = local_target; (*f)()` resolve when the binding
    initializer path proves one local function.
  - parenthesized initialized function-item binding calls such as
    `let f = local_target; (f)()` resolve to local `DynamicFunction` edges when
    the initializer path proves one local function.
- Database projection in `ploke-transform`:
  - `call_site`
  - `call_site_edge`
  - `call_relation`
  - `call_resolution_status`
  - resolved dynamic function edges preserve `relation_kind = "DynamicFunction"`
    with `source_kind = "Dynamic"` and `target_kind = "Function"`.
- Feature-gated typed query helpers in `ploke-db`:
  - `Database::call_sites_for_owner(...)`
  - `Database::call_targets_for_site(...)`
  - `Database::call_resolution_for_site(...)`
  - `Database::call_context_for_owner(...)`
  - `Database::callers_for_target(...)`
  - fresh fixture-backed DB tests now parse and transform
    `fixture_call_graph` and `fixture_nodes` before asserting persisted context
    rows for resolved path calls, local/initialized/typed-local method
    receivers including parenthesized receivers and Rust type-alias receiver
    annotations, associated functions, imported/re-exported type and trait
    associated functions, tuple and enum constructors, nested returned-function
    calls, dynamic function calls including cast/deref/block, branch, field,
    indexed, exact member/index alias callee shapes, and parenthesized
    function-item / typed function-pointer alias bindings,
    fail-closed guarded/nested branch and opaque closure/index dynamic callees,
    trait-dispatch method calls,
    Rust type-alias associated-function and instance-method calls,
    method-as-associated-function calls, function-item and typed function
    pointer binding calls, imported function-item binding calls,
    generic-bound, trait-object, aliased/reference concrete trait-object,
    constrained generic self-type, imported-trait, and blanket-trait method
    calls, method-body owner contexts for trait defaults and impl methods,
    const/static and associated-const initializer owner contexts,
    borrowed/dereferenced method receivers, path/method/await/try result
    receivers, tuple-field method/dynamic calls, raw identifier path/method
    calls, prelude `drop(...)` vs local shadowed `drop` resolution, explicit
    inherent `drop(self)` calls, inherent-over-trait method precedence,
    literal/prelude method classification, `String::new` / `Vec::new`
    targetless external rows, local shadowed `Vec::len` resolution, macro
    statuses, boxed/generic Fn-style dynamic failures, and bare callable-value
    path failures with no fabricated targets. They also assert
    external/ambiguous/unsupported statuses with no local target edges,
    target-centered incoming caller rows for real local function, method, and
    associated-function targets, and project resolved, external, and mixed
    multi-row call contexts into proof facts from the same real transformed
    fixture owners.
- Feature-gated proof-fact projection in `ploke-db`:
  - `Database::call_proof_facts_for_owner(...)`
  - `Database::project_call_proof_facts_for_owner(...)`
  - `Database::call_proof_facts_for_target(...)`
  - `Database::project_call_proof_facts_for_target(...)`
  - projects existing proof JSON facts for call sites, resolved call edges, and
    call-resolution blockers from persisted owner-scoped and target-centered
    call graph rows
- Feature-gated RAG/TUI payload plumbing:
  - `ContextPart.call_context`
  - `ConciseContext.call_context`
  - `RagService` call-context collection from persisted call graph rows
  - `RagService` target-centered caller expansion through
    `Database::callers_for_target(...)` before reranking/context assembly
  - fresh RAG fixture test now parses/transforms `fixture_call_graph` and
    asserts `RagService::collect_call_context` preserves the real
    `call_try_result_instance_method` rows for the unsupported `Ok(...)`
    wrapper, resolved `try_local_assoc()` path call, and resolved
    try-result method receiver.
  - fresh RAG fixture coverage also asserts real dynamic outgoing rows preserve
    resolved `DynamicFunction` targets and targetless unsupported dynamic calls.
  - fresh RAG fixture coverage also asserts real external targetless rows
    preserve path, literal receiver, and typed-local receiver payloads without
    fabricating targets.
  - fresh RAG fixture coverage also asserts returned-function mixed rows,
    callable-value path blockers, boxed `dyn Fn` setup/failure rows, and
    prelude `Vec::new()` preserve their RAG call-context payloads without
    fabricating targets.
  - fresh RAG fixture coverage also asserts real targetless blocker rows
    preserve macro callees as `Unsupported` and ambiguous method calls as
    `Ambiguous` with no fabricated targets.
  - fresh RAG expansion coverage seeds retrieval with `try_local_assoc` and
    asserts the caller owner is materialized with outgoing call context that
    still points back to the seed target.
  - fresh RAG expansion coverage seeds retrieval with `local_target` and asserts
    dynamic-function caller owners are materialized with outgoing call context
    that still points back to the seed target.
  - fresh RAG expansion coverage also seeds retrieval with the
    `LocalAssoc::instance_value` method target and asserts both method-call and
    associated-function caller owners are materialized with outgoing call
    context pointing back to the seed target.
  - fresh RAG expansion coverage also seeds retrieval with the
    `LocalAssoc::make` associated-function target and asserts both
    method-owner `Self::make` and qualified function-owner `LocalAssoc::make`
    callers are materialized with outgoing `AssociatedFunction` call context.
  - fresh RAG expansion coverage also seeds retrieval with tuple-struct and
    enum-variant constructor targets and asserts real caller owners are
    materialized with outgoing constructor-family call context.
  - public `get_context` coverage also proves sparse retrieval seeded by the
    `NewType` tuple-struct constructor target materializes the constructor
    caller owner while preserving the outgoing `TupleStructConstructor` edge
    and incoming-caller expansion provenance through final context assembly.
    Enum-variant constructor expansion remains helper-level coverage until
    variant nodes are sparse retrieval seeds.
  - public `get_context` coverage proves sparse retrieval seeded by
    `try_local_assoc` materializes the incoming caller owner and preserves that
    outgoing call-context edge through final context assembly, with
    `ContextPart.call_expansion` explaining the incoming-caller provenance.
  - public `get_context` coverage also proves sparse retrieval seeded by the
    `LocalAssoc::instance_value` method target materializes both method-call
    and associated-function caller owners while preserving the outgoing
    call-context edges and incoming-caller expansion provenance through final
    context assembly.
  - public `get_context` coverage also proves sparse retrieval seeded by the
    `LocalAssocFunctionTrait::trait_make` associated-function target
    materializes the trait associated-function caller owner while preserving
    the outgoing `AssociatedFunction` edge and incoming-caller expansion
    provenance through final context assembly.
  - public `get_context` coverage also proves sparse retrieval seeded by the
    `ImportedAssocFunctionTrait::imported_trait_make` associated-function
    target materializes direct, alias, glob, re-export, and grouped-import
    caller owners while preserving the outgoing `AssociatedFunction` edges and
    incoming-caller expansion provenance through final context assembly.
  - public `get_context` coverage also proves sparse retrieval seeded by
    `call_crate_local_target` materializes its outgoing callee target with an
    `OutgoingTarget` expansion reason, including the DB call-site ID used to
    derive the candidate.
  - `RagService` now collects outgoing call-context rows for call-expanded
    owners that survive into the final assembled hit set even when ordinary
    retrieval/type-context hits fill the default owner collection window first.
  - TUI context-plan/system formatting with outgoing-call summaries including
    callee shape, span, status/resolution, and target relation IDs
  - TUI context-plan/system/tool carriers now preserve and render
    `call_expansion` metadata with expansion relation, seed ID, call-site ID,
    target ID, and distance.
  - TUI formatter coverage now asserts the fixture-derived
    `call_try_result_instance_method` payload shape: unsupported `Ok(...)`,
    resolved `try_local_assoc()`, and resolved `try_local_assoc()?.instance_value()`
    in both model-facing context text and expanded context-plan overlay details.
    The same formatter/overlay coverage now also asserts associated-function
    targets render as `AssociatedFunction:<id>` for `Self::make`-style payloads
    and dynamic targets render as `DynamicFunction:<id>`.
    Separate formatter/overlay coverage asserts concrete trait-dispatch method
    rows render initialized-local receiver proof such as
    `value = TraitDispatchTarget` while preserving the `Method:<id>` target.
    Separate compact formatter/overlay coverage asserts external targetless rows
    render with path, literal receiver, and typed-local receiver details.
    Separate formatter/overlay coverage asserts callable-path blocker rows,
    returned-function mixed rows, boxed `dyn Fn` setup/failure rows, and
    `Vec::new()` external rows render without fabricated targets.
    It also asserts macro blocker rows and ambiguous method blocker rows render
    under the existing call-context row cap without inventing targets.
  - Tool-carrier coverage asserts `request_code_context` JSON roundtrips
    preserve `ConciseContext.call_context` through the real
    `ContextPart -> ConciseContext` conversion, including local and imported
    trait associated-function, dynamic-function, constructor, external
    targetless, macro blocker, and ambiguous blocker rows.
  - Direct production-tool coverage asserts `request_code_context` over a fresh
    `fixture_call_graph` database returns an incoming method caller part with
    both `call_expansion` provenance and the matching outgoing method
    call-context row in the model-visible JSON payload.
    It also asserts the production-style payload returns the tuple-constructor
    caller with the matching outgoing `TupleStructConstructor` row when sparse
    retrieval is seeded by `NewType`; production type-context expansion may
    materialize that caller before call-context expansion adds provenance.
- GREEN fixture tests now use a call-site paranoid harness and cover 206 concrete call expressions:
  - `fixture_nodes_public_method_records_and_resolves_self_private_method_call_site`
  - `fixture_nodes_get_secret_len_records_self_field_len_external_method_call_site`
  - `fixture_nodes_get_str_len_records_self_field_len_method_call_site`
  - `fixture_nodes_generic_simple_trait_method_records_self_field_into_method_call_site`
  - `fixture_nodes_use_imported_items_records_hashmap_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_fs_read_to_string_path_call_site`
  - `fixture_nodes_use_imported_items_records_pathbuf_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_enum_variant1_path_call_site`
  - `fixture_nodes_use_imported_items_records_alias_checker_value_binding_path_call_site`
  - `fixture_nodes_use_imported_items_records_duration_from_secs_path_call_site`
  - `fixture_nodes_use_imported_items_records_arc_new_path_call_site`
  - `fixture_nodes_use_imported_items_records_tuple_struct_path_call_site`
  - `fixture_nodes_use_imported_items_records_documented_macro_call_site`
  - `fixture_nodes_use_all_const_static_records_println_macro_call_site`
  - `fixture_path_resolution_call_restricted_resolves_super_restricted_func_path_call_site`
  - `fixture_path_resolution_root_func_records_std_path_new_external_path_call_site`
  - `fixture_path_resolution_root_func_records_regex_new_external_path_call_site`
  - `fixture_path_resolution_root_func_records_regex_unwrap_external_method_call_site`
  - `fixture_path_resolution_root_func_records_typeid_synthetic_external_path_call_site`
  - `fixture_path_resolution_root_func_records_nodeid_generate_synthetic_external_path_call_site`
  - `fixture_path_resolution_root_func_records_uuid_nil_external_path_call_site`
  - `fixture_path_resolution_root_func_records_nodeid_uuid_external_method_call_site`
  - `fixture_path_resolution_root_func_records_info_macro_call_site`
  - `fixture_path_resolution_root_func_records_debug_macro_call_site`
  - `fixture_macros_use_local_macro_records_local_macro_call_site`
  - `fixture_macros_use_local_macro_records_println_macro_call_site`
  - `fixture_impls_main_resolves_func_test_one_initialized_local_method_call_site`
  - `fixture_impls_main_resolves_func_test_two_initialized_local_method_call_site`
  - `fixture_impls_main_resolves_func_test_three_initialized_local_method_call_site`
  - `fixture_impls_main_resolves_func_test_four_associated_function_path_call_site`
  - `fixture_impls_main_resolves_func_test_five_initialized_local_method_call_site`
  - `fixture_impls_main_records_println_macro_call_site`
  - `fixture_edge_cases_use_imports_resolves_direct_imported_helper_method_call_site`
  - `fixture_edge_cases_use_imports_resolves_reexported_helper_method_call_site`
  - `fixture_edge_cases_use_imports_records_literal_to_string_external_method_call_site`
  - `fixture_edge_cases_processor_trait_impl_records_format_macro_call_site`
  - `fixture_edge_cases_generic_item_new_records_t_default_unsupported_path_call_site`
  - `fixture_edge_cases_test_visibility_resolves_internal_helper_path_call_site`
  - `fixture_edge_cases_test_visibility_resolves_super_helper_path_call_site`
  - `fixture_edge_cases_test_visibility_resolves_restricted_func_path_call_site`
  - `fixture_generics_generic_function_records_t_default_unsupported_path_call_site`
  - `fixture_generics_trait_impl_process_records_format_macro_call_site`
  - `fixture_type_resolution_v2_generic_assoc_const_records_panic_macro_call_site`
  - `fixture_call_graph_dynamic_calls_records_parenthesized_binding_dynamic_call_site`
  - `fixture_call_graph_dynamic_calls_records_closure_literal_dynamic_call_site`
  - `fixture_call_graph_call_crate_local_target_resolves_crate_path_call_site`
  - `fixture_call_graph_call_self_nested_target_resolves_self_path_call_site`
  - `fixture_call_graph_call_crate_module_nested_target_resolves_crate_module_path_call_site`
  - `fixture_call_graph_call_self_module_nested_target_resolves_self_module_path_call_site`
  - `fixture_call_graph_call_super_local_target_resolves_super_path_call_site`
  - `fixture_call_graph_call_raw_identifier_function_resolves_raw_identifier_path_call_site`
  - `fixture_call_graph_call_raw_identifier_method_resolves_raw_identifier_method_call_site`
  - `fixture_call_graph_call_unqualified_local_target_resolves_local_path_call_site`
  - `fixture_call_graph_call_returned_function_resolves_inner_make_fn_path_call_site`
  - `fixture_call_graph_call_returned_function_records_outer_dynamic_call_site`
  - `fixture_call_graph_call_self_make_resolves_self_associated_function_path_call_site`
  - `fixture_call_graph_call_local_assoc_make_resolves_type_associated_function_path_call_site`
  - `fixture_call_graph_call_qualified_local_assoc_make_resolves_type_associated_function_path_call_site`
  - `fixture_call_graph_call_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`
  - `fixture_call_graph_call_glob_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`
  - `fixture_call_graph_call_reexported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`
  - `fixture_call_graph_call_imported_alias_target_resolves_imported_path_call_site`
  - `fixture_call_graph_call_glob_imported_target_resolves_glob_path_call_site`
  - `fixture_call_graph_call_reexported_target_resolves_reexport_path_call_site`
  - `fixture_call_graph_call_imported_module_target_resolves_module_alias_path_call_site`
  - `fixture_call_graph_call_grouped_imported_alias_target_resolves_imported_path_call_site`
  - `fixture_call_graph_call_grouped_imported_globbed_alias_target_resolves_imported_path_call_site`
  - `fixture_call_graph_call_param_instance_method_resolves_local_binding_method_call_site`
  - `fixture_call_graph_call_typed_local_instance_method_resolves_typed_local_binding_method_call_site`
  - `fixture_call_graph_call_initialized_local_instance_method_resolves_initialized_local_binding_method_call_site`
  - `fixture_call_graph_call_parenthesized_typed_local_instance_method_resolves_typed_local_binding_method_call_site`
  - `fixture_nodes_fn_call_const_resolves_const_initializer_path_call_site`
  - `fixture_call_graph_impl_assoc_const_resolves_initializer_path_call_site`
  - `fixture_call_graph_trait_assoc_const_resolves_initializer_path_call_site`
  - `fixture_nodes_static_fn_call_resolves_static_initializer_path_call_site`
  - `fixture_call_graph_call_param_trait_method_resolves_local_trait_impl_method_call_site`
  - `fixture_call_graph_call_typed_local_trait_method_resolves_local_trait_impl_method_call_site`
  - `fixture_call_graph_call_initialized_local_trait_method_resolves_local_trait_impl_method_call_site`
  - `fixture_call_graph_call_parenthesized_initialized_local_trait_method_resolves_local_trait_impl_method_call_site`
  - `fixture_call_graph_call_parenthesized_local_target_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_trait_associated_function_resolves_trait_assoc_function_path_call_site`
  - `fixture_call_graph_call_shadowed_local_target_binding_records_value_binding_path_call_site`
  - `fixture_call_graph_call_local_function_item_binding_resolves_initialized_value_binding_path_call_site`
  - `fixture_call_graph_call_typed_function_pointer_binding_resolves_initialized_value_binding_path_call_site`
  - `fixture_call_graph_call_typed_function_pointer_alias_binding_resolves_initialized_value_binding_path_call_site`
  - `fixture_call_graph_call_parenthesized_typed_function_pointer_alias_binding_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_generic_fn_once_value_binding_records_value_binding_path_call_site`
  - `fixture_call_graph_call_boxed_dyn_fn_value_binding_records_box_new_external_path_call_site`
  - `fixture_call_graph_call_boxed_dyn_fn_value_binding_records_value_binding_path_call_site`
  - `fixture_call_graph_call_generic_identity_turbofish_resolves_generic_function_path_call_site`
  - `fixture_call_graph_call_method_turbofish_resolves_generic_method_call_site`
  - `fixture_call_graph_call_prelude_drop_value_records_external_path_call_site`
  - `fixture_call_graph_call_local_drop_shadow_resolves_local_function_path_call_site`
  - `fixture_call_graph_call_crate_scoped_macro_records_crate_path_macro_call_site`
  - `fixture_call_graph_call_borrowed_typed_local_instance_method_resolves_borrowed_typed_local_binding_method_call_site`
  - `fixture_call_graph_call_dereferenced_local_instance_method_resolves_dereferenced_initialized_local_binding_method_call_site`
  - `fixture_call_graph_call_prelude_string_new_records_external_path_call_site`
  - `fixture_call_graph_call_prelude_vec_new_records_external_path_call_site`
  - `fixture_call_graph_call_path_result_instance_method_resolves_returned_type_method_call_site`
  - `fixture_call_graph_call_method_result_instance_method_resolves_returned_type_method_call_site`
  - `fixture_call_graph_call_tuple_field_instance_method_resolves_field_type_method_call_site`
  - `fixture_call_graph_call_tuple_field_function_resolves_constructed_field_dynamic_call_site`
  - `fixture_call_graph_call_await_path_result_instance_method_resolves_returned_type_method_call_site`
  - `fixture_call_graph_call_try_path_result_instance_method_resolves_result_ok_type_method_call_site`
  - `fixture_call_graph_call_literal_str_to_string_records_external_method_call_site`
  - `fixture_call_graph_call_typed_vec_len_records_external_method_call_site`
  - `fixture_call_graph_call_shadowed_typed_vec_len_resolves_local_method_call_site`
  - `fixture_call_graph_call_ambiguous_trait_method_records_ambiguous_method_call_site`
  - `fixture_call_graph_call_inherent_over_trait_method_resolves_inherent_method_call_site`
  - `fixture_call_graph_call_inline_generic_bound_method_resolves_trait_method_call_site`
  - `fixture_call_graph_call_where_generic_bound_method_resolves_trait_method_call_site`
  - `fixture_call_graph_call_trait_object_method_resolves_trait_method_call_site`
  - `fixture_call_graph_trait_default_method_body_resolves_local_target_path_call_site`
  - `fixture_call_graph_call_parenthesized_function_item_binding_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_if_same_function_item_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_if_ambiguous_function_item_records_ambiguous_dynamic_call_site`
  - `fixture_call_graph_call_match_same_function_item_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_match_ambiguous_function_item_records_ambiguous_dynamic_call_site`
  - `fixture_call_graph_call_aliased_function_item_binding_resolves_initialized_value_binding_path_call_site`
  - `fixture_call_graph_call_parenthesized_aliased_function_item_binding_resolves_dynamic_function_call_site`
  - `fixture_call_graph_call_impl_trait_method_resolves_trait_method_call_site`
  - `fixture_call_graph_call_direct_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`
  - `fixture_call_graph_call_alias_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`
  - `fixture_call_graph_call_glob_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`
  - `fixture_call_graph_call_grouped_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`
  - `fixture_call_graph_call_method_as_associated_function_resolves_inherent_method_path_call_site`
  - `fixture_call_graph_call_direct_imported_trait_method_resolves_visible_trait_impl_method_call_site`
  - `fixture_call_graph_call_alias_imported_trait_method_resolves_visible_trait_impl_method_call_site`
  - `fixture_call_graph_call_glob_imported_trait_method_resolves_visible_trait_impl_method_call_site`
  - `fixture_call_graph_call_unimported_trait_method_fails_closed_without_visible_trait_call_site`
  - plus three negative owner-attribution checks for closure, async block, and
    async closure bodies.

Not implemented yet:

- Dynamic/Fn-call resolution beyond parenthesized exact local function path
  callees, exact single-path block callees, exact two-branch if-expression
  path callees, exact path-valued match-arm dynamic callees, exact bare
  function-pointer casts over unshadowed local function paths,
  exact initialized-binding
  function-pointer casts and dereferenced initialized function-pointer
  bindings, exact local function-item bindings and one-step alias propagation
  in path-call and parenthesized dynamic-call form, typed function-pointer
  aliases backed by initializer-path proof, exact indexed calls over untyped,
  typed, one-step aliased, and constructed named-field local array initializer
  proof, exact indexed calls over tuple-constructor array-field proof, exact
  indexed calls over named/tuple field array-alias proof, one-step aliases of
  constructed holder bindings carrying exact field initializer proof, and
  value-binding fail-closed classification including function-pointer parameter
  casts, generic `F: FnOnce`, and boxed `dyn Fn` calls.
  Async closure literal calls are covered as unsupported structural
  `DynamicCall` rows; closure/coroutine target modeling remains future work.
- Non-`self` method receiver classification beyond named owner parameters,
  explicitly typed local bindings, path-initialized local bindings, explicit
  dereferences of borrowed owner parameters, and one explicit reference layer
  of borrowed owner parameters with direct local inherent method proof.
- Trait dispatch beyond exact local trait-impl methods on typed receiver
  bindings plus exact local generic-bound, trait-object, same-trait
  default-method, and bounded one-parameter blanket-impl declarations,
  including non-direct concrete dispatch through trait objects, broader trait
  import/scope forms, richer blanket-bound shapes, and trait associated
  functions beyond direct local fully qualified `<Type as Trait>::method()` and
  visible local shorthand `Trait::method()` path calls.
- Import/re-export/glob-aware path-call resolution beyond direct local function
  bindings, local module-qualified paths, and imported local module aliases.
- Associated-function path-call resolution beyond conservative inherent
  `Self::method()`, directly visible local `Type::method()`, method-as-
  associated `Type::method(&value)`, qualified local `<Type>::method()`, and
  fully qualified `<Type as Trait>::method()` cases.
- Closure and async body ownership expansion; current parser-native extraction
  explicitly skips nested closure/async bodies rather than attributing their
  calls to the enclosing owner.

## 2026-06-23 workspace test audit

The initial call-graph implementation was verified with focused parser and
transform commands only. A later `cargo test --workspace --no-fail-fast` audit
surfaced failures that the plan did not anticipate:

- persisted backup fixtures created before the DB projection slice are stale
  after adding `call_site`, `call_site_edge`, `call_relation`, and
  `call_resolution_status`; typed corpus fixtures fail with
  `Cannot find requested stored relation 'call_relation'` until regenerated;
- active checkout-local fixtures can be repaired with
  `cargo xtask fixtures ensure --snapshots`, but the typed corpus shared
  snapshots also need typed fixture regeneration or refreshed reviewed seeds;
- unrelated broad-test failures must not be hidden under the call-graph plan;
  classify and fix them separately instead of ignoring them.

Treat this as a correction to the verification policy below: any future
call-graph slice that changes parser relations, transform schema, fixture
shape, or downstream DB import expectations must run a workspace checkpoint and
record the result before moving to the next slice. Incomplete call-graph surfaces
must be isolated with a Cargo feature, not `#[ignore]`.

## Workspace checkpoint and feature-gate protocol

Use `call_graph` as the single Cargo feature name for this rollout. As the
feature crosses crate boundaries, propagate that same feature through dependency
features instead of inventing crate-local names. Typed type graph support is now
baseline and no longer has a `typed_type_graph` Cargo feature.

At the start of a call-graph work session, after every schema/fixture-affecting
slice, and before handoff, run and log the default workspace checkpoint:

```bash
cargo xtask verify-fixtures 2>&1 | tee target/test-output/call-graph/<slug>-verify-fixtures.log
cargo xtask verify-backup-dbs 2>&1 | tee target/test-output/call-graph/<slug>-verify-backup-dbs.log
cargo test --workspace --no-fail-fast 2>&1 | tee target/test-output/call-graph/<slug>-workspace.log
```

Then run feature-enabled checks for every crate touched by the slice. Use the
same propagated feature name at each layer, for example:

```bash
cargo test -p syn_parser --features call_graph <focused-call-graph-filter> -- --nocapture
cargo test -p ploke-transform --features call_graph <focused-call-graph-filter> -- --nocapture
cargo test -p ploke-db --features call_graph <focused-call-graph-filter> -- --nocapture
```

Once all workspace members that expose the call graph have propagated the
feature, add a full feature-enabled checkpoint in addition to the default one
(if Cargo feature selection for the workspace supports the exact crate set):

```bash
cargo test --workspace --features call_graph --no-fail-fast
```

If that command is not supported for the selected virtual-workspace package set,
record the equivalent `-p <crate> --features call_graph` matrix instead.
Default `cargo test --workspace --no-fail-fast` must remain green throughout the
rollout.

If a DB relation was added, removed, or renamed, first refresh fixtures with the
registry-backed commands rather than weakening import validation:

```bash
cargo xtask fixtures ensure --snapshots
cargo xtask fixtures regenerate --typed
```

Fixture lifecycle docs:

- [`docs/testing/BACKUP_DB_FIXTURES.md`](../../../testing/BACKUP_DB_FIXTURES.md)
  is the fixture registry/consumer inventory and command reference.
- [`docs/how-to/recreate-backup-db-fixtures.md`](../../../how-to/recreate-backup-db-fixtures.md)
  is the operator workflow for `xtask` validation, recreation, and regeneration.

If the typed pass requires provider-backed embedding snapshots and credentials
are unavailable, record that as a blocker with the exact fixture ids; do not
silently skip stale shared snapshots.

Broad-test failures discovered at a checkpoint must be classified before the
next implementation task starts:

1. **Unexpected regression**: stop and fix or explicitly split into a new
   blocker before continuing.
2. **Environment or stale fixture**: repair/regenerate fixtures or configure the
   environment; do not ignore the test.
3. **Incomplete call-graph feature**: gate the new production surface and its
   tests behind `call_graph`, propagate that same feature through inter-crate
   dependencies, and keep the default workspace tests green. Do not add
   `#[ignore]` for this rollout.

Feature-gated tests should be strict when the feature is enabled. If a
fail-first test is added for a later slice, keep it behind `call_graph` and run
it in that slice's feature-enabled checkpoint; do not commit an ignored
placeholder test.

## Gate/triage marker convention

Use the searchable marker `CALL_GRAPH_GATE:<id>` in source comments and planning
rows for every temporary `call_graph` gate. The marker must say which slice is
expected to remove or update the gate, and tests behind the gate must keep strict
assertions when the feature is enabled.

Current workspace audit rows:

| Marker | Classification | Evidence | Gate or owner | Expected pass/update point |
| --- | --- | --- | --- | --- |
| `CALL_GRAPH_GATE:db-projection` | Recent call-graph DB projection changed default schema/import expectations. | `ploke-db --test mod` and `ploke-rag --lib` fail on stale backups with `Cannot find requested stored relation 'call_relation'`; `git log` points at `a07c4b4e Project call graph facts into Cozo`. | Gate DB schema/projection and consuming tests behind Cargo feature `call_graph`; regenerate fixtures before ungating. | DB projection integration slice is complete, registered active + typed corpus fixtures have current call-graph relations, and default `cargo test --workspace --no-fail-fast` is green without the feature. |
| `CALL_GRAPH_GATE:fixture-regeneration` | Fixture maintenance required by schema-affecting work, not a reason to weaken import validation. | Typed corpus backups predate `call_relation`; active checkout-local snapshots may need `cargo xtask fixtures ensure --snapshots`; typed shared snapshots may need `cargo xtask fixtures regenerate --typed`. | Fixture registry/docs owner; see `docs/testing/BACKUP_DB_FIXTURES.md` and `docs/how-to/recreate-backup-db-fixtures.md`. | Fixture regeneration/review slice updates registry/docs/seeds or records a credential/provider blocker. |
| `CALL_GRAPH_GATE:non-callgraph-reds` | Broad-run failures not explained by call-graph DB projection. | Current examples: `ploke-eval` traversal/history failures, `ploke-tree` `todo!()`, `ploke-tui` `SampleStruct` edit-apply resolution failures, and `ploke-tui --test integration` workspace subset interference. | Do not hide these under `call_graph`; route to their owning plans or fix separately. | Each owning plan either makes the test green or records a separate strict feature gate/fixture contract without weakening assertions. |

Post-gate evidence, 2026-06-23:

- Implemented `CALL_GRAPH_GATE:db-projection` with Cargo feature `call_graph`
  on `syn_parser`, `ploke-transform`, `ploke-db`, `ploke-rag`, `ploke-tui`,
  `ploke-test-utils`, and `xtask`.
- Default `ploke-transform` schema/transform no longer creates or imports
  `call_site`, `call_site_edge`, `call_relation`, or `call_resolution_status`.
- Feature-enabled projection tests still assert real call graph rows with strict
  counts and no fabricated unsupported edges.
- Feature-enabled `ploke-db` helper tests assert typed call-site context rows,
  semantic target rows, resolved/unsupported status rows, and fail-closed
  behavior when a structural call site lacks `call_resolution_status`. They now
  also assert target-centered incoming caller rows preserve the originating
  call-site payload, status row, and matching semantic target edge.
- Feature-enabled `ploke-db` fixture tests assert parser -> transform -> DB
  contracts over real `fixture_call_graph` and `fixture_nodes` rows for path
  calls including unqualified local, self/super, import alias, glob import,
  grouped import, re-export, and module-alias resolution forms, method calls, associated
  functions including inherent `Self::make()` and qualified
  `<LocalAssoc>::make()` owner contexts plus fully qualified local trait
  associated-function calls, imported/re-exported type and trait associated
  functions, tuple and enum constructors, dynamic function calls,
  trait-dispatch method calls, borrowed/dereferenced receivers,
  path/method/await/try result receivers, tuple-field method/dynamic calls,
  raw identifier path/method calls, prelude `drop(...)` vs local shadowed
  `drop` resolution, explicit inherent `drop(self)` calls, literal/prelude
  method classification, local shadowed `Vec::len` resolution, macro statuses,
  external/ambiguous/unsupported statuses, target edges, incoming caller rows
  for real local function, method, associated-function, tuple-struct
  constructor, and enum-variant constructor targets, closure/async body call
  non-projection onto enclosing owners, and proof-fact projection from real
  fixture owners.
- Feature-enabled `ploke-db` proof projection tests assert resolved call graph
  rows are stored through the existing proof graph as `call_site`, `call_edge`,
  and `call_resolution` facts, with source provenance retained. They also assert
  external, unsupported, and ambiguous call statuses project fail-closed blocker
  reasons without fabricating local call edges. Fixture-backed proof-store
  lookup coverage now verifies `proof_symbol_lookup` links both owner-scoped
  and target-centered real resolved callee hits back to companion `call_site`
  and `call_resolution` facts by call-site identity, including the
  target-centered `local_target` dynamic caller. Fixture-backed proof tests now
  include local, self/super/crate/module-qualified, import-alias, glob,
  re-export, imported-module, and grouped-import function path resolution forms,
  inherent/type-import/type-alias/trait associated-function path forms,
  local/initialized/typed/type-alias/borrowed/dereferenced method receiver
  forms, generic-bound, imported-trait, constrained-generic-self, and blanket
  trait method receiver forms, path/method/await result receiver chains, and
  tuple-field receiver method forms,
  a mixed owner with unsupported `Ok(...)`, resolved `try_local_assoc()`, and a
  resolved try-result method receiver, plus target-centered projection for the
  incoming `try_local_assoc` caller edge without including unrelated owner
  blockers. Target-centered method proof coverage now projects the real
  `LocalAssoc::instance_value` target and verifies both method-call and
  associated-function incoming edges are stored as resolved proof edges. The
  target-centered method-family proof tests now share common resolved-caller,
  checker-edge, blocker-absence, and provenance assertions while keeping
  family-specific relation checks.
  Fixture-derived external call-resolution blockers now also feed
  `proof_invariant_findings` when proof-only effect evidence references the same
  call site.
  Target-centered associated-function proof coverage now projects the real
  `LocalAssoc::make` target and verifies multiple incoming path callers are
  stored as resolved proof edges with source provenance. Imported
  trait-associated function target-centered proof coverage now projects the
  real `ImportedAssocFunctionTrait::imported_trait_make` target and verifies
  direct, alias, glob, re-export, and grouped-import callers. Owner-scoped
  method-family proof tests now share checker-edge, provenance, and blocker
  assertions while retaining per-family call-context target checks; the same
  helper also covers resolved callable and field dynamic proof rows with an
  explicit edge-count mode for field owners that project additional setup edges. Dynamic
  proof coverage now projects real resolved `DynamicFunction` calls including
  parenthesized path/binding callees, function-pointer cast/deref callees,
  block callees, indexed array callees, named-field/tuple-field callees, and
  same-target branch/match dynamic callees, real unsupported closure-binding
  cast and dereferenced closure-binding dynamic calls as
  `dynamic_dispatch_unbounded` blockers with no edge, real ambiguous
  branch/match dynamic calls as `type_resolution_missing` blockers, real
  guarded/opaque/nested branch/match dynamic calls as
  `dynamic_dispatch_unbounded` blockers, and target-centered `local_target`
  proof facts that include a dynamic
  incoming caller without pulling unrelated unsupported dynamic blockers or
  closure/async body outer owners.
  Returned-function proof coverage now projects a mixed owner with a resolved
  inner `make_fn()` edge and an unsupported outer dynamic blocker. Callable-path
  proof coverage now projects function-pointer parameter, generic `FnOnce`,
  boxed `dyn Fn`, and prelude `Vec::new()` targetless rows with the expected
  `type_resolution_missing` or `external_dependency_summary_missing` reasons.
  Initializer-owner proof coverage now projects top-level const/static and
  associated-const owners as resolved proof edges to their local initializer
  functions with source provenance preserved. Constructor proof coverage now
  uses shared fixture cases to project real tuple struct and enum variant
  constructor targets as resolved proof edges to `StructNodeId` and
  `VariantNodeId` callees, both owner-scoped and target-centered. Macro proof coverage
  now projects real targetless macro calls as `macro_expansion_not_available`
  blockers, and ambiguous proof coverage projects a real targetless ambiguous
  method call as a `type_resolution_missing` blocker.
  Synthetic proof coverage now rejects owner-scoped and target-centered local
  target edges whose call status is not resolved, including ambiguous rows, and
  verifies no partial proof facts are stored after that rejection. It also
  asserts owner-scoped and target-centered call-proof generation/projection
  reject empty `build_domain_id` values and missing source provenance before
  storing any proof facts, including mixed target-centered caller sets where an
  earlier caller has valid source provenance but a later caller does not.
- `cargo xtask verify-backup-dbs`, `cargo xtask verify-fixtures`, and
  `cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture`
  passed after the gate.
- `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`
  passed for the typed DB helper and proof-projection slices, including a
  strict rejection test for non-resolved call statuses that still carry local
  targets and a stable proof-fact identity contract across owner-scoped and
  target-centered projection. Generated call proof facts are also covered for
  GraphRAG query linkage by `call_site_id`; synthetic DB helper coverage now
  includes `expand_call_context` owner and target seeds plus the rule that
  non-resolved target rows remain queryable through low-level helpers but are
  not promoted as expansion candidates, plus endpoint-family and
  endpoint-existence validation for malformed persisted `call_relation` rows
  and source-kind validation for persisted `call_resolution_status` rows and
  target-kind validation for persisted `BodyContainsCall` rows. It also
  validates `BodyContainsCall`
  source-owner kind against the stored owner node family and rejects malformed
  `call_site` row shapes and malformed `call_resolution_status`
  status/resolution pairs. Owner context also rejects `Resolved(LocalExact)`
  rows unless they have exactly one target, and target-centered callers plus
  target-seeded expansion reject relation rows whose target endpoint is missing,
  whose call-site status is missing, or whose resolved call site has multiple
  valid semantic targets instead of silently promoting incomplete or
  contradictory rows. Synthetic DB
  helper coverage now also explicitly decodes the remaining method receiver
  payload families: `SelfValue`, borrowed/dereferenced locals, field
  receivers, path/method result receivers, await/try result receivers, and
  literal receivers.
- `cargo test -p ploke-db --features call_graph callers_for_target -- --nocapture`
  passed with `2 passed` for the target-centered incoming caller helper over
  both synthetic DB rows and fresh fixture-backed rows. Fresh fixture coverage
  now also pins target-seeded expansion for tuple-struct and enum-variant
  constructor caller rows through their typed `Struct` and `Variant` endpoint
  families.
- `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture`
  passed for fresh parser -> transform -> DB fixture-backed call graph
  contracts, including real outgoing and incoming `expand_call_context`
  candidates over the transformed `fixture_call_graph`, with exact persisted
  call-site identity preserved for ordinary path callers and resolved dynamic
  callers. Coverage now also proves owner-seeded expansion over mixed
  resolved/unsupported rows promotes only resolved callees with call-site
  provenance, and target-seeded method expansion preserves both method-call and
  associated-function caller sites.
  Target-centered `local_target` coverage now also proves closure/async body
  calls are excluded from `callers_for_target` and target-seeded expansion for
  the enclosing owners.
  Additional owner-context coverage now proves local/initialized/parenthesized
  alias receiver rows, parenthesized function-item / typed function-pointer
  alias dynamic rows, inherent-over-trait precedence, nested returned-function
  rows, targetless callable-value path failures, and prelude `Vec::new`.
  Proof coverage now also asserts parenthesized path/binding, cast/deref,
  block, indexed-array, field/tuple-field, and same-target branch/match dynamic
  calls project as resolved proof edges, and opaque closure-binding cast/deref,
  ambiguous branch/match, guarded/opaque/nested branch/match, parenthesized
  generic `FnOnce`, and boxed `dyn Fn` dynamic call rows project as the
  expected fail-closed blockers, while external setup calls remain
  `external_dependency_summary_missing` blockers and no proof edges are
  fabricated. It also asserts real
  trait-dispatch method rows project as proof edges both owner-scoped and
  target-centered, including concrete trait-object alias/chained-reference
  callers. Target-centered proof coverage now also asserts closure/async body
  outer owners do not appear in `local_target` proof edges or proof facts.
  Raw persisted-relation invariant coverage now proves every transformed
  `call_site` has exactly one matching `BodyContainsCall` edge, every call site
  has exactly one matching `call_resolution_status`, and every persisted
  `call_relation` is anchored to an existing call site and endpoint node.
  It also proves persisted `call_site.id` values stay disjoint from stored
  code-node and type-use/type IDs, preserving the `CallId` universe through DB
  projection.
  Inverse raw-row coverage also proves every persisted `BodyContainsCall` and
  `call_resolution_status` row points back to an existing matching owner and
  call site, with no orphaned rows. Status-to-relation cardinality coverage
  now proves every raw `Resolved(LocalExact)` site has exactly one semantic
  `call_relation`, and every non-resolved site has none. Proof projection
  linkage coverage now checks a mixed real owner so each projected call site
  has one `call_site` fact, one `call_resolution` fact, resolved rows have one
  `call_edge` fact, and non-resolved rows have no edge plus a blocker reason.
  Target-centered proof projection linkage coverage now checks the real
  `local_target` incoming caller set so each caller projects one `call_site`,
  one `call_resolution`, and one matching `call_edge` fact, with no unrelated
  proof rows and with source provenance matching the originating call-site
  span.
  Persisted DB owner-context coverage now also proves closure,
  move-closure, async-block, and async-closure body calls do not leak
  `local_target()` rows or target edges into the enclosing owner, currently
  `93 passed`.
- `cargo test -p ploke-db --features call_graph path_resolution_call_proof -- --nocapture`
  passed with `1 passed` for real resolved local, self/super/crate/module,
  import-alias, glob, re-export, imported-module, and grouped-import function
  path proof edge projection.
- `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_associated_function_call_proof_facts -- --nocapture`
  passed with `1 passed` for real resolved inherent, imported type,
  type-alias, method-as-associated, and trait associated-function proof edge
  projection.
- `cargo test -p ploke-db --features call_graph local_receiver_method_call_proof -- --nocapture`
  passed with `1 passed` for real resolved local, initialized, typed,
  type-alias, borrowed, and dereferenced method receiver proof edge projection.
- `cargo test -p ploke-db --features call_graph trait_family_method_call_proof -- --nocapture`
  passed with `1 passed` for real resolved generic-bound, imported-trait,
  constrained-generic-self, and blanket trait method proof edge projection.
- `cargo test -p ploke-db --features call_graph result_and_field_receiver_method_call_proof -- --nocapture`
  passed with `1 passed` for real resolved path/method/await result receiver
  method chains and tuple-field receiver method proof edge projection.
- `cargo test -p ploke-db --features call_graph target_centered_call_proof -- --nocapture`
  passed with `1 passed` for target-centered proof projection from the real
  `try_local_assoc` incoming caller edge. `cargo test -p ploke-db --features call_graph target_centered_method_call_proof -- --nocapture`
  passed with `1 passed` for target-centered proof projection from the real
  `LocalAssoc::instance_value` method target.
  `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_associated_function_call_proof_facts -- --nocapture`
  passed with `1 passed` for target-centered proof projection from the real
  `LocalAssoc::make` associated-function target.
  `cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_imported_trait_assoc_function_call_proof_facts -- --nocapture`
  passed with `1 passed` for target-centered proof projection from the real
  `ImportedAssocFunctionTrait::imported_trait_make` target.
- `cargo test -p ploke-db --features call_graph dynamic_call_proof -- --nocapture`
  passed with `2 passed` for real resolved dynamic owner proof and
  target-centered dynamic proof projection. `cargo test -p ploke-db --features call_graph unsupported_dynamic_call_without_edges -- --nocapture`
  passed with `1 passed` for real unsupported closure-binding cast/deref
  dynamic blocker projection.
- `cargo test -p ploke-db --features call_graph branch_and_match_dynamic_call_proof -- --nocapture`
  passed with `1 passed` for real resolved same-target branch/match dynamic
  proof edge projection.
- `cargo test -p ploke-db --features call_graph callable_expression_dynamic_call_proof -- --nocapture`
  passed with `1 passed` for real resolved parenthesized path/binding,
  cast/deref, block, and indexed-array dynamic proof edge projection.
- `cargo test -p ploke-db --features call_graph field_dynamic_call_proof -- --nocapture`
  passed with `1 passed` for real resolved named-field, indexed named-field,
  and indexed tuple-field dynamic proof edge projection.
- `cargo test -p ploke-db --features call_graph branch_and_match_dynamic_failures -- --nocapture`
  passed with `1 passed` for real ambiguous branch/match and
  guarded/opaque/nested branch/match dynamic blocker projection.
- `cargo test -p ploke-db --features call_graph initializer_call_proof -- --nocapture`
  passed with `2 passed` for real const/static and associated-const
  initializer owner proof projection.
- Exact constructor fixture/proof batch passed after the shared-case
  consolidation; each command passed with `1 passed`:
  ```bash
  cargo test -p ploke-db --features call_graph fixture_expand_call_context_target_seed_preserves_constructor_callers -- --nocapture
  cargo test -p ploke-db --features call_graph fixture_projection_stores_real_constructor_call_proof_facts -- --nocapture
  cargo test -p ploke-db --features call_graph fixture_projection_stores_real_target_centered_constructor_call_proof_facts -- --nocapture
  ```
- `cargo test -p ploke-db --features call_graph macro_call_without_edges -- --nocapture`
  passed with `1 passed` for real targetless macro blocker projection.
  `cargo test -p ploke-db --features call_graph ambiguous_call_without_edges -- --nocapture`
  passed with `1 passed` for real targetless ambiguous blocker projection.
- `cargo test -p ploke-db --features call_graph ambiguous_local_targets -- --nocapture`
  passed with `2 passed` for strict rejection of owner-scoped and
  target-centered ambiguous rows that still carry local target edges.
- `cargo test -p ploke-db --features call_graph target_centered_proof_projection_rejects_non_resolved_local_targets -- --nocapture`
  passed with `1 passed` for strict target-centered rejection of inconsistent
  incoming rows.
- `cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture`
  passed for the synthetic DB helper/proof module, currently `34 passed`.
- `cargo test -p ploke-db --features call_graph call_graph_fixture_queries -- --nocapture`
  passed for the fresh parser -> transform -> DB fixture-backed module in the
  last full fixture-module run before constructor proof assertions were
  consolidated into shared fixture cases.
- Latest unfiltered `cargo test -p ploke-db --features call_graph -- --nocapture`
  now passes `src/lib.rs`, `callsite_logging_tests`, and `debug_obsv`, then
  remains red only in backup-backed type-graph tests under `tests/mod.rs`: 34
  failures all report `Cannot find requested stored relation 'call_relation'`.
- `cargo test -p ploke-rag --features call_graph call_context -- --nocapture`
  passed for synthetic collection, fresh fixture-backed collection, and
  owner-seeded outgoing callee expansion and target-centered incoming caller
  expansion through both the helper and public `get_context` path, including
  real trait-dispatch method target expansion to concrete trait-object callers
  through both the helper and public sparse `get_context` path, local and
  imported trait associated-function target expansion through public sparse
  `get_context`, and both helper and public sparse `get_context` `local_target`
  expansion excluding closure/async body outer owners while preserving both
  ordinary path callers and real dynamic callers, plus call-expansion
  provenance on final assembled expansion candidates and fail-safe degradation
  when call-graph relations are absent,
  currently `23 passed`.
  `cargo test -p ploke-rag --features call_graph real_fixture_constructor_rows -- --nocapture`
  passed with `1 passed` for real tuple struct and enum variant constructor
  target-family payloads. `cargo test -p ploke-rag --features call_graph real_fixture_blocker_rows -- --nocapture`
  passed with `1 passed` for real targetless macro and ambiguous method blocker
  payloads. `cargo test -p ploke-rag --features call_graph real_fixture_external_rows -- --nocapture`
  passed with `1 passed` for real targetless external path, literal receiver,
  and typed-local receiver payloads.
- Full `cargo test -p ploke-rag --features call_graph -- --nocapture` remains
  red with stale backup fixtures missing `call_relation` plus pre-existing
  search/snippet fixture failures; keep this under
  `CALL_GRAPH_GATE:fixture-regeneration` / `CALL_GRAPH_GATE:non-callgraph-reds`
  instead of weakening call-graph relation checks.
- `cargo test -p ploke-tui --features call_graph format_call_context_block_renders_fixture_derived_rows -- --nocapture`
  passed for TUI rendering of the fixture-derived `Ok(...)`,
  `try_local_assoc()`, try-result method receiver payload shape, and
  `AssociatedFunction`, `DynamicFunction`, `TupleStructConstructor`, and
  `EnumVariantConstructor` target relation rendering, plus targetless macro and
  ambiguous method blockers under the existing row cap.
- `cargo test -p ploke-tui --features call_graph call_context -- --nocapture`
  passed for the broader TUI call-context filter, currently `10 passed`,
  including prompt formatting, expanded context-plan overlay details,
  callable-path blocker row rendering, trait-dispatch initialized-local receiver
  rendering, call-expansion provenance rendering, and model-visible call-context
  degradation notes.
- `cargo test -p ploke-tui --features call_graph tool_io_roundtrip -- --nocapture`
  passed for the public tool result carrier roundtrips, currently `3 passed`,
  including `ConciseContext.call_context` JSON roundtrip and `from_assembled`
  preservation for `request_code_context` with call-expansion provenance,
  separate ordinary path and dynamic incoming caller payloads, trait-dispatch
  initialized-local receiver rows, local and imported trait associated-function
  rows, dynamic and constructor call-context rows plus targetless external,
  macro, and ambiguous blocker rows.
- `cargo test -p ploke-db --features call_graph -- --nocapture` remains red in
  backup-backed type-graph tests because the registered typed corpus backups
  still predate `call_relation`; `src/lib.rs` passed with `98 passed`, and the
  remaining failures were all in `ploke-db --test mod`, with `195 passed`, `34
  failed`, and all failures reporting `Cannot find requested stored relation
  'call_relation'`. This is classified under `CALL_GRAPH_GATE:fixture-regeneration`,
  not as a DB helper or proof projection regression.
- `docs/testing/BACKUP_DB_FIXTURES.md` was last reviewed on 2026-06-12; as of
  2026-06-23 the fixture review is overdue before any backup-fixture changes.
- `cargo test --workspace --no-fail-fast` no longer reports `call_relation`
  missing from stale backups. Remaining red targets are non-call-graph or typed
  fixture/context issues: `ploke-eval --lib`, `ploke-rag --lib`,
  `ploke-test-utils --lib`, `ploke-tree --lib`, `ploke-tui --lib`, and
  `ploke-tui --test integration`. See
  `target/test-output/workspace-after-call-graph-gate.log` in the local run.

## Binding design decisions

### Call-site IDs are not NodeIds

The parser now uses three distinct identity universes:

```text
V_node = code item / code-graph node vertices, keyed by NodeId
V_type = structural type occurrence vertices, keyed by TypeId
V_call = call expression occurrence vertices, keyed by CallId
```

Call-site typed IDs must not be added to `AnyNodeId`, `PrimaryNodeId`, `AssociatedItemNodeId`, or any other node endpoint family.

### Constructors stay narrow

Production code should not expose broad constructors such as:

```rust
MethodCallSiteId::generate_synthetic(...)
```

The parser extraction path may use parser-internal helpers after seeing the corresponding `syn` expression shape. Tests use explicit test/paranoid helpers for deterministic regeneration, primarily through the call-site paranoid harness in `tests/common/call_site_paranoid.rs` and `paranoid_call_site_test!`.

### Structural facts are not semantic edges

A `MethodCallSiteId` proves structural syntax class only. It does not prove the target method. Resolved call edges must be introduced later through typed `CallRelation` / `CallResolutionStatus` data.

### Fail closed

Unsupported, dynamic, macro, external, and ambiguous call shapes must remain visible as structural facts or explicit statuses. They must not become fake local function/method edges.

## Completed implementation slice: call-site paranoid test harness

Implemented in this slice:

1. Added `tests/common/call_site_paranoid.rs` as the call-site analogue of the node-level paranoid helpers.
2. Added `paranoid_call_site_test!` to generate exact fixture-backed call-site tests.
3. Each generated test regenerates the typed `CallId`, checks exact-ID lookup, checks value lookup, checks the `BodyContainsCall` relation, and checks resolver status plus expected semantic edge/no-edge policy.
4. Converted the focused call-site coverage from hand-written tests/table rows to named paranoid tests.

Primary implementation files:

- `crates/ingest/syn_parser/tests/common/call_site_paranoid.rs`
- `crates/ingest/syn_parser/tests/common/macro_rule_tests.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: structural `self.method()` extraction

Implemented in this slice:

1. The parser visits original `syn` bodies for standalone functions and impl/trait methods after the typed body owner ID is known.
2. It observes `self.private_method()` as `syn::ExprMethodCall`.
3. It emits exactly one `CallNode::MethodCall` with:
   - owner `CallBodyOwnerId::Method(public_method_id)`;
   - receiver `MethodCallReceiver::SelfValue`;
   - method name `private_method`;
   - arg count `0`;
   - generic arg count `0`;
   - expected byte span and empty cfgs for the fixture row.
4. It emits exactly one `BodyContainsCall` relation from the owner to the call site.
5. Structural extraction itself still does not emit any resolved `CallRelation`.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
- `crates/ingest/syn_parser/src/parser/visitor/mod.rs`

## Completed implementation slice: exact inherent `self.method()` resolution

Implemented in this slice:

1. The resolver consumes structural call sites after `ModuleTree` construction/pruning.
2. It handles only `MethodCallReceiver::SelfValue` where the call owner is a method.
3. It proves the owner method belongs to an inherent impl through `ImplAssociatedItem`.
4. It searches the same impl for a unique method with the call site's method name.
5. It emits `CallRelation::Method { source, target }` and `CallResolutionStatus::Resolved { kind: LocalExact, .. }` only on exact proof.
6. It fails closed with statuses and no fake target edge for unsupported/unresolved/ambiguous cases.

Primary implementation files:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/src/parser/relations.rs`
- `crates/ingest/syn_parser/src/parser/graph/{code_graph.rs,mod.rs,parsed_graph.rs}`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: self-field method-call extraction

Implemented in this slice:

1. Method-call receiver classification now recognizes field projections rooted at `self`, such as `self.secret`.
2. `self.secret.len()` in `fixture_nodes::impls::PrivateStruct::get_secret_len` is recorded as `CallNode::MethodCall` with `MethodCallReceiver::SelfField { field_path: ["secret"] }`.
3. The resolver classifies this call as `External` with no local edge when the
   owner impl self type resolves to the local struct and the field type is a
   proven external/prelude concrete type such as `String`.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/nodes/call.rs`
- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: structural path-call extraction

Implemented in this slice:

1. The body visitor records `syn::ExprCall` expressions whose callee is `syn::Expr::Path`.
2. It emits `CallNode::PathCall` with path segments, value arg count, generic arg count, span, owner, and cfgs.
3. It emits `BodyContainsCall` for the path call site.
4. The focused fixture target is `PathBuf::new()` inside `fixture_nodes::imports::use_imported_items`.
5. The resolver initially failed closed for path-call sites; local explicit path-call resolution was added in a later slice.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Completed implementation slice: dynamic call extraction

Implemented in this slice:

1. Added parser-internal `generate_dynamic_call_site_id(...)` for `DynamicCallSiteId` construction in the `CallId` universe.
2. The body visitor records non-path `syn::ExprCall` callees as `CallNode::DynamicCall`.
3. Added `tests/fixture_crates/fixture_call_graph` for focused dynamic/Fn-like syntax coverage absent from existing fixtures.
4. `(closure)()` and `(|| 11)()` are covered by paranoid call-site tests and currently receive `Unsupported` status with no semantic edge.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/tests/common/call_site_paranoid.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `tests/fixture_crates/fixture_call_graph/src/lib.rs`
- `docs/active/agents/call-graph/2026-06-22_dynamic-call-extraction-plan.md`

## Completed implementation slice: direct external-root path-call classification

Implemented in this slice:

1. Path calls whose first segment is `std`, `core`, `alloc`, or a parsed dependency name now receive `CallResolutionStatus::External`; dependency names with `-` also match Rust path segments spelled with `_`.
2. These external-root calls emit no local `CallRelation` edges.
3. `std::path::Path::new("")` in `fixture_path_resolution::root_func` is covered by a paranoid call-site test.
4. Directly imported external-root calls such as `PathBuf::new()`, `HashMap::new()`, `fs::read_to_string(...)`, `Duration::from_secs(...)`, and `Arc::new(...)` are classified as `External` with no local edge.

Primary implementation files:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `docs/active/agents/call-graph/2026-06-22_external-root-path-call-classification-plan.md`

## Completed implementation slice: call graph database projection

Implemented in this slice:

1. Added Cozo schema for parser-owned call graph facts:
   - `call_site`
   - `call_site_edge`
   - `call_relation`
   - `call_resolution_status`
2. Extended `transform_parsed_graph` to run `resolve_call_relations_after_tree(...)` at the transform boundary, mirroring `type_relation` projection.
3. Persisted structural call sites, body containment, resolved semantic call edges, and explicit resolution statuses.
4. Added transform tests proving resolved path-call, resolved method-call, and unsupported/no-edge path-call rows appear correctly in the persisted relation families.

Primary implementation files:

- `crates/ingest/ploke-transform/src/schema/edges.rs`
- `crates/ingest/ploke-transform/src/schema/mod.rs`
- `crates/ingest/ploke-transform/src/transform/edges.rs`
- `crates/ingest/ploke-transform/src/transform/mod.rs`
- `docs/active/agents/call-graph/2026-06-22_call-graph-db-projection-plan.md`

## Completed implementation slice: local free-function path-call resolution

Implemented in this slice:

1. `CallRelationResolver` now attempts local standalone-function resolution for explicit `crate::`, `self::`, and `super::` path calls.
2. The resolver traverses from the call owner's containing module, handles local path prefixes, walks intermediate module segments, and proves terminal `FunctionNodeId` targets.
3. `super::restricted_func()` in `fixture_path_resolution::restricted_vis_mod::inner::call_restricted` resolves to `restricted_func` with `CallRelation::Function` and `Resolved(LocalExact)` status.
4. Unsupported path-call shapes remain explicit statuses with no fake local edge.

Primary implementation files:

- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `docs/active/agents/call-graph/2026-06-22_local-free-function-path-call-resolution-plan.md`

## Completed implementation slice: unqualified local free-function path-call resolution

Implemented in this slice:

1. `CallRelationResolver` now treats single-segment path calls such as `local_target()` as local function path candidates.
2. The resolver reuses the existing containing-module terminal lookup and emits a local exact `CallRelation::Function` only when exactly one local `FunctionNodeId` target is proven.
3. `fixture_call_graph::call_unqualified_local_target` records `local_target()` as a `PathCall` and resolves it to `fixture_call_graph::local_target`.
4. Ambiguous single-segment local function candidates still fail closed as `Ambiguous`; missing or non-function single-segment targets stay `Unsupported` so constructor/import/binding cases are not treated as supported resolver misses.

Primary implementation files:

- `tests/fixture_crates/fixture_call_graph/src/lib.rs`
- `crates/ingest/syn_parser/src/resolve/call_resolution.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md`

## Completed implementation slice: chained call-callee structural coverage

Implemented in this slice:

1. Added `fixture_call_graph::call_returned_function` with `make_fn()()` to cover Rust's chained call-callee syntax.
2. The existing visitor records the inner `make_fn()` as `CallNode::PathCall` and the outer returned-function invocation as `CallNode::DynamicCall`.
3. The inner `make_fn()` path call resolves to the local `FunctionNodeId`; the outer dynamic call remains `Unsupported` with no fabricated edge.

Primary implementation files:

- `tests/fixture_crates/fixture_call_graph/src/lib.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`
- `docs/active/agents/call-graph/2026-06-22_call-site-coverage-matrix.md`

## Completed implementation slice: structural macro-call extraction

Implemented in this slice:

1. The body visitor records `syn::ExprMacro` invocations and statement-position `syn::StmtMacro` invocations.
2. It emits `CallNode::MacroCall` with macro path/name, span, owner, and cfgs.
3. It emits `BodyContainsCall` for the macro call site.
4. Focused fixture targets include `documented_macro!(fixture alias coverage)` inside `fixture_nodes::imports::use_imported_items` and `println!(...)` inside `fixture_nodes::const_static::use_all_const_static`.
5. No macro expansion or macro target resolution is attempted.

Primary implementation files:

- `crates/ingest/syn_parser/src/parser/visitor/call_extraction.rs`
- `crates/ingest/syn_parser/src/parser/nodes/ids/internal/call_ids.rs`
- `crates/ingest/syn_parser/tests/uuid_phase3_resolution/call_sites.rs`

## Verification commands

Known-good baseline after the ID-domain correction:

```bash
cargo check -p syn_parser
cargo check -p syn_parser
cargo test -p syn_parser type_relations_v2 -- --nocapture
```

Focused structural/resolver call-site fixture tests now expected GREEN:

```bash
cargo test -p syn_parser call_sites -- --nocapture
```

## Drift prevention checklist

Before implementing new call-graph work, verify:

- call-site wrappers are still backed by `CallId`, not `NodeId`;
- no call-site IDs appear in `AnyNodeId` or node-category enums;
- no public production call-site ID generator has been added;
- tests regenerate call IDs through test-only helpers;
- structural call-site facts remain separate from resolved semantic call edges;
- planning docs still point to this restart spine and the ID-domain correction note.

## Last verification summary

Commands run after structural extraction, resolver slices, path-call extraction, and macro-call extraction landed:

```bash
cargo test -p syn_parser --features call_graph call_sites -- --nocapture
cargo test -p ploke-transform --features call_graph transform::tests -- --nocapture
cargo test -p ploke-db --features call_graph call_graph_queries -- --nocapture
cargo test -p ploke-rag --features call_graph call_context_collection_attaches_outgoing_call_payloads -- --nocapture
cargo check -p ploke-tui --features call_graph
```

Result: parser, transform, DB helper, and RAG checks passed. The latest
`call_sites` filter ran 206 paranoid fixture tests; transform projection tests
passed for resolved function/method/associated-function/constructor call edges
and unsupported call statuses.

## Next implementation slice

Use [`2026-06-22_call-site-coverage-matrix.md`](2026-06-22_call-site-coverage-matrix.md) as the test-selection source of truth. Continue broadening semantic coverage without weakening fail-closed status reporting. Recommended next RED tests:

1. broader function-pointer and generic `F: FnOnce` target resolution beyond
   the current exact local/direct-import function-item binding, exact bare
   cast-path, exact initialized-binding cast/deref, and value-binding
   fail-closed statuses;
2. broader trait dispatch for remaining multi-step trait-object concrete proof,
   richer blanket-bound shapes beyond exact recursive one-parameter local
   impls, and broader imported trait scope forms;
3. richer closure/async body ownership expansion beyond the current explicit
   extraction boundary.
