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

- `corpus_axum_call_graph_2026-06-28.sqlite`
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
- `fallback-source`: source exists in another registered real GitHub corpus fixture, but that fixture is not currently the axum call-graph DB fixture. Treat it as source evidence until a call-graph fixture/test target is prepared for that crate.
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
| Local generic function turbofish | `axum-fixture` | axum-core | `axum-core/src/body.rs:224` | `try_downcast::<i32, _>(5_u32)` | Path call should preserve generic argument count and resolve to local helper where selected by fixture scope. |
| External turbofish path call | `axum-fixture` | axum-macros | `axum-macros/src/attr_parsing.rs:22` | `std::any::type_name::<K>()` | Status should classify external and preserve generic args. |
| Turbofish method call | `axum-fixture` | axum-macros | `axum-macros/src/attr_parsing.rs:66` | `attr.parse_args::<T>()` | Method call should preserve generic args; target may remain external to `syn`. |
| Tuple struct constructor | `axum-fixture` | axum | `axum/src/boxed.rs:38` | `BoxedIntoRoute(Box::new(Map { ... }))` | Constructor edge should target tuple struct `BoxedIntoRoute` if constructor resolution is enabled for the fixture. |
| Enum variant constructor | `axum-fixture` | axum-macros | `axum-macros/src/with_position.rs:92` | `Some(Position::First(item))` | Enum variant constructor edge should target local `Position::First`; `Some` remains std/prelude external. |
| `Self::associated_function()` | `axum-fixture` | axum | `axum/src/json.rs:112` | `Self::from_bytes(&bytes)` | Associated-function edge should target `Json<T>::from_bytes`. |
| `Type::associated_function()` | `axum-fixture` | axum | `axum/src/error_handling/mod.rs:65` | `HandleError::new(inner, self.f.clone())` | Associated-function edge should target local inherent `HandleError::new`. |
| Method as associated function / UFCS-like call | `axum-fixture` | axum | `axum/src/handler/service.rs:171` | `Handler::call(handler, req, self.state.clone())` | Query should preserve this as associated/path-call syntax and resolve through trait/inherent proof only when exact. |
| Type-alias associated constructor | `fallback-source` | chrono | `src/offset/mod.rs:77`, `src/offset/mod.rs:143` | `type MappedLocalTime<T> = LocalResult<T>;` then `MappedLocalTime::Single(...)` | Good real source case for alias-target constructor proof; needs a chrono call-graph fixture before DB assertion. |
| Raw identifier call | `not-found` | n/a | n/a | Searched selected axum members and fallback real corpus for `r#name(...)`. | Keep synthetic coverage for raw identifier function/method calls. |

## Receiver And Method Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Same-impl `self.method()` | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:268` | `self.extract_with_state(&())` | Method edge should resolve to same impl method when exact. |
| Structural `self.field.method()` | `axum-fixture` | axum | `axum/src/extension.rs:180` | `self.inner.poll_ready(cx)` | Structural method site should be persisted; semantic target may be external/unsupported depending receiver proof. |
| Explicitly typed local receiver | `axum-fixture` | axum | `axum/src/serve/mod.rs:561`, `axum/src/serve/mod.rs:574` | `let router: Router = Router::new();` then `router.clone()` | Receiver type proof should let the query classify/resolve `clone` conservatively. |
| Path-initialized local receiver | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:297`, `axum-core/src/ext_traits/request.rs:302` | `let mut req = Request::new(());` then `req.extensions_mut()` | Local binding initializer proof should attach the method site to `Request`. |
| Parameter receiver | `axum-fixture` | axum | `axum/src/extension.rs:184` | `req.extensions_mut().insert(...)` | Parameter type proof should classify/resolve method calls on `req`. |
| Tuple-field receiver | `axum-fixture` | axum-core | `axum-core/src/body.rs:127` | `self.0.size_hint()` | Structural site should persist; exact semantic resolution depends tuple-field type proof. |
| Path-call result receiver | `axum-fixture` | axum | `axum/src/middleware/from_fn.rs:411` | `Request::builder().uri("/").body(...).unwrap()` | Query can assert chained method sites after a path-call result receiver. |
| Method-call result receiver | `axum-fixture` | axum | `axum/src/routing/route.rs:51` | `self.0.clone().oneshot(req)` | Query should traverse nested receiver expression and classify the outer method site. |
| Await result receiver | `axum-fixture` | axum | `axum/src/test_helpers/test_client.rs:134` | `self.builder.send().await.unwrap()` | Query should preserve method site after `.await` receiver shape. |
| Try result receiver | `fallback-source` | chrono | `src/format/parsed.rs:836` | `DateTime::from_timestamp_secs(ts).ok_or(OUT_OF_RANGE)?.naive_utc()` | Good real source case for `?` success-type receiver proof; needs chrono call-graph fixture before DB assertion. |
| Generic turbofish method call | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request_parts.rs:164` | `.extract_with_state::<State<String>, String>(&state)` | Method site should preserve generic argument count. |
| Local shadowed callable value | `axum-fixture` | axum | `axum/src/routing/tests/mod.rs:423` | local `get` closure is later called as `get("/").await` | This is a real local callable shadowing case; query should not emit a fake edge to `routing::get`. |

