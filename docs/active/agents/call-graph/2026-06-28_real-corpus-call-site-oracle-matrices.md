# Real Corpus Call-Site Oracle Matrices

Date: 2026-06-28
Status: active call-graph planning note
Short description: Definition/binding sites, discovered callsites, and source evidence chains for the real-corpus call-site matrix.

Related files:

- [CALL_GRAPH_COVERAGE.md](../../../testing/CALL_GRAPH_COVERAGE.md)
- [BACKUP_DB_FIXTURES.md](../../../testing/BACKUP_DB_FIXTURES.md)

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

## Generated Handler And Service Function Fanout

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `routing::post` | `axum/src/json.rs:248,264,279,299,318,353` | JSON extractor tests | grouped import at `json.rs:237` -> `routing::post` re-export `routing/mod.rs:45-48` -> generated top-level handler fn from `method_routing.rs:165` / `:445`. |
| `routing::post` | `axum/src/extract/multipart.rs:381,404,420,448` | multipart tests | grouped import at `multipart.rs:362` -> same generated function binding. |
| `routing::post` | `axum/src/routing/method_routing.rs:1448,1660` | `merge`; `merge_accessing_state` | same-module test visibility through `use super::*` at `method_routing.rs:1391` -> generated function binding. |
| `routing::post` | `axum/src/routing/tests/mod.rs:88,624,666,744,745,746,772,792,812,838,842,844,899,1071,1162` | routing tests | grouped `crate::routing::{..., post, ...}` import at `routing/tests/mod.rs:8-11` -> generated function binding. |
| `routing::get_service` | `axum/src/routing/tests/get_to_head.rs:46`; `handle_error.rs:88`; `merge.rs:197,203`; `method_routing.rs:1415`; `routing/tests/mod.rs:173,231,279`; `routing/tests/fallback.rs:203` | service tests | imports, inherited test module visibility, and explicit `crate::routing::get_service` bind to generated `top_level_service_fn!(get_service, GET)` from `method_routing.rs:31-91` / `:337`. |
| `routing::delete_service` | `axum/src/routing/method_routing.rs:1500` | method routing tests | same-module test visibility binds to generated `top_level_service_fn!(delete_service, DELETE)` from `method_routing.rs:31-91` / `:336`. |
| `routing::patch_service` | `axum/src/routing/tests/mod.rs:280` | routing tests | grouped `crate::routing::{..., patch_service, ...}` import binds to generated `top_level_service_fn!(patch_service, PATCH)` from `method_routing.rs:31-91` / `:340`. |
| `routing::post_service` | `axum/src/routing/method_routing.rs:1620` | method routing tests | same-module test visibility binds the top-level `post_service(...)` call to generated `top_level_service_fn!(post_service, POST)` from `method_routing.rs:31-91` / `:341`; chained `.post_service(...)` method rows are separate. |

## Re-Exported Body Constructor Fanout

| Target | Callsites | Evidence chain |
| --- | --- | --- |
| `Body::empty` | `axum-core/src/response/into_response.rs:128,163`; `axum-core/src/ext_traits/request.rs:346,364,377,390` | direct `axum_core::body::Body` path -> `axum-core/src/body.rs:52`. |
| `Body::empty` | `axum/src/form.rs:158`; `extract/query.rs:106`; `extract/raw_form.rs:65`; `extract/ws.rs:394,400,1129,1191`; `serve/mod.rs:799`; `middleware/from_fn.rs:411`; `routing/route.rs:161,174`; `routing/method_routing.rs:1700`; `routing/tests/get_to_head.rs:25,59`; `routing/tests/merge.rs:198,204`; `routing/tests/mod.rs:228,1133,1151` | local/imported `Body` -> axum re-export `axum/src/body/mod.rs:10` or direct `axum_core::body::Body` import -> `axum-core/src/body.rs:52`. |

## High-Fanout Test Helper Matrix

`TestClient::new` has 172 selected-member text callsites. The full fanout is useful for fixture coverage; strict DB assertions should be generated from parser facts because owner recovery by hand is noisy.

