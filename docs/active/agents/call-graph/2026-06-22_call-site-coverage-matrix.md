# Rust call-site total coverage matrix

Date: 2026-06-22
Status: active exhaustive test-planning matrix
Scope: deliberately over-complete list of Rust call-like cases we want represented in tests, with explicit expected structural classification and resolver outcome.

## Reference sources consulted

Rust Analyzer is useful here because it already separates syntax-level callable expressions from semantic callable kinds:

- `/home/team_ploke_dev/work/reference/rust-analyzer/crates/parser/src/grammar/expressions.rs`
  - `call_expr` grammar tests include `f()`, chained `f()(1)(1, 2,)`, and qualified callee paths like `<Foo>::func()` / `<Foo as Trait>::func()`.
  - `method_call_expr` grammar tests include `x.foo()`, turbofish method calls, tuple-field receivers like `x.0.0.call()`, and field-vs-call ambiguity like `x.0()`.
- `/home/team_ploke_dev/work/reference/rust-analyzer/crates/syntax/src/ast/expr_ext.rs`
  - `CallableExpr = CallExpr | MethodCallExpr`; macro invocations are not part of this RA callable expression union.
- `/home/team_ploke_dev/work/reference/rust-analyzer/crates/hir/src/lib.rs`
  - semantic callable kinds include `Function`, `TupleStruct`, `TupleEnumVariant`, `Closure`, `FnPtr`, and `FnImpl`.
- `/home/team_ploke_dev/work/reference/rust-analyzer/crates/ide/src/call_hierarchy.rs`
  - outgoing call hierarchy resolves `CallExpr` to functions/tuple constructors and `MethodCallExpr` via method resolution; macro outgoing coverage has FIXME gaps in RA too.

This matrix is Ploke-native, not a copy of RA behavior. RA is a reference for case discovery and terminology.

## Current Ploke structural policy

| Rust syntax family | Current/future structural class | Notes |
|---|---|---|
| `ExprCall` with `ExprPath` callee | `CallNode::PathCall` | Current implementation. This is intentionally syntactic; `closure_binding()` and `fn_ptr()` are path-shaped even if semantic target is dynamic. |
| `ExprCall` with non-path callee | `CallNode::DynamicCall` | Current implementation. Examples: `(f)()`, `(|| 1)()`, `make_fn()()`, `funcs[0]()`. |
| `ExprMethodCall` | `CallNode::MethodCall` | Current implementation records literal `self`, field projections rooted at `self`, named owner-parameter receivers, explicitly typed local bindings, call-result receivers, field-local receivers, and await/try result receivers; broader untyped local variable/literal receivers still need classification. |
| `ExprMacro` / statement-position `StmtMacro` | `CallNode::MacroCall` | Current implementation. Invocation site only, no expansion. |
| Calls inside top-level const/static initializers | `CallBodyOwnerId::{Const, Static}` | Current implementation records initializer expression call sites in both `syn` and legacy `syn1` visitor paths. |
| Calls inside associated const initializers | `CallBodyOwnerId::Const` | Current implementation records trait default and inherent impl associated const initializer call sites under the associated const's existing `ConstNodeId`. |
| Calls inside closure/async/block bodies | Future owner/nesting model | Closure and async block bodies are explicit extraction boundaries today; inner calls are not attributed to the enclosing owner until closure/body-owner IDs or nested-body metadata exist. |
| Desugared/implicit calls | Separate future effect/call layer | Operators, `for`, `?`, `.await`, drop, deref coercions, etc. should be explicit matrix rows, not silently ignored. |

## Resolver status policy

Every structural call site considered by `resolve_call_relations_after_tree` should eventually have exactly one status:

| Status | Meaning |
|---|---|
| `Resolved(LocalExact)` | A typed local target edge is proven. |
| `Unresolved` | The call shape is supported, local lookup was attempted, and no local target was found. |
| `Ambiguous` | More than one plausible target exists and resolver refuses to choose. |
| `External` | Target is known or strongly inferred to be external/std/prelude/FFI. |
| `Unsupported` | Structural call is recorded but this resolver slice intentionally does not resolve it. |

Current green behavior:

All focused call-site rows now run through the `paranoid_call_site_test!` harness, which regenerates the typed `CallId`, checks exact-ID and value lookup, checks `BodyContainsCall`, and checks resolver status/edge policy.

- `self.private_method()` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `self.secret.len()` -> `MethodCall` with `SelfField { field_path: ["secret"] }`, `External`, no local edge after proving the concrete field type is `String`.
- `self.value.len()` -> `MethodCall` with `SelfField { field_path: ["value"] }`, `Unsupported`, no edge before generic field receiver typing.
- `self.value.into()` -> `MethodCall` with `SelfField { field_path: ["value"] }`, `Unsupported`, no edge before generic field receiver typing.
- `self.value.instance_value()` where `value: LocalAssoc` is a field on the
  enclosing impl self type -> `MethodCall` with `SelfField { field_path:
  ["value"] }`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `"literal".to_string()` -> `MethodCall` with `Literal`, `External`, no local edge.
- `PathBuf::new()` -> `PathCall`, `External`, no edge.
- `HashMap::<String, i32>::new()`, `fs::read_to_string(...)`, `Duration::from_secs(1)`, and `Arc::new(1)` -> `PathCall`, `External`, no edge.
- `Regex::new(...).unwrap()` -> inner `PathCall` for `Regex::new`, `External`, plus outer `MethodCall` with `PathCallResult`, `External`.
- `EnumWithData::Variant1(1)` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::EnumVariantConstructor`.
- `TupleStruct(1, 2)` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::TupleStructConstructor`.
- `NewType(value)` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::TupleStructConstructor`.
- `local_target()`, `super::restricted_func()`, `super::local_target()`, `crate::local_target()`, and `self::nested_target()` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `crate::local_mod::nested_target()` and `self::local_mod::nested_target()` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `generic_identity::<i32>(123)` -> `PathCall` with `generic_arg_count = 1`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `r#match()` -> `PathCall` preserving raw identifier spelling, `Resolved(LocalExact)`, `CallRelation::Function`.
- `value.r#type()` -> `MethodCall` preserving raw identifier spelling, `Resolved(LocalExact)`, `CallRelation::Method`.
- `imported_alias()`, `globbed_target()`, grouped import aliases, `reexported_target()`, and `targets_alias::globbed_target()` in `fixture_call_graph` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `Self::make()` inside an inherent impl and `LocalAssoc::make()` for a directly visible local type -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction`.
- `LocalAssoc::instance_value(&value)` as method-as-associated-function syntax -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction`.
- `<LocalAssoc>::make()` for a qualified directly visible local type -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction`.
- `ImportedAssocAlias::make()`, `ImportedAssoc::make()`, and `ReexportedAssoc::make()` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction` through local type import, glob import, and re-export bindings.
- `LocalAssocTypeAlias::make()`, `LocalAssocAliasChain::make()`, and `ImportedLocalAssocAlias::make()` where each alias target is proven exactly -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction` through alias target proof from existing type relations and imported local alias bindings.
- `value.instance_value()` where `value: LocalAssoc` is a named owner parameter -> `MethodCall` with `LocalBinding { name: "value" }`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `let value: LocalAssoc = ...; value.instance_value()` -> `MethodCall` with `TypedLocalBinding { name: "value", type_path: ["LocalAssoc"] }`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `let value: LocalAssocAliasChain = ...; value.instance_value()` -> `MethodCall` with `TypedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method` through alias target proof from existing type relations.
- `let value = LocalAssoc; value.instance_value()` -> `MethodCall` with `InitializedLocalBinding { name: "value", init_path: ["LocalAssoc"] }`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `let x = TestImplStruct { ... }; x.func_test_*()` -> `MethodCall` with `InitializedLocalBinding { name: "x", init_path: ["TestImplStruct"] }`, `Resolved(LocalExact)`, `CallRelation::Method` for exact inherent methods visible from `fixture_impls::main`.
- `(value).instance_value()` where `value` is a typed local binding -> `MethodCall` with the same local binding proof as the unparenthesized receiver.
- `let value = &LocalAssoc; value.instance_value()` -> `MethodCall` with `InitializedLocalBinding { name: "value", init_path: ["LocalAssoc"] }`, `Resolved(LocalExact)`, `CallRelation::Method` through direct referenced-local proof.
- `(*value).instance_value()` where `value: &LocalAssoc` is a named owner parameter -> `MethodCall` with `DereferencedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method` through one explicit reference layer.
- `value.priority()` where the exact local receiver type has both inherent and trait methods named `priority` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the inherent method.
- `value.trait_value()` where `value: TraitDispatchTarget` is a named owner parameter or explicitly typed local binding -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the concrete local trait impl method.
- `let value = TraitDispatchTarget; value.trait_value()` -> `MethodCall` with `InitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the concrete local trait impl method.
- `let value: &dyn LocalDispatchTrait = &TraitDispatchTarget; value.trait_value()` -> `MethodCall` with `InitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the concrete local trait impl method when the direct reference initializer proves the concrete type exactly.
- `let source = TraitDispatchTarget; let value: &dyn LocalDispatchTrait = &source; value.trait_value()` -> `MethodCall` with `InitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method` when the referenced local binding already carries exact concrete initializer proof.
- `(value).trait_value()` where `value` is an initialized local binding -> `MethodCall` with the same local binding proof as the unparenthesized receiver.
- `value.scoped_value()` where the concrete receiver type has a local trait impl in another module -> `MethodCall`, `Resolved(LocalExact)` only when the trait is visible through direct, alias, glob, or local re-exported import; missing trait visibility fails closed with `Unsupported`.
- `value.bound_value()` where `T: GenericBoundTrait` inline or in a `where` clause -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the local trait method declaration.
- `value.bound_value()` where `value: impl GenericBoundTrait` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the local trait method declaration.
- `value.bound_value()` where `value: &dyn GenericBoundTrait` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the local trait method declaration.
- `value.overlap()` where `AmbiguousTraitTarget` has two local trait impls declaring the same method -> `MethodCall`, `Ambiguous`, no semantic edge.
- `value.blanket_value()` where `impl<T> BlanketDispatchTrait for T` is visible -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the conservative unconstrained blanket impl method.
- `value.inline_bound_value()` through `impl<T: BlanketBound> InlineBoundBlanketTrait for T` and `value.where_bound_value()` through `impl<T> WhereBoundBlanketTrait for T where T: BlanketBound` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method` when the concrete receiver has exact local impl evidence for every blanket bound.
- `value.transitive_bound_value()` through `impl<T: TransitiveDerivedBound> TransitiveBoundBlanketTrait for T`, where `TransitiveDerivedBound` is itself proven by `impl<T: TransitiveBaseBound>`, resolves to the concrete blanket impl method when every bound in the chain has exact local evidence.
- `value.constrained_generic_self_value()` where the visible trait impl is `impl<T: GenericWrapperBound> ConstrainedGenericSelfTrait for GenericWrapper<T>` resolves only after proving the receiver generic argument `GenericBoundValue` satisfies `GenericWrapperBound`.
- `five()` in `const FN_CALL_CONST` -> `PathCall` owned by `CallBodyOwnerId::Const`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `five()` in `static STATIC_FN_CALL` -> `PathCall` owned by `CallBodyOwnerId::Static`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `assoc_const_value()` in inherent impl and trait associated const initializers -> `PathCall` owned by the associated const's `CallBodyOwnerId::Const`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `local_target()` in a trait default method body -> `PathCall` owned by `CallBodyOwnerId::Method`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `self.required()` in a trait default method body -> `MethodCall` with `SelfValue`, `Resolved(LocalExact)`, `CallRelation::Method` targeting the same-trait required method declaration.
- `Self::required_assoc()` in a trait default method body -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction` targeting the same-trait associated function declaration with no `self` receiver.
- `local_target()` inside closure and async block bodies in `fixture_call_graph` -> no outer-owner call site until nested closure/async owners exist.
- `<TraitAssocFunctionTarget as LocalAssocFunctionTrait>::trait_make()` for a directly visible local trait associated function with no `self` receiver -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction`.
- `ImportedAssocFunctionTrait::imported_trait_make()` and imported aliases of the same shorthand trait path -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::AssociatedFunction` for direct, alias, glob, grouped, and local re-export imported local traits with no `self` receiver.
- `alias_checker(&_trait_user)` where `alias_checker` is a local closure binding -> `PathCall` with `ValueBinding { path: ["alias_checker"] }`, `Unsupported`, no edge, so syntactic path calls to closure bindings stay visible without fake local function edges.
- `let local_target = || 377; local_target()` -> `PathCall` with `ValueBinding { path: ["local_target"] }`, `Unsupported`, no edge, so local bindings do not fake-resolve to same-named module functions.
- `let f = local_target; f()` -> `PathCall` with `InitializedValueBinding { path: ["f"], init_path: ["local_target"] }`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `let f: fn() -> i32 = local_target; f()` -> `PathCall` with `InitializedValueBinding`, `Resolved(LocalExact)`, `CallRelation::Function`.
- `let f = local_target; let g = f; g()` and `(g)()` -> initialized binding proof propagates through one exact local alias and resolves to `CallRelation::{Function, DynamicFunction}`.
- `let f: fn() -> i32 = local_target; let g: fn() -> i32 = f; g()` and `(g)()` -> typed initialized binding proof propagates through one exact local alias and resolves to `CallRelation::{Function, DynamicFunction}`.
- `f()` where `f: fn() -> i32` is an owner parameter -> `PathCall` with `ValueBinding { path: ["f"] }`, `Unsupported`, no edge.
- `(f)()` where `f: fn() -> i32` is an owner parameter -> `DynamicCall` with `LocalBinding { path: ["f"] }`, `Unsupported`, no edge.
- `(f as fn() -> i32)()` where `f: fn() -> i32` is an owner parameter -> `DynamicCall`, `Unsupported`, no edge.
- `(closure as fn() -> i32)()` where `closure` is a local closure binding -> `DynamicCall`, `Unsupported`, no edge; closure bindings are not admitted into function-pointer path proof.
- `(*closure)()` where `closure` is a local closure binding -> `DynamicCall`, `Unsupported`, no edge; dereferenced opaque closure bindings are not treated as initialized function-pointer proofs.
- `(move || local_target())()` -> outer closure-literal `DynamicCall`, `Unsupported`, no edge; the inner closure-body `local_target()` is not attributed to the enclosing function owner.
- `generic_f()` where `F: FnOnce() -> i32` -> `PathCall` with `ValueBinding { path: ["generic_f"] }`, `Unsupported`, no edge.
- `(generic_f)()` where `F: FnOnce() -> i32` -> `DynamicCall` with `LocalBinding { path: ["generic_f"] }`, `Unsupported`, no edge.
- `Box::new(local_target)` in boxed `dyn Fn` setup -> `PathCall`, `External`, no edge after local type/module lookup declines to produce a local target.
- `String::new()` and `Vec::new()` -> `PathCall`, `External`, no edge after local lookup declines to produce a local target.
- `let value: Vec<i32> = Vec::new(); value.len()` -> `MethodCall` with `TypedLocalBinding`, `External`, no edge after local shadow checks decline a local `Vec`.
- `let value: Vec = Vec; value.len()` inside a module with a local `Vec` type -> `MethodCall` with `TypedLocalBinding`, `Resolved(LocalExact)`, proving local shadowing wins before prelude method classification.
- `boxed_fn()` where `boxed_fn: Box<dyn Fn() -> i32>` -> `PathCall` with `ValueBinding { path: ["boxed_fn"] }`, `Unsupported`, no edge.
- `(boxed_fn)()` where `boxed_fn: Box<dyn Fn() -> i32>` -> `DynamicCall` with `LocalBinding { path: ["boxed_fn"] }`, `Unsupported`, no edge.
- `value.generic_instance::<i32>(123)` -> `MethodCall` with `TypedLocalBinding`, `generic_arg_count = 1`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `drop(value)` -> `PathCall`, `External`, no edge after local lookup declines to produce a local function target.
- `drop(1)` in a module with a local `fn drop(...)` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::Function`, proving local shadowing wins before prelude classification.
- `value.drop()` where `value = ExplicitDropTarget` and `ExplicitDropTarget` has an inherent `drop(self)` method -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method`; no special destructor handling is applied.
- `unsafe { unsafe_target() }` -> `PathCall`, `Resolved(LocalExact)`, `CallRelation::Function`; unsafe-specific metadata remains future.
- `(&value).instance_value()` where `value: LocalAssoc` -> `MethodCall` with `BorrowedTypedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `(*value).instance_value()` where `value = &LocalAssoc` -> `MethodCall` with `DereferencedInitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `make_local_assoc().instance_value()` -> outer `MethodCall` with `PathCallResult`, `Resolved(LocalExact)` through the local function return type, plus the inner path call is still recorded independently.
- `value.clone_assoc().instance_value()` -> outer `MethodCall` with `MethodCallResult`, `Resolved(LocalExact)` through the direct inner method call's exact local target return type; the inner method call is still recorded independently.
- `value.0.instance_value()` -> `MethodCall` with `FieldInitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::Method`, after proving the root tuple-struct constructor and exact field type.
- `value.0()` -> `DynamicCall` with field-initializer proof from the root tuple-constructor argument, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `(holder.callback)()` where `holder` is a parameter -> `DynamicCall` with `FieldLocalBinding`, `Unsupported`, no edge.
- `funcs[0]()` where `funcs` is a function-pointer array parameter -> `DynamicCall`, `Unsupported`, no edge.
- `make_ready_local_assoc().await.instance_value()` -> outer `MethodCall` with `AwaitPathCallResult`, `Resolved(LocalExact)` through the local async function return type, plus the inner path call is still recorded independently.
- `try_local_assoc()?.instance_value()` -> outer `MethodCall` with `TryPathCallResult`, `Resolved(LocalExact)` through the local function's syntactic `Result<T, E>` success type, plus the inner path call is still recorded independently.
- `std::path::Path::new("")` -> `PathCall`, `External`, no edge.
- `(closure)()` and `(|| 11)()` -> `DynamicCall`, `Unsupported`, no edge.
- `make_fn()()` -> inner `PathCall` for `make_fn()`, `Resolved(LocalExact)`, plus outer `DynamicCall`, `Unsupported`, no edge.
- `make_unary_fn()(5)` -> inner `PathCall` for `make_unary_fn()`, `Resolved(LocalExact)`, plus outer one-argument `DynamicCall`, `Unsupported`, no edge.
- `let f = local_target; (f)()` -> `DynamicCall` with `InitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `(local_target)()` -> `DynamicCall`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `(local_target as fn() -> i32)()` -> `DynamicCall` with `FnPointerCastPath`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction` when the cast path is unshadowed and resolves to one local function.
- `(f as fn() -> i32)()` where `f` is a local binding initialized from one local function -> `DynamicCall` with `FnPointerCastInitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `(*f)()` where `f` is a local binding initialized from one local function -> `DynamicCall` with `DereferencedInitializedLocalBinding`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `({ local_target })()` where the parenthesized block contains exactly one unshadowed path expression -> `DynamicCall` with `Path`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction`.
- `(if flag { local_target } else { local_target })()` -> `DynamicCall` with `IfBranchPaths`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction` when every supported branch path resolves to the same local function.
- `(if flag { local_target } else { other_target })()` -> `DynamicCall` with `IfBranchPaths`, `Ambiguous`, no edge when supported branch paths prove different local functions.
- `(if flag { f } else { f })()` where `f` is a function-pointer parameter -> `DynamicCall`, `Unsupported`, no edge; opaque parameter paths are not admitted into exact branch-path resolution.
- `(if flag { if flag { local_target } else { local_target } } else { local_target })()` -> `DynamicCall` with `IfBranchPaths`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction` when every nested branch leaf is an unshadowed item path resolving to the same local function.
- `(match flag { true => local_target, false => local_target })()` -> `DynamicCall` with `MatchArmPaths`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction` when every supported arm path resolves to the same local function.
- `(match flag { true => local_target, false => other_target })()` -> `DynamicCall` with `MatchArmPaths`, `Ambiguous`, no edge when supported arm paths prove different local functions.
- `(match flag { true => f, false => f })()` where `f` is a function-pointer parameter -> `DynamicCall`, `Unsupported`, no edge; opaque parameter paths are not admitted into exact arm-path resolution.
- `(match flag { true => match flag { true => local_target, false => local_target }, false => local_target })()` -> `DynamicCall` with `MatchArmPaths`, `Resolved(LocalExact)`, `CallRelation::DynamicFunction` when every nested unguarded arm leaf is an unshadowed item path resolving to the same local function.
- `(match flag { true if flag => local_target, _ => local_target })()` -> `DynamicCall`, `Unsupported`, no edge; guarded arms are not admitted into the exact arm-path resolver slice.
- `(if flag { local_target } else { || 8 })()` -> `DynamicCall`, `Unsupported`, no edge; non-path branches are not admitted into the exact branch-path resolver slice.
- `(match flag { true => local_target, false => || 13 })()` -> `DynamicCall`, `Unsupported`, no edge; non-path arms are not admitted into the exact arm-path resolver slice.
- `documented_macro!(...)` and statement-position `println!(...)` -> `MacroCall`, `Unsupported`, no edge.
- `info!(...)` and `debug!(...)` in `fixture_path_resolution::root_func` -> `MacroCall`, `Unsupported`, no edge.
- `crate::crate_scoped_macro!()` -> `MacroCall` preserving the path discriminator, `Unsupported`, no edge.
- `vec![1, 2, 3]` -> `MacroCall`, `Unsupported`, no edge; macro expansion remains out of scope.
- `imported_macro_alias!()` -> `MacroCall` preserving the visible alias spelling, `Unsupported`, no edge.
- `call_graph_item_macro!();` inside a function body -> `MacroCall`, `Unsupported`, no expansion-derived item/call edge.

