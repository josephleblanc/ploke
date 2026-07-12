# Real Corpus Call-Site Case Matrix

Date: 2026-06-28
Status: active call-graph planning note
Short description: Source-grounded candidate locations for real-target call-graph query coverage, prioritizing the registered axum call-graph fixture and falling back to other registered real GitHub corpus fixtures when axum does not contain a credible case.

Related files:

- [README.md](README.md)
- [2026-06-25_call-graph-coverage-inventory.md](2026-06-25_call-graph-coverage-inventory.md)
- [2026-06-22_call-site-coverage-matrix.md](2026-06-22_call-site-coverage-matrix.md)
- [../../../testing/BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md)

## Scope

This inventory is source evidence for future real-target DB query tests. It was built from six independent survey passes, then spot-verified locally with `rg` against the pinned fixture source checkouts.

Primary target:

- `corpus_axum_call_graph_2026-07-05.sqlite`
- `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- Selected workspace members for the current call-graph fixture: `axum`, `axum-core`, `axum-macros`

Fallback targets from [../../../testing/BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md):

- `github:dtolnay/semver@8591f2344b52b31d85b538de58b76a676fe9ff90`
- `github:BurntSushi/memchr@24f5daa5257e00e87007c936761600e034827905`
- `github:fizyk20/generic-array@80bab87431c2e29823dc551a3311324812838a23`
- `github:chronotope/chrono@120686c82c5da90377e815edb82c9a80b6b4f2be`

Status values:

- `axum-fixture`: source is inside a selected member of the current axum call-graph fixture and can be converted directly into a DB query assertion.
- `axum-unsupported`: source is inside a selected axum member, but the current semantic model should fail closed or mark it unsupported.
- `fallback-source`: source exists in another registered real GitHub corpus fixture. The current DB assertions live in the matching fallback call-graph fixture rather than the primary axum fixture.
- `not-found`: no credible case was found in axum or the registered real GitHub fallback corpus during this survey.

## Path, Import, And Constructor Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Explicit `crate::module::function()` path call | `axum-fixture` | axum-macros | `axum-macros/src/typed_path.rs:23` | `crate::attr_parsing::parse_attrs("typed_path", attrs)?` | Owner `typed_path::expand` should have a path-call edge to `attr_parsing::parse_attrs`. |
| Explicit `crate::function()` path call | `axum-fixture` | axum-macros | `axum-macros/src/typed_path.rs:443` | `crate::run_ui_tests("typed_path");` | Test-module owner should resolve to local helper `run_ui_tests`. |
| Explicit `super::function()` path call | `axum-fixture` | axum | `axum/src/routing/tests/mod.rs:56` | `super::take_route_or_internal_error(&mut service)` | Path-call edge should resolve to the parent-module helper. |
| `super::module::Type::new()` associated call | `axum-fixture` | axum | `axum/src/handler/service.rs:174` | `super::future::IntoServiceFuture::new(future)` | Associated-function edge should target the local inherent `new` item when type path proof is exact. |
| `self::function()` path call | `not-found` | n/a | n/a | Searched selected axum members and fallback real corpus. | Keep covered by synthetic fixtures until a real corpus case appears. |
| Grouped import free function | `axum-fixture` | axum | `axum/src/json.rs:237`, `axum/src/json.rs:248` | `use crate::{routing::post, test_helpers::*, Router};` then `post(...)` | Import-aware path lookup should bind `post` through the grouped import. |
| Glob import type call | `axum-fixture` | axum | `axum/src/json.rs:237`, `axum/src/json.rs:250` | `test_helpers::*` then `TestClient::new(app)` | Glob-import lookup should make `TestClient` visible before resolving `new`. |
| Re-exported local type associated call | `axum-fixture` | axum | `axum/src/extract/raw_form.rs:65` | `Body::empty()` | `Body` is reachable through axum's local body re-export; query should assert the call is represented, then tighten target proof if re-export type proof is exact. |
| External dependency path call | `axum-fixture` | axum | `axum/src/json.rs:184` | `serde_json::Deserializer::from_slice(bytes)` | Status should classify as external/dependency-root, not unresolved local. |
| External std path call | `axum-fixture` | axum | `axum/src/error_handling/mod.rs:138` | `std::mem::replace(&mut self.inner, clone)` | Status should classify as external/std-root. |
| Local generic function turbofish | `axum-fixture` | axum-core | `axum-core/src/body.rs:251` | `try_downcast::<i32, _>(5_u32)` | Path call should preserve generic argument count and resolve to local helper where selected by fixture scope. |
| External turbofish path call | `axum-fixture` | axum-macros | `axum-macros/src/attr_parsing.rs:22` | `std::any::type_name::<K>()` | Status should classify external and preserve generic args. |
| Turbofish method call | `axum-fixture` | axum-macros | `axum-macros/src/attr_parsing.rs:66` | `attr.parse_args::<T>()` | Covered in `paths.rs`: closure-owned method row preserves one generic arg and remains unsupported/targetless for the external `syn::Attribute` receiver. |
| Tuple struct constructor | `axum-fixture` | axum | `axum/src/boxed.rs:38` | `BoxedIntoRoute(Box::new(Map { ... }))` | Constructor edge should target tuple struct `BoxedIntoRoute` if constructor resolution is enabled for the fixture. |
| Enum variant constructor | `axum-fixture` | axum-macros | `axum-macros/src/with_position.rs:92` | `Some(Position::First(item))` | Enum variant constructor edge should target local `Position::First`; `Some` remains std/prelude external. |
| `Self::associated_function()` | `axum-fixture` | axum | `axum/src/json.rs:112` | `Self::from_bytes(&bytes)` | Associated-function edge should target `Json<T>::from_bytes`. |
| `Type::associated_function()` | `axum-fixture` | axum | `axum/src/error_handling/mod.rs:65` | `HandleError::new(inner, self.f.clone())` | Associated-function edge should target local inherent `HandleError::new`. |
| Method as associated function / UFCS-like call | `axum-fixture` | axum | `axum/src/handler/service.rs:171` | `Handler::call(handler, req, self.state.clone())` | Query should preserve this as associated/path-call syntax and resolve through trait/inherent proof only when exact. |
| Type-alias associated constructor | `fallback-source` | chrono | `src/offset/mod.rs:77`, `src/offset/mod.rs:143` | `type MappedLocalTime<T> = LocalResult<T>;` then `MappedLocalTime::Single(...)` | Covered in `fallback.rs`: chrono rows now resolve to `LocalResult::Single` while preserving the literal `MappedLocalTime::Single` callsite path. |
| Raw identifier call | `not-found` | n/a | n/a | Searched selected axum members and fallback real corpus for `r#name(...)`. | Keep synthetic coverage for raw identifier function/method calls. |