| Target | Exact callsite fanout | Evidence chain |
| --- | --- | --- |
| `TestClient::new` | `axum/src/form.rs:262`; `json.rs:250,266,281,301,320,355`; `extension.rs:228`; `response/sse.rs:714,756,793`; `response/mod.rs:529`; `middleware/from_extractor.rs:351`; `middleware/map_request.rs:412,432`; `middleware/map_response.rs:357`; `extract/query.rs:158`; `extract/connect_info.rs:386`; `extract/multipart.rs:383,423,449`; `extract/mod.rs:103`; `extract/matched_path.rs:162,178,197,217,237,254,271,291,312,326,346,361,374,394`; `extract/nested_path.rs:136,154,172,190,205,224`; `extract/path/mod.rs:619,632,645,664,687,700,716,732,751,784,798,822,854,912,946,974,989,1010,1034`; `handler/mod.rs:418,443`; `routing/tests/nest.rs:41,65,135,159,182,193,210,229,280,298,309,328,365,408,431,489`; `routing/tests/merge.rs:14,63,81,85,96,116,136,150,162,179,208,234,267,301,345,379`; `routing/tests/handle_error.rs:25,42,60,76,90`; `routing/tests/fallback.rs:10,25,40,53,69,89,101,118,134,150,171,190,207,221,241,261,280,299,314,325,338,359,377,389,402`; `routing/tests/mod.rs:90,118,150,188,217,233,242,282,307,323,339,352,365,377,396,416,454,472,489,506,527,540,572,589,599,626,643,668,685,700,717,738,748,775,798,815,846,905,952,967,984,1027,1047,1073,1164,1201`; `axum-core/src/extract/request_parts.rs:193` | common chain: callsite -> visible `TestClient` import, direct module scope, or `test_helpers::*` -> re-export `axum/src/test_helpers/mod.rs:5-6` -> struct `test_client.rs:30` -> `new` function `test_client.rs:36`. |

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