Current DB helper behavior:

`ploke-db --features call_graph` exposes typed helpers over the persisted
relations: `call_sites_for_owner`, `call_targets_for_site`,
`call_resolution_for_site`, `call_context_for_owner`, and
`callers_for_target`, plus the application-facing `expand_call_context` helper
modeled after `expand_type_context`. The first helper tests cover resolved
function targets, resolved method targets, resolved associated-function targets
with method endpoint kind, constructor targets, resolved dynamic function
targets with `relation_kind = "DynamicFunction"`, unsupported dynamic sites
with no edge, `SelfField`, `LocalBinding`, and `TypedLocalBinding` and
`InitializedLocalBinding` method receiver payload decoding, target-centered
incoming caller rows, owner/target call-context expansion candidates, and
fail-closed behavior when a persisted call site lacks
`call_resolution_status`. Expansion promotes only resolved call edges; raw
low-level helpers still expose persisted non-resolved target rows for stricter
proof validation. The DB read helpers now also validate call-relation endpoint
families before surfacing persisted rows, so malformed `call_relation` facts
whose `source_kind` disagrees with the actual call-site kind or whose
`relation_kind/source_kind/target_kind` tuple is not one of the typed parser
families are excluded from owner context, target-centered callers, and
call-context expansion. `call_resolution_status` rows are also checked against
the actual call-site kind before downstream helpers trust the status.
`BodyContainsCall` edges are likewise checked against the actual call-site kind
before owner context or target-centered callers assemble the site. Fixture DB
invariants now also assert every persisted `call_site` has one matching
`BodyContainsCall` edge, every call site has one matching
`call_resolution_status`, and every `call_relation` is anchored to an existing
call site and endpoint node. They also assert persisted `call_site.id` values
stay disjoint from stored code-node and type-use/type IDs, so the DB projection
keeps call occurrences in the `CallId` universe instead of a `NodeId` or
`TypeId` endpoint family. The inverse checks assert every persisted
`BodyContainsCall` and `call_resolution_status` row points back to an existing
matching owner and call site, with no orphaned rows. Status-to-relation
cardinality checks assert each raw `Resolved(LocalExact)` site has exactly one
semantic `call_relation`, while non-resolved sites have none. Proof projection
linkage checks assert a mixed real owner projects one `call_site` and one
`call_resolution` fact per call site, resolved rows project exactly one
`call_edge`, and non-resolved rows project no edge plus a blocker reason.
Target-centered proof projection linkage checks assert the real `local_target`
incoming caller set projects one `call_site`, one `call_resolution`, and one
matching `call_edge` fact per caller row, with no unrelated proof rows and with
source provenance matching the originating call-site span.
Synthetic receiver-decoder coverage now also pins the remaining method receiver
families exposed by the typed DB helper, including `SelfValue`,
borrowed/dereferenced locals, field receivers, path/method result receivers,
await/try result receivers, and literal receivers.

The first fixture-backed DB contracts live in
`crates/ploke-db/tests/unit/call_graph_fixture_queries.rs`. They parse and
transform `fixture_call_graph` and `fixture_nodes` into an in-memory DB before
asserting persisted rows for resolved path calls, local/initialized/typed-local
method receivers including parenthesized receivers and Rust type-alias receiver
annotations, associated-function calls, imported/re-exported type and trait
associated-function calls, tuple and enum constructors, nested
returned-function calls, dynamic function calls, trait-dispatch method calls,
borrowed/dereferenced method receiver rows,
path/method/await/try result receiver rows, tuple-field method/dynamic rows,
raw identifier path/method calls, prelude `drop(...)` vs local shadowed `drop`
resolution, explicit inherent `drop(self)` calls, literal/prelude method
classification, `String::new` / `Vec::new` targetless external rows, local
shadowed `Vec::len` resolution, inherent-over-trait precedence,
cast/deref/block/branch, field, and indexed dynamic function rows including
exact member/index aliases and parenthesized function-item / typed
function-pointer alias bindings, boxed/generic Fn-style targetless dynamic
failures, bare callable-value path failures, fail-closed guarded/nested branch
and opaque closure/index dynamic callees, Rust type-alias associated-function
and instance-method rows, method-as-associated-function rows, function-item and
typed function pointer binding rows, imported function-item binding rows,
generic-bound and trait-object declaration-target rows, aliased/reference
trait-object concrete receiver rows, constrained generic self-type trait impl
rows, imported-trait impl method rows,
blanket-trait impl method rows, method-body owner rows for trait defaults and
impl method bodies, const/static and associated-const initializer owner rows,
expanded borrowed/reference/dereferenced receiver rows, closure/async body
call non-projection onto enclosing owners, and macro status rows.
Ordinary path-call resolution rows now include unqualified local functions,
self/super paths, import aliases, glob imports, grouped imports, re-exports,
and module aliases;
associated-function owner rows now include inherent
`Self::make()`, qualified `<LocalAssoc>::make()`, and fully qualified local
trait associated-function forms. They also assert real transformed
external, ambiguous, and unsupported statuses carry no semantic target rows,
assert target-centered incoming caller rows for real local function, method,
associated-function, tuple-struct constructor, and enum-variant constructor
targets, assert target-centered local-function caller queries and
target-seeded expansion exclude closure/async body outer owners,
assert real outgoing and incoming `expand_call_context` candidates preserve
persisted call-site identity for ordinary path and resolved dynamic callers,
and project resolved/external rows into proof facts from real fixture owner
provenance.
Fixture proof-store coverage now also checks `proof_symbol_lookup` links real
owner-scoped and target-centered resolved callee hits back to companion
`call_site` and `call_resolution` facts by call-site identity, including
target-centered `local_target` lookup with both an ordinary path caller and a
resolved dynamic caller.

Downstream RAG coverage now exercises the DB call-context expansion helper
through both private expansion and the public sparse `get_context` path for
owner-seeded outgoing callee targets and target-seeded incoming callers,
including function and method targets plus private expansion for
associated-function targets. The method-target public path seeds
`LocalAssoc::instance_value`, materializes both the instance-method caller and
the associated-function path-call caller, and verifies the final assembled
context keeps the outgoing call edges pointed at the seeded method target and
keeps `CallExpansionInfo` with seed, relation, call-site ID, target ID, and
distance. Owner-seeded public coverage also verifies an outgoing callee target
part carries an `OutgoingTarget` expansion reason. The associated-function
expansion path seeds `LocalAssoc::make` and verifies both method-owner
`Self::make` and qualified function-owner `LocalAssoc::make` callers keep
outgoing `AssociatedFunction` context.
TUI formatter and context-plan overlay coverage now render the same
associated-function payload shape as `AssociatedFunction:<id>` rather than
collapsing it into a method or function target label, and now render
call-expansion provenance separately from outgoing call-context rows.

Current proof projection behavior:

`ploke-db --features call_graph` projects owner-scoped persisted call graph rows
into the existing `proof_fact` store through `call_proof_facts_for_owner` and
`project_call_proof_facts_for_owner`, and projects target-centered incoming
call sites through full `CallContextRow` values in
`call_proof_facts_for_target` and `project_call_proof_facts_for_target`. The
projection requires an explicit `build_domain_id`, derives source-file
provenance through existing module/file ancestry rules, stores resolved local
targets as proof `call_edge` facts, and maps
external/unsupported/unresolved/ambiguous call statuses to fail-closed
`call_resolution` blocker reasons without inventing local edges. Ambiguous
dynamic candidate sets stay attached to the target-centered call site rather
than collapsing to the matched target. Any local target row on a non-resolved
call status is rejected before proof facts are stored.
Fixture-backed proof coverage now includes both single-row resolved/external
owners, local/self/super/crate/module-qualified/imported function path
resolution owners, owner-scoped associated-function path owners across inherent,
type-import, type-alias, method-as-associated, and trait associated-function
forms, owner-scoped local/initialized/typed/type-alias/borrowed/dereferenced
method receiver owners, owner-scoped generic-bound/imported-trait/constrained
generic-self/blanket trait method owners, owner-scoped path/method/await result
receiver and tuple-field receiver method owners, a mixed multi-row owner with
unsupported `Ok(...)`, resolved
`try_local_assoc()`, and a resolved try-result method receiver, plus
target-centered projection of the incoming `try_local_assoc` caller edge without
including unrelated unsupported owner calls. Method-target proof projection now
also covers the real `LocalAssoc::instance_value` target and verifies resolved
proof edges for both incoming method-call and associated-function path-call
owners. Trait-dispatch proof projection now covers owner-scoped and
target-centered resolved proof edges for real `LocalDispatchTrait for
TraitDispatchTarget` method calls, including concrete trait-object alias and
chained-reference callers. Dynamic proof projection now covers real resolved
`DynamicFunction` calls including parenthesized path/binding, cast/deref, block,
indexed-array, named-field/tuple-field, and same-target branch/match callees,
unsupported
closure-binding cast and dereferenced closure-binding dynamic calls as
`dynamic_dispatch_unbounded` blockers, ambiguous branch/match dynamic calls as
`type_resolution_missing` blockers, guarded/opaque/nested branch/match dynamic
calls as `dynamic_dispatch_unbounded` blockers, and target-centered projection
from `local_target` that preserves a dynamic incoming caller edge without
pulling in unrelated unsupported dynamic blockers or closure/async body
outer-owner proof facts. Initializer-owner proof projection now
covers top-level const/static and associated-const owners, preserving the value
owner as the proof caller and storing resolved edges to the local initializer
functions. Constructor proof projection now uses shared fixture cases to cover
resolved tuple struct and enum variant constructor target families as proof
edges to `StructNodeId` and `VariantNodeId` callees in both owner-scoped and
target-centered projections.
Macro proof projection now covers real targetless macro
calls as `macro_expansion_not_available` blockers, and ambiguous method
projection covers real targetless ambiguity as a `type_resolution_missing`
blocker. Real fixture-derived external call-resolution blockers now also feed
`proof_invariant_findings` when proof-only effect evidence references the same
call site. Synthetic DB coverage also asserts owner-scoped and target-centered
projection reject local target edges whose call status is not resolved,
including ambiguous rows, and does not store partial proof facts after the
rejection.

Current RAG/TUI call-context behavior:

