# Real Corpus Call-Site Oracle Matrices

Date: 2026-06-28
Status: active call-graph planning note
Short description: Definition/binding sites, discovered callsites, and source evidence chains for the real-corpus call-site matrix.

Related files:

- [2026-06-28_real-corpus-call-site-case-matrix.md](2026-06-28_real-corpus-call-site-case-matrix.md)
- [2026-06-25_call-graph-coverage-inventory.md](2026-06-25_call-graph-coverage-inventory.md)
- [../../../testing/BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md)

## Scope

This companion note turns the source-case inventory into query-test oracle material. It records the target definition or binding site, the discovered callsites, and the evidence chain from each callsite back to the target or unsupported boundary.

The primary source is the registered axum call-graph fixture:

- `github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1`
- selected members: `axum`, `axum-core`, `axum-macros`

For this note, "call stack" means source evidence chain, not runtime stack:

`callsite -> import/type/receiver/callable binding proof -> definition or unsupported status`

High-fanout targets are grouped by identical evidence chain. Before turning high-fanout rows such as `TestClient::new` into strict tests, generate owner rows mechanically from parser facts instead of maintaining a hand-written list.

## Definition And Binding Summary

| Target | Definition or binding site | Status | Notes |
| --- | --- | --- | --- |
| `parse_attrs` | `axum-macros/src/attr_parsing.rs:59` | local | Imported by `from_ref` and `from_request`; called explicitly through `crate::` in `typed_path`. |
| `run_ui_tests` | `axum-macros/src/lib.rs:797` | local | Test helper called through `crate::run_ui_tests`. |
| `take_route_or_internal_error` | `axum/src/routing/mod.rs:63` | local | Called same-module and through `super::` from tests. |
| `routing::post` | template `axum/src/routing/method_routing.rs:165`; macro invocation `:445`; re-export `axum/src/routing/mod.rs:45-48` | local generated | The call graph query should bind to the generated top-level handler function, not `TestClient::post`. |
| `TestClient::new` | struct `axum/src/test_helpers/test_client.rs:30`; fn `:36`; re-export `axum/src/test_helpers/mod.rs:5-6` | local test helper | 172 text callsites in selected members; the matrix below records the exact fanout by file/line and common evidence chain. |
| `Body::empty` | body type `axum-core/src/body.rs:39`; fn `:52`; re-export `axum/src/body/mod.rs:10` | local/re-exported | Direct `axum_core::body::Body` imports and axum re-export paths both reach this function. |
| `IntoServiceFuture::new` | macro invocation `axum/src/handler/future.rs:11-18`; generated `new` template `axum/src/macros.rs:19-20` | local generated | Macro-generated inherent constructor. |
| `try_downcast` | `axum-core/src/body.rs:26`; separate axum helper `axum/src/util.rs:99` | local | Two same-named helpers exist in different crates. |
| `BoxedIntoRoute` constructor | tuple struct `axum/src/boxed.rs:12` | local constructor | Includes explicit `BoxedIntoRoute(...)` and `Self(...)` constructor syntax. |
| `Position::First` | enum `axum-macros/src/with_position.rs:65`; variant `:66` | local variant constructor | Pattern matches are not constructor callsites. |
| `Json::from_bytes` | inherent impl `axum/src/json.rs:157`; fn `:164` | local | `Self::from_bytes` inside `Json<T>` impls. |
| `HandleError::new` | inherent impl `axum/src/error_handling/mod.rs:78`; fn `:80` | local | Called by layer impl and trait default method. |
| `Handler::call` | trait method `axum/src/handler/mod.rs:153` | local trait | `Handler::call(...)` syntax now resolves to the trait method binding; concrete runtime impl dispatch remains type-parameter dependent. |
| `FromRequest::from_request` | trait method `axum-core/src/extract/mod.rs:85` | local trait | `E::from_request(...)` and blanket-impl `T::from_request(...)` now resolve to the trait method binding through owner generic bounds; concrete runtime impl dispatch remains type-parameter dependent. |
| `FromRequestParts::from_request_parts` | trait method `axum-core/src/extract/mod.rs:59` | local trait | `E::from_request_parts(...)`, blanket-impl `T::from_request_parts(...)`, and async-block-owned `Self::from_request_parts(...)` now resolve to the trait method binding through owner generic bounds; concrete runtime impl dispatch remains type-parameter dependent. |

## Path, Import, And External Call Oracles

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `parse_attrs` | `axum-macros/src/typed_path.rs:23`; `from_ref.rs:30`; `from_request/mod.rs:112,196,471,598,727,892,908,1029,1039` | `expand`; `expand_field`; `extract_fields`; `impl_struct_by_extracting_all_at_once`; `impl_enum_by_extracting_all_at_once`; `infer_state_type_from_field_attributes` | explicit `crate::attr_parsing::parse_attrs` or local import at `from_ref.rs:9` / `from_request/mod.rs:3` -> `attr_parsing.rs:59`. |
| `run_ui_tests` | `axum-macros/src/debug_handler.rs:885,890`; `typed_path.rs:443`; `from_ref.rs:104`; `from_request/mod.rs:1050` | UI test helper functions | `crate::run_ui_tests(...)` -> crate-root helper `lib.rs:797`. |
| `take_route_or_internal_error` | `axum/src/routing/mod.rs:410,430`; `routing/tests/mod.rs:56,59` | `Router<S>::fallback_endpoint`; `take_route_or_internal_error_panics_on_second_call` | same-module call or `super::take_route_or_internal_error` -> parent module definition `routing/mod.rs:63`. |
| `serde_json::Deserializer::from_slice` | `axum/src/json.rs:184` | `Json<T>::from_bytes` | `serde_json::...` path -> external dependency root in `axum/Cargo.toml:135` and dev dependency `:189`. |
| `std::mem::replace` | `axum/src/error_handling/mod.rs:138,181`; `middleware/map_request.rs:281`; `middleware/from_fn.rs:285`; `middleware/map_response.rs:260`; `response/sse.rs:449` | service call bodies and `EventDataWriter::write_buf` | `std::mem::replace` path -> std-root external classification. |