## Receiver And Method Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `self.extract_with_state` | `axum-core/src/ext_traits/request.rs:268` | `RequestExt::extract` | impl `RequestExt for Request` at `request.rs:262`; `Request` alias at `extract/mod.rs:29`; same impl method `extract_with_state` at `request.rs:271`; trait declaration `:122`. |
| `self.inner.poll_ready` | `axum/src/extension.rs:180` | `AddExtension::poll_ready` | field `inner: S` at `extension.rs:164-166`; impl bound `S: Service<Request<ResBody>>` at `:171`; imported `tower_service::Service` at `:12`; external trait method. |
| `Router::new` / `Router::clone` | `Router::new` at `axum/src/serve/mod.rs:756`; `crate::Router::new` at `axum/src/routing/method_routing.rs:1494`; workspace-import `Router::new` at `axum-core/src/extract/request_parts.rs:193`; `router.clone` in `serve/mod.rs`, `app.clone` at `routing/tests/mod.rs:804`, and `self.router.clone` at `boxed.rs:134` / `routing/mod.rs:673` | `if_it_compiles_it_works`; `building_complex_router`; `extract_request_parts`; serve local address tests; wrapper clone impls | typed local `let router: Router = Router::new()` -> `Router` import/re-export -> struct `routing/mod.rs:86` -> `Router::new` `:162`; explicit `crate::Router::new()` also targets the same inherent method; axum-core imports `axum::Router` and now has an admitted proof-only `dependency_root` row for the workspace import; exact local external-trait impl receiver proof resolves 26 total `Router::clone` rows to `impl<S> Clone for Router<S>::clone` at `routing/mod.rs:90`, including 13 method-result receiver rows. |
| `CountingCloneableState::new` / `clone` / `setup_done` | `new` at `axum/src/routing/tests/mod.rs:1165,1187` and `routing/tests/fallback.rs:396`; `clone` at `mod.rs:1170,1190` and `fallback.rs:400`; `setup_done` at `mod.rs:1174,1194` and `fallback.rs:405` | `state_isnt_cloned_too_much`; `state_isnt_cloned_too_much_in_layer`; `state_isnt_cloned_too_much_with_fallback` | `test_helpers::*` imports `CountingCloneableState`; `let state = CountingCloneableState::new()` resolves to the helper constructor at `test_helpers/counting_cloneable_state.rs:15`; the constructor's `Self` return type proves the initialized local receiver type, so `state.clone()` reaches the local `Clone` impl and `state.setup_done()` reaches the inherent method. |
| local `req.extensions_mut` | `axum-core/src/ext_traits/request.rs:302` | `RequestExt::extract_parts_with_state` | `let mut req = Request::new(())` at `:297`; `Request` alias to `http::Request` at `extract/mod.rs:29`; `Request::new` is an external targetless alias frontier; external `http::Request::extensions_mut`. |
| parameter `req.extensions_mut` | `axum-core/src/extract/default_body_limit.rs:183,225`; `axum/src/extension.rs:184`; `axum/src/extract/nested_path.rs:95,103`; `axum/src/routing/path_router.rs:336` | `DefaultBodyLimit::apply`; `DefaultBodyLimitService::call`; `AddExtension::call`; `SetNestedPath::call`; `PathRouter::call_with_state` | parameters `req: &mut Request<B>`, `mut req: Request<B>`, and request-routing `req` local bindings; `Request` imported from `http`; current DB projection classifies these six inspected `req.extensions_mut()` local-binding rows as external targetless frontiers. |
| impl Trait parameter `error.into` | `axum-core/src/error.rs:14` | `Error::new` | parameter `error: impl Into<BoxError>` at `:12`; `BoxError` alias at `axum-core/src/lib.rs:31`; current DB projection classifies `error.into()` as a targetless external/prelude `Into` frontier with no local traversal target. |
| `self.0.size_hint` | `axum-core/src/body.rs:127` | `impl http_body::Body for Body::size_hint` | tuple field `Body(BoxBody)` at `:39`; `BoxBody` alias at `:13`; external `http_body::Body` trait method. |
| path-call result receiver chain | `axum/src/middleware/from_fn.rs:411` | test `basic` | `Request` alias from `axum_core::extract` at `:1` -> external `http::Request::builder` / builder chain; nested `Body::empty` is local at `axum-core/src/body.rs:52`. |
| self-field method-result receiver | `axum/src/routing/route.rs:51` | `Route::oneshot_inner` | `self.0.clone().oneshot(req)` is persisted as `MethodResultField(clone, ["0"])`; `Route<E>(BoxCloneSyncService<...>)` at `:31` and the imported tower traits prove an External, targetless frontier. |
| await result receiver | `axum/src/test_helpers/test_client.rs:134` | `RequestBuilder::into_future` | field `builder: reqwest::RequestBuilder` at `:90-92`; `.send()` external reqwest method; `.await.unwrap()` external result handling. |
| await result receiver helpers | `axum/src/test_helpers/test_client.rs:156,160,168,172` | `TestResponse::{bytes,text,json,chunk}` | response helper awaited `unwrap()` rows are visible and targetless; they should not resolve to concrete callee edges. |
| turbofish method call | `axum-core/src/ext_traits/request_parts.rs:164` | test `extract_with_state` | `parts: http::request::Parts` from `Request::new(()).into_parts()` at `:159`; impl `RequestPartsExt for Parts` at `:117`; method impl at `:125`; trait decl `:108`; current DB projection preserves the two explicit method generic arguments and the tuple-method-return receiver shape, and now resolves through the exact external tuple-return summary for `http::Request::into_parts` returning `http::request::Parts`. |
| local `Parts` extension-trait receiver | `axum-core/src/ext_traits/request_parts.rs:186` | `WorksForCustomExtractor::from_request_parts` | parameter `parts: &mut Parts` at `:184`; `Parts` imported from `http::request` at `:2`; impl `RequestPartsExt for Parts` at `:117`; method impl at `:125`; trait decl `:108`; exact imported external receiver type proof resolves the row to the local impl method. |
| shadowed callable `get` | setup `axum/src/routing/tests/mod.rs:412,413`; closure calls at `:423,424,425,426,427,429,430,431,432,433,434` | test `what_matches_wildcard` | module imports routing `get` at `:8-10`, but local `let get = |path| ...` at `:418` shadows it; the later calls target the local closure, not `routing::get`. DB resolves only the two setup `get(...)` path rows to `routing::method_routing::get` and does not fabricate closure-to-routing edges. |