`ploke-rag --features call_graph` collects outgoing call context for materialized
owner hits when the active DB has all four call graph relations. It also expands
target-centered hits to materializable caller owners through
`Database::callers_for_target(...)` before optional reranking/context assembly.
The payload is attached to `ContextPart.call_context` and forwarded to
`ConciseContext.call_context`; `RagService` degrades call-context collection and
expansion off for databases missing those relations so stale backups do not
make ordinary RAG context retrieval fail. TUI context-plan/system formatting now
displays outgoing-call summaries with callee shape, span, status/resolution, and
target relation IDs; RAG preserves `LocalBinding` and `TypedLocalBinding` and
`InitializedLocalBinding` method receiver payloads plus `DynamicFunction`,
`AssociatedFunction`, `TupleStructConstructor`, and `EnumVariantConstructor`
relation kinds in target payloads. Fixture-backed RAG coverage now
parses/transforms `fixture_call_graph` and asserts real
`call_try_result_instance_method` outgoing rows are collected for unsupported
`Ok(...)`, resolved `try_local_assoc()`, and the resolved try-result method
receiver. It also asserts real dynamic outgoing rows are collected for resolved
`DynamicFunction` calls and targetless unsupported dynamic calls, and real
constructor rows are collected for `TupleStructConstructor` and
`EnumVariantConstructor` target families. It also asserts real external
targetless rows preserve path, literal receiver, and typed-local receiver
payloads without semantic targets, and real targetless macro and ambiguous
method blocker rows are collected without semantic targets.
Incoming
expansion coverage seeds with `try_local_assoc`, `local_target`, method
targets, associated-function targets, constructor targets, and concrete
trait-dispatch method targets, and asserts caller owners are materialized with
outgoing call context pointing back to the seed target while helper-level
`local_target` expansion excludes closure/async body outer owners and preserves
exact path/dynamic call-site provenance. Public
`get_context` coverage now proves the function, method, tuple-struct
constructor, enum-variant constructor, concrete trait-dispatch, and
`local_target` closure/async-exclusion target-centered expansions survive
sparse retrieval and final context assembly, with `local_target` preserving
both ordinary path and resolved dynamic callers. TUI formatter coverage asserts
the same payload shape renders with
callee shape, span, status/resolution, and target relation IDs intact in both
model-facing context text and expanded context-plan overlay details, including
associated-function, dynamic-function, tuple-struct-constructor, and
enum-variant-constructor target labels, plus targetless macro and ambiguous
method blocker rows under the existing call-context row cap. Separate compact
formatter/overlay/tool-carrier coverage asserts external targetless path,
literal receiver, typed-local receiver, separate ordinary path and dynamic
incoming caller payloads, callable-path blocker, returned-function mixed,
boxed `dyn Fn` setup/failure, and `Vec::new()` external rows render without
targets. TUI formatter/overlay coverage also asserts concrete trait-dispatch
method rows render initialized-local receiver proof such as
`value = TraitDispatchTarget` and preserve the `Method:<id>` target label. Tool JSON roundtrip
coverage asserts `request_code_context` preserves both
`ConciseContext.call_expansion` and `ConciseContext.call_context` through
`ContextPart -> ConciseContext` conversion and serde, including
trait-dispatch initialized-local receiver rows, dynamic-function, constructor,
external targetless, macro blocker, and ambiguous blocker call-context rows.

## Fixture-backed target index

This section maps the exhaustive rows below to concrete fixtures we can use. Prefer rows marked **ready** before adding new fixture code. Rows marked **needs fixture** are known gaps.

### Artificial parser fixtures: ready or near-ready