## Free-Function Multi-Hop Oracle

| Chain | Definition/binding sites | Callsites | Evidence chain |
| --- | --- | --- | --- |
| `from_request::expand -> impl_struct_by_extracting_each_field -> extract_fields` | `axum-macros/src/from_request/mod.rs:93`; `:330`; `:412` | `axum-macros/src/from_request/mod.rs:145`; `:342` | Both calls are unqualified local path calls between normal functions in the `crate::from_request` module. This is the current regular free-function two-hop proof case for DB/RAG/TUI reachability. |

Current executable coverage: `ploke-db` real-target matrix tests assert eleven
one-hop `parse_attrs` edges, including the explicit
`crate::attr_parsing::parse_attrs` row through the file-module declaration at
`axum-macros/src/lib.rs:9` and three nested closure-body rows from
`from_request/mod.rs:471,1029,1039`. The external path test asserts
`serde_json::Deserializer::from_slice`, the two currently projected
`std::mem::replace` external rows at
`axum/src/error_handling/mod.rs:138` and
`axum/src/response/sse.rs:449`, and the current absent wrapper-body owner rows
for `middleware/{map_request,from_fn,map_response}.rs`.

## Generated Handler Function Fanout

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `routing::post` | `axum/src/json.rs:248,264,279,299,318,353` | JSON extractor tests | grouped import at `json.rs:237` -> `routing::post` re-export `routing/mod.rs:45-48` -> generated top-level handler fn from `method_routing.rs:165` / `:445`. |
| `routing::post` | `axum/src/extract/multipart.rs:381,404,420,448` | multipart tests | grouped import at `multipart.rs:362` -> same generated function binding. |
| `routing::post` | `axum/src/routing/method_routing.rs:1448,1660` | `merge`; `merge_accessing_state` | same-module test visibility through `use super::*` at `method_routing.rs:1391` -> generated function binding. |
| `routing::post` | `axum/src/routing/tests/mod.rs:88,624,666,744,745,746,772,792,812,838,842,844,899,1071,1162` | routing tests | grouped `crate::routing::{..., post, ...}` import at `routing/tests/mod.rs:8-11` -> generated function binding. |

Current executable coverage: `ploke-db` real-target matrix tests assert that no
generated `post` function node exists yet, then pin the 23 currently projected
`post(...)` rows as unsupported, targetless, and non-traversable. Exact
source-line fanout covers the 22 module-anchored rows: six JSON rows, two
`method_routing.rs` rows, and fourteen `routing/tests/mod.rs` rows. The
remaining raw row is owned by a nested async-block executable owner. The
multipart rows remain absent in the current fixture; the
`routing/tests/mod.rs:1215` row is no longer flattened into the parent function
owner.

## Re-Exported Body Constructor Fanout

| Target | Callsites | Evidence chain |
| --- | --- | --- |
| `Body::empty` | `axum-core/src/response/into_response.rs:128,163`; `axum-core/src/ext_traits/request.rs:346,364,377,390` | direct `axum_core::body::Body` path -> `axum-core/src/body.rs:52`. |
| `Body::empty` | `axum/src/form.rs:158`; `extract/query.rs:106`; `extract/raw_form.rs:65`; `extract/ws.rs:394,400,1129,1191`; `serve/mod.rs:799`; `middleware/from_fn.rs:411`; `routing/route.rs:161,174`; `routing/method_routing.rs:1700`; `routing/tests/get_to_head.rs:25,59`; `routing/tests/merge.rs:198,204`; `routing/tests/mod.rs:228,1133,1151` | local/imported `Body` -> axum re-export `axum/src/body/mod.rs:10` or direct `axum_core::body::Body` import -> `axum-core/src/body.rs:52`. |

Current executable coverage: DB target traversal, proof projection, RAG exact
call context, `code_item_lookup`, and `code_item_edges` assert twenty-three
resolved `Body::empty` caller edges: six literal `Body::empty()` rows from
axum-core `response/into_response.rs` and `ext_traits/request.rs`, the two
`axum-core/src/body.rs:{83,89}` `Self::empty()` rows, four direct axum
parsed-workspace import rows in `extract/query.rs`, `extract/raw_form.rs`,
`form.rs`, and `serve/mod.rs`, plus eleven local re-exported, inherited,
closure-owned, and local-item workspace rows in `middleware/from_fn.rs`,
`routing/route.rs:{161,174}`, `routing/method_routing.rs`,
`routing/tests/get_to_head.rs`, `routing/tests/merge.rs`, and
`routing/tests/mod.rs:{228,1133,1151}`. The source-oracle rows in
`extract/ws.rs:{394,400,1129,1191}` remain listed above for future tightening
but are not part of the current resolved target-centered DB contract. The
strict DB test now pins all twenty-three current resolved rows by source file
and line, including executable-owner rows that are reached through
`call_body_owner` parent metadata before file attribution.