## Trait And Body-Owner Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `E::from_request` / `T::from_request` | `axum-core/src/ext_traits/request.rs:279`; `extract/mod.rs:127` | `RequestExt::extract_with_state`; blanket `FromRequest<S> for Result<T, T::Rejection>` | trait `FromRequest` at `extract/mod.rs:79`; method `:85`; bounds `E: FromRequest<S, M>` at `request.rs:276` and `T: FromRequest<S>` at `extract/mod.rs:121`; concrete impl remains type-parameter dependent. |
| `E::from_request_parts` / `T::from_request_parts` | `axum-core/src/ext_traits/request.rs:305`; `ext_traits/request_parts.rs:133`; `extract/mod.rs:115`; `axum/src/middleware/from_extractor.rs:220` | request and request-parts extraction helpers; blanket `FromRequestParts<S> for Result<T, T::Rejection>`; middleware async block nested in `FromExtractor::call` | trait `FromRequestParts` at `extract/mod.rs:53`; method `:59`; bounds at call owner -> trait-associated dispatch. |
| handler macro `$ty::from_request_parts` | `axum/src/handler/mod.rs:242` | generated `Handler::call`, async block starts `:240` | current fixture projects stable generated rows such as `T1::from_request_parts` on generated async-block owners and resolves them through generated impl where-clause proof plus the `crate::extract::FromRequestParts` re-export chain to `FromRequestParts::from_request_parts`. |
| handler macro `$last::from_request` | `axum/src/handler/mod.rs:250` | generated `Handler::call`, async block starts `:240` | current fixture projects stable generated rows such as `T1::from_request` / `T2::from_request` on generated async-block owners and resolves them through generated impl where-clause proof plus the `crate::extract::FromRequest` re-export chain to `FromRequest::from_request`. |
| error-handling service macro `$ty::from_request_parts` | `axum/src/error_handling/mod.rs:187`; invocations `:207-222` | generated `HandleError<S, F, T>::call`, async block starts in macro template at `:185` | current fixture projects bounded, module-specific generated rows such as `T1::from_request_parts` through `T16::from_request_parts` on generated async-block owners and resolves them through generated `FromRequestParts<()>` where-clause proof to `FromRequestParts::from_request_parts`. The synthesized body is intentionally limited to extractor proof rows and does not claim general macro expansion of the rest of the service body. |
| tuple extractor macro `$ty::from_request_parts` / `$last::from_request` | `axum-core/src/extract/tuple.rs:29,52,57`; invocation `:77` | generated tuple `FromRequestParts` and `FromRequest` impl owners; `FromRequest::from_request` async block starts in macro template at `:49` | current fixture projects bounded, module-specific generated tuple extractor impls for arities 1 through 16. Their stable rows such as `T16::from_request_parts(parts, state)` and `T2::from_request(req, state)` resolve through generated where-clause proof to `FromRequestParts::from_request_parts` and `FromRequest::from_request`. The synthesized body is intentionally limited to extractor proof rows and does not claim general macro expansion of the error-conversion closure bodies. |
| `FromRequest` ViaParts blanket inner call | `axum-core/src/extract/mod.rs:103` | blanket impl method body, async block | marker `private::ViaParts` at `:31`; blanket impl `:91`; bound `T: FromRequestParts<S>` at `:94`; call `Self::from_request_parts`; regenerated fixture owns this as an async-block path row that resolves through the parent blanket impl bounds to `FromRequestParts::from_request_parts`. |
| `FromRef::from_ref` same-crate bounded calls | `axum-core/src/ext_traits/mod.rs:25,45` | axum-core state extraction test helpers | trait `FromRef` at `extract/from_ref.rs:13`; method `:15`; same-crate bounds now traverse to the trait method binding; concrete impl dispatch remains type-dependent. |
| `FromRef::from_ref` dependency-root bounded calls | `axum/src/extract/state.rs:309`; `middleware/from_extractor.rs:328` | axum state extraction helpers and middleware tests | `FromRef` is imported through `axum_core::extract::FromRef`; both the top-level `State` extractor row and the nested middleware `local_impl_method:from_request_parts` row traverse through parsed workspace dependency proof to the axum-core `FromRef::from_ref` trait method binding. |
| `ServiceExt::handle_error` user call | `axum/src/routing/tests/handle_error.rs:86` | `handler_service_ext` | `.handle_error(...)` -> trait default `service_ext.rs:42` -> `HandleError::new` call `:43` -> inherent fn `error_handling/mod.rs:80`. |
| `Self::accept(self).await` | `axum/src/serve/listener.rs:41,61` | `Listener for TcpListener::accept`; `Listener for UnixListener::accept` | trait item `listener.rs:29`; impl self types at `:35` and `:55`; `Self::accept` targets external tokio listener inherent method, so projected rows should be external and targetless, not recursive trait calls. |
| dyn `Future::poll` | `axum/src/error_handling/mod.rs:251` | `HandleErrorFuture::poll` | `self.project().future.poll(cx)` is persisted as `MethodResultField(project, ["future"])` and remains Unsupported with zero targets; the post-result field must not inherit direct self-field-result proof. The field type is `Pin<Box<dyn Future<...>>>` at `:240`, and concrete runtime future dispatch remains unresolved. |
| `<dyn Any>::downcast_mut` | `axum-core/src/body.rs:32`; `axum/src/util.rs:105` | `try_downcast` helpers | external `std::any::Any` trait-object associated call. Regenerated axum fixture coverage now asserts both rows as external, targetless, non-traversable std-root path frontiers with one explicit generic argument. |
| const initializer call owner | `routing/route.rs:202`; `axum/src/extract/ws.rs:382,384` | const initializer bodies | `routing/route.rs:202` is owned by an executable `LocalItem` owner, not an item-level `Const` node or enclosing function owner; websocket upgrade local const rows remain absent in this fixture. |
| closure body boundary | `axum-macros/src/from_ref.rs:23` | closure inside `from_ref::expand` | closure call to `expand_field` is nested-owner owned in the regenerated fixture and resolves to target fn `from_ref.rs:29`. |
| async block boundary | `axum/src/handler/mod.rs:217,240` | handler `call` async blocks | calls inside async blocks should not be flattened into outer function owner once nested async owners are modeled. |