| Fixture | File | Expression / target | Matrix rows | Status | Notes |
|---|---|---|---|---|---|
| `fixture_nodes` | `src/impls.rs:45` | `self.private_method()` | M01, M22 | **green** | Covered by `fixture_nodes_public_method_records_and_resolves_self_private_method_call_site`; structural `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method`. |
| `fixture_nodes` | `src/impls.rs:52` | `self.secret.len()` | M02, M07 | **green** | Covered by `fixture_nodes_get_secret_len_records_self_field_len_external_method_call_site`; records `SelfField` and classifies `String::len` as `External` with no local edge. |
| `fixture_nodes` | `src/impls.rs:77` | `self.value.len()` | M03, M07 | **green** | Covered by `fixture_nodes_get_str_len_records_self_field_len_method_call_site`; records `SelfField` and fails closed before generic field receiver typing. |
| `fixture_nodes` | `src/impls.rs:103` | `self.value.into()` | M04 | **green** | Covered by `fixture_nodes_generic_simple_trait_method_records_self_field_into_method_call_site`; records `SelfField` and fails closed before generic field receiver typing. |
| `fixture_nodes` | `src/imports.rs:108` | `HashMap::<String, i32>::new()` | P18 | **green** | Covered by `fixture_nodes_use_imported_items_records_hashmap_new_path_call_site`; imported std type path call, `External`, no edge. |
| `fixture_nodes` | `src/imports.rs:120` | `fs::read_to_string("dummy")` | P07 | **green** | Covered by `fixture_nodes_use_imported_items_records_fs_read_to_string_path_call_site`; imported std module path call, `External`, no edge. |
| `fixture_nodes` | `src/imports.rs:125` | `EnumWithData::Variant1(1)` | P23 | **green** | Covered by `fixture_nodes_use_imported_items_records_enum_variant1_path_call_site`; resolves to `CallRelation::EnumVariantConstructor`. |
| `fixture_nodes` | `src/imports.rs:134` | `alias_checker(&_trait_user)` | P25, D12/D13 policy | **green** | Covered by `fixture_nodes_use_imported_items_records_alias_checker_value_binding_path_call_site`; syntactically path call to closure binding records `ValueBinding` and fails closed with `Unsupported`. |
| `fixture_nodes` | `src/imports.rs:152` | `documented_macro!(fixture alias coverage)` | X01 | **green** | Structural `MacroCall`, `Unsupported`. |
| `fixture_nodes` | `src/imports.rs:165` | `Duration::from_secs(1)` | P20 | **green** | Covered by `fixture_nodes_use_imported_items_records_duration_from_secs_path_call_site`; imported absolute std type path call, `External`, no edge. |
| `fixture_nodes` | `src/imports.rs:172` | `Arc::new(1)` | P21 | **green** | Covered by `fixture_nodes_use_imported_items_records_arc_new_path_call_site`; imported std type path call, `External`, no edge. |
| `fixture_nodes` | `src/imports.rs:174` | `TupleStruct(1, 2)` | P22 | **green** | Covered by `fixture_nodes_use_imported_items_records_tuple_struct_path_call_site`; resolves to `CallRelation::TupleStructConstructor`. |
| `fixture_nodes` | `src/const_static.rs:54` | `five()` in `const FN_CALL_CONST` | owner matrix const | **green** | Covered by `fixture_nodes_fn_call_const_resolves_const_initializer_path_call_site`; records `CallBodyOwnerId::Const` and resolves to `CallRelation::Function`. |
| `fixture_nodes` | `src/const_static.rs:151` | `five()` in `static STATIC_FN_CALL` | owner matrix static | **green** | Covered by `fixture_nodes_static_fn_call_resolves_static_initializer_path_call_site`; records `CallBodyOwnerId::Static` and resolves to `CallRelation::Function`. |
| `fixture_nodes` | `src/const_static.rs:148` | `println!(...)` | X02, X09 | **green** | Covered by `fixture_nodes_use_all_const_static_records_println_macro_call_site`; statement-position macro call in ordinary function body. |
| `fixture_macros` | `src/lib.rs:23` | `local_macro!(my_var)` | X08, X09 | **green** | Covered by `fixture_macros_use_local_macro_records_local_macro_call_site`; local macro invocation in function body. |
| `fixture_macros` | `src/lib.rs:24` | `println!("{}", my_var)` | X02, X09 | **green** | Covered by `fixture_macros_use_local_macro_records_println_macro_call_site`; standard macro invocation adjacent to local macro. |
| `fixture_path_resolution` | `src/lib.rs:137` | `Regex::new(...).unwrap()` | P20-like, M13 | **green** | Covered by `fixture_path_resolution_root_func_records_regex_new_external_path_call_site` and `fixture_path_resolution_root_func_records_regex_unwrap_external_method_call_site`; inner dependency path call is `External`, and the outer unwrap records `PathCallResult`/`External` with no local edge. |
| `fixture_path_resolution` | `src/lib.rs:142` | `std::path::Path::new("")` | P07/P20-like | **green** | Covered by `fixture_path_resolution_root_func_records_std_path_new_external_path_call_site`; direct external-root path call, `External`, no edge. |
| `fixture_path_resolution` | `src/lib.rs:139-149` | `TypeId::Synthetic(NodeId::generate_synthetic(...).uuid())`, `uuid::Uuid::nil()` | P07/P20-like, M14 | **green** | Covered by `fixture_path_resolution_root_func_records_typeid_synthetic_external_path_call_site`, `fixture_path_resolution_root_func_records_nodeid_generate_synthetic_external_path_call_site`, `fixture_path_resolution_root_func_records_uuid_nil_external_path_call_site`, and `fixture_path_resolution_root_func_records_nodeid_uuid_external_method_call_site`; imported workspace/dependency calls are `External`, and the outer `.uuid()` method receiver records as `PathCallResult`/`External`. |
| `fixture_path_resolution` | `src/lib.rs:135,152` | `info!(...)`, `debug!(...)` | X01/X02-like | **green** | Covered by `fixture_path_resolution_root_func_records_info_macro_call_site` and `fixture_path_resolution_root_func_records_debug_macro_call_site`; logging macro invocations record as `MacroCall` and fail closed. |
| `fixture_path_resolution` | `src/lib.rs:70` | `super::restricted_func()` | P04 | **green** | Covered by `fixture_path_resolution_call_restricted_resolves_super_restricted_func_path_call_site`; super-qualified local path call resolves to `CallRelation::Function`. |
| `fixture_impls` | `src/main.rs:9-13` | `x.func_test_one()`, `x.func_test_two()`, `x.func_test_three()`, `x.func_test_five()` | M05, M20-ish | **green** | Covered by `fixture_impls_main_resolves_func_test_*_initialized_local_method_call_site`; struct-literal initializer proof records `InitializedLocalBinding` and resolves exact local inherent methods. |
| `fixture_impls` | `src/main.rs:12` | `TestImplStruct::func_test_four()` | P12/P14 | **green** | Covered by `fixture_impls_main_resolves_func_test_four_associated_function_path_call_site`; associated-function-shaped path call resolves to the local impl method. |
| `fixture_impls` | `src/main.rs:15` | `println!(...)` | X02, X09 | **green** | Covered by `fixture_impls_main_records_println_macro_call_site`; macro call in binary main. |
| `fixture_edge_cases` | `src/lib.rs:137-142` | `h.help()`, `uh.help()`, `"hello".to_string()` | M05, M06 | **green** | Covered by `fixture_edge_cases_use_imports_resolves_direct_imported_helper_method_call_site`, `fixture_edge_cases_use_imports_resolves_reexported_helper_method_call_site`, and `fixture_edge_cases_use_imports_records_literal_to_string_external_method_call_site`; direct and re-exported local type imports resolve initialized-local method receivers, while literal `.to_string()` is classified as `External`. |
| `fixture_edge_cases` | `src/lib.rs:65` | `format!(...)` in trait impl method body | X02, X09 | **green** | Covered by `fixture_edge_cases_processor_trait_impl_records_format_macro_call_site`; macro invocation is attributed to the trait impl method owner and remains `Unsupported` with no expansion edge. |
| `fixture_edge_cases` | `src/lib.rs:51` | `T::default()` in `GenericItem::new` | P14/P20-like | **green** | Covered by `fixture_edge_cases_generic_item_new_records_t_default_unsupported_path_call_site`; generic type-parameter associated-function-looking path calls are recorded structurally and fail closed as `Unsupported` before generic trait associated function resolution. |
| `fixture_edge_cases` | `src/lib.rs:115-117` | `utils::internal_helper()`, `utils::super_helper()`, `restricted::restricted_func()` in cfg-gated visibility function | P03/P04 | **green** | Covered by `fixture_edge_cases_test_visibility_resolves_internal_helper_path_call_site`, `fixture_edge_cases_test_visibility_resolves_super_helper_path_call_site`, and `fixture_edge_cases_test_visibility_resolves_restricted_func_path_call_site`; effective cfg is `not (feature = "type_bearing_ids")`, and local module-relative path calls resolve to `CallRelation::Function`. |
| `fixture_type_resolution_v2` | `src/lib.rs:66` | `panic!(...)` in generic associated const initializer | X02/X09, owner associated const | **green** | Covered by `fixture_type_resolution_v2_generic_assoc_const_records_panic_macro_call_site`; macro invocation is attributed to the impl associated const owner and remains `Unsupported` with no expansion edge. |
| `fixture_generics` | `src/lib.rs:32,62` | `T::default()` in `generic_function`, `format!(...)` in generic trait impl method | P14/P20-like, X02/X09 | **green** | Covered by `fixture_generics_generic_function_records_t_default_unsupported_path_call_site` and `fixture_generics_trait_impl_process_records_format_macro_call_site`; generic type-parameter associated-function-looking path calls fail closed as `Unsupported`, and macro invocation is recorded structurally without expansion. |
| `fixture_edge_cases` | `src/lib.rs` | remaining unusual syntax | P28/raw identifiers maybe | **partial scan** | `use_imports`, visibility-call, trait-impl macro, and generic-associated rows are green; raw identifier coverage is owned by `fixture_call_graph`. |
| `fixture_call_graph` | `src/lib.rs:5` | `(closure)()` | D01/D02 | **green** | Focused fixture added for dynamic call syntax; covered by `fixture_call_graph_dynamic_calls_records_parenthesized_binding_dynamic_call_site`. |
| `fixture_call_graph` | `src/lib.rs:6` | `(|| 11)()` | D03 | **green** | Focused fixture added for closure literal call syntax; covered by `fixture_call_graph_dynamic_calls_records_closure_literal_dynamic_call_site`. |
| `fixture_call_graph` | `src/lib.rs:710` | `(move || local_target())()` | D03 / owner matrix closure | **green** | Covered by `fixture_call_graph_call_move_closure_literal_with_body_call_records_outer_dynamic_call_site` and `fixture_call_graph_move_closure_body_call_is_not_recorded_as_outer_call_site`; outer dynamic call is visible, inner closure-body call is not owned by the enclosing function. |
| `fixture_call_graph` | `src/lib.rs:15` | `crate::local_target()` | P02/P04 | **green** | Covered by `fixture_call_graph_call_crate_local_target_resolves_crate_path_call_site`; explicit crate-root local function path resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:24` | `self::nested_target()` | P03/P04 | **green** | Covered by `fixture_call_graph_call_self_nested_target_resolves_self_path_call_site`; explicit self-module local function path resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:412` | `crate::local_mod::nested_target()` | P02/P03 | **green** | Covered by `fixture_call_graph_call_crate_module_nested_target_resolves_crate_module_path_call_site`; explicit crate-root module-qualified local path resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:416` | `self::local_mod::nested_target()` | P02/P03 | **green** | Covered by `fixture_call_graph_call_self_module_nested_target_resolves_self_module_path_call_site`; explicit self-root module-qualified local path resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:29` | `local_target()` | P01 | **green** | Covered by `fixture_call_graph_call_unqualified_local_target_resolves_local_path_call_site`; unqualified local function path resolves in the owner's containing module. |
| `fixture_call_graph` | `src/lib.rs:37` | `make_fn()()` | D04/P30 | **green** | Covered by `fixture_call_graph_call_returned_function_resolves_inner_make_fn_path_call_site` and `fixture_call_graph_call_returned_function_records_outer_dynamic_call_site`; nested visitor records inner path call plus outer dynamic call. |
| `fixture_call_graph` | `src/lib.rs:751` | `make_unary_fn()(5)` | P30 | **green** | Covered by `fixture_call_graph_call_chained_returned_function_resolves_inner_make_unary_fn_path_call_site` and `fixture_call_graph_call_chained_returned_function_records_outer_dynamic_call_site`; the outer dynamic call records `arg_count = 1` and remains unsupported. |
| `fixture_call_graph` | `src/lib.rs:763` | `unsafe { unsafe_target() }` | P10 | **green** | Covered by `fixture_call_graph_call_unsafe_function_resolves_unsafe_target_path_call_site`; records the unsafe function invocation as a local `PathCall` and resolves the function edge. |
| `fixture_call_graph` | `src/lib.rs:769` | `NewType(value)` | P24 | **green** | Covered by `fixture_call_graph_call_new_type_constructor_resolves_tuple_struct_constructor_call_site`; resolves to `CallRelation::TupleStructConstructor` with a `StructNodeId` target. |
| `fixture_call_graph` | `src/lib.rs:48` | `Self::make()` | P13 | **green** | Covered by `fixture_call_graph_call_self_make_resolves_self_associated_function_path_call_site`; inherent same-impl associated function resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:53` | `LocalAssoc::make()` | P14 | **green** | Covered by `fixture_call_graph_call_local_assoc_make_resolves_type_associated_function_path_call_site`; directly visible local type plus inherent impl self-type proof resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:57` | `<LocalAssoc>::make()` | P15 | **green** | Covered by `fixture_call_graph_call_qualified_local_assoc_make_resolves_type_associated_function_path_call_site`; qualified local type path normalizes to `[LocalAssoc, make]` and resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:366` | `ImportedAssocAlias::make()` | P14/P05 | **green** | Covered by `fixture_call_graph_call_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`; local type alias import resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:370` | `ImportedAssoc::make()` from glob-imported type | P14/P06 | **green** | Covered by `fixture_call_graph_call_glob_imported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`; local glob import resolves type segment before inherent associated-function resolution. |
| `fixture_call_graph` | `src/lib.rs:374` | `ReexportedAssoc::make()` | P14/P05 | **green** | Covered by `fixture_call_graph_call_reexported_type_assoc_make_resolves_imported_type_associated_function_path_call_site`; local type re-export resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:981` | `LocalAssocTypeAlias::make()` where `type LocalAssocTypeAlias = LocalAssoc` | P14 | **green** | Covered by `fixture_call_graph_call_type_alias_assoc_make_resolves_aliased_type_associated_function_path_call_site`; one-hop Rust type-alias target proof resolves to the inherent associated function on `LocalAssoc`. |
| `fixture_call_graph` | `src/lib.rs:987` | `LocalAssocAliasChain::make()` where `type LocalAssocAliasChain = LocalAssocTypeAlias` | P14 | **green** | Covered by `fixture_call_graph_call_type_alias_chain_assoc_make_resolves_aliased_type_associated_function_path_call_site`; bounded Rust type-alias-chain target proof resolves to the inherent associated function on `LocalAssoc`. |
| `fixture_call_graph` | `src/lib.rs:1002` | `ImportedLocalAssocAlias::make()` where an imported Rust type alias targets `LocalAssoc` | P14/P05 | **green** | Covered by `fixture_call_graph_call_imported_type_alias_assoc_make_resolves_aliased_type_associated_function_path_call_site`; the import binding resolves the alias segment and bounded alias target proof resolves to the inherent associated function on `LocalAssoc`. |
| `fixture_call_graph` | `src/lib.rs:72` | `imported_alias()` | P05 | **green** | Covered by `fixture_call_graph_call_imported_alias_target_resolves_imported_path_call_site`; local alias import resolves through `ImportedBy` to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:76` | `globbed_target()` | P06 | **green** | Covered by `fixture_call_graph_call_glob_imported_target_resolves_glob_path_call_site`; local glob import resolves through `ImportedBy` to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:80` | `reexported_target()` | P05 | **green** | Covered by `fixture_call_graph_call_reexported_target_resolves_reexport_path_call_site`; local re-export alias resolves through import binding backlinks to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:84` | `targets_alias::globbed_target()` | P03/P05 | **green** | Covered by `fixture_call_graph_call_imported_module_target_resolves_module_alias_path_call_site`; imported local module alias resolves then terminal function resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:1182,1186` | `grouped_alias()`, `grouped_globbed_alias()` from `use super::import_targets::{...}` | P05 | **green** | Covered by `fixture_call_graph_call_grouped_imported_*_target_resolves_imported_path_call_site`; grouped import syntax expands into the same import backlink proof as direct aliases. |
| `fixture_call_graph` | `src/lib.rs:458` | `super::local_target()` | P04 | **green** | Covered by `fixture_call_graph_call_super_local_target_resolves_super_path_call_site`; explicit super local path resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:94` | `value.instance_value()` | M05 | **green** | Covered by `fixture_call_graph_call_param_instance_method_resolves_local_binding_method_call_site`; named owner parameter receiver resolves to inherent `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:99` | `value.instance_value()` after `let value: LocalAssoc = ...` | M05 | **green** | Covered by `fixture_call_graph_call_typed_local_instance_method_resolves_typed_local_binding_method_call_site`; explicit local type annotation resolves to inherent `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:104` | `value.instance_value()` after `let value = LocalAssoc` | M05 | **green** | Covered by `fixture_call_graph_call_initialized_local_instance_method_resolves_initialized_local_binding_method_call_site`; path initializer proof resolves to inherent `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:343` | `(value).instance_value()` after `let value: LocalAssoc = ...` | M05 | **green** | Covered by `fixture_call_graph_call_parenthesized_typed_local_instance_method_resolves_typed_local_binding_method_call_site`; parenthesized receiver reuses typed local binding proof. |
| `fixture_call_graph` | `src/lib.rs:1211` | `self.value.instance_value()` where `value: LocalAssoc` is a field on the enclosing impl self type | M26 | **green** | Covered by `fixture_call_graph_call_self_field_instance_method_resolves_self_field_method_call_site`; single-segment `SelfField` receiver resolves through the proven local field type to `CallRelation::Method`, with DB/RAG/TUI target-centered caller and proof-context coverage. |
| `fixture_call_graph` | `src/lib.rs:992` | `value.instance_value()` after `let value: LocalAssocAliasChain = ...` | M05 | **green** | Covered by `fixture_call_graph_call_type_alias_chain_instance_method_resolves_aliased_type_method_call_site`; typed local receiver annotations can resolve through bounded Rust type-alias chains when existing type relations prove each alias target exactly. |
| `fixture_call_graph` | `src/lib.rs:1007` | `value.instance_value()` after `let value: ImportedLocalAssocAlias = ...` | M05/P05 | **green** | Covered by `fixture_call_graph_call_imported_type_alias_instance_method_resolves_aliased_type_method_call_site`; imported Rust type aliases in typed local receiver annotations resolve through import binding proof plus bounded alias target proof. |
| `fixture_call_graph` | `src/lib.rs:421` | `LocalAssoc::instance_value(&value)` | P12/P14 | **green** | Covered by `fixture_call_graph_call_method_as_associated_function_resolves_inherent_method_path_call_site`; inherent method-as-associated-function syntax resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:114` | `assoc_const_value()` in `impl AssocConstCarrier` associated const | owner matrix associated const | **green** | Covered by `fixture_call_graph_impl_assoc_const_resolves_initializer_path_call_site`; records associated const `CallBodyOwnerId::Const` and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:118` | `assoc_const_value()` in `trait LocalAssocConstTrait` associated const default | owner matrix associated const | **green** | Covered by `fixture_call_graph_trait_assoc_const_resolves_initializer_path_call_site`; records associated const `CallBodyOwnerId::Const` and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:134` | `value.trait_value()` where `value: TraitDispatchTarget` is a named parameter | M19 | **green** | Covered by `fixture_call_graph_call_param_trait_method_resolves_local_trait_impl_method_call_site`; exact receiver type, local visible trait target, and concrete impl method resolve to `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:139` | `value.trait_value()` after `let value: TraitDispatchTarget = ...` | M19 | **green** | Covered by `fixture_call_graph_call_typed_local_trait_method_resolves_local_trait_impl_method_call_site`; explicit local type annotation resolves through the local trait impl method. |
| `fixture_call_graph` | `src/lib.rs:144` | `value.trait_value()` after `let value = TraitDispatchTarget` | M19 | **green** | Covered by `fixture_call_graph_call_initialized_local_trait_method_resolves_local_trait_impl_method_call_site`; path initializer proof resolves through the local trait impl method. |
| `fixture_call_graph` | `src/lib.rs:348` | `(value).trait_value()` after `let value = TraitDispatchTarget` | M05/M19 | **green** | Covered by `fixture_call_graph_call_parenthesized_initialized_local_trait_method_resolves_local_trait_impl_method_call_site`; parenthesized receiver reuses initialized local binding proof. |
| `fixture_call_graph` | `src/lib.rs:148` | `(local_target)()` | D01 | **green** | Covered by `fixture_call_graph_call_parenthesized_local_target_resolves_dynamic_function_call_site`; parenthesized non-local-binding callee path resolves to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:614` | `(local_target as fn() -> i32)()` | D11 | **green** | Covered by `fixture_call_graph_call_function_pointer_cast_path_resolves_dynamic_function_call_site`; bare function-pointer cast over an unshadowed local function path resolves to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:619` | `(f as fn() -> i32)()` after `let f: fn() -> i32 = local_target` | D11/P26 | **green** | Covered by `fixture_call_graph_call_function_pointer_cast_binding_resolves_initialized_dynamic_function_call_site`; initialized local binding proof resolves the cast callee to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:624` | `(*f)()` after `let f: fn() -> i32 = local_target` | D10/P26 | **green** | Covered by `fixture_call_graph_call_dereferenced_function_pointer_binding_resolves_initialized_dynamic_function_call_site`; initialized local binding proof resolves the dereferenced callee to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:628` | `({ local_target })()` | D07 | **green** | Covered by `fixture_call_graph_call_block_function_item_resolves_dynamic_function_call_site`; a parenthesized block with a single unshadowed path expression reuses dynamic path resolution and resolves to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:636` | `(if flag { local_target } else { local_target })()` | D05 | **green** | Covered by `fixture_call_graph_call_if_same_function_item_resolves_dynamic_function_call_site`; supported branch paths resolving to the same local function emit one `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:640` | `(if flag { local_target } else { other_target })()` | D05 | **green** | Covered by `fixture_call_graph_call_if_ambiguous_function_item_records_ambiguous_dynamic_call_site`; supported branch paths resolving to different local functions fail closed with `Ambiguous` and no edge. |
| `fixture_call_graph` | `src/lib.rs:644` | `(match flag { true => local_target, false => local_target })()` | D06 | **green** | Covered by `fixture_call_graph_call_match_same_function_item_resolves_dynamic_function_call_site`; supported arm paths resolving to the same local function emit one `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:651` | `(match flag { true => local_target, false => other_target })()` | D06 | **green** | Covered by `fixture_call_graph_call_match_ambiguous_function_item_records_ambiguous_dynamic_call_site`; supported arm paths resolving to different local functions fail closed with `Ambiguous` and no edge. |
| `fixture_call_graph` | `src/lib.rs:658` | `(match flag { true if flag => local_target, _ => local_target })()` | D06 | **green** | Covered by `fixture_call_graph_call_match_guarded_function_item_fails_closed_dynamic_call_site`; guarded arms remain `Unsupported` and do not enter the exact arm-path resolver. |
| `fixture_call_graph` | `src/lib.rs:665` | `(if flag { local_target } else { || 8 })()` | D05 | **green** | Covered by `fixture_call_graph_call_if_closure_branch_fails_closed_dynamic_call_site`; non-path branches remain `Unsupported` and do not enter the exact branch-path resolver. |
| `fixture_call_graph` | `src/lib.rs:669` | `(match flag { true => local_target, false => || 13 })()` | D06 | **green** | Covered by `fixture_call_graph_call_match_closure_arm_fails_closed_dynamic_call_site`; non-path arms remain `Unsupported` and do not enter the exact arm-path resolver. |
| `fixture_call_graph` | `src/lib.rs:714` | `(if flag { f } else { f })()` where `f: fn() -> i32` is a parameter | D05/P26 | **green** | Covered by `fixture_call_graph_call_if_function_pointer_param_branch_fails_closed_dynamic_call_site`; opaque parameter branches remain unsupported and do not enter the exact branch-path resolver. |
| `fixture_call_graph` | `src/lib.rs:718` | `(match flag { true => f, false => f })()` where `f: fn() -> i32` is a parameter | D06/P26 | **green** | Covered by `fixture_call_graph_call_match_function_pointer_param_arm_fails_closed_dynamic_call_site`; opaque parameter arms remain unsupported and do not enter the exact arm-path resolver. |
| `fixture_call_graph` | `src/lib.rs:730` | `(if flag { if flag { local_target } else { local_target } } else { local_target })()` | D05 | **green** | Covered by `fixture_call_graph_call_if_nested_branch_expression_resolves_dynamic_function_call_site`; nested branch leaves that are all unshadowed item paths enter the exact branch-path resolver and emit one `CallRelation::DynamicFunction` when they prove the same local function. |
| `fixture_call_graph` | `src/lib.rs:740` | `(match flag { true => match flag { true => local_target, false => local_target }, false => local_target })()` | D06 | **green** | Covered by `fixture_call_graph_call_match_nested_arm_expression_resolves_dynamic_function_call_site`; nested unguarded arm leaves that are all unshadowed item paths enter the exact arm-path resolver and emit one `CallRelation::DynamicFunction` when they prove the same local function. |
| `fixture_call_graph` | `src/lib.rs:152` | `local_target()` inside closure body | owner matrix closure | **green** | Covered by `fixture_call_graph_closure_body_call_is_not_recorded_as_outer_call_site`; parser does not attribute nested closure-body calls to the outer function owner. |
| `fixture_call_graph` | `src/lib.rs:158` | `local_target()` inside async block | owner matrix async | **green** | Covered by `fixture_call_graph_async_block_call_is_not_recorded_as_outer_call_site`; parser does not attribute nested async-block calls to the outer function owner. |
| `fixture_call_graph` | `src/lib.rs:849` | `(async || local_target())()` | D14 / owner matrix closure | **green** | Covered by `fixture_call_graph_call_async_closure_literal_with_body_call_records_outer_dynamic_call_site` and `fixture_call_graph_async_closure_body_call_is_not_recorded_as_outer_call_site`; the outer async closure call is an unsupported `DynamicCall`, and the inner closure-body call is not attributed to the outer function owner. |
| `fixture_call_graph` | `src/lib.rs:173` | `<TraitAssocFunctionTarget as LocalAssocFunctionTrait>::trait_make()` | P16/P17 | **green** | Covered by `fixture_call_graph_call_trait_associated_function_resolves_trait_assoc_function_path_call_site`; fully qualified local trait associated function normalizes to the trait path and resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:390` | `ImportedAssocFunctionTrait::imported_trait_make()` with direct imported trait | P17/P05 | **green** | Covered by `fixture_call_graph_call_direct_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`; shorthand imported local trait associated function resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:398` | `VisibleAssocFunctionTrait::imported_trait_make()` with aliased imported trait | P17/P05 | **green** | Covered by `fixture_call_graph_call_alias_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`; shorthand alias-imported local trait associated function resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:406` | `ImportedAssocFunctionTrait::imported_trait_make()` with glob imported trait | P17/P06 | **green** | Covered by `fixture_call_graph_call_glob_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`; shorthand glob-imported local trait associated function resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:1150` | `ReexportedAssocFunctionTrait::imported_trait_make()` with local re-exported trait import | P17/P05 | **green** | Covered by `fixture_call_graph_call_reexported_trait_associated_function_resolves_trait_assoc_function_path_call_site`; shorthand local re-exported trait associated function resolves to `CallRelation::AssociatedFunction`. |
| `fixture_call_graph` | `src/lib.rs:1196` | `GroupedAssocFunctionTrait::imported_trait_make()` with grouped aliased imported trait | P17/P05 | **green** | Covered by `fixture_call_graph_call_grouped_imported_trait_associated_function_resolves_trait_assoc_function_path_call_site`; shorthand grouped alias-imported local trait associated function resolves to `CallRelation::AssociatedFunction` and is projected through `ploke-db` call context. |
| `fixture_call_graph` | `src/lib.rs:182` | `local_target()` after `let local_target = || 377` | P25 | **green** | Covered by `fixture_call_graph_call_shadowed_local_target_binding_records_value_binding_path_call_site`; records value-binding path callee and fails closed with `Unsupported`, no fake edge to the module function. |
| `fixture_call_graph` | `src/lib.rs:187` | `f()` after `let f = local_target` | P25/P26-ish | **green** | Covered by `fixture_call_graph_call_local_function_item_binding_resolves_initialized_value_binding_path_call_site`; records initialized value-binding path callee and resolves the initializer path to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:1119` | `f()` after `let f = imported_alias` | P25/P26/P05 | **green** | Covered by `fixture_call_graph_call_imported_function_item_binding_resolves_initialized_value_binding_path_call_site`; initialized function-item binding proof preserves the import-alias initializer path and resolves through import backlinks to `import_targets::imported_target`. |
| `fixture_call_graph` | `src/lib.rs:192` | `f()` after `let f: fn() -> i32 = local_target` | P26 | **green** | Covered by `fixture_call_graph_call_typed_function_pointer_binding_resolves_initialized_value_binding_path_call_site`; typed function pointer binding keeps initializer path proof and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:218` | `value.overlap()` where two local trait impls match | M21 | **green** | Covered by `fixture_call_graph_call_ambiguous_trait_method_records_ambiguous_method_call_site`; records the method call and fails closed with `Ambiguous`, no fake edge. |
| `fixture_call_graph` | `src/lib.rs:241` | `value.priority()` where inherent and trait methods share a name | M20 | **green** | Covered by `fixture_call_graph_call_inherent_over_trait_method_resolves_inherent_method_call_site`; exact local receiver type resolves to the inherent method before trait-impl candidates. |
| `fixture_call_graph` | `src/lib.rs:249` | `value.bound_value()` where `T: GenericBoundTrait` is inline | M18 | **green** | Covered by `fixture_call_graph_call_inline_generic_bound_method_resolves_trait_method_call_site`; resolves the generic receiver to the exact local trait method declaration. |
| `fixture_call_graph` | `src/lib.rs:256` | `value.bound_value()` where `T: GenericBoundTrait` is in a `where` clause | M18 | **green** | Covered by `fixture_call_graph_call_where_generic_bound_method_resolves_trait_method_call_site`; resolves the generic receiver through where-predicate type relations. |
| `fixture_call_graph` | `src/lib.rs:260` | `value.bound_value()` where `value: &dyn GenericBoundTrait` | M17 | **green** | Covered by `fixture_call_graph_call_trait_object_method_resolves_trait_method_call_site`; resolves the trait-object bound to the exact local trait method declaration, not a concrete runtime impl. |
| `fixture_call_graph` | `src/lib.rs:1065` | `value.bound_value()` after `let value: &dyn GenericBoundTrait = input` | M17 | **green** | Covered by `fixture_call_graph_call_local_trait_object_binding_method_resolves_trait_method_call_site`; records the one-bound trait-object local annotation as typed local proof and resolves to the exact local trait method declaration, not a concrete runtime impl. |
| `fixture_call_graph` | `src/lib.rs:1086` | `value.trait_value()` after `let value: &dyn LocalDispatchTrait = &TraitDispatchTarget` | M17/M19 | **green** | Covered by `fixture_call_graph_call_concrete_trait_object_binding_method_resolves_impl_method_call_site`; records the direct reference initializer as concrete initialized local proof and resolves to the exact `LocalDispatchTrait for TraitDispatchTarget` impl method. |
| `fixture_call_graph` | `src/lib.rs:1092` | `value.trait_value()` after `let source = TraitDispatchTarget; let value: &dyn LocalDispatchTrait = &source` | M17/M19 | **green** | Covered by `fixture_call_graph_call_aliased_concrete_trait_object_binding_method_resolves_impl_method_call_site`; referenced local bindings that already carry exact concrete initializer proof are admitted as concrete trait-object receiver proof. |
| `fixture_call_graph` | `src/lib.rs:1126` | `value.trait_value()` after `let alias = &source; let value: &dyn LocalDispatchTrait = alias` | M17/M19 | **green** | Covered by `fixture_call_graph_call_reference_alias_trait_object_binding_method_resolves_impl_method_call_site`; trait-object initializer proof now admits a local reference binding that already proves one concrete local receiver type. |
| `fixture_call_graph` | `src/lib.rs:1134` | `value.trait_value()` after `let first = &source; let second = first; let value: &dyn LocalDispatchTrait = second` | M17/M19 | **green** | Covered by `fixture_call_graph_call_reference_chain_trait_object_binding_method_resolves_impl_method_call_site`; untyped aliases of already-proven local reference bindings preserve concrete receiver proof for trait-object resolution. |
| `fixture_call_graph` | `src/lib.rs:265` | `local_target()` in trait default method body | owner matrix trait default method | **green** | Covered by `fixture_call_graph_trait_default_method_body_resolves_local_target_path_call_site`; records method owner `CallBodyOwnerId::Method` and resolves the path call to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:1080` | `self.required_impl_call()` in a trait impl method body | M01 / owner matrix trait impl method | **green** | Covered by `fixture_call_graph_trait_impl_method_body_resolves_same_impl_self_method_call_site`; records `SelfValue` and resolves to the exact same-impl method definition when the target method is present in the trait impl block. |
| `fixture_call_graph` | `src/lib.rs:776` | `self.required()` in trait default method body | M23 | **green** | Covered by `fixture_call_graph_trait_default_method_body_records_required_self_method_call_site`; records `SelfValue` and resolves to the same-trait `required` method declaration as `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:974` | `Self::required_assoc()` in trait default method body | P13/P17 | **green** | Covered by `fixture_call_graph_trait_default_method_body_resolves_same_trait_assoc_function_path_call_site`; resolves to the same-trait associated function declaration when it has no `self` receiver. |
| `fixture_call_graph` | `src/lib.rs:271` | `(f)()` after `let f = local_target` | D01/P25/P26 | **green** | Covered by `fixture_call_graph_call_parenthesized_function_item_binding_resolves_dynamic_function_call_site`; records initialized dynamic local binding and resolves the initializer path to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:277` | `g()` after `let f = local_target; let g = f` | P25/P26 | **green** | Covered by `fixture_call_graph_call_aliased_function_item_binding_resolves_initialized_value_binding_path_call_site`; initialized local binding proof propagates through the alias and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:283` | `(g)()` after `let f = local_target; let g = f` | D01/P25/P26 | **green** | Covered by `fixture_call_graph_call_parenthesized_aliased_function_item_binding_resolves_dynamic_function_call_site`; initialized dynamic local binding proof propagates through the alias and resolves to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:427` | `g()` after `let f: fn() -> i32 = local_target; let g: fn() -> i32 = f` | P26 | **green** | Covered by `fixture_call_graph_call_typed_function_pointer_alias_binding_resolves_initialized_value_binding_path_call_site`; typed initialized binding proof propagates through the alias and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:433` | `(g)()` after `let f: fn() -> i32 = local_target; let g: fn() -> i32 = f` | D01/P26 | **green** | Covered by `fixture_call_graph_call_parenthesized_typed_function_pointer_alias_binding_resolves_dynamic_function_call_site`; typed initialized dynamic binding proof propagates through the alias and resolves to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:676` | `f()` where `f: fn() -> i32` is a parameter | P26 | **green** | Covered by `fixture_call_graph_call_function_pointer_param_records_value_binding_path_call_site`; opaque function-pointer parameters record a value-binding path call and fail closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:680` | `(f)()` where `f: fn() -> i32` is a parameter | D01/P26 | **green** | Covered by `fixture_call_graph_call_parenthesized_function_pointer_param_records_dynamic_local_binding_call_site`; opaque function-pointer parameters record a dynamic local binding and fail closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:684` | `(f as fn() -> i32)()` where `f: fn() -> i32` is a parameter | D11/P26 | **green** | Covered by `fixture_call_graph_call_function_pointer_param_cast_records_cast_local_binding_dynamic_call_site`; opaque parameter casts preserve the local binding path and fail closed instead of using same-name local function lookup. |
| `fixture_call_graph` | `src/lib.rs:689` | `(closure as fn() -> i32)()` where `closure` is a local closure binding | D11/P25 | **green** | Covered by `fixture_call_graph_call_closure_binding_cast_fails_closed_dynamic_call_site`; opaque closure-binding casts fail closed instead of using same-name local function lookup. |
| `fixture_call_graph` | `src/lib.rs:694` | `(*closure)()` where `closure` is a local closure binding | D10/P25 | **green** | Covered by `fixture_call_graph_call_dereferenced_closure_binding_fails_closed_dynamic_call_site`; dereferenced closure bindings remain unsupported until closure/Fn semantic target modeling exists. |
| `fixture_call_graph` | `src/lib.rs:702` | `(holder.callback)()` where `holder: CallbackHolder` is a parameter | D08 | **green** | Covered by `fixture_call_graph_call_field_function_param_records_dynamic_field_call_site`; field callees rooted at opaque parameters record `FieldLocalBinding` and fail closed. |
| `fixture_call_graph` | `src/lib.rs:866` | `(holder.callback)()` where `holder` is a `NamedCallbackHolder { callback: local_target }` local | D08/P26 | **green** | Covered by `fixture_call_graph_call_named_field_function_binding_resolves_initialized_field_dynamic_call_site`; direct named struct-literal field initializer proof records `FieldInitializedLocalBinding` and resolves the field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:874` | `(alias.callback)()` after `let holder = NamedCallbackHolder { callback: local_target }; let alias = holder` | D08/P26 | **green** | Covered by `fixture_call_graph_call_aliased_named_field_function_binding_resolves_dynamic_call_site`; constructed-holder aliases preserve exact named-field initializer proof and resolve the field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:706` | `funcs[0]()` where `funcs: [fn() -> i32; 1]` is a parameter | D09 | **green** | Covered by `fixture_call_graph_call_indexed_function_pointer_fails_closed_dynamic_call_site`; indexed dynamic callees remain unsupported and edge-free. |
| `fixture_call_graph` | `src/lib.rs:878` | `holder.callbacks[0]()` where `holder: CallbackArrayHolder` is a parameter | D09 | **green** | Covered by `fixture_call_graph_call_indexed_field_function_param_fails_closed_dynamic_call_site`; indexed field callees rooted at opaque parameters remain unsupported and edge-free. |
| `fixture_call_graph` | `src/lib.rs:885` | `holder.callbacks[0]()` where `holder` is a `CallbackArrayHolder { callbacks: [local_target] }` local | D09/P26 | **green** | Covered by `fixture_call_graph_call_indexed_named_field_function_binding_resolves_dynamic_call_site`; direct named struct-literal array field proof records `IndexedInitializedLocalBinding` and resolves the indexed field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:891` | `holder.callbacks[0]()` where `callbacks` is initialized from local `funcs = [local_target]` | D09/P26 | **green** | Covered by `fixture_call_graph_call_indexed_named_field_array_alias_binding_resolves_dynamic_call_site`; named struct-literal array field aliases preserve exact element initializer proof and resolve the indexed field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:899` | `alias.callbacks[0]()` after `let holder = CallbackArrayHolder { callbacks: [local_target] }; let alias = holder` | D09/P26 | **green** | Covered by `fixture_call_graph_call_aliased_indexed_named_field_function_binding_resolves_dynamic_call_site`; constructed-holder aliases preserve exact named-field array proof and resolve the indexed field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:903` | `holder.0[0]()` where `holder: TupleCallbackArrayHolder` is a parameter | D09 | **green** | Covered by `fixture_call_graph_call_indexed_tuple_field_function_param_fails_closed_dynamic_call_site`; indexed tuple-field callees rooted at opaque parameters remain unsupported and edge-free. |
| `fixture_call_graph` | `src/lib.rs:908` | `holder.0[0]()` where `holder` is a `TupleCallbackArrayHolder([local_target])` local | D09/P26 | **green** | Covered by `fixture_call_graph_call_indexed_tuple_field_function_binding_resolves_dynamic_call_site`; tuple-constructor array field proof records `IndexedInitializedLocalBinding` and resolves the indexed tuple-field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:914` | `holder.0[0]()` where tuple field is initialized from local `funcs = [local_target]` | D09/P26 | **green** | Covered by `fixture_call_graph_call_indexed_tuple_field_array_alias_binding_resolves_dynamic_call_site`; tuple-constructor array field aliases preserve exact element initializer proof and resolve the indexed tuple-field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:920` | `alias.0[0]()` after `let holder = TupleCallbackArrayHolder([local_target]); let alias = holder` | D09/P26 | **green** | Covered by `fixture_call_graph_call_aliased_indexed_tuple_field_function_binding_resolves_dynamic_call_site`; constructed-holder aliases preserve exact tuple-field array proof and resolve the indexed tuple-field call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:925` | `funcs[0]()` after `let funcs = [local_target]` | D09/P26 | **green** | Covered by `fixture_call_graph_call_indexed_initialized_function_array_resolves_dynamic_call_site`; exact array-literal element proof records `IndexedInitializedLocalBinding` and resolves the indexed call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:930` | `funcs[0]()` after `let funcs: [fn() -> i32; 1] = [local_target]` | D09/P26 | **green** | Covered by `fixture_call_graph_call_typed_indexed_initialized_function_array_resolves_dynamic_call_site`; typed array bindings preserve exact element initializer proof and resolve the indexed call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:936` | `alias[0]()` after `let funcs = [local_target]; let alias = funcs` | D09/P26 | **green** | Covered by `fixture_call_graph_call_aliased_indexed_initialized_function_array_resolves_dynamic_call_site`; one-step array binding aliases preserve exact element initializer proof and resolve the indexed call to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:440` | `generic_f()` where `F: FnOnce() -> i32` | P27/D13 | **green** | Covered by `fixture_call_graph_call_generic_fn_once_value_binding_records_value_binding_path_call_site`; records a value-binding path call and fails closed with `Unsupported`, no fake local edge. |
| `fixture_call_graph` | `src/lib.rs:823` | `(generic_f)()` where `F: FnOnce() -> i32` | D13 | **green** | Covered by `fixture_call_graph_call_parenthesized_generic_fn_once_value_binding_fails_closed_dynamic_call_site`; records a dynamic local binding and fails closed with `Unsupported`, no fake local edge. |
| `fixture_call_graph` | `src/lib.rs:444` | `Box::new(local_target)` in boxed `dyn Fn` setup | P14/D12 | **green** | Covered by `fixture_call_graph_call_boxed_dyn_fn_value_binding_records_box_new_external_path_call_site`; records the prelude-shaped associated-function call and classifies it as `External` after local lookup declines a local target. |
| `fixture_call_graph` | `src/lib.rs:445` | `boxed_fn()` where `boxed_fn: Box<dyn Fn() -> i32>` | P25/D12 | **green** | Covered by `fixture_call_graph_call_boxed_dyn_fn_value_binding_records_value_binding_path_call_site`; records a value-binding path call and fails closed with `Unsupported`, no fake local edge. |
| `fixture_call_graph` | `src/lib.rs:827-828` | `Box::new(local_target); (boxed_fn)()` | D12/P14 | **green** | Covered by `fixture_call_graph_call_parenthesized_boxed_dyn_fn_value_binding_records_box_new_external_path_call_site` and `fixture_call_graph_call_parenthesized_boxed_dyn_fn_value_binding_fails_closed_dynamic_call_site`; setup call is external and parenthesized boxed Fn call remains unsupported. |
| `fixture_call_graph` | `src/lib.rs:837` | `abs(value)` declared in `unsafe extern "C"` block | P09 | **green** | Covered by `fixture_call_graph_call_extern_c_function_records_external_path_call_site`; records the foreign declaration as an `ExternFunction` import binding and classifies the path call as `External` with no local edge. |
| `fixture_call_graph` | `src/lib.rs:453` | `generic_identity::<i32>(123)` | P11 | **green** | Covered by `fixture_call_graph_call_generic_identity_turbofish_resolves_generic_function_path_call_site`; preserves one explicit generic argument and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:467` | `r#match()` | P28 | **green** | Covered by `fixture_call_graph_call_raw_identifier_function_resolves_raw_identifier_path_call_site`; preserves raw identifier spelling and resolves to `CallRelation::Function`. |
| `fixture_call_graph` | `src/lib.rs:480` | `value.r#type()` | M24 | **green** | Covered by `fixture_call_graph_call_raw_identifier_method_resolves_raw_identifier_method_call_site`; preserves raw method spelling and resolves to `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:493` | `value.generic_instance::<i32>(123)` | M08 | **green** | Covered by `fixture_call_graph_call_method_turbofish_resolves_generic_method_call_site`; preserves one explicit method generic argument and resolves to `CallRelation::Method`. |
| `fixture_call_graph` | `src/lib.rs:498` | `drop(value)` | P08 | **green** | Covered by `fixture_call_graph_call_prelude_drop_value_records_external_path_call_site`; records the prelude-shaped path call and classifies it as `External` after local lookup declines a local function target. |
| `fixture_call_graph` | `src/lib.rs:509` | `crate::crate_scoped_macro!()` | X06 | **green** | Covered by `fixture_call_graph_call_crate_scoped_macro_records_crate_path_macro_call_site`; records a path-qualified macro invocation and fails closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:755` | `vec![1, 2, 3]` | X04 | **green** | Covered by `fixture_call_graph_call_vec_macro_records_macro_call_site`; records the macro invocation only and fails closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:844` | `assert_eq!(1 + 1, 2)` in a `#[cfg(test)]` body | X05 | **green** | Covered by `fixture_call_graph_assert_eq_macro_call_records_test_body_macro_call_site`; records the test-body macro invocation with cfg `test` and fails closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:790` | `imported_macro_alias!()` | X07 | **green** | Covered by `fixture_call_graph_call_imported_macro_alias_records_macro_call_site`; records the visible macro alias invocation and fails closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:815` | `call_graph_item_macro!();` inside a function body | X10/X11 | **green** | Covered by `fixture_call_graph_call_item_macro_inside_body_records_macro_call_site`; records the invocation only and fails closed with `Unsupported`. |
| `fixture_call_graph` | `src/lib.rs:518` | `drop(1)` with local `fn drop(...)` in scope | P08/P01 | **green** | Covered by `fixture_call_graph_call_local_drop_shadow_resolves_local_function_path_call_site`; local function resolution wins before prelude `drop` external classification. |
| `fixture_call_graph` | `src/lib.rs:803` | `value.drop()` with inherent `drop(self)` method | M25 | **green** | Covered by `fixture_call_graph_call_explicit_drop_method_resolves_inherent_method_call_site`; resolves as an ordinary inherent method call. |
| `fixture_call_graph` | `src/lib.rs:524` | `(&value).instance_value()` | M11 | **green** | Covered by `fixture_call_graph_call_borrowed_typed_local_instance_method_resolves_borrowed_typed_local_binding_method_call_site`; records an explicitly typed borrowed local receiver and resolves through the exact local receiver type. |
| `fixture_call_graph` | `src/lib.rs:529` | `(*value).instance_value()` | M12 | **green** | Covered by `fixture_call_graph_call_dereferenced_local_instance_method_resolves_dereferenced_initialized_local_binding_method_call_site`; records a dereferenced local receiver whose binding is initialized as a direct reference to `LocalAssoc` and resolves through that exact type. |
| `fixture_call_graph` | `src/lib.rs:1046` | `(*value).instance_value()` where `value: &LocalAssoc` is a named parameter | M12 | **green** | Covered by `fixture_call_graph_call_dereferenced_param_instance_method_resolves_dereferenced_param_method_call_site`; explicit dereference of a borrowed owner parameter resolves through the referenced local type when the method target is proven exactly. |
| `fixture_call_graph` | `src/lib.rs:1050` | `value.instance_value()` where `value: &LocalAssoc` is a named parameter | M11/M12 | **green** | Covered by `fixture_call_graph_call_borrowed_param_instance_method_resolves_borrowed_param_method_call_site`; implicit method-call autoderef on a borrowed owner parameter resolves through one explicit reference layer when the method target is proven exactly. |
| `fixture_call_graph` | `src/lib.rs:1055` | `value.instance_value()` after `let value = &LocalAssoc` | M11/M12 | **green** | Covered by `fixture_call_graph_call_referenced_local_instance_method_resolves_referenced_local_method_call_site`; direct referenced-local binding proof is classified as initialized-local receiver proof and resolves through the referenced local type. |
| `fixture_call_graph` | `src/lib.rs:1060` | `value.instance_value()` after `let value: &LocalAssoc = &LocalAssoc` | M11 | **green** | Covered by `fixture_call_graph_call_typed_reference_local_instance_method_resolves_typed_reference_local_method_call_site`; typed reference local binding proof records the referenced local type path and resolves through one explicit reference layer when the method target is proven exactly. |
| `fixture_call_graph` | `src/lib.rs:1202` | `value.instance_value()` after `let value: &&LocalAssoc = &&LocalAssoc` | M11 | **green** | Covered by `fixture_call_graph_call_typed_double_reference_local_instance_method_resolves_typed_reference_local_method_call_site`; nested typed reference local binding proof records the terminal local type path and resolves through bounded reference unwrapping when the method target is proven exactly. |
| `fixture_call_graph` | `src/lib.rs:533` | `String::new()` | P14/P20 | **green** | Covered by `fixture_call_graph_call_prelude_string_new_records_external_path_call_site`; exact unshadowed prelude constructor shape is classified as `External`. |
| `fixture_call_graph` | `src/lib.rs:537` | `Vec::new()` | P14/P20 | **green** | Covered by `fixture_call_graph_call_prelude_vec_new_records_external_path_call_site`; exact unshadowed prelude constructor shape is classified as `External`. |
| `fixture_call_graph` | `src/lib.rs:551` | `make_local_assoc().instance_value()` | M14 | **green** | Covered by `fixture_call_graph_call_path_result_instance_method_resolves_returned_type_method_call_site`; records an outer method call on a path-call result and resolves through the local function return type. |
| `fixture_call_graph` | `src/lib.rs:556` | `value.clone_assoc().instance_value()` | M13 | **green** | Covered by `fixture_call_graph_call_method_result_instance_method_resolves_returned_type_method_call_site`; records an outer method call on a direct inner method-call result and resolves through the inner method target's exact return type. |
| `fixture_call_graph` | `src/lib.rs:563` | `value.0.instance_value()` | M09 | **green** | Covered by `fixture_call_graph_call_tuple_field_instance_method_resolves_field_type_method_call_site`; records a tuple-field receiver rooted at a constructed local binding and resolves through the exact tuple field type. |
| `fixture_call_graph` | `src/lib.rs:570` | `value.0()` | M10 | **green** | Covered by `fixture_call_graph_call_tuple_field_function_resolves_constructed_field_dynamic_call_site`; records a field-expression dynamic call with constructor-argument initializer proof and resolves the selected argument path to `CallRelation::DynamicFunction`. |
| `fixture_call_graph` | `src/lib.rs:576` | `make_ready_local_assoc().await.instance_value()` | M15 | **green** | Covered by `fixture_call_graph_call_await_path_result_instance_method_resolves_returned_type_method_call_site`; records an awaited path-call result receiver and resolves through the local async function return type. |
| `fixture_call_graph` | `src/lib.rs:584` | `try_local_assoc()?.instance_value()` | M16 | **green** | Covered by `fixture_call_graph_call_try_path_result_instance_method_resolves_result_ok_type_method_call_site`; records a try path-call result receiver and resolves through the local function's syntactic `Result<T, E>` success type. |
| `fixture_call_graph` | `src/lib.rs:590` | `"literal".to_string()` | M06 | **green** | Covered by `fixture_call_graph_call_literal_str_to_string_records_external_method_call_site`; records a literal receiver and classifies exact literal `.to_string()` as `External` with no local edge. |
| `fixture_call_graph` | `src/lib.rs:595` | `value.len()` where `value: Vec<i32>` | M07 | **green** | Covered by `fixture_call_graph_call_typed_vec_len_records_external_method_call_site`; records an explicitly typed local receiver and classifies unshadowed prelude `Vec::len` as `External`. |
| `fixture_call_graph` | `src/lib.rs:609` | `value.len()` where module-local `Vec` shadows prelude | M07/M05 | **green** | Covered by `fixture_call_graph_call_shadowed_typed_vec_len_resolves_local_method_call_site`; local shadowing wins before prelude method classification and resolves to the local inherent method. |
| `fixture_call_graph` | `src/lib.rs:287` | `value.bound_value()` where `value: impl GenericBoundTrait` | M18 | **green** | Covered by `fixture_call_graph_call_impl_trait_method_resolves_trait_method_call_site`; resolves the impl-trait bound to the exact local trait method declaration. |
| `fixture_call_graph` | `src/lib.rs:310` | `value.scoped_value()` with direct imported trait | M19 | **green** | Covered by `fixture_call_graph_call_direct_imported_trait_method_resolves_visible_trait_impl_method_call_site`; concrete local trait impl resolves only after visible trait proof through `ImportedBy`. |
| `fixture_call_graph` | `src/lib.rs:319` | `value.scoped_value()` with aliased imported trait | M19 | **green** | Covered by `fixture_call_graph_call_alias_imported_trait_method_resolves_visible_trait_impl_method_call_site`; alias imports count as visible trait proof. |
| `fixture_call_graph` | `src/lib.rs:328` | `value.scoped_value()` with glob imported trait | M19 | **green** | Covered by `fixture_call_graph_call_glob_imported_trait_method_resolves_visible_trait_impl_method_call_site`; glob imports count as visible trait proof through linked module-scope items. |
| `fixture_call_graph` | `src/lib.rs:336` | `value.scoped_value()` without importing the trait | M19 | **green** | Covered by `fixture_call_graph_call_unimported_trait_method_fails_closed_without_visible_trait_call_site`; fails closed with `Unsupported` and no semantic edge. |
| `fixture_call_graph` | `src/lib.rs:945` | `value.scoped_value()` with local re-exported trait import | M19 | **green** | Covered by `fixture_call_graph_call_reexported_trait_method_resolves_visible_trait_impl_method_call_site`; re-exported trait bindings count as visible trait proof through import backlinks. |
| `fixture_call_graph` | `src/lib.rs:1141` | `value.scoped_value()` with grouped imported trait | M19 | **green** | Covered by `fixture_call_graph_call_grouped_imported_trait_method_resolves_visible_trait_impl_method_call_site`; grouped import syntax expands into the same visible trait proof path as direct imports. |
| `fixture_call_graph` | `src/lib.rs:1173` | `value.constrained_generic_self_value()` through `impl<T: GenericWrapperBound> ConstrainedGenericSelfTrait for GenericWrapper<T>` | M19 | **green** | Covered by `fixture_call_graph_call_constrained_generic_self_trait_method_resolves_exact_generic_arg_call_site`; constrained nominal generic self-type impls resolve only when receiver generic arguments and every bound are proven exactly, avoiding erased `GenericWrapper<_>` matches. |
| `fixture_call_graph` | `src/lib.rs:962` | `value.blanket_value()` through `impl<T> BlanketDispatchTrait for T` | M19 | **green** | Covered by `fixture_call_graph_call_blanket_trait_method_resolves_blanket_impl_method_call_site`; exact unconstrained one-parameter blanket impls resolve to the impl method after local trait visibility proof. |
| `fixture_call_graph` | `src/lib.rs:1025` | `value.inline_bound_value()` through `impl<T: BlanketBound> InlineBoundBlanketTrait for T` | M19 | **green** | Covered by `fixture_call_graph_call_inline_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site`; exact one-parameter inline-bound blanket impls resolve when the concrete receiver has exact local impl evidence for every bound. |
| `fixture_call_graph` | `src/lib.rs:1042` | `value.where_bound_value()` through `impl<T> WhereBoundBlanketTrait for T where T: BlanketBound` | M19 | **green** | Covered by `fixture_call_graph_call_where_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site`; exact one-parameter where-bound blanket impls resolve when the concrete receiver has exact local impl evidence for every where-predicate bound. |
| `fixture_call_graph` | `src/lib.rs:1114` | `value.transitive_bound_value()` through a bound-satisfied blanket impl chain | M19 | **green** | Covered by `fixture_call_graph_call_transitive_bound_blanket_trait_method_resolves_constrained_blanket_impl_call_site`; exact recursive one-parameter blanket-bound proof resolves the final concrete blanket impl method, with unproven/cyclic chains failing closed. |