The DB matrix also pins eight currently projected external
`HeaderValue::from_static` rows by exact owner and source line: four `axum-core`
response conversion rows, the HTML response owner, two JSON response rows owned
by the nested local `make_response` `LocalItem` owner, and one function-local
const initializer row owned by a `LocalItem` executable owner in
`set_content_length`. The websocket upgrade local const initializer rows remain
absent in the current axum fixture.

## High-Fanout Test Helper Matrix

`TestClient::new` has 172 selected-member text callsites. The full fanout is useful for fixture coverage; strict DB assertions should be generated from parser facts because owner recovery by hand is noisy.

| Target | Exact callsite fanout | Evidence chain |
| --- | --- | --- |
| `TestClient::new` | `axum/src/form.rs:262`; `json.rs:250,266,281,301,320,355`; `extension.rs:228`; `response/sse.rs:714,756,793`; `response/mod.rs:529`; `middleware/from_extractor.rs:351`; `middleware/map_request.rs:412,432`; `middleware/map_response.rs:357`; `extract/query.rs:158`; `extract/connect_info.rs:386`; `extract/multipart.rs:383,423,449`; `extract/mod.rs:103`; `extract/matched_path.rs:162,178,197,217,237,254,271,291,312,326,346,361,374,394`; `extract/nested_path.rs:136,154,172,190,205,224`; `extract/path/mod.rs:619,632,645,664,687,700,716,732,751,784,798,822,854,912,946,974,989,1010,1034`; `handler/mod.rs:418,443`; `routing/tests/nest.rs:41,65,135,159,182,193,210,229,280,298,309,328,371,408,431,489`; `routing/tests/merge.rs:14,63,81,85,96,116,136,150,162,179,208,234,267,301,345,379`; `routing/tests/handle_error.rs:25,42,60,76,90`; `routing/tests/fallback.rs:10,25,40,53,69,89,101,118,134,150,171,190,207,221,241,261,280,299,314,325,338,359,377,389,402`; `routing/tests/mod.rs:90,118,150,188,217,233,242,282,307,323,339,352,365,377,396,416,454,472,489,506,527,540,572,589,599,626,643,668,685,700,717,738,748,775,798,815,846,905,952,967,984,1027,1047,1073,1164,1201`; `axum-core/src/extract/request_parts.rs:193` | common chain: callsite -> visible `TestClient` import, direct module scope, or `test_helpers::*` -> re-export `axum/src/test_helpers/mod.rs:5-6` -> struct `test_client.rs:30` -> `new` function `test_client.rs:36`. |

Current registered-backup coverage: DB target traversal and proof projection
assert the regenerated high-fanout contract: 168 projected structural rows
resolve to `TestClient::new`. The resolved set includes nested/direct
re-export import rows, routing-test child modules where `use super::*`
inherits the parent `test_helpers::*` glob import, and axum-core
`request_parts.rs:193` through `axum::test_helpers::*` workspace dependency
glob proof. RAG exact call context, `code_item_lookup`, and `code_item_edges`
preserve the 168 resolved target-centered caller rows and their
`TestClient::new` associated-function callee shape. Known source-oracle
frontiers remain around multipart, closure-body, and macro-template
projection gaps.

The strict DB source-line fanout is now table-driven in
`ploke-db`'s real-target matrix. It pins the 168 resolved projected rows by
file/line and keeps the current exclusions explicit: the three multipart
callsites, the three `routing/route.rs` route-construction callsites,
`json.rs:281`, `routing/tests/merge.rs:63`, and the remaining
assertion/macro-template or nested-body text callsites are not counted as
resolved `TestClient::new` traversal rows in the current backup fixture.

## Constructors And Associated Calls

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `IntoServiceFuture::new` | `axum/src/handler/service.rs:174` | `impl Service for HandlerService::call` | `type Future = super::future::IntoServiceFuture<H::Future>` at `service.rs:155` -> call at `:174` -> macro-generated inherent `new` from `handler/future.rs:11-18` and `macros.rs:19-20`. |
| `try_downcast` in `axum-core` | `axum-core/src/body.rs:23,51,251,252` | `boxed`; `Body::new`; `test_try_downcast` | same-module unqualified call -> `axum-core/src/body.rs:26`. |
| `try_downcast` in `axum` | `axum/src/routing/mod.rs:205`; `axum/src/util.rs:114,115` | `Router<S>::route_service`; `test_try_downcast` | import `crate::util::try_downcast` at `routing/mod.rs:8-13` or same-module test -> `axum/src/util.rs:99`. |
| `BoxedIntoRoute` tuple constructor | `axum/src/boxed.rs:23,38,51` | `from_handler`; `map`; `Clone::clone` | explicit constructor or `Self(...)` inside impls -> tuple struct binding `boxed.rs:12`. |
| `Position::First` | `axum-macros/src/with_position.rs:92` | `WithPosition<I>::next` | `Position::First(item)` -> enum variant `with_position.rs:66`; pattern hits elsewhere are not constructor callsites. |
| `Json::from_bytes` | `axum/src/json.rs:112,128` | `FromRequest for Json<T>`; `OptionalFromRequest for Json<T>` | `Self::from_bytes` inside `Json<T>` impls -> inherent fn `json.rs:164`. |
| `HandleError::new` | `axum/src/error_handling/mod.rs:65`; `service_ext.rs:43` | `HandleErrorLayer::layer`; trait default `ServiceExt::handle_error` | local inherent associated function -> `error_handling/mod.rs:80`; service extension chain also has user `.handle_error(...)` at `routing/tests/handle_error.rs:86`. |
| `Handler::call` | `axum/src/handler/service.rs:171` | `impl Service for HandlerService::call` | import `super::Handler` at `service.rs:1`; bound `H: Handler<T, S>` at `:148`; call `Handler::call(...)` -> trait method `handler/mod.rs:153`; current traversal stops at the trait binding, not concrete impl dispatch. |