## Receiver And Method Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Same-impl `self.method()` | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:268` | `self.extract_with_state(&())` | Method edge should resolve to same impl method when exact. |
| Structural `self.field.method()` | `axum-fixture` | axum | `axum/src/extension.rs:180` | `self.inner.poll_ready(cx)` | Structural method site should be persisted; semantic target may be external/unsupported depending receiver proof. |
| Explicitly typed local receiver | `axum-fixture` | axum | `axum/src/serve/mod.rs:756`, `axum/src/serve/mod.rs:769` | `let router: Router = Router::new();` then `router.clone()` | Covered in `receivers.rs`: exact local external-trait impl proof resolves typed-local `Router::clone` rows to `impl<S> Clone for Router<S>::clone`. |
| Path-initialized local receiver | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:297`, `axum-core/src/ext_traits/request.rs:302` | `let mut req = Request::new(());` then `req.extensions_mut()` | Local binding initializer proof should attach the method site to `Request`. |
| Associated-constructor initialized local receiver | `axum-fixture` | axum | `axum/src/routing/tests/mod.rs:1165,1170,1174`; `:1187,1190,1194`; `axum/src/routing/tests/fallback.rs:396,400,405` | `let state = CountingCloneableState::new();` then `state.clone()` and `state.setup_done()` | Covered in `receivers.rs`: the constructor path resolves to `CountingCloneableState::new`, its `Self` return type proves the local receiver type, and the six later receiver calls traverse to local impl methods. |
| Parameter receiver | `axum-fixture` | axum | `axum-core/src/extract/default_body_limit.rs:183,225`, `axum/src/extension.rs:184`, `axum/src/extract/nested_path.rs:95,103`, `axum/src/routing/path_router.rs:336` | `req.extensions_mut().insert(...)` and `req.extensions_mut().get_mut(...)` on `req: &mut Request<_>`, `mut req: Request<_>`, and request-routing local bindings | Covered in `receivers.rs`: the six inspected `req.extensions_mut()` local-binding rows classify as external targetless frontiers. |
| impl Trait parameter receiver | `axum-fixture` | axum-core | `axum-core/src/error.rs:12,14` | `error: impl Into<BoxError>` then `error.into()` | Covered in `receivers.rs`: the parameter-bound `Into` call classifies as an external targetless frontier with no local traversal target. |
| Tuple-field receiver | `axum-fixture` | axum-core | `axum-core/src/body.rs:127` | `self.0.size_hint()` | Structural site should persist; exact semantic resolution depends tuple-field type proof. |
| Path-call result receiver | `axum-fixture` | axum | `axum/src/middleware/from_fn.rs:411` | `Request::builder().uri("/").body(...).unwrap()` | Query can assert chained method sites after a path-call result receiver. |
| Method-call result receiver | `axum-fixture` | axum | `axum/src/routing/route.rs:51` | `self.0.clone().oneshot(req)` | Query should traverse nested receiver expression and classify the outer method site. |
| Await result receiver | `axum-fixture` | axum | `axum/src/test_helpers/test_client.rs:134` | `self.builder.send().await.unwrap()` | Query should preserve method site after `.await` receiver shape. |
| Try method-call result receiver | `fallback-source` | chrono | `src/format/parsed.rs:836,953` | `DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()` and `DateTime::from_timestamp(...).ok_or(OUT_OF_RANGE)?.naive_utc()` | Covered in `fallback.rs`: chrono `TryMethodCallResult(ok_or)` receiver rows are visible, unsupported, targetless, and non-traversable. |
| Generic turbofish method call | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request_parts.rs:164` | `.extract_with_state::<State<String>, String>(&state)` | Covered in `receivers.rs`: method site preserves two generic args plus `TupleMethodReturn(parts, into_parts, index 0)` receiver proof, and resolves to `RequestPartsExt for Parts::extract_with_state` through the exact external tuple-return summary for `http::Request::into_parts`. |
| Local shadowed callable value | `axum-fixture` | axum | `axum/src/routing/tests/mod.rs:423` | local `get` closure is later called as `get("/").await` | Covered by DB/RAG/TUI targetless tests: the current fixture exposes only the two setup `get(...)` rows and does not emit a fake edge from shadowed closure calls to `routing::get`. |