## Dynamic And Unsupported Callable Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `(self.into_route)(...)` handler | `axum/src/boxed.rs:85` | `MakeErasedHandler::into_route` | field `into_route: fn(H, S) -> Route` at `boxed.rs:72`; initialized in `BoxedIntoRoute::from_handler` at `:23-25` with a unique non-capturing closure; now resolved as a `DynamicClosure` edge by the function-pointer self-field proof. |
| `(self.into_route)(...)` router | `axum/src/boxed.rs:120` | `MakeErasedRouter::into_route` | field `into_route: fn(Router<S>, S) -> Route` at `boxed.rs:108`; construction site not found in selected `axum/src`; unsupported/fail-closed. |
| `(self.layer)(...)` | `axum/src/boxed.rs:159,163` | `Map::into_route`; `Map::call_with_state` | field `layer: Box<dyn LayerFn<E, E2>>` at `boxed.rs:142`; boxed from `BoxedIntoRoute::map(self, f)` at `:31-40`; regenerated fixture now preserves three finite ambiguous `DynamicClosure` candidates from the visible `MethodRouter::{layer,route_layer}` `layer_fn` bindings and the transparent `Router::layer` `map_inner!` source expression `|route| route.layer(layer)` while admitting no local traversal edge. The `this.catch_all_fallback.map(...)` source row at `routing/mod.rs:307` remains targetless because its receiver has no local binding/type proof. |
| `(self.tap_fn)(...)` | `axum/src/serve/listener.rs:236` | `TapIo<L, F>::accept` | field `tap_fn: F` at `listener.rs:212`; set by `ListenerExt::tap_io(self, tap_fn)` at `:116-123`; bound `F: FnMut(&mut L::Io)` at `:118,229`; example closure passed at `serve/mod.rs:566`. |
| `and_then(f)` | `axum-macros/src/lib.rs:724` | `expand_with` | parameter `f: F` at `:718`; bound `F: FnOnce(I) -> syn::Result<K>` at `:720`; callback passed into external `and_then`; current fixture projects four finite ambiguous method-callback candidates and still admits no resolved traversal edge. |
| `expand_with(item, from_ref::expand)` | `axum-macros/src/lib.rs:715` | `derive_from_ref` | proc-macro owner now resolves the direct `expand_with(...)` helper edge; function item `from_ref::expand` at `from_ref.rs:11` appears as the `MethodCallbackFunction` candidate for `and_then(f)`, not as a resolved traversal target. |
| other `expand_with(...)` callers | `axum-macros/src/lib.rs:377,426,665` | derive macro entrypoints | proc-macro owners now resolve direct `expand_with(...)` helper edges; closure arguments passed to `expand_with` appear as the three `MethodCallbackClosure` candidates for `and_then(f)`, but callback body traversal remains unresolved. |
| direct callable parameter `f(attr,input)` | `axum-macros/src/lib.rs:737` | closure owner under `expand_attr_with` | parameter `f: F` at `:727`; bound `F: FnOnce(A, I) -> K` at `:729`; closure body captures `f`, so the persisted row is a path call to an opaque value binding with no traversal target. |
| `expand_attr_with(...)` callers | `axum-macros/src/lib.rs:581,637,655` | `debug_handler`; `debug_middleware`; `__private_axum_test` | active proc-macro owners at `:581` and `:637` now resolve direct `expand_attr_with(...)` helper edges; `:655` is cfg-inactive in the current fixture profile; resolving `f(...)` still requires interprocedural callback proof. |
| `debug_handler::expand(...)` callback rows | `axum-macros/src/lib.rs:581,637` | closure callbacks passed to `expand_attr_with` | regenerated fixture owns both callback body path calls under closure executable owners; they traverse to `debug_handler::expand` without making the enclosing macro owners direct callers. |
| IIFE closure expression | `axum-macros/src/lib.rs:734-738`; `from_request/mod.rs:200-203` | `expand_attr_with`; `from_request::expand` | closure literal immediately invoked; regenerated fixture resolves the outer dynamic call to its closure owner without inventing a named function target. |