Current executable coverage: the DB and RAG matrices assert that
`axum/src/handler/service.rs:174`
`super::future::IntoServiceFuture::new(future)` is visible but targetless
because the concrete method is generated by `opaque_future!`, and macro-expanded
items are not modeled as traversal targets yet. The `try_downcast` DB test
asserts the current one-hop resolved subset for the two same-named helpers. It
also pins the axum-core `try_downcast::<i32, _>` test rows as unsupported
macro-bound rows, not traversal edges, because the calls occur inside
`assert_eq!` macro arguments.
The DB matrix also asserts that `axum/src/handler/service.rs:171`
`Handler::call(handler, req, self.state.clone())` now traverses to the
`axum/src/handler/mod.rs:153` trait method binding in one edge.
The real-target constructor matrix also asserts the current split for
`BoxedIntoRoute`: the explicit `BoxedIntoRoute(...)` call and both
`Self(...)` constructor rows traverse to the tuple struct in one edge each.
It also asserts that `axum-macros/src/with_position.rs:92`
`Position::First(item)` traverses to the local enum variant constructor at
`axum-macros/src/with_position.rs:66` in one call edge.
RAG exact call context, `code_item_lookup`, and `code_item_edges` now also assert
that the real-corpus `BoxedIntoRoute(...)` and `Self(...)` constructor edges are
visible downstream with their target-centered proof rows. Exact TUI tools also accept
`node_kind=variant` and assert that the real-corpus
`Position::First(item)` constructor edge is visible from the exact enum-variant
target.

The same DB test also pins the macro-bound
`try_downcast::<i32, _>(...)` rows in both
`axum-core/src/body.rs:{251,252}` and `axum/src/util.rs:{114,115}` as
unsupported `assert_eq!` macro callsites with zero `try_downcast` path rows and
zero traversal candidates.

The regenerated axum call-graph fixture also resolves both
`axum/src/json.rs:{112,128}` `Self::from_bytes(&bytes)` rows to the inherent
`Json::from_bytes` method at `axum/src/json.rs:164`. The DB matrix asserts both
one-hop owner-to-target traversal edges and includes this target in the
consolidated supported traversal table. RAG exact call context,
`code_item_lookup`, and `code_item_edges` also assert both downstream
`Self::from_bytes` caller-site rows with target-centered proof rows.

The same regeneration resolves `axum/src/routing/mod.rs:109` `Self::new()` from
`Default for Router` to `Router::new`. The real-target receiver matrix now pins
310 `Router::new` caller edges and 204 incoming expansion candidates, including
the explicit `axum/src/routing/method_routing.rs:1494`
`crate::Router::new()` row and the axum-core `request_parts.rs:193`
workspace-import `Router::new()` row. TUI `code_item_lookup` and
`code_item_edges` now
accept `owner_type` for exact inherent-method disambiguation, so the downstream
real-corpus matrix asserts both `HandleError::new` caller rows and all 310
`Router::new` caller rows with target-centered proof rows.
The same table-driven TUI matrix now also preserves the two DB/RAG-supported
same-impl `self.extract_with_state(&())` method-call rows through
`code_item_lookup` and `code_item_edges`, including their `self` receiver callee
shape and target-centered proof rows.
The axum-core `request_parts.rs:186` local receiver row
`parts.extract_with_state(state)` also traverses to the local
`RequestPartsExt for Parts::extract_with_state` impl method through imported
external receiver type proof.