### Workspace/mock fixtures

| Fixture | Files | Useful for | Status | Notes |
|---|---|---|---|---|
| `tests/fixture_workspace/fixture_mock_serde` | mock serde crates + build.rs | build scripts, proc-macro-ish crate layout, multi-crate external-ish calls | **later** | Useful after per-workspace call reports are stable. |
| `tests/fixture_workspace/ws_fixture_02_assoc_local_enum_ids` | many focused repro members | local enum/assoc ID edge cases | **needs scan** | Good source for constructor and associated item edge cases. |
| `tests/fixture_workspace/ws_fixture_03_cli_collision` | CLI collision repro | duplicate names / path ambiguity | **later** | Useful for ambiguous path-call resolution tests. |

### Real Rust corpus fixtures

| Corpus fixture | File / grep target | Matrix rows | Status | Notes |
|---|---|---|---|---|
| `fixture_github_clones/corpus/axum` | `examples/http-proxy/src/main.rs` | P20/P21, M13-M16, D02/D04, X02 | **later smoke** | Dense async/fluent chains: `Router::new().route(...)`, `get(|| async { ... })`, `service_fn(move |req| ...)`, `tokio::task::spawn(async move { ... })`, `.await`. |
| `fixture_github_clones/corpus/axum` | `examples/testing/src/main.rs` | P20, M13, D02, X03 | **later smoke** | `Router::new().route(...)`, closure handlers, `Request::get(format!(...))`, `.ready().await.unwrap().call(...)`. |
| `fixture_github_clones/corpus/axum` | `examples/chat/src/main.rs` | P20/P21, M13/M17, D02, X03 | **later smoke** | `Arc::new`, `ws.on_upgrade(|socket| ...)`, `tokio::spawn(async move { ... })`, websocket stream `.next().await`. |
| `fixture_github_clones/corpus/serde` | `serde*/build.rs`, derive crates | owner matrix build/proc macro rows, implicit/macro rows | **later smoke** | Good for build scripts/proc macro sequencing once parser call facts are robust. |