## Trait And Body-Owner Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Trait-associated function dispatch | `axum-fixture` | axum-core | `axum-core/src/ext_traits/request.rs:279` | `E::from_request(self, state)` | Should model trait-bound associated call separately from inherent associated calls. |
| Concrete trait impl method body | `axum-fixture` | axum | `axum/src/handler/mod.rs:242`, `axum/src/handler/mod.rs:250` | `$ty::from_request_parts(...)` and `$last::from_request(...)` in generated impl body | Query should surface calls owned by impl method bodies. |
| Trait default method body | `axum-fixture` | axum | `axum/src/service_ext.rs:42`, `axum/src/service_ext.rs:43` | default `handle_error` calls `HandleError::new(self, f)` | Query should surface calls owned by trait default method bodies. |
| Blanket impl | `axum-fixture` | axum-core | `axum-core/src/extract/from_ref.rs:18` | `impl<T> FromRef<T> for T` | Useful source site for future trait-impl completeness tests; no call site on this line by itself. |
| Constrained blanket impl | `axum-fixture` | axum-core | `axum-core/src/extract/mod.rs:91` | `impl<S, T> FromRequest<S, private::ViaParts> for T` | Useful source site for future bounded trait-impl lookup tests. |
| Inherent-over-trait or explicit self dispatch | `axum-fixture` | axum | `axum/src/serve/listener.rs:41` | `match Self::accept(self).await { ... }` | Query should preserve `Self::accept` and avoid confusing the call with an unrelated free function. |
| Trait object future dispatch | `axum-fixture` | axum | `axum/src/error_handling/mod.rs:240`, `axum/src/error_handling/mod.rs:251` | `Pin<Box<dyn Future<...>>>` then `self.project().future.poll(cx)` | This is a real trait-object dispatch site; concrete runtime callee remains intentionally not resolved. |
| Concrete `dyn Any` associated call | `axum-fixture` | axum-core | `axum-core/src/body.rs:29` | `<dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k)` | External trait-object associated call should be represented and classified external. |
| Const initializer body owner | `axum-fixture` | axum | `axum/src/extract/ws.rs:382` | `const UPGRADE: HeaderValue = HeaderValue::from_static("upgrade");` | Calls in const initializers should be owned by `CallBodyOwnerId::Const`. |
| Closure body boundary | `axum-fixture` | axum-macros | `axum-macros/src/from_ref.rs:23` | `.map(|(idx, field)| expand_field(...))` | Current parser boundary should not wrongly attribute closure-body calls to the enclosing function without nested owner support. |
| Async block body boundary | `axum-fixture` | axum | `axum/src/handler/mod.rs:240` | `Box::pin(async move { ... })` | Current parser boundary should not flatten async-block body calls into the enclosing owner unless nested owner modeling is added. |

## Dynamic And Unsupported Callable Cases