## Receiver And Method Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `self.extract_with_state` | `axum-core/src/ext_traits/request.rs:268` | `RequestExt::extract` | impl `RequestExt for Request` at `request.rs:262`; `Request` alias at `extract/mod.rs:29`; same impl method `extract_with_state` at `request.rs:271`; trait declaration `:122`. |
| `self.inner.poll_ready` | `axum/src/extension.rs:180` | `AddExtension::poll_ready` | field `inner: S` at `extension.rs:164-166`; impl bound `S: Service<Request<ResBody>>` at `:171`; imported `tower_service::Service` at `:12`; external trait method. |
| `Router::new` / `Router::clone` | `Router::new` at `axum/src/serve/mod.rs:756`; `crate::Router::new` at `axum/src/routing/method_routing.rs:1494`; `router.clone` in `serve/mod.rs`, `app.clone` at `routing/tests/mod.rs:804`, and `self.router.clone` at `boxed.rs:134` / `routing/mod.rs:673` | `if_it_compiles_it_works`; `building_complex_router`; serve local address tests; wrapper clone impls | typed local `let router: Router = Router::new()` -> `Router` import/re-export -> struct `routing/mod.rs:86` -> `Router::new` `:162`; explicit `crate::Router::new()` also targets the same inherent method; exact local external-trait impl receiver proof now resolves 13 `Router::clone` rows to `impl<S> Clone for Router<S>::clone` at `routing/mod.rs:90`. |
| local `req.extensions_mut` | `axum-core/src/ext_traits/request.rs:302` | `RequestExt::extract_parts_with_state` | `let mut req = Request::new(())` at `:297`; `Request` alias to `http::Request` at `extract/mod.rs:29`; `Request::new` is an external targetless alias frontier; external `http::Request::extensions_mut`. |
| parameter `req.extensions_mut` | `axum-core/src/extract/default_body_limit.rs:183,225`; `axum/src/extension.rs:184` | `DefaultBodyLimit::apply`; `DefaultBodyLimitService::call`; `AddExtension::call` | parameters `req: &mut Request<B>`, `mut req: Request<B>`, and `mut req: Request<ResBody>`; `Request` imported from `http`; current DB projection classifies direct and single-reference parameter receiver rows as external targetless frontiers. Remaining generic/request-routing rows stay unresolved targetless rows. |
| `self.0.size_hint` | `axum-core/src/body.rs:127` | `impl http_body::Body for Body::size_hint` | tuple field `Body(BoxBody)` at `:39`; `BoxBody` alias at `:13`; external `http_body::Body` trait method. |
| path-call result receiver chain | `axum/src/middleware/from_fn.rs:411` | test `basic` | `Request` alias from `axum_core::extract` at `:1` -> external `http::Request::builder` / builder chain; nested `Body::empty` is local at `axum-core/src/body.rs:52`. |
| method-call result receiver | `axum/src/routing/route.rs:51` | `Route::oneshot_inner` | `Route<E>(BoxCloneSyncService<...>)` at `:31`; imports `BoxCloneSyncService`, `Oneshot`, `ServiceExt` at `:20-22`; external tower trait methods. |
| await result receiver | `axum/src/test_helpers/test_client.rs:134` | `RequestBuilder::into_future` | field `builder: reqwest::RequestBuilder` at `:90-92`; `.send()` external reqwest method; `.await.unwrap()` external result handling. |
| await result receiver helpers | `axum/src/test_helpers/test_client.rs:156,160,168,172` | `TestResponse::{bytes,text,json,chunk}` | response helper awaited `unwrap()` rows are visible and targetless; they should not resolve to concrete callee edges. |
| turbofish method call | `axum-core/src/ext_traits/request_parts.rs:164` | test `extract_with_state` | `parts: http::request::Parts` from `Request::new(()).into_parts()` at `:159`; impl `RequestPartsExt for Parts` at `:117`; method impl at `:125`; trait decl `:108`; current DB projection preserves the two explicit method generic arguments, but the method-chain receiver remains unsupported and targetless. |
| local `Parts` extension-trait receiver | `axum-core/src/ext_traits/request_parts.rs:186` | `WorksForCustomExtractor::from_request_parts` | parameter `parts: &mut Parts` at `:184`; `Parts` imported from `http::request` at `:2`; impl `RequestPartsExt for Parts` at `:117`; method impl at `:125`; trait decl `:108`; exact imported external receiver type proof resolves the row to the local impl method. |
| shadowed callable `get` | `axum/src/routing/tests/mod.rs:423,424,425,426,427,429,430,431,432,433,434` | test `what_matches_wildcard` | module imports routing `get` at `:8-10`, but local `let get = |path| ...` at `:418` shadows it; calls target local closure, not `routing::get`. |