### Known fixture gaps

| Missing case | Matrix rows | Suggested fixture action |
|---|---|---|
| Broader explicit local paths beyond current crate/self/super module traversal | P02/P03/P04 | Add focused fixture rows when a new path form is not covered by `crate::local_mod::nested_target()`, `self::local_mod::nested_target()`, or `super::restricted_func()`. |
| Broader associated-function paths beyond current local type/trait proofs | P12/P15/P16/P17 | Keep adding explicit artificial fixture rows only when the existing exact local type/trait/import proof does not cover the syntax. |
| Broader `if`/`match` dynamic callee shapes | D05-D06 | Basic resolved, ambiguous, guarded, non-path, opaque parameter, and nested expression branch/arm cases are green; add future rows only for materially new syntax. |
| Function pointer and `Fn` trait calls beyond exact initializer-path aliases and fail-closed generic/boxed/opaque binding coverage | P26/P27/D10-D13 | Add remaining nested/member dynamic callee fixtures and broader Fn-flow forms; closure-binding cast/deref and field/index guardrails are green. |
| Raw identifier calls/methods | P28/M24 | Add if not found in `fixture_edge_cases`. |
| Ambiguous trait methods | M21 | Covered by `fixture_call_graph::call_ambiguous_trait_method`; fail-closed resolver coverage is green. |
| Closure body ownership | owner matrix closure | Add after closure owner/nesting design. |

## Body-owner coverage matrix

