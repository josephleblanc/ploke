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
| `ExprCall` with non-path callee | `CallNode::DynamicCall` | Future implementation. Examples: `(f)()`, `(|| 1)()`, `make_fn()()`, `funcs[0]()`. |
| `ExprMethodCall` | `CallNode::MethodCall` | Current implementation only records literal `self` receiver; receiver classification needs to broaden. |
| `ExprMacro` | `CallNode::MacroCall` | Current implementation. Invocation site only, no expansion. |
| Calls inside const/static/associated const initializers | Future owner expansion | Requires extending `CallBodyOwnerId`; do not force these into function/method owners. |
| Calls inside closure/async/block bodies | Future owner/nesting model | Need closure/body-owner IDs or explicit containment under nearest item plus nested-body metadata. |
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

- `self.private_method()` -> `MethodCall`, `Resolved(LocalExact)`, `CallRelation::Method`.
- `PathBuf::new()` -> `PathCall`, `Unsupported`, no edge.
- `documented_macro!(...)` -> `MacroCall`, `Unsupported`, no edge.

## Body-owner coverage matrix

| Owner context | Example | Current support | Expected structural behavior | Future work |
|---|---|---:|---|---|
| Free function body | `fn f() { g(); }` | yes | Calls owned by `CallBodyOwnerId::Function` | Add many more fixture rows. |
| Inherent impl method body | `impl T { fn f(&self) { self.g(); } }` | yes | Calls owned by `CallBodyOwnerId::Method` | Broaden receivers. |
| Trait impl method body | `impl Trait for T { fn f(&self) { ... } }` | partial | Method owner works; resolution mostly unsupported | Trait dispatch/resolution later. |
| Trait default method body | `trait T { fn f() { g(); } }` | partial | Method owner works structurally | Add tests. |
| Const item initializer | `const X: i32 = f();` | no | Future owner `ConstNodeId` | Extend owner family deliberately. |
| Static initializer | `static X: T = T::new();` | no | Future owner `StaticNodeId` | Extend owner family deliberately. |
| Associated const initializer | `impl T { const X: U = f(); }` | no | Future owner `ConstNodeId` or assoc const owner | Add tests after owner design. |
| Enum discriminant | `A = f()` if const-call legal | no | Future const-expression owner | Need legality-focused fixture. |
| Closure body | `let c = || f();` | no nested owner | Future closure owner or nested call metadata | Needs closure ID design. |
| Async block/body | `async { f().await }` | no nested owner | Future async/closure-like owner | Also `.await` effect rows. |
| Macro-expanded body | `macro_rules! m { () => { f() } }` | no expansion | Record invocation only | Rustc/RA backend phase. |
| Build script/proc macro generated code | `build.rs` / proc macro output | no | Detached process/backend phase | Formal sequencing docs. |

## Explicit `PathCall` matrix

`PathCall` means `ExprCall` whose callee is syntactically a path. Semantic target may be function, constructor, associated function, closure binding, fn pointer, external item, or unresolved.