## Axum Module-Qualified Constructor Oracle

| Target / case | Callsites | Definition or binding | Evidence chain |
| --- | --- | --- | --- |
| `private::ServeFuture(...)` tuple constructor | `axum/src/serve/mod.rs:389,541` | `axum/src/serve/mod.rs:672` `pub struct ServeFuture<T = Infallible>(...)` inside `mod private` | Both `IntoFuture` impl methods construct the local `private::ServeFuture` tuple struct. The regenerated `corpus_axum_call_graph` fixture now resolves both rows as `TupleStructConstructor` edges to `ServeFuture`; the shared call-shape matrix pins the `Serve::into_future` row as the representative DB/RAG traversal case. |

## Fallback Fixture Oracle Matrix

These rows extend the axum matrix with registered source-pinned call-graph
fixtures for the fallback crates. The DB assertions live beside the axum cases
in `crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/fallback.rs`.

| Fixture crate | Target / case | Callsites | Definition or binding | Evidence chain |
| --- | --- | --- | --- | --- |
| fixture_nodes | generic self-field receiver frontiers | `src/impls.rs:77` `self.value.len()`; `src/impls.rs:103` `self.value.into()` | `GenericStruct<T>::value` at `src/impls.rs:58`; `impl<'a> GenericStruct<&'a str>::get_str_len` at `:75`; `impl<T> SimpleTrait for GenericStruct<T>` where-bound at `:98-100` | `fixture_nodes` now projects both rows as targetless `External` `SelfField(["value"])` frontiers: `self.value.len()` through the concrete `GenericStruct<&str>` impl argument, and `self.value.into()` through the source-visible external `Into<i32>` trait bound. DB and RAG tests assert that no local traversal edge or target is fabricated for either row. |
| chrono | `MappedLocalTime::Single` alias constructor | `src/offset/mod.rs:143,156,468,502,535`; `src/offset/fixed.rs:135,138`; `src/offset/utc.rs:122,125`; `src/offset/local/unix.rs:159`; `src/datetime/tests.rs:75,79` | alias `src/offset/mod.rs:77`; enum `LocalResult` at `:81`; variant `Single(T)` at `:83` | `corpus_chrono_call_graph` now resolves these 12 alias constructor rows to `LocalResult::Single` through the typed alias relation, preserving the literal `MappedLocalTime::Single` callsite path and one `EnumVariantConstructor` edge per callsite. The unix row is visible after active fixture regeneration because bare `#[cfg(unix)]` is evaluated as target-family evidence. The `map_or(..., MappedLocalTime::Single)` rows in `offset/mod.rs` are function-item arguments, not projected callsites in the current fixture. |
| chrono | `DateTime<Tz>::naive_utc` seven-caller receiver fanout | `src/format/parsed.rs:836,953`; two cfg(test, feature = "clock") `Local::now()` rows; `src/offset/mod.rs:520`; `src/offset/mod.rs:468,502` | `DateTime<Tz>::naive_utc` at `src/datetime/mod.rs:563`; constructors return `Option<Self>` at `:768,:803` | `corpus_chrono_call_graph` resolves all seven callers with their exact receiver proofs: two `TryMethodCallResult(ok_or)`, two `InitializedLocalBinding(Local::now)`, one `PathCallResult(DateTime::from_timestamp_nanos)`, and two `SelfValue`. Each source callsite preserves one exact method edge to `DateTime<Tz>::naive_utc`; macro-equivalent `src/naive/datetime/mod.rs:140,157,174,195` rows remain outside this proof until macro-expanded constructor return evidence is modeled. |
| chrono | guarded match arm method guard | `src/format/strftime.rs:635` | `StrftimeItems.queue: &'static [Item<'static>]` field `src/format/strftime.rs:198` | `corpus_chrono_call_graph` now projects one targetless `SelfField(["queue"]).is_empty()` row with `External` status, using the source-visible slice receiver type without fabricating a local traversal edge. |
| memchr | arbitrary-expression dynamic callee | macro source `src/arch/x86_64/memchr.rs:153`; macro instantiations at `:180,203,227,252,278,305,326` | macro-local aliases `type Fn = *mut ()`, `type RealFn = $fnty` at `:72-73`; static pointer `FN` at `:74` | `corpus_memchr_call_graph` now projects seven bounded generated rows for `core::mem::transmute::<Fn, RealFn>(fun)(...)`: an inner targetless external path row and an outer targetless external returned-path dynamic row per macro instantiation. The test asserts the rows and argument counts but still requires no concrete function-pointer target or traversal edge. |
| memchr | function-pointer field call | `src/memmem/searcher.rs:222,718` | `Searcher.call` at `:34`; alias `SearcherKindFn` at `:273`; `Prefilter.call` at `:605`; alias `PrefilterKindFn` at `:774` | `corpus_memchr_call_graph` now projects two ambiguous `DynamicFunction` rows owned by methods named `find`, with argument counts 4 and 2. Shorthand local aliases in the real constructors preserve the cfg-visible helper candidates; cfg-gated architecture helpers that are absent from the fixture are omitted rather than guessed. |
| memchr | callable trait object field | `src/tests/substring/mod.rs:94,110` | `Runner.fwd` at `:67-68`; `Runner.rev` at `:70-71`; setters box closures at `:137,153` | `corpus_memchr_call_graph` projects these boxed `dyn FnMut` local-binding calls as targetless unsupported path rows under `Runner::run`; it does not project them as dynamic rows or fabricate edges to the setter closures. RAG collection and exact TUI lookup/edges now preserve both targetless path blockers. |
| generic-array | guarded match arm method-result receiver | `src/lib.rs:1239,1241,1243,1276,1278,1280` | `ArrayLength` bound at `src/lib.rs:245`; `LengthError` at `:1197`; `I: IntoIterator` owner bounds on `try_from_iter` and `try_from_fallible_iter` | `corpus_generic_array_call_graph` now projects two targetless `iter.size_hint()` method rows with `External` status and `MethodResultLocalBinding(method_name = "into_iter")` receiver proof. No local traversal edge is fabricated for the external iterator frontier. |