| Owner context | Example | Current support | Expected structural behavior | Future work |
|---|---|---:|---|---|
| Free function body | `fn f() { g(); }` | yes | Calls owned by `CallBodyOwnerId::Function` | Add many more fixture rows. |
| Inherent impl method body | `impl T { fn f(&self) { self.g(); } }` | yes | Calls owned by `CallBodyOwnerId::Method` | Broaden receivers. |
| Trait impl method body | `impl Trait for T { fn f(&self) { ... } }` | partial | Method owner works; same-impl `self.method()` resolution is covered | Broader trait dispatch/resolution later. |
| Trait default method body | `trait T { fn f() { g(); } }` | yes | Calls owned by `CallBodyOwnerId::Method` | Covered by `fixture_call_graph::TraitDefaultCall::default_calls_local`. |
| Const item initializer | `const X: i32 = f();` | yes | Calls owned by `CallBodyOwnerId::Const` | Add more call-shape rows beyond the current path-call fixture. |
| Static initializer | `static X: T = T::new();` | yes | Calls owned by `CallBodyOwnerId::Static` | Covered by `fixture_nodes::STATIC_FN_CALL`. |
| Associated const initializer | `impl T { const X: U = f(); }` | yes | Calls owned by the associated const's `CallBodyOwnerId::Const` | Add more call-shape rows beyond the current path-call fixture. |
| Enum discriminant | `A = f()` if const-call legal | no | Future const-expression owner | Need legality-focused fixture. |
| Closure body | `let c = || f();` | explicit extraction boundary | Inner calls are not attributed to the enclosing owner | Needs closure ID design. |
| Async block/body | `async { f().await }` | explicit extraction boundary | Inner calls are not attributed to the enclosing owner | Also `.await` effect rows. |
| Macro-expanded body | `macro_rules! m { () => { f() } }` | no expansion | Record invocation only | Rustc/RA backend phase. |
| Build script/proc macro generated code | `build.rs` / proc macro output | no | Detached process/backend phase | Formal sequencing docs. |

## Explicit `PathCall` matrix

`PathCall` means `ExprCall` whose callee is syntactically a path. Semantic target may be function, constructor, associated function, closure binding, fn pointer, external item, or unresolved.