| ID | Rust expression | Fixture candidate | Expected structural class | Current resolver expectation | Target family eventually |
|---|---|---|---|---|---|
| P01 | `callee()` | Needs local fixture or existing simple crate scan | `PathCall { path: [callee] }` | future `Resolved(LocalExact)` if local | `FunctionNodeId` |
| P02 | `crate::m::callee()` | fixture_path_resolution / add row | `PathCall` | future resolved if local | `FunctionNodeId` |
| P03 | `self::callee()` in module | fixture_path_resolution | `PathCall` | future resolved if local | `FunctionNodeId` |
| P04 | `super::callee()` | fixture_path_resolution | `PathCall` | future resolved if local | `FunctionNodeId` |
| P05 | renamed local import `alias()` | fixture imports with local fn alias if present/add | `PathCall` | future resolved through import | `FunctionNodeId` |
| P06 | glob-imported local fn `callee()` | fixture imports/add | `PathCall` | future resolved through glob | `FunctionNodeId` |
| P07 | external imported fn `read_to_string(...)` | `fs::read_to_string("dummy")` in `fixture_nodes/src/imports.rs` | `PathCall` | `External` eventually, currently `Unsupported` | external summary |
| P08 | std/prelude fn `drop(x)` | add fixture | `PathCall` | `External` or builtin | external/builtin summary |
| P09 | extern C function `ffi()` | add FFI fixture | `PathCall` | `External`/FFI | external FFI target |
| P10 | unsafe fn `unsafe_fn()` | add fixture | `PathCall` | resolved plus unsafe marker eventually | `FunctionNodeId` |
| P11 | generic fn `foo::<T>()` | add fixture | `PathCall`, generic count > 0 | future resolved | `FunctionNodeId` |
| P12 | method-as-associated fn `Type::method(&x)` | fixture_impls or add | `PathCall` | future associated/inherent method resolution | `MethodNodeId` |
| P13 | `Self::new()` | `fixture_nodes/src/impls.rs` has `Self { data }`, not call; add row | `PathCall` | future same impl assoc fn | `MethodNodeId` |
| P14 | `SimpleStruct::new(1)` | add/use fixture | `PathCall` | future local associated fn | `MethodNodeId` |
| P15 | `<Type>::new()` | RA grammar reference; add fixture | `PathCall` with qself path handling | future local assoc fn | `MethodNodeId` |
| P16 | `<Type as Trait>::assoc_fn()` | RA grammar reference; add fixture | `PathCall` with qself/trait qualifier | future trait assoc fn | `MethodNodeId` |
| P17 | `Trait::assoc_fn(&x)` | RA call hierarchy trait test uses `S1::callee()` for trait method | `PathCall` | future trait dispatch | `MethodNodeId` |
| P18 | `HashMap::<String, i32>::new()` | `fixture_nodes/src/imports.rs` | `PathCall`, generic count on path | `External` eventually, currently `Unsupported` | external assoc fn |
| P19 | `PathBuf::new()` | current green test | `PathCall` | currently `Unsupported` | external assoc fn |
| P20 | `Duration::from_secs(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | `External` eventually | external assoc fn |
| P21 | `Arc::new(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | `External` eventually | external assoc fn |
| P22 | `TupleStruct(1, 2)` | `fixture_nodes/src/imports.rs` | `PathCall` | future constructor relation | tuple struct constructor target |
| P23 | `EnumWithData::Variant1(1)` | `fixture_nodes/src/imports.rs` | `PathCall` | future tuple variant constructor relation | enum variant constructor target |
| P24 | `NewType(value)` | add fixture if needed | `PathCall` | constructor relation | tuple struct constructor target |
| P25 | `closure_binding()` | `alias_checker(&_trait_user)` in `fixture_nodes/src/imports.rs` | syntactically `PathCall` today; policy TBD | future dynamic/binding-aware status | closure target or Fn impl |
| P26 | `fn_ptr()` | add fixture | syntactically `PathCall`; semantic fn ptr | future `Unsupported`/dynamic or fn-ptr target | fn pointer |
| P27 | generic `F: Fn`, `f()` | add fixture | syntactically `PathCall`; semantic Fn impl | future dynamic/FnImpl | Fn trait impl |
| P28 | raw identifier function `r#match()` | add fixture | `PathCall` | future resolved | `FunctionNodeId` |
| P29 | call in argument `outer(inner())` | many fixtures | two `PathCall` sites | each gets independent status | varies |
| P30 | chained call callee `f()(1)` | RA grammar reference; add fixture | inner `PathCall`, outer `DynamicCall` | inner maybe resolved, outer unsupported | function + dynamic |

## Explicit `MethodCall` matrix

| ID | Rust expression | Fixture candidate | Receiver class needed | Current resolver expectation | Target family eventually |
|---|---|---|---|---|---|
| M01 | `self.private_method()` | current green test | `SelfValue` | `Resolved(LocalExact)` | `MethodNodeId` |
| M02 | `self.secret.len()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self | should be structural, no fake local edge | external/std method or unsupported |
| M03 | `self.value.len()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self/generic | unsupported/external until typing | trait/inherent method |
| M04 | `self.value.into()` | `fixture_nodes/src/impls.rs` | field receiver rooted at self/generic | unsupported/external until typing | trait method |
| M05 | `local.clone()` | add/use fixture | local binding receiver | future trait/inherent resolution | `MethodNodeId` or external |
| M06 | `"x".to_string()` | many fixtures (`to_string`) | literal receiver | external/std trait method | external/prelude method |
| M07 | `vec.len()` / slice len | existing `len()` candidates | local binding/field receiver | external/builtin method | builtin/external |
| M08 | `x.foo::<T>(arg)` | RA grammar reference; add fixture | turbofish method | future generic arg count | method target |
| M09 | `x.0.call()` | RA grammar reference; add fixture | tuple-field receiver | unsupported until receiver typing | method target |
| M10 | `x.0()` | RA grammar reference | field expression then call, not method call | future dynamic/path? | fn field/constructor ambiguity |
| M11 | `(&x).method()` | add fixture | parenthesized ref receiver | future autoderef/autoref | method target |
| M12 | `(*x).method()` | add fixture | deref receiver | future autoderef | method target |
| M13 | `x.borrow().method()` | add fixture | chained receiver | nested call sites | method target |
| M14 | `make().method()` | add fixture | call receiver | nested path/dynamic + method | method target |
| M15 | `async_fn().await.method()` | add fixture | await receiver | future await modeling | method target |
| M16 | `x?.method()` | add fixture | try receiver | future try modeling | method target |
| M17 | trait object `obj.method()` | add fixture | dyn trait receiver | dynamic dispatch | trait method/vtable |
| M18 | generic bound `t.method()` | fixture_type_resolution_v2 maybe | generic receiver | bound-based resolution | trait method |
| M19 | imported trait method `x.method()` | add fixture | local receiver + trait import | future trait lookup | trait method |
| M20 | inherent-vs-trait same name | add fixture | local receiver | inherent should win | method target |
| M21 | ambiguous trait methods | add fixture | local receiver | `Ambiguous`, no edge | candidates later |
| M22 | private method visibility | current impl private method resolved inside same impl | `SelfValue` | resolved if accessible | local method |
| M23 | trait default method body `self.required()` | add/fixture traits | self receiver in trait context | unsupported until trait rules | trait method |
| M24 | raw identifier method `x.r#type()` | add fixture | receiver | future resolved | method target |
| M25 | explicit drop `x.drop()` | add fixture | method receiver | should not special-case yet | method target/status |

## Explicit `MacroCall` matrix

| ID | Rust expression | Fixture candidate | Structural expectation | Resolver expectation |
|---|---|---|---|---|
| X01 | `documented_macro!(...)` | current green test | `MacroCall` | `Unsupported`, no edge |
| X02 | `println!(...)` | `fixture_nodes/src/const_static.rs` | `MacroCall` once owner supports const/static? actually in function body `use_all_const_static` | `Unsupported` |
| X03 | `format!(...)` | `new_test_module.rs`, currently not module-routed | `MacroCall` after fixture routing | `Unsupported` |
| X04 | `vec![...]` | add fixture | `MacroCall` | `Unsupported` |
| X05 | `assert_eq!(...)` in tests | fixture test modules may be cfg/test | `MacroCall` if test bodies parsed | `Unsupported` |
| X06 | `crate::documented_macro!(...)` | add fixture | `MacroCall` with path | `Unsupported` |
| X07 | imported/renamed macro `alias!(...)` | imports fixture candidate | `MacroCall` | `Unsupported` |
| X08 | local `macro_rules!` invocation | `fixture_nodes/src/macros.rs` currently commented usage | `MacroCall` if uncomment/add fixture | `Unsupported` |
| X09 | macro in statement position | e.g. `println!();` | `MacroCall` via `ExprMacro` or stmt macro visitor | `Unsupported` |
| X10 | item macro inside body? | add fixture if legal | likely stmt/item macro, not `ExprMacro` | `Unsupported` |
| X11 | macro expands to call | RA call hierarchy has FIXME outgoing macro gaps | invocation only in Ploke-native pass | no expanded edge until backend |
| X12 | proc-macro/derive/attribute generated calls | fixture_macros / future backend | not a body call site | rustc/proc-macro backend only |

## Explicit `DynamicCall` matrix

`DynamicCall` should be reserved for syntactically non-path callees unless we deliberately add a later binding-aware reclassification layer.

| ID | Rust expression | Fixture candidate | Expected structural class | Resolver expectation |
|---|---|---|---|---|
| D01 | `(f)()` | add fixture | `DynamicCall` | `Unsupported` |
| D02 | `(|| 1)()` | add fixture | `DynamicCall` | future closure-local target maybe |
| D03 | `(move || f())()` | add fixture | `DynamicCall`, nested closure body call future | unsupported now |
| D04 | `make_fn()()` | RA grammar `f()(1)`; add fixture | inner `PathCall`, outer `DynamicCall` | outer unsupported |
| D05 | `(if cond { f } else { g })()` | add fixture | `DynamicCall` | ambiguous/dynamic |
| D06 | `(match x { A => f, B => g })()` | add fixture | `DynamicCall` | ambiguous/dynamic |
| D07 | `{ f }()` | add fixture | `DynamicCall` | maybe local fn if proven later |
| D08 | `(s.callback)()` | add fixture | `DynamicCall` | fn field/closure field target later |
| D09 | `funcs[0]()` | add fixture | `DynamicCall` | dynamic/unsupported |
| D10 | `(*fp)()` | add fixture | `DynamicCall` | fn pointer target maybe |
| D11 | `(foo as fn())()` | add fixture | `DynamicCall` | fn pointer cast target maybe |
| D12 | `boxed_fn()` where `boxed_fn: Box<dyn Fn()>` | add fixture | syntactically `PathCall` unless parenthesized; semantic dynamic | policy TBD |
| D13 | `generic_f()` where `F: FnOnce()` | add fixture | syntactically `PathCall`; semantic FnImpl | policy TBD |
| D14 | coroutine/async closure call | RA has `CoroutineClosure` callable kind | future dynamic/closure | unsupported |

## Resolution edge target matrix

| Target kind | RA reference kind | Ploke target status |
|---|---|---|
| Local free function | `CallableKind::Function` | future `CallRelation::Function { PathCallSiteId, FunctionNodeId }` |
| Local inherent method | method resolution | current `CallRelation::Method { MethodCallSiteId, MethodNodeId }` for exact `self.method()` |
| Local associated function | function/method def via path | future relation variant or reuse `Method` with `PathCallSiteId` source requires design |
| Trait associated function | method def via path | future trait-aware relation/status |
| Tuple struct constructor | `CallableKind::TupleStruct` | future constructor target family; do not fake `FunctionNodeId` |
| Tuple enum variant constructor | `CallableKind::TupleEnumVariant` | future constructor target family; do not fake `FunctionNodeId` |
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
3. For dynamic calls, decide the path-to-binding policy first:
   - syntactic policy: `alias_checker(...)` remains `PathCall` with dynamic/unsupported semantic status;
   - binding-aware policy: a later resolver reclassifies or statuses it as closure/FnPtr/FnImpl without changing structural class;
   - avoid changing structural ID class after extraction unless we have a strong invariant for doing so.
4. Do not add constructor/associated-function relations by pretending they are ordinary functions. Add typed target families when the tests require them.
5. For formal verification, keep implicit/desugared rows separate from explicit parser call sites so proof coverage can distinguish syntax-level evidence from compiler-lowered behavior.