## Trait And Body-Owner Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `E::from_request` / `T::from_request` | `axum-core/src/ext_traits/request.rs:279`; `extract/mod.rs:127` | `RequestExt::extract_with_state`; blanket `FromRequest<S> for Result<T, T::Rejection>` | trait `FromRequest` at `extract/mod.rs:79`; method `:85`; bounds `E: FromRequest<S, M>` at `request.rs:276` and `T: FromRequest<S>` at `extract/mod.rs:121`; concrete impl remains type-parameter dependent. |
| `E::from_request_parts` / `T::from_request_parts` | `axum-core/src/ext_traits/request.rs:305`; `ext_traits/request_parts.rs:133`; `extract/mod.rs:115` | request and request-parts extraction helpers; blanket `FromRequestParts<S> for Result<T, T::Rejection>` | trait `FromRequestParts` at `extract/mod.rs:53`; method `:59`; bounds at call owner -> trait-associated dispatch. |
| handler macro `$ty::from_request_parts` | `axum/src/handler/mod.rs:242` | generated `Handler::call`, async block starts `:240` | bound `$ty: FromRequestParts<S> + Send` at `:233` -> trait method. |
| handler macro `$last::from_request` | `axum/src/handler/mod.rs:250` | generated `Handler::call`, async block starts `:240` | bound `$last: FromRequest<S, M> + Send` at `:234` -> trait method. |
| `FromRequest` ViaParts blanket inner call | `axum-core/src/extract/mod.rs:103` | blanket impl method body, async block | marker `private::ViaParts` at `:31`; blanket impl `:91`; bound `T: FromRequestParts<S>` at `:94`; call `Self::from_request_parts`; regenerated fixture owns this as an async-block path row that resolves through the parent blanket impl bounds to `FromRequestParts::from_request_parts`. |
| `FromRef::from_ref` same-crate bounded calls | `axum-core/src/ext_traits/mod.rs:25,45` | axum-core state extraction test helpers | trait `FromRef` at `extract/from_ref.rs:13`; method `:15`; same-crate bounds now traverse to the trait method binding; concrete impl dispatch remains type-dependent. |
| `FromRef::from_ref` dependency-root bounded calls | `axum/src/extract/state.rs:309`; `middleware/from_extractor.rs:328` | axum state extraction helpers and middleware tests | `FromRef` is imported through `axum_core::extract::FromRef`; both the top-level `State` extractor row and the nested middleware `local_impl_method:from_request_parts` row traverse through parsed workspace dependency proof to the axum-core `FromRef::from_ref` trait method binding. |
| `ServiceExt::handle_error` user call | `axum/src/routing/tests/handle_error.rs:86` | `handler_service_ext` | `.handle_error(...)` -> trait default `service_ext.rs:42` -> `HandleError::new` call `:43` -> inherent fn `error_handling/mod.rs:80`. |
| `Self::accept(self).await` | `axum/src/serve/listener.rs:41,61` | `Listener for TcpListener::accept`; `Listener for UnixListener::accept` | trait item `listener.rs:29`; impl self types at `:35` and `:55`; `Self::accept` targets external tokio listener inherent method, so projected rows should be external and targetless, not recursive trait calls. |
| dyn `Future::poll` | `axum/src/error_handling/mod.rs:251` | `HandleErrorFuture::poll` | field type `Pin<Box<dyn Future<...>>>` at `:240`; dispatch to trait-object `Future::poll`; concrete runtime future unresolved. |
| `<dyn Any>::downcast_mut` | `axum-core/src/body.rs:32`; `axum/src/util.rs:105` | `try_downcast` helpers | external `std::any::Any` trait-object associated call. Regenerated axum fixture coverage now asserts both rows as external, targetless, non-traversable std-root path frontiers with one explicit generic argument. |
| const initializer call owner | `routing/route.rs:202`; `axum/src/extract/ws.rs:382,384` | const initializer bodies | `routing/route.rs:202` is owned by an executable `LocalItem` owner, not an item-level `Const` node or enclosing function owner; websocket upgrade local const rows remain absent in this fixture. |
| closure body boundary | `axum-macros/src/from_ref.rs:23` | closure inside `from_ref::expand` | closure call to `expand_field` is nested-owner owned in the regenerated fixture and resolves to target fn `from_ref.rs:29`. |
| async block boundary | `axum/src/handler/mod.rs:217,240` | handler `call` async blocks | calls inside async blocks should not be flattened into outer function owner once nested async owners are modeled. |

Current executable coverage: DB target traversal now asserts the two one-hop
`E::from_request` / `T::from_request` edges to `FromRequest::from_request`, the four one-hop
`E::from_request_parts`, `T::from_request_parts`, and async-block-owned
`Self::from_request_parts` edges to `FromRequestParts::from_request_parts`, and
the two same-crate axum-core `FromRef::from_ref` bounded associated-path edges.
These are trait method binding edges only; concrete runtime impl dispatch
remains a documented future slice. The top-level axum dependency-root
`FromRef::from_ref` path row at `axum/src/extract/state.rs:309` and the nested
`axum/src/middleware/from_extractor.rs:328` `local_impl_method:from_request_parts`
row now traverse to the axum-core trait method binding. Exact TUI lookup/edges
tests assert the enclosing `test_from_extractor` item does not flatten that
nested row.
Receiver tests now assert exact owner-count buckets and source-line
fanout for the projected `req.extensions_mut()` local-binding rows, split into
direct/single-reference external parameter receivers and still-unresolved
generic receiver rows, plus the initialized external `Request::new` receiver
row, the seven projected `self.inner.poll_ready(cx)` external frontier rows,
and the three projected `self.0.poll_ready(cx)` external frontier rows. The
tuple-field coverage splits the item-level method row from the two nested local
`impl Service` rows now owned by `local_impl_method:poll_ready` executable
owners in `routing/tests/mod.rs` and `routing/tests/nest.rs`.
The result-chain coverage also pins all 14 projected `Request::builder()` rows
by owner and source line: five external rows, two unresolved axum-core
ext-trait test rows, and seven unsupported rows, including the matrix chain in
`middleware/from_fn.rs:411`.
The exact current source-line fanout is external
`extract/query.rs:104`, `extract/raw_form.rs:65`, and
`form.rs:{156,164,226}`; unresolved
`axum-core/src/ext_traits/request.rs:{375,388}`; unsupported
`middleware/from_fn.rs:411`, `routing/method_routing.rs:1697`, and
`routing/tests/get_to_head.rs:{22,56}`,
`routing/tests/mod.rs:{1129,1147}`, and `serve/mod.rs:799`.
Await-result receiver coverage now
asserts 44 raw targetless `unwrap()` rows; the source-line fanout helper covers
the 43 module-anchored rows while the additional row is owned by a nested async
block.
The targetless receiver rows for `self.0.size_hint()`, the turbofish
`parts.extract_with_state::<State<String>, String>(&state)` test call at
`request_parts.rs:164`, and `Route::oneshot` are also pinned. The
request-parts turbofish row is
projected on the test function owner, preserves two method generic arguments,
and remains `Unsupported`/targetless because the method-chain receiver is not
classified yet. The non-turbofish
`parts.extract_with_state(state)` blanket-helper call at `request_parts.rs:186`
now resolves to the local `RequestPartsExt for Parts` impl method through exact
imported external receiver type proof. RAG call-context and proof-context tests
now preserve the `axum-core/src/body.rs:127` `self.0.size_hint()` row as an
`External` tuple-field frontier with an `external_dependency_summary_missing`
proof reason. Exact TUI `code_item_lookup` and `code_item_edges` tests assert
the same owner-seeded rows and targetless proof facts.
The exact local external-trait impl receiver coverage now pins all 13 projected
`Router::clone` rows: ten typed-local `router.clone()` rows across
`serve/mod.rs`, one typed-local `app.clone()` row in `routing/tests/mod.rs`,
and two `self.router.clone()` field receiver rows in `boxed.rs` and
`routing/mod.rs`. DB target traversal, RAG exact call context, and TUI
`code_item_lookup` / `code_item_edges` preserve the same caller-site identities
and receiver buckets.
It also pins both projected `Route` `oneshot` receiver rows in
`routing/route.rs`: the method-call-result receiver at `:51` and the tuple-field
receiver at `:57`. RAG call-context and proof-context tests now preserve those
two targetless rows with zero traversal targets and `type_resolution_missing`
blocked proof rows. Exact TUI `code_item_lookup` and `code_item_edges` tests
assert the same two owner-seeded Route receiver rows and blocked proof facts.