| ID | Rust expression | Fixture candidate | Expected structural class | Current resolver expectation | Target family eventually |
|---|---|---|---|---|---|
| P01 | `callee()` | `fixture_call_graph::call_unqualified_local_target` | `PathCall { path: [callee] }` | `Resolved(LocalExact)` if exactly one local function exists in the containing module | `FunctionNodeId` |
| P02 | `crate::m::callee()` | `fixture_call_graph::call_crate_module_nested_target` | `PathCall` | `Resolved(LocalExact)` for explicit crate-root local module paths when the terminal function is proven exactly | `FunctionNodeId` |
| P03 | `self::callee()` in module / `self::m::callee()` | `fixture_call_graph::{local_mod::call_self_nested_target, call_self_module_nested_target}` | `PathCall` | `Resolved(LocalExact)` for explicit self-root local module paths when the terminal function is proven exactly | `FunctionNodeId` |
| P04 | `super::callee()` | `fixture_path_resolution`, `fixture_call_graph::super_path_scope::call_super_local_target` | `PathCall` | `Resolved(LocalExact)` for explicit super paths when the terminal function is proven exactly | `FunctionNodeId` |
| P05 | renamed local import `alias()` | `fixture_call_graph::call_imported_alias_target` / `call_reexported_target` / `grouped_function_import_scope::*` | `PathCall` | `Resolved(LocalExact)` through local import/re-export/grouped-import binding backlinks | `FunctionNodeId` |
| P06 | glob-imported local fn `callee()` | `fixture_call_graph::call_glob_imported_target` | `PathCall` | `Resolved(LocalExact)` through local glob import backlinks | `FunctionNodeId` |
| P07 | external imported fn `read_to_string(...)` | `fs::read_to_string("dummy")` in `fixture_nodes/src/imports.rs` | `PathCall` | `External`, no edge | external summary |
| P08 | std/prelude fn `drop(x)` | `fixture_call_graph::call_prelude_drop_value` plus `prelude_shadow_scope::call_local_drop_shadow` | `PathCall` | `External`, no edge only after local lookup declines a local function target; local `fn drop` still resolves locally | external/builtin summary |
| P09 | extern C function `ffi()` | `fixture_call_graph::call_extern_c_function` | `PathCall` | `External`, no edge through scoped `ExternFunction` binding | external FFI target |
| P10 | unsafe fn `unsafe_fn()` | `fixture_call_graph::call_unsafe_function` | `PathCall` | resolved to exact local function when proven; unsafe marker remains future metadata | `FunctionNodeId` |
| P11 | generic fn `foo::<T>()` | `fixture_call_graph::call_generic_identity_turbofish` | `PathCall`, generic count > 0 | `Resolved(LocalExact)` for exactly proven local functions | `FunctionNodeId` |
| P12 | method-as-associated fn `Type::method(&x)` | `fixture_call_graph::call_method_as_associated_function` | `PathCall` | `Resolved(LocalExact)` for directly visible local type + inherent impl self-type proof | `MethodNodeId` |
| P13 | `Self::new()` | `fixture_call_graph::call_self_make` covers `Self::make()`; `TraitDefaultAssocCall::default_calls_assoc` covers same-trait `Self::required_assoc()` | `PathCall` | `Resolved(LocalExact)` for inherent same-impl associated function, or for a same-trait associated function declaration when the trait-default owner method belongs to that local trait and the target has no `self` receiver | `MethodNodeId` |
| P14 | `SimpleStruct::new(1)` | `fixture_call_graph::{call_local_assoc_make, call_imported_type_assoc_make, call_glob_imported_type_assoc_make, call_reexported_type_assoc_make, call_type_alias_assoc_make, call_type_alias_chain_assoc_make}` covers local, imported, glob-imported, re-exported, and Rust type-alias local type associated functions | `PathCall` | `Resolved(LocalExact)` for visible local type + inherent impl self-type proof, including bounded Rust type-alias chains when type relations prove each alias target exactly | `MethodNodeId` |
| P15 | `<Type>::new()` | `fixture_call_graph::call_qualified_local_assoc_make` | `PathCall` with qself type qualifier normalized to `[Type, new]` | `Resolved(LocalExact)` for directly visible local type + inherent impl self-type proof | `MethodNodeId` |
| P16 | `<Type as Trait>::assoc_fn()` | `fixture_call_graph::call_trait_associated_function` | `PathCall` with qself/trait qualifier normalized to `[Trait, assoc_fn]` | `Resolved(LocalExact)` when `Trait` is a directly visible local trait and the associated method has no `self` receiver; broader trait dispatch remains future | `MethodNodeId` |
| P17 | `Trait::assoc_fn()` | `fixture_call_graph::trait_assoc_function_scope::*`, `fixture_call_graph::grouped_trait_assoc_function_scope::*`, and `fixture_call_graph::trait_assoc_reexport_scope::*` | `PathCall` | `Resolved(LocalExact)` for visible local traits through direct, alias, glob, grouped, and local re-export imports when the associated method has no `self` receiver; broader trait dispatch remains future | `MethodNodeId` |
| P18 | `HashMap::<String, i32>::new()` | `fixture_nodes/src/imports.rs` | `PathCall`, generic count on path | `External`, no edge | external assoc fn |
| P19 | `PathBuf::new()` | current green test | `PathCall` | `External`, no edge | external assoc fn |
| P20 | `Duration::from_secs(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | `External`, no edge | external assoc fn |
| P21 | `Arc::new(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | `External`, no edge | external assoc fn |
| P22 | `TupleStruct(1, 2)` | `fixture_nodes/src/imports.rs` | `PathCall` | `Resolved(LocalExact)` as `CallRelation::TupleStructConstructor` when the local tuple struct and arity are proven | `StructNodeId` |
| P23 | `EnumWithData::Variant1(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | `Resolved(LocalExact)` as `CallRelation::EnumVariantConstructor` when the local tuple variant and arity are proven | `VariantNodeId` |
| P24 | `NewType(value)` | `fixture_call_graph::call_new_type_constructor` | `PathCall` | `Resolved(LocalExact)` as `CallRelation::TupleStructConstructor` when the local tuple struct and arity are proven | `StructNodeId` |
| P25 | `closure_binding()` / same-name shadowing `local_target()` / function-item binding `f()` or alias `g()` | `fixture_call_graph::{call_shadowed_local_target_binding, call_local_function_item_binding, call_imported_function_item_binding, call_parenthesized_function_item_binding, call_aliased_function_item_binding, call_parenthesized_aliased_function_item_binding, call_closure_binding_cast, call_dereferenced_closure_binding}` | syntactically `PathCall` with value-binding or initialized-value-binding callee classifier when local binding is visible; parenthesized form is `DynamicCall` with matching initialized-binding proof; opaque closure-binding cast/deref forms are plain `DynamicCall` | `Unsupported`, no edge for opaque value bindings; `Resolved(LocalExact)` when the binding initializer path, imported initializer path, or exact local alias resolves to one local function | closure target or Fn impl |
| P26 | `fn_ptr()` / typed function pointer binding `f()` or alias `g()` | `fixture_call_graph::{call_typed_function_pointer_binding, call_typed_function_pointer_alias_binding, call_parenthesized_typed_function_pointer_alias_binding, call_function_pointer_param, call_parenthesized_function_pointer_param, call_function_pointer_param_cast}` | syntactically `PathCall` for bare calls and `DynamicCall` for parenthesized/cast calls; local binding keeps initializer path proof when available, and function-pointer casts over opaque parameters preserve the local binding path | `Resolved(LocalExact)` for typed function pointer bindings initialized directly from one local function or exact alias of one; opaque function-pointer parameters fail closed with `Unsupported` and no edge | fn pointer |
| P27 | generic `F: Fn`, `f()` | `fixture_call_graph::call_generic_fn_once_value_binding` | syntactically `PathCall`; semantic Fn impl | `Unsupported`, no edge for generic value-binding calls until Fn/FnOnce semantic resolution exists | Fn trait impl |
| P28 | raw identifier function `r#match()` | `fixture_call_graph::call_raw_identifier_function` | `PathCall` preserving raw spelling | `Resolved(LocalExact)` for exactly proven local functions | `FunctionNodeId` |
| P29 | call in argument `outer(inner())` | many fixtures | two `PathCall` sites | each gets independent status | varies |
| P30 | chained call callee `f()(1)` | `fixture_call_graph::call_chained_returned_function` | inner `PathCall`, outer `DynamicCall` with `arg_count = 1` | inner resolved to exact local function when proven; outer unsupported with no edge | function + dynamic |

## Explicit `MethodCall` matrix

| ID | Rust expression | Fixture candidate | Receiver class needed | Current resolver expectation | Target family eventually |
|---|---|---|---|---|---|
| M01 | `self.private_method()` | current green test | `SelfValue` | `Resolved(LocalExact)` | `MethodNodeId` |
| M02 | `self.secret.len()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self | structural `SelfField`, `External`, no local edge when the field type is a proven external/prelude concrete type | external/std method or unsupported |
| M03 | `self.value.len()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self/generic | currently structural `SelfField`, `Unsupported`, no edge before generic field receiver typing | trait/inherent method |
| M04 | `self.value.into()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self/generic | currently structural `SelfField`, `Unsupported`, no edge before generic field receiver typing | trait method |
| M05 | `local.clone()` / parameter `value.instance_value()` / typed local `value.instance_value()` / path-initialized local `value.instance_value()` / struct-literal initialized local `x.method()` / parenthesized local `(value).method()` | `fixture_call_graph::{call_param_instance_method, call_typed_local_instance_method, call_initialized_local_instance_method, call_parenthesized_typed_local_instance_method, call_parenthesized_initialized_local_trait_method, call_type_alias_chain_instance_method, call_imported_type_alias_instance_method}` plus `fixture_impls::main` | local binding receiver | `Resolved(LocalExact)` for named owner parameters, explicit local type annotations including bounded Rust type-alias chains and imported Rust type aliases, path-initialized locals, struct-literal initialized locals, and parenthesized local receivers with direct local proof; broader untyped locals still future | `MethodNodeId` or external |
| M26 | `self.field.method()` where the field has one exact local concrete type | `fixture_call_graph::SelfFieldAssocOwner::call_self_field_instance_method` | field receiver rooted at self with exact local field type | `SelfField`, `Resolved(LocalExact)`, method edge when the enclosing impl self type and a single named field type are proven exactly; nested or generic self fields still fail closed | `MethodNodeId` |
| M06 | `"x".to_string()` | `fixture_call_graph::call_literal_str_to_string` | literal receiver | structural `Literal`, `External`, no local edge for exact `to_string` | external/prelude method |
| M07 | `vec.len()` / slice len | `fixture_call_graph::call_typed_vec_len_external` plus `local_prelude_shadow::call_shadowed_typed_vec_len` | local binding/field receiver | explicitly typed external/prelude `Vec::len` is `External` only when local shadowing declines; local shadowed `Vec::len` resolves to the local inherent method | builtin/external/local shadowing |
| M08 | `x.foo::<T>(arg)` | `fixture_call_graph::call_method_turbofish` | local binding receiver with method turbofish | `Resolved(LocalExact)` when receiver type and inherent method are proven exactly; preserves method generic arg count | method target |
| M09 | `x.0.call()` | `fixture_call_graph::call_tuple_field_instance_method` | tuple-field local receiver rooted at a constructed local binding | `FieldInitializedLocalBinding`, `Resolved(LocalExact)`, method edge when tuple field type is proven exactly | nested fields/autoderef/trait dispatch |
| M10 | `x.0()` | `fixture_call_graph::call_tuple_field_function` | field-expression dynamic callee | `Resolved(LocalExact)` only when the root binding was constructed by a direct tuple-constructor call and the selected constructor argument path resolves to one local function; otherwise fail closed | fn field/constructor ambiguity |
| M11 | `(&x).method()` / `x.method()` where `x: &LocalType` or `x: &&LocalType` / `let x = &LocalType; x.method()` | `fixture_call_graph::{call_borrowed_typed_local_instance_method, call_borrowed_param_instance_method, call_referenced_local_instance_method, call_typed_reference_local_instance_method, call_typed_double_reference_local_instance_method}` | borrowed typed local receiver, borrowed owner parameter receiver, direct referenced-local binding receiver, or typed reference local receiver | `BorrowedTypedLocalBinding`, `TypedLocalBinding`, `LocalBinding`, and referenced `InitializedLocalBinding`, `Resolved(LocalExact)`, method edge when the underlying local type, an owner parameter/local reference layer, nested typed local reference annotations, or direct referenced-local binding is proven exactly | broader autoderef/autoref cases |
| M12 | `(*x).method()` | `fixture_call_graph::{call_dereferenced_local_instance_method, call_dereferenced_param_instance_method}` | dereferenced initialized local or borrowed owner parameter receiver | `DereferencedInitializedLocalBinding` and `DereferencedLocalBinding`, `Resolved(LocalExact)`, method edge when the binding is initialized as a direct reference to a proven local type path or the named parameter type is an explicit reference to one proven local type | broader autoderef chains |
| M13 | `x.borrow().method()` | `fixture_call_graph::call_method_result_instance_method` | chained method-call receiver | structural `MethodCallResult`, `Resolved(LocalExact)` when the direct inner method call occurrence, inner method target, return type, and outer method target are all proven exactly | method target |
| M14 | `make().method()` | `fixture_call_graph::call_path_result_instance_method` plus external path-result rows in `fixture_path_resolution` | path-call result receiver | exact local function path results resolve through a unique concrete return type; exact external path-call results for known external methods are `External` with no local edge; other path-call results fail closed | method target |
| M15 | `async_fn().await.method()` | `fixture_call_graph::call_await_result_instance_method` | awaited path-call result receiver | `AwaitPathCallResult`, `Resolved(LocalExact)`, method edge when the awaited local function return type is proven exactly | broader future/async return inference |
| M16 | `x?.method()` | `fixture_call_graph::call_try_result_instance_method` | try path-call result receiver | `TryPathCallResult`, `Resolved(LocalExact)`, method edge when the local function return type is syntactic `Result<T, E>` and `T` is proven exactly | broader `Try` trait/output inference |
| M17 | trait object `obj.method()` | `fixture_call_graph::{call_trait_object_method, call_local_trait_object_binding_method, call_concrete_trait_object_binding_method, call_aliased_concrete_trait_object_binding_method, call_reference_alias_trait_object_binding_method, call_reference_chain_trait_object_binding_method}` | dyn trait receiver or local one-bound trait-object annotation, including direct, one-step local-reference, reference-alias, or one-step reference-chain concrete initializer proof | declaration edge resolved for opaque trait-object proof; exact concrete impl edge resolved only when initializer proof proves one local receiver type | trait method/vtable |
| M18 | generic bound `t.method()` | `fixture_call_graph::{call_inline_generic_bound_method, call_where_generic_bound_method, call_impl_trait_method}` | generic or impl-trait receiver | bound-based resolution to exact local trait method declaration | trait method |
| M19 | local trait impl method `x.method()` | `fixture_call_graph::{call_param_trait_method, call_typed_local_trait_method, call_initialized_local_trait_method, call_concrete_trait_object_binding_method, call_aliased_concrete_trait_object_binding_method, call_reference_alias_trait_object_binding_method, call_reference_chain_trait_object_binding_method, trait_scope::*, trait_reexport_scope::*, grouped_trait_import_scope::*, call_blanket_trait_method, call_inline_bound_blanket_trait_method, call_where_bound_blanket_trait_method, call_transitive_bound_blanket_trait_method, call_constrained_generic_self_trait_method}` | typed/path-initialized local, direct/one-step/reference-alias/reference-chain trait-object local, or parameter receiver + local trait impl | `Resolved(LocalExact)` when receiver type, visible trait target, and concrete impl method are all exact; direct, alias, glob, grouped, and local re-export trait imports covered; exact unconstrained and exactly bound-satisfied one-parameter blanket impls covered, including bounded recursive local blanket-bound proof and exact receiver generic-argument proof; missing trait visibility and unproven/cyclic blanket bounds fail closed | trait method |
| M20 | inherent-vs-trait same name | `fixture_call_graph::call_inherent_over_trait_method` | local receiver | inherent wins | method target |
| M21 | ambiguous trait methods | `fixture_call_graph::call_ambiguous_trait_method` | local receiver | `Ambiguous`, no edge | no fabricated method target |
| M22 | private method visibility | current impl private method resolved inside same impl | `SelfValue` | resolved if accessible | local method |
| M23 | trait default method body `self.required()` | `fixture_call_graph::DefaultRequiredCall::default_calls_required` | self receiver in trait context | `Resolved(LocalExact)` to the same local trait method declaration when one self-receiver candidate is proven | trait method |
| M24 | raw identifier method `x.r#type()` | `fixture_call_graph::call_raw_identifier_method` | typed local receiver | `Resolved(LocalExact)` when receiver type and local method are proven exactly | method target |
| M25 | explicit drop `x.drop()` | `fixture_call_graph::call_explicit_drop_method` | method receiver | valid inherent `drop(self)` method resolves like any other inherent method; destructor-call semantics remain out of scope | method target/status |

## Explicit `MacroCall` matrix

| ID | Rust expression | Fixture candidate | Structural expectation | Resolver expectation |
|---|---|---|---|---|
| X01 | `documented_macro!(...)` | current green test | `MacroCall` | `Unsupported`, no edge |
| X02 | `println!(...)` | `fixture_nodes/src/const_static.rs` | `MacroCall` including statement-position macro syntax | `Unsupported` |
| X03 | `format!(...)` | `new_test_module.rs`, currently not module-routed | `MacroCall` after fixture routing | `Unsupported` |
| X04 | `vec![...]` | `fixture_call_graph::call_vec_macro` | `MacroCall` | `Unsupported`, no edge |
| X05 | `assert_eq!(...)` in tests | `fixture_call_graph::call_graph_tests::assert_eq_macro_call` | `MacroCall` with cfg `test` | `Unsupported`, no edge |
| X06 | `crate::documented_macro!(...)` / crate-qualified macro path | `fixture_call_graph::call_crate_scoped_macro` | `MacroCall` with path | `Unsupported` |
| X07 | imported/renamed macro `alias!(...)` | `fixture_call_graph::call_imported_macro_alias` | `MacroCall` preserving visible alias spelling | `Unsupported`, no edge |
| X08 | local `macro_rules!` invocation | `fixture_nodes/src/macros.rs` currently commented usage | `MacroCall` if uncomment/add fixture | `Unsupported` |
| X09 | macro in statement position | e.g. `println!();` | `MacroCall` via `ExprMacro` or stmt macro visitor | `Unsupported` |
| X10 | item macro inside body | `fixture_call_graph::call_item_macro_inside_body` | statement-position `MacroCall` invocation only | `Unsupported`, no expansion-derived item/call edge |
| X11 | macro expands to call | RA call hierarchy has FIXME outgoing macro gaps | invocation only in Ploke-native pass | no expanded edge until backend |
| X12 | proc-macro/derive/attribute generated calls | fixture_macros / future backend | not a body call site | rustc/proc-macro backend only |

## Explicit `DynamicCall` matrix

`DynamicCall` should be reserved for syntactically non-path callees unless we deliberately add a later binding-aware reclassification layer.

| ID | Rust expression | Fixture candidate | Expected structural class | Resolver expectation |
|---|---|---|---|---|
| D01 | `(f)()` | `fixture_call_graph::{call_parenthesized_local_target, call_parenthesized_function_item_binding}` | `DynamicCall` | `Resolved(LocalExact)` when the parenthesized callee path is proven to be a local function or an initialized binding whose initializer path resolves to one local function; opaque visible bindings remain unsupported |
| D02 | `(|| 1)()` | `fixture_call_graph::dynamic_calls` | `DynamicCall` | closure literals are structurally visible and remain `Unsupported`, no edge until closure target modeling exists |
| D03 | `(move || f())()` | `fixture_call_graph::call_move_closure_literal_with_body_call` | `DynamicCall`; nested closure body calls are not attributed to the enclosing owner | outer call is `Unsupported`, no edge; closure-body ownership remains future |
| D04 | `make_fn()()` | `fixture_call_graph::call_returned_function` | inner `PathCall`, outer `DynamicCall` | inner resolved if local function, outer unsupported |
| D05 | `(if cond { f } else { g })()` | `fixture_call_graph::{call_if_same_function_item, call_if_ambiguous_function_item, call_if_closure_branch, call_if_function_pointer_param_branch, call_if_nested_branch_expression}` | `DynamicCall` with `IfBranchPaths` for unshadowed item-path branch leaves, including nested branch expressions; opaque or non-path branches remain plain dynamic calls | `Resolved(LocalExact)` when all branch paths prove the same local function; `Ambiguous` with no edge when branch paths prove different local functions; opaque/non-path branches remain unsupported |
| D06 | `(match x { A => f, B => g })()` | `fixture_call_graph::{call_match_same_function_item, call_match_ambiguous_function_item, call_match_guarded_function_item, call_match_closure_arm, call_match_function_pointer_param_arm, call_match_nested_arm_expression}` | `DynamicCall` with `MatchArmPaths` for unguarded unshadowed item-path arm leaves, including nested arm expressions; guarded, opaque, or non-path arms remain plain dynamic calls | `Resolved(LocalExact)` when all arm paths prove the same local function; `Ambiguous` with no edge when arm paths prove different local functions; guarded/opaque/non-path arms remain unsupported |
| D07 | `{ f }()` | `fixture_call_graph::call_block_function_item` uses `({ local_target })()` | `DynamicCall` with `Path` when the parenthesized block contains exactly one path expression | `Resolved(LocalExact)` when that path resolves to one local function; nontrivial blocks remain unsupported |
| D08 | `(s.callback)()` | `fixture_call_graph::{call_field_function_param, call_named_field_function_binding, call_aliased_named_field_function_binding}` | `DynamicCall` with `FieldLocalBinding` when rooted at an opaque parameter; `FieldInitializedLocalBinding` when rooted at a directly constructed local or one-step alias with a path-valued named field initializer | opaque field callees remain `Unsupported`; exact named-field initializer proofs resolve when the initializer path proves one local function |
| D09 | `funcs[0]()` / `alias[0]()` / `holder.callbacks[0]()` / `holder.0[0]()` | `fixture_call_graph::{call_indexed_function_pointer, call_indexed_field_function_param, call_indexed_tuple_field_function_param, call_indexed_initialized_function_array, call_typed_indexed_initialized_function_array, call_aliased_indexed_initialized_function_array, call_indexed_named_field_function_binding, call_indexed_named_field_array_alias_binding, call_aliased_indexed_named_field_function_binding, call_indexed_tuple_field_function_binding, call_indexed_tuple_field_array_alias_binding, call_aliased_indexed_tuple_field_function_binding}` | `DynamicCall`; opaque indexed parameters and indexed opaque fields remain `Other`; exact local array-literal index proof records `IndexedInitializedLocalBinding` for untyped, typed, one-step aliased, constructed named-field, constructed tuple-field, array-alias field, and constructed-holder alias bindings | opaque indexed callees remain `Unsupported`; exact path-valued array elements resolve when the literal index selects one proven local function |
| D10 | `(*fp)()` | `fixture_call_graph::{call_dereferenced_function_pointer_binding, call_dereferenced_closure_binding}` | `DynamicCall` with `DereferencedInitializedLocalBinding` when the dereferenced operand is an initialized local binding; opaque deref operands remain plain dynamic calls | `Resolved(LocalExact)` when that initializer path resolves to one local function; opaque deref calls remain unsupported |
| D11 | `(foo as fn())()` | `fixture_call_graph::{call_function_pointer_cast_path, call_function_pointer_cast_binding, call_closure_binding_cast}` | `DynamicCall` with `FnPointerCastPath` for unshadowed paths or `FnPointerCastInitializedLocalBinding` for initialized local bindings; opaque closure-binding casts remain plain dynamic calls | `Resolved(LocalExact)` when the cast operand or initializer path resolves to one local function; opaque value casts remain unsupported |
| D12 | `boxed_fn()` / `(boxed_fn)()` where `boxed_fn: Box<dyn Fn()>` | `fixture_call_graph::{call_boxed_dyn_fn_value_binding, call_parenthesized_boxed_dyn_fn_value_binding}` | syntactically `PathCall` unless parenthesized dynamic `LocalBinding`; semantic dynamic | `Unsupported`, no edge |
| D13 | `generic_f()` / `(generic_f)()` where `F: FnOnce()` | `fixture_call_graph::{call_generic_fn_once_value_binding, call_parenthesized_generic_fn_once_value_binding}` | syntactically `PathCall` unless parenthesized dynamic `LocalBinding`; semantic FnImpl | `Unsupported`, no edge |
| D14 | `(async || f())()` | `fixture_call_graph::call_async_closure_literal_with_body_call` | `DynamicCall`; nested async-closure body calls are not attributed to the enclosing owner | outer call is `Unsupported`, no edge; closure/coroutine target modeling remains future |

## Resolution edge target matrix

| Target kind | RA reference kind | Ploke target status |
|---|---|---|
| Local free function | `CallableKind::Function` | current `CallRelation::Function { PathCallSiteId, FunctionNodeId }` for proven local path calls and `CallRelation::DynamicFunction { DynamicCallSiteId, FunctionNodeId }` for proven parenthesized local function path callees |
| Local inherent method | method resolution | current `CallRelation::Method { MethodCallSiteId, MethodNodeId }` for exact `self.method()` |
| Local associated function | function/method def via path | current `CallRelation::AssociatedFunction { PathCallSiteId, MethodNodeId }` for conservative inherent `Self::method()`, `Type::method()`, and `<Type>::method()` cases |
| Trait associated function | method def via path | current `CallRelation::AssociatedFunction { PathCallSiteId, MethodNodeId }` for fully qualified local `<Type as Trait>::method()` calls when the trait method has no `self`; broader trait dispatch future |
| Tuple struct constructor | `CallableKind::TupleStruct` | current `CallRelation::TupleStructConstructor { PathCallSiteId, StructNodeId }` for proven local tuple structs |
| Tuple enum variant constructor | `CallableKind::TupleEnumVariant` | current `CallRelation::EnumVariantConstructor { PathCallSiteId, VariantNodeId }` for proven local tuple variants |
| Closure | `CallableKind::Closure` | future closure ID/owner; currently unsupported |
| Function pointer | `CallableKind::FnPtr` | future fn-ptr/dynamic status; no local edge |
| Fn/FnMut/FnOnce impl | `CallableKind::FnImpl` | future trait-call/dynamic status; no fake edge |
| External/std/prelude function/method | import/dependency lookup | future `External` status or summary target |
| Builtin/primitive operation | RA/builtin semantics | future builtin/external status |
| Macro invocation | not `CallableExpr` in RA | structural invocation only; expansion backend later |

## Implicit/desugared call/effect matrix

These are correct Rust expressions that may imply calls/effects but are not explicit `ExprCall` / `ExprMethodCall` / `ExprMacro` body call sites in our current parser-native pass. They should remain visible in the plan so formal verification does not silently forget them.

| ID | Rust expression | Desugared/future semantic concern | Current Ploke expectation |
|---|---|---|---|
| I01 | `a + b`, `a - b`, etc. | `Add::add`, `Sub::sub`, etc. for overloaded operators | not extracted as call site |
| I02 | `-a`, `!a` | `Neg::neg`, `Not::not` | not extracted |
| I03 | `a == b`, `a < b` | `PartialEq` / `PartialOrd` methods | not extracted |
| I04 | `a += b` | `AddAssign` etc. | not extracted |
| I05 | `a[i]` / `a[i] = x` | `Index` / `IndexMut` | not extracted |
| I06 | `*x`, autoderef method lookup | `Deref` / `DerefMut` | not explicit call site; method resolver later uses typing |
| I07 | `for x in iter` | `IntoIterator::into_iter`, `Iterator::next` | not extracted |
| I08 | `?` | `Try::branch`, `FromResidual` | not extracted |
| I09 | `.await` | `Future::poll` in generated state machine | not extracted |
| I10 | `async fn f()` call | explicit call returns future; body executes on poll | explicit call can be path/method; poll effects future layer |
| I11 | drop at scope end | `Drop::drop` | not extracted |
| I12 | `format!`, `println!`, `vec!` | macro expansion may introduce calls/allocations | invocation only as `MacroCall` |
| I13 | `#[derive(...)]` | generated impl methods | not body call; macro/backend layer |
| I14 | closure creation `|| f()` | closure body contains future nested call sites | not owned separately yet |
| I15 | generator/coroutine resume | poll/resume generated code | future backend/formal layer |
| I16 | indexing/slicing/range syntax | builtin or trait methods depending type | not extracted |
| I17 | pattern matching/destructuring | may drop/move, no direct call except guards | guards contain explicit calls |
| I18 | `let _ = expr;` with temporaries | destructor scheduling | future effect layer |

## Immediate test strategy

1. Keep the matrix rows even when unsupported. Unsupported rows should become tests that assert structural classification plus `Unsupported`/`External`/`Unresolved`, not disappear.
2. Prefer existing fixture rows from `fixture_nodes` and uuid phase-2 fixtures. Add new fixture code only when no correct Rust row exists.
3. For dynamic/value-binding calls, keep the current structural split:
   - `alias_checker(...)` remains `PathCall` because the callee is syntactically a path;
   - visible local value bindings are classified on the path-call node and get
     `Unsupported` with no item edge until target proof exists;
   - bindings initialized directly from a local function path may resolve to
     that function exactly;
   - avoid changing structural ID class after extraction unless we have a strong invariant for doing so.
4. Do not add constructor/associated-function relations by pretending they are ordinary functions. Add typed target families when the tests require them.
5. For formal verification, keep implicit/desugared rows separate from explicit parser call sites so proof coverage can distinguish syntax-level evidence from compiler-lowered behavior.