| Case | Status | Fixture / crate | File:line | Source shape | Future DB contract |
| --- | --- | --- | --- | --- | --- |
| Opaque callable field value | `axum-unsupported` | axum | `axum/src/boxed.rs:85` | `(self.into_route)(self.handler, state)` | Structural dynamic call should be persisted; semantic resolver should fail closed until field callable proof exists. |
| Another opaque callable field value | `axum-unsupported` | axum | `axum/src/boxed.rs:159` | `(self.layer)(self.inner.into_route(state))` | Good real case for callable struct field dispatch. |
| `FnMut` field call | `axum-unsupported` | axum | `axum/src/serve/listener.rs:236` | `(self.tap_fn)(&mut io);` | Good real case for mutable callable field dispatch. |
| Callable parameter without initializer proof | `axum-unsupported` | axum-macros | `axum-macros/src/lib.rs:718`, `axum-macros/src/lib.rs:724` | `F: FnOnce(I) -> syn::Result<K>` then `and_then(f)` | Callable value is a parameter, not locally initialized; semantic resolution should remain unsupported. |
| Callable parameter direct invocation | `axum-unsupported` | axum-macros | `axum-macros/src/lib.rs:727`, `axum-macros/src/lib.rs:737` | `F: FnOnce(A, I) -> K` then `f(attr, input)` | Structural dynamic call should be persisted; target is intentionally unknown. |
| Function item passed as callable argument | `axum-fixture` | axum-macros | `axum-macros/src/lib.rs:715` | `expand_with(item, from_ref::expand)` | Useful positive source for future function-item argument proof; not enough alone to resolve `f` inside `expand_with`. |
| IIFE closure expression | `axum-unsupported` | axum-macros | `axum-macros/src/lib.rs:734`, `axum-macros/src/lib.rs:738` | `(|| { ... })()` | Dynamic callee is a non-path closure expression; structural call should persist and semantic target should remain unsupported. |
| Dynamic callee from arbitrary expression | `fallback-source` | memchr | `src/arch/x86_64/memchr.rs:153` | `core::mem::transmute::<Fn, RealFn>(fun)(...)` | Strong real fallback for arbitrary expression callee syntax; needs memchr call-graph fixture before DB assertion. |
| Function-pointer field call | `fallback-source` | memchr | `src/memmem/searcher.rs:222` | `unsafe { (self.call)(self, prestate, haystack, needle) }` | Strong real fallback for callable field/function-pointer dispatch; needs memchr call-graph fixture before DB assertion. |
| Callable trait object field | `fallback-source` | memchr | `src/tests/substring/mod.rs:68`, `src/tests/substring/mod.rs:94` | `Box<dyn FnMut...>` field then `fwd(...)` | Strong real fallback for `dyn FnMut` dispatch; likely test-only member coverage must be confirmed before fixture use. |
| Guarded match arm | `fallback-source` | generic-array | `src/lib.rs:1241` | `(n, _) if n > N::USIZE => return Err(LengthError)` | Real guarded arm syntax, but not a dynamic callee. Useful only for receiver/control-flow frontier tests. |
| Guarded match arm with method in guard | `fallback-source` | chrono | `src/format/strftime.rs:635` | `Item::Numeric(...) if self.queue.is_empty() => ...` | Real guard contains method call; needs chrono call-graph fixture before DB assertion. |
| Non-path branch expression used as callee | `not-found` | n/a | n/a | Searched for credible `(if ... { f } else { g })()` or `(match ... { ... })()` shape. | Keep synthetic coverage; no real corpus example found. |
| Returned closure later called | `not-found` | n/a | n/a | Searched selected axum members and fallback real crate sources for `-> impl Fn...` plus call use. | Keep synthetic coverage until a parsed real corpus case is found. |
| Parenthesized function path / `as fn(...)` cast | `not-found` | n/a | n/a | Searched selected axum members and fallback real corpus for direct parenthesized function-path calls and `as fn(...)` cast calls. | Keep synthetic coverage. |
| Indexed function value / array of function pointers | `not-found` | n/a | n/a | No credible parsed real corpus case found. | Keep synthetic coverage. |

## Immediate Test Candidates

These rows are the most useful near-term additions because they are inside the current axum call-graph fixture and should produce stable DB query assertions without creating a new fixture:

| Candidate | Source | Assertion shape |
| --- | --- | --- |
| Grouped import free function | `axum/src/json.rs:237`, `axum/src/json.rs:248` | Find the call site owned by `deserialize_body` and assert it resolves/classifies `post` through `crate::routing::post`. |
| Glob import type call | `axum/src/json.rs:237`, `axum/src/json.rs:250` | Find `TestClient::new(app)` and assert the type path is visible through `test_helpers::*`. |
| Trait-associated dispatch | `axum-core/src/ext_traits/request.rs:279` | Find `E::from_request(self, state)` and assert the call is represented as trait-associated or conservatively unsupported, not lost. |
| Opaque callable field | `axum/src/boxed.rs:85` | Find `(self.into_route)(...)` and assert a dynamic call site exists with unsupported/fail-closed status. |
| IIFE closure expression | `axum-macros/src/lib.rs:734`, `axum-macros/src/lib.rs:738` | Find the dynamic call for `(|| { ... })()` and assert it is persisted without a fake semantic target. |

## Notes

- The fallback rows are useful for planning but should not be turned into DB assertions until the corresponding crate has a call-graph fixture or the axum fixture is expanded with an equivalent source case.
- The `not-found` rows are deliberate. They prevent future work from inventing weak real-corpus examples for shapes that are currently better covered by synthetic fixtures.
- Several axum rows are source cases where current semantic resolution is intentionally incomplete. Those tests should assert structural presence plus `Unsupported` or fail-closed status, not a guessed target.