## Trait And Body-Owner Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Trait-associated function dispatch | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:279` | `E::from_request(self, state)` | Covered in `trait_body.rs`: bounded type-parameter associated paths traverse to trait method bindings without guessing concrete impl dispatch. |
| Concrete trait impl method body | `axum-fixture` | axum | `axum/src/handler/mod.rs:242`, `axum/src/handler/mod.rs:250` | `$ty::from_request_parts(...)` and `$last::from_request(...)` in generated impl body | Query should surface calls owned by impl method bodies. |
| Trait default method body | `axum-fixture` | axum | `axum/src/service_ext.rs:42`, `axum/src/service_ext.rs:43` | default `handle_error` calls `HandleError::new(self, f)` | Query should surface calls owned by trait default method bodies. |
| Blanket impl | `axum-fixture` | axum-core | `axum-core/src/extract/from_ref.rs:18` | `impl<T> FromRef<T> for T` | Useful source site for future trait-impl completeness tests; no call site on this line by itself. |
| Constrained blanket impl | `axum-fixture` | axum-core | `axum-core/src/extract/mod.rs:91` | `impl<S, T> FromRequest<S, private::ViaParts> for T` | Covered in `trait_body.rs`: the async-block-owned `Self::from_request_parts` call resolves through the parent blanket impl bound `T: FromRequestParts<S>` to the trait method binding. |
| Inherent-over-trait or explicit self dispatch | `axum-fixture` | axum | `axum/src/serve/listener.rs:41,61` | `match Self::accept(self).await { ... }` | Query preserves both cfg-unix visible `Self::accept` rows as external targetless calls and avoids confusing them with recursive trait dispatch or unrelated free functions. |
| Trait object future dispatch | `axum-fixture` | axum | `axum/src/error_handling/mod.rs:240`, `axum/src/error_handling/mod.rs:251` | `Pin<Box<dyn Future<...>>>` then `self.project().future.poll(cx)` | This is a real trait-object dispatch site; concrete runtime callee remains intentionally not resolved. |
| Concrete `dyn Any` associated call | `axum-fixture` | axum-core | `axum-core/src/body.rs:32` | `<dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k)` | Covered in `trait_body.rs`: external trait-object associated calls are represented as targetless std-root path rows, not local traversal edges. |
| Const initializer body owner | `axum-fixture` | axum | `axum/src/routing/route.rs:202`; `axum/src/extract/ws.rs:382,384` | `const ZERO: HeaderValue = HeaderValue::from_static("0");`; `const UPGRADE: HeaderValue = HeaderValue::from_static("upgrade");`; `const WEBSOCKET: HeaderValue = HeaderValue::from_static("websocket");` | Route local const initializer calls are owned by executable `LocalItem` owners, not item-level `Const` nodes or enclosing function owners; websocket local const rows remain absent in this fixture. |
| Local function body owner | `axum-fixture` | axum | `axum/src/json.rs:203`; `axum/src/json.rs:208,217` | nested `fn make_response(...)` calls `HeaderValue::from_static(...)` from match arms | Covered in `trait_body.rs`: nested local `fn` body calls are owned by executable `LocalItem` owners and are not flattened into the outer `Json::into_response` method owner. |
| Closure body boundary | `axum-fixture` | axum-macros | `axum-macros/src/from_ref.rs:23` | `.map(|(idx, field)| expand_field(...))` | Covered by `axum_closure_body_call_is_documented_unsupported_gap`: regenerated fixture owns the row on the nested closure executable and traverses to `expand_field`, without flattening it into the enclosing function. |
| Async block body boundary | `axum-fixture` | axum | `axum/src/handler/mod.rs:217` | `Box::pin(async move { self().await.into_response() })` | Covered by `axum_real_target_handler_async_block_body_calls_are_async_block_owned`: concrete async-block body calls are owned by a nested `AsyncBlock` owner and remain targetless; they are not flattened into the enclosing `Handler::call` owner. |