## Dynamic And Unsupported Callable Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `(self.into_route)(...)` handler | `axum/src/boxed.rs:85` | `MakeErasedHandler::into_route` | field `into_route: fn(H, S) -> Route` at `boxed.rs:72`; initialized in `BoxedIntoRoute::from_handler` at `:23-25` with non-capturing closure; structural dynamic call, no named callee without field/closure proof. |
| `(self.into_route)(...)` router | `axum/src/boxed.rs:120` | `MakeErasedRouter::into_route` | field `into_route: fn(Router<S>, S) -> Route` at `boxed.rs:108`; construction site not found in selected `axum/src`; unsupported/fail-closed. |
| `(self.layer)(...)` | `axum/src/boxed.rs:159,163` | `Map::into_route`; `Map::call_with_state` | field `layer: Box<dyn LayerFn<E, E2>>` at `boxed.rs:142`; boxed from `BoxedIntoRoute::map(self, f)` at `:31-40`; `LayerFn` blanket impl `:167-173`; dynamic trait-object callable. |
| `(self.tap_fn)(...)` | `axum/src/serve/listener.rs:236` | `TapIo<L, F>::accept` | field `tap_fn: F` at `listener.rs:212`; set by `ListenerExt::tap_io(self, tap_fn)` at `:116-123`; bound `F: FnMut(&mut L::Io)` at `:118,229`; example closure passed at `serve/mod.rs:566`. |
| `and_then(f)` | `axum-macros/src/lib.rs:724` | `expand_with` | parameter `f: F` at `:718`; bound `F: FnOnce(I) -> syn::Result<K>` at `:720`; callback passed into external `and_then`, no concrete callee in this owner. |
| `expand_with(item, from_ref::expand)` | `axum-macros/src/lib.rs:715` | `derive_from_ref` | proc-macro owner now resolves the direct `expand_with(...)` helper edge; function item `from_ref::expand` at `from_ref.rs:11` still requires interprocedural callback proof before resolving `and_then(f)`. |
| other `expand_with(...)` callers | `axum-macros/src/lib.rs:377,426,665` | derive macro entrypoints | proc-macro owners now resolve direct `expand_with(...)` helper edges; closure bodies passed to `expand_with` remain unsupported as callback bodies. |
| direct callable parameter `f(attr,input)` | `axum-macros/src/lib.rs:737` | closure owner under `expand_attr_with` | parameter `f: F` at `:727`; bound `F: FnOnce(A, I) -> K` at `:729`; closure body captures `f`, so the persisted row is a path call to an opaque value binding with no traversal target. |
| `expand_attr_with(...)` callers | `axum-macros/src/lib.rs:581,637,655` | `debug_handler`; `debug_middleware`; `__private_axum_test` | active proc-macro owners at `:581` and `:637` now resolve direct `expand_attr_with(...)` helper edges; `:655` is cfg-inactive in the current fixture profile; resolving `f(...)` still requires interprocedural callback proof. |
| `debug_handler::expand(...)` callback rows | `axum-macros/src/lib.rs:581,637` | closure callbacks passed to `expand_attr_with` | regenerated fixture owns both callback body path calls under closure executable owners; they traverse to `debug_handler::expand` without making the enclosing macro owners direct callers. |
| IIFE closure expression | `axum-macros/src/lib.rs:734-738`; `from_request/mod.rs:200-203` | `expand_attr_with`; `from_request::expand` | closure literal immediately invoked; regenerated fixture resolves the outer dynamic call to its closure owner without inventing a named function target. |

Current executable coverage: the real-target DB matrix asserts the visible
dynamic callable-field rows, the `expand_with` callback setup, the
`expand_attr_with` IIFE row, the closure-owned `f(attr, input)` opaque callback
row, and the `from_request::expand` enum-state IIFE row. The regenerated axum
fixture resolves the IIFE dynamic rows to their closure owners with
`DynamicClosure` edges while leaving callable-parameter calls such as
`f(attr, input)` opaque and targetless.
It also asserts that proc-macro callback arguments such as `from_ref::expand` and
`axum_test::expand` are not fabricated as ordinary path-call edges before
interprocedural callback proof exists. RAG call-context and proof-context tests
now preserve the four currently projected dynamic callable-field blockers in
`axum/src/boxed.rs:{85,120,159}` and `axum/src/serve/listener.rs:236`, including
zero traversal targets and `dynamic_dispatch_unbounded` proof rows. Exact TUI
`code_item_lookup` and `code_item_edges` tests assert the same four owner-seeded
targetless rows and blocked proof facts.