## Boundary Items

| Item | Current state | Required next step before stricter DB test |
| --- | --- | --- |
| `TestClient::new` owner-per-callsite matrix | Exact 172 file/line callsites are listed. `ploke-db` now asserts the registered axum backup's 168 projected source-line rows resolve to `TestClient::new`, including nested/direct re-export import rows, inherited `use super::*` parent-glob rows from routing child modules, and the axum-core `request_parts.rs:193` workspace dependency glob row. A focused owner traversal also covers the `#[crate::test]` owner in `routing/tests/nest.rs:365`, though the current fixture span maps that persisted call row to `nest.rs:309` in the strict file/line fanout. Known remaining source-oracle frontiers are the three `multipart` feature-gated callsites plus this span-provenance mismatch. | Keep the 168 resolved source-line DB contract; if the registered axum fixture is intentionally built with `multipart` and call-site span provenance is tightened, move this toward full 172-callsite parity. |
| Markdown/doc-comment examples | Excluded from oracle rows. | Keep excluded unless parser fixture intentionally ingests docs as Rust examples. |
| Dynamic callback targets | Source binding chains are recorded, but concrete targets remain intentionally edge-free. | Zero-proof rows stay `Unsupported`; finite complete candidate sets stay `Ambiguous`. Neither status admits a guessed traversal edge. |