## Dynamic And Unsupported Callable Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Opaque callable field value | `axum-unsupported` | axum | `axum/src/boxed.rs:85` | `(self.into_route)(self.handler, state)` | Structural dynamic call should be persisted; semantic resolver should fail closed until field callable proof exists. |
| Another opaque callable field value | `axum-unsupported` | axum | `axum/src/boxed.rs:159` | `(self.layer)(self.inner.into_route(state))` | Good real case for callable struct field dispatch. |
| `FnMut` field call | `axum-unsupported` | axum | `axum/src/serve/listener.rs:236` | `(self.tap_fn)(&mut io);` | Good real case for mutable callable field dispatch. |
| Callable parameter without initializer proof | `axum-unsupported` | axum-macros | `axum-macros/src/lib.rs:718`, `axum-macros/src/lib.rs:724` | `F: FnOnce(I) -> syn::Result<K>` then `and_then(f)` | Callable value is a parameter, not locally initialized; semantic resolution should remain unsupported. |
| Callable parameter direct invocation | `axum-unsupported` | axum-macros | `axum-macros/src/lib.rs:727`, `axum-macros/src/lib.rs:737` | `F: FnOnce(A, I) -> K` then `f(attr, input)` | Covered by `axum_macro_callback_rows_are_visible_or_explicitly_absent`: the closure-owned path call to captured parameter `f` is persisted as unsupported, targetless, and non-traversable. |
| Proc-macro body helper calls | `axum-fixture` | axum-macros | `axum-macros/src/lib.rs:377,426,665,715` | proc-macro entrypoints call `expand_with(...)` | Covered in `proc_macros.rs`: macro owners traverse to the local `expand_with` helper in one resolved path edge each. |
| Proc-macro attribute helper calls | `axum-fixture` | axum-macros | `axum-macros/src/lib.rs:581,637` | proc-macro attribute entrypoints call `expand_attr_with(...)`; callback closures call `debug_handler::expand(...)` | Covered in `proc_macros.rs`: active macro owners traverse to `expand_attr_with`; closure executable owners traverse to `debug_handler::expand`; inactive `__private_axum_test` remains absent under the fixture cfg. |
| Function item passed as callable argument | `axum-fixture` | axum-macros | `axum-macros/src/lib.rs:715` | `expand_with(item, from_ref::expand)` | Useful positive source for future function-item argument proof; not enough alone to resolve `f` inside `expand_with`. |
| IIFE closure expression | `axum-fixture` | axum-macros | `axum-macros/src/lib.rs:734`, `axum-macros/src/lib.rs:738` | `(|| { ... })()` | Covered by `axum_macro_callback_rows_are_visible_or_explicitly_absent`: regenerated fixture resolves the outer dynamic call to its closure owner while preserving callable-parameter body calls as opaque targetless rows. |
| Dynamic callee from arbitrary expression | `fallback-source` | memchr | `src/arch/x86_64/memchr.rs:153` | `core::mem::transmute::<Fn, RealFn>(fun)(...)` | Covered in `fallback.rs`: current fixture absence is asserted for the transmute path and outer dynamic call. |
| Function-pointer field call | `fallback-source` | memchr | `src/memmem/searcher.rs:222` | `unsafe { (self.call)(self, prestate, haystack, needle) }` | Covered in `fallback.rs`: memchr projects two targetless dynamic rows owned by `find` methods. |
| Callable trait object field | `fallback-source` | memchr | `src/tests/substring/mod.rs:68`, `src/tests/substring/mod.rs:94` | `Box<dyn FnMut...>` field then `fwd(...)` | Covered in `fallback.rs` and downstream RAG/TUI targetless tests: `Runner::run` preserves the boxed callable calls as targetless unsupported path rows without guessing boxed closure targets. |
| Guarded match arm | `fallback-source` | generic-array | `src/lib.rs:1239,1276` | `let iter = iter.into_iter(); match iter.size_hint()` with guarded tuple arms | Covered in `fallback.rs`: the fixture now asserts two targetless external `MethodResultLocalBinding(method_name = "into_iter")` `size_hint` rows. |
| Guarded match arm with method in guard | `fallback-source` | chrono | `src/format/strftime.rs:635` | `Item::Numeric(...) if self.queue.is_empty() => ...` | Covered in `fallback.rs`: chrono guard receiver row is visible, unsupported, targetless, and non-traversable. |
| Non-path branch expression used as callee | `not-found` | n/a | n/a | Searched for credible `(if ... { f } else { g })()` or `(match ... { ... })()` shape. | Keep synthetic coverage; no real corpus example found. |
| Returned closure later called | `not-found` | n/a | n/a | Searched selected axum members and fallback real crate sources for `-> impl Fn...` plus call use. | Keep synthetic coverage until a parsed real corpus case is found. |
| Parenthesized function path / `as fn(...)` cast | `not-found` | n/a | n/a | Searched selected axum members and fallback real corpus for direct parenthesized function-path calls and `as fn(...)` cast calls. | Keep synthetic coverage. |
| Indexed function value / array of function pointers | `not-found` | n/a | n/a | No credible parsed real corpus case found. | Keep synthetic coverage. |