## Fallback Fixture Oracle Matrix

These rows extend the axum matrix with registered source-pinned call-graph
fixtures for the fallback crates. The DB assertions live beside the axum cases
in `crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/fallback.rs`.

| Fixture crate | Target / case | Callsites | Definition or binding | Evidence chain |
| --- | --- | --- | --- | --- |
| chrono | `MappedLocalTime::Single` alias constructor | `src/offset/mod.rs:143,156,468,502,535`; `src/offset/fixed.rs:135,138`; `src/offset/utc.rs:122,125`; `src/datetime/tests.rs:75,79` | alias `src/offset/mod.rs:77`; enum `LocalResult` at `:81`; variant `Single(T)` at `:83` | `corpus_chrono_call_graph` now resolves these 11 alias constructor rows to `LocalResult::Single` through the typed alias relation, preserving the literal `MappedLocalTime::Single` callsite path and one `EnumVariantConstructor` edge per callsite. The `map_or(..., MappedLocalTime::Single)` rows in `offset/mod.rs` are function-item arguments, not projected callsites in the current fixture. |
| chrono | `DateTime...?.naive_utc()` try receiver | `src/format/parsed.rs:836,953`; macro-equivalent `src/naive/datetime/mod.rs:140,157,174,195` | `DateTime<Tz>::naive_utc` at `src/datetime/mod.rs:563`; constructors return `Option<Self>` at `:768,:803` | `corpus_chrono_call_graph` currently projects two `TryResult` receiver method rows for `naive_utc` with `Unsupported` status and no traversal edge to the method. |
| chrono | guarded match arm method guard | `src/format/strftime.rs:635` | `StrftimeItems.queue` field `src/format/strftime.rs:198` | `corpus_chrono_call_graph` currently projects one targetless `SelfField(["queue"]).is_empty()` row with `Unsupported` status. |
| memchr | arbitrary-expression dynamic callee | macro source `src/arch/x86_64/memchr.rs:153`; macro instantiations at `:180,203,227,252,278,305,326` | macro-local aliases `type Fn = *mut ()`, `type RealFn = $fnty` at `:72-73`; static pointer `FN` at `:74` | `corpus_memchr_call_graph` currently does not project the `core::mem::transmute::<Fn, RealFn>(fun)(...)` path or outer dynamic call, so the test asserts absence rather than guessed callees. |
| memchr | function-pointer field call | `src/memmem/searcher.rs:222,718` | `Searcher.call` at `:34`; alias `SearcherKindFn` at `:273`; `Prefilter.call` at `:605`; alias `PrefilterKindFn` at `:774` | `corpus_memchr_call_graph` currently projects two targetless dynamic rows owned by methods named `find`, with argument counts 4 and 2. |
| memchr | callable trait object field | `src/tests/substring/mod.rs:94,110` | `Runner.fwd` at `:67-68`; `Runner.rev` at `:70-71`; setters box closures at `:137,153` | `corpus_memchr_call_graph` currently does not project these boxed `dyn FnMut` local-binding calls as dynamic rows under `Runner::run`. |
| generic-array | guarded match arm | `src/lib.rs:1241,1243,1278,1280` | `ArrayLength` bound at `src/lib.rs:245`; `LengthError` at `:1197` | `corpus_generic_array_call_graph` now projects two targetless `iter.size_hint()` method rows with `Unsupported` status and no traversal edge. |

Current executable coverage: `fallback.rs` now pins the chrono alias rows by
exact source owner and resolved traversal to `LocalResult::Single`, pins chrono
try-receiver and guarded-receiver rows by owner and source-line fanout, and
pins the memchr function-pointer rows by owner and source-line fanout. RAG exact
call-context and TUI `code_item_lookup` tests also preserve the 11 chrono alias
constructor caller-site identities. RAG collection now preserves the two memchr
function-pointer field blockers with argument counts 4 and 2, and TUI
`code_item_lookup` plus `code_item_edges` preserve the same targetless dynamic
rows and blocked proof facts. It also pins memchr callable trait-object cases by
explicit owner-scoped absence. The generic-array guarded match-arm case remains
a fixture-wide absence assertion because the source checkout is not present in
`tests/fixture_github_clones/corpus`.

## Boundary Items

| Item | Current state | Required next step before stricter DB test |
| --- | --- | --- |
| `TestClient::new` owner-per-callsite matrix | Exact 172 file/line callsites are listed. `ploke-db` now asserts the registered axum backup's 168 projected structural rows resolve to `TestClient::new`, including nested/direct re-export import rows, inherited `use super::*` parent-glob rows from routing child modules, and the axum-core `request_parts.rs:193` workspace dependency glob row. Known remaining source-oracle frontiers include multipart, closure-body, and macro-template projection gaps. | Keep the 168 resolved DB contract; when multipart, closure-body, macro-template ownership, or broader structural/import evidence lands, tighten this toward full 172-callsite parity. |
| Markdown/doc-comment examples | Excluded from oracle rows. | Keep excluded unless parser fixture intentionally ingests docs as Rust examples. |
| Dynamic callback targets | Source binding chains are recorded, but concrete targets remain intentionally unresolved. | Tests should assert structural dynamic call plus `Unsupported`/fail-closed status, not guessed callees. |