## Test Candidate Status

These rows are inside the current axum call-graph fixture and produce stable DB query assertions without creating a new fixture:

| Candidate | Source | Assertion shape |
| --- | --- | --- |
| Grouped import free function | `axum/src/json.rs:237`, `axum/src/json.rs:248` | Covered by `axum_real_target_generated_post_function_resolves`: the `deserialize_body` owner has a resolved `post` edge to the generated `routing::method_routing::post` function. |
| Glob import type call | `axum/src/json.rs:237`, `axum/src/json.rs:250` | Covered by `axum_real_target_test_client_new_high_fanout_is_documented_gap`: the `deserialize_body` owner traverses to `TestClient::new` through `test_helpers::* -> pub use test_client::*`; the same test now asserts 168 projected `TestClient::new` rows resolve, includes a routing child-module inherited-glob traversal from `fallback.rs:10`, and includes the axum-core `request_parts.rs:193` workspace dependency glob import. |
| Trait-associated dispatch | `axum-core/src/ext_traits/request.rs:279` | Covered by `axum_real_target_trait_associated_paths_reach_trait_methods`: `E::from_request` traverses to the `FromRequest::from_request` trait method binding without guessing concrete impl dispatch. |
| Opaque callable field | `axum/src/boxed.rs:85` | Covered by `axum_dynamic_callable_fields_are_visible_unsupported_blockers`: the dynamic row is visible, unsupported, targetless, and non-traversable. |
| IIFE closure expression | `axum-macros/src/lib.rs:734`, `axum-macros/src/lib.rs:738` | Covered by `axum_macro_callback_rows_are_visible_or_explicitly_absent`: the IIFE dynamic row resolves to its closure owner without a fabricated named function target. |

## Notes

- The fallback rows are useful for planning and DB regression coverage; their assertions live in `real_target_matrix/fallback.rs` against the registered fallback call-graph fixtures.
- The `not-found` rows are deliberate. They prevent future work from inventing weak real-corpus examples for shapes that are currently better covered by synthetic fixtures.
- Several axum rows are source cases where current semantic resolution is intentionally incomplete. Those tests should assert structural presence plus `Unsupported` or fail-closed status, not a guessed target.
