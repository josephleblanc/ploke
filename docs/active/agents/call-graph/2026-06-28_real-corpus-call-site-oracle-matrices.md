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
| `try_downcast` | `axum-core/src/body.rs:23`; separate axum helper `axum/src/util.rs:99` | local | Two same-named helpers exist in different crates. |
| `BoxedIntoRoute` constructor | tuple struct `axum/src/boxed.rs:12` | local constructor | Includes explicit `BoxedIntoRoute(...)` and `Self(...)` constructor syntax. |
| `Position::First` | enum `axum-macros/src/with_position.rs:65`; variant `:66` | local variant constructor | Pattern matches are not constructor callsites. |
| `Json::from_bytes` | inherent impl `axum/src/json.rs:157`; fn `:164` | local | `Self::from_bytes` inside `Json<T>` impls. |
| `HandleError::new` | inherent impl `axum/src/error_handling/mod.rs:78`; fn `:80` | local | Called by layer impl and trait default method. |
| `Handler::call` | trait method `axum/src/handler/mod.rs:153` | local trait | Concrete impl bodies exist, but `Handler::call(...)` syntax targets the trait method binding. |

## Path, Import, And External Call Oracles

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `parse_attrs` | `axum-macros/src/typed_path.rs:23`; `from_ref.rs:30`; `from_request/mod.rs:112,196,471,592,715,880,896,1017,1027` | `expand`; `expand_field`; `extract_fields`; `impl_struct_by_extracting_all_at_once`; `impl_enum_by_extracting_all_at_once`; `infer_state_type_from_field_attributes` | explicit `crate::attr_parsing::parse_attrs` or local import at `from_ref.rs:9` / `from_request/mod.rs:3` -> `attr_parsing.rs:59`. |
| `run_ui_tests` | `axum-macros/src/debug_handler.rs:885,890`; `typed_path.rs:443`; `from_ref.rs:104`; `from_request/mod.rs:1050` | UI test helper functions | `crate::run_ui_tests(...)` -> crate-root helper `lib.rs:797`. |
| `take_route_or_internal_error` | `axum/src/routing/mod.rs:410,430`; `routing/tests/mod.rs:56,59` | `Router<S>::fallback_endpoint`; `take_route_or_internal_error_panics_on_second_call` | same-module call or `super::take_route_or_internal_error` -> parent module definition `routing/mod.rs:63`. |
| `serde_json::Deserializer::from_slice` | `axum/src/json.rs:184` | `Json<T>::from_bytes` | `serde_json::...` path -> external dependency root in `axum/Cargo.toml:135` and dev dependency `:189`. |
| `std::mem::replace` | `axum/src/error_handling/mod.rs:138,181`; `middleware/map_request.rs:281`; `middleware/from_fn.rs:285`; `middleware/map_response.rs:260`; `response/sse.rs:449` | service call bodies and `EventDataWriter::write_buf` | `std::mem::replace` path -> std-root external classification. |

## Generated Handler Function Fanout

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `routing::post` | `axum/src/json.rs:248,264,279,299,318,353` | JSON extractor tests | grouped import at `json.rs:237` -> `routing::post` re-export `routing/mod.rs:45-48` -> generated top-level handler fn from `method_routing.rs:165` / `:445`. |
| `routing::post` | `axum/src/extract/multipart.rs:381,404,420,448` | multipart tests | grouped import at `multipart.rs:362` -> same generated function binding. |
| `routing::post` | `axum/src/routing/method_routing.rs:1448,1660` | `merge`; `merge_accessing_state` | same-module test visibility through `use super::*` at `method_routing.rs:1391` -> generated function binding. |
| `routing::post` | `axum/src/routing/tests/mod.rs:88,624,666,744,745,746,772,792,812,838,842,844,899,1071,1162` | routing tests | grouped `crate::routing::{..., post, ...}` import at `routing/tests/mod.rs:8-11` -> generated function binding. |

## Re-Exported Body Constructor Fanout

| Target | Callsites | Evidence chain |
| --- | --- | --- |
| `Body::empty` | `axum-core/src/response/into_response.rs:128,163`; `axum-core/src/ext_traits/request.rs:346,364,377,390` | direct `axum_core::body::Body` path -> `axum-core/src/body.rs:52`. |
| `Body::empty` | `axum/src/form.rs:158`; `extract/query.rs:106`; `extract/raw_form.rs:65`; `extract/ws.rs:394,400,1129,1191`; `serve/mod.rs:799`; `middleware/from_fn.rs:411`; `routing/route.rs:161,174`; `routing/method_routing.rs:1700`; `routing/tests/get_to_head.rs:25,59`; `routing/tests/merge.rs:198,204`; `routing/tests/mod.rs:228,1133,1151` | local/imported `Body` -> axum re-export `axum/src/body/mod.rs:10` or direct `axum_core::body::Body` import -> `axum-core/src/body.rs:52`. |

## High-Fanout Test Helper Matrix

`TestClient::new` has 172 selected-member text callsites. The full fanout is useful for fixture coverage, but a strict DB oracle should be generated from parser facts because owner recovery by hand is noisy.

| Target | Exact callsite fanout | Evidence chain |
| --- | --- | --- |
| `TestClient::new` | `axum/src/form.rs:262`; `json.rs:250,266,281,301,320,355`; `extension.rs:228`; `response/sse.rs:714,756,793`; `response/mod.rs:529`; `middleware/from_extractor.rs:351`; `middleware/map_request.rs:412,432`; `middleware/map_response.rs:357`; `extract/query.rs:158`; `extract/connect_info.rs:386`; `extract/multipart.rs:383,423,449`; `extract/mod.rs:103`; `extract/matched_path.rs:162,178,197,217,237,254,271,291,312,326,346,361,374,394`; `extract/nested_path.rs:136,154,172,190,205,224`; `extract/path/mod.rs:619,632,645,664,687,700,716,732,751,784,798,822,854,912,946,974,989,1010,1034`; `handler/mod.rs:418,443`; `routing/tests/nest.rs:41,65,135,159,182,193,210,229,280,298,309,328,371,408,431,489`; `routing/tests/merge.rs:14,63,81,85,96,116,136,150,162,179,208,234,267,301,345,379`; `routing/tests/handle_error.rs:25,42,60,76,90`; `routing/tests/fallback.rs:10,25,40,53,69,89,101,118,134,150,171,190,207,221,241,261,280,299,314,325,338,359,377,389,402`; `routing/tests/mod.rs:90,118,150,188,217,233,242,282,307,323,339,352,365,377,396,416,454,472,489,506,527,540,572,589,599,626,643,668,685,700,717,738,748,775,798,815,846,905,952,967,984,1027,1047,1073,1164,1201`; `axum-core/src/extract/request_parts.rs:193` | common chain: callsite -> visible `TestClient` import, direct module scope, or `test_helpers::*` -> re-export `axum/src/test_helpers/mod.rs:5-6` -> struct `test_client.rs:30` -> `new` function `test_client.rs:36`. |

## Constructors And Associated Calls

| Target | Callsites | Owner(s) | Evidence chain |
| --- | --- | --- | --- |
| `IntoServiceFuture::new` | `axum/src/handler/service.rs:174` | `impl Service for HandlerService::call` | `type Future = super::future::IntoServiceFuture<H::Future>` at `service.rs:155` -> call at `:174` -> macro-generated inherent `new` from `handler/future.rs:11-18` and `macros.rs:19-20`. |
| `try_downcast` in `axum-core` | `axum-core/src/body.rs:20,48,224,225` | `boxed`; `Body::new`; `test_try_downcast` | same-module unqualified call -> `axum-core/src/body.rs:23`. |
| `try_downcast` in `axum` | `axum/src/routing/mod.rs:205`; `axum/src/util.rs:114,115` | `Router<S>::route_service`; `test_try_downcast` | import `crate::util::try_downcast` at `routing/mod.rs:8-13` or same-module test -> `axum/src/util.rs:99`. |
| `BoxedIntoRoute` tuple constructor | `axum/src/boxed.rs:23,38,51` | `from_handler`; `map`; `Clone::clone` | explicit constructor or `Self(...)` inside impls -> tuple struct binding `boxed.rs:12`. |
| `Position::First` | `axum-macros/src/with_position.rs:92` | `WithPosition<I>::next` | `Position::First(item)` -> enum variant `with_position.rs:66`; pattern hits elsewhere are not constructor callsites. |
| `Json::from_bytes` | `axum/src/json.rs:112,128` | `FromRequest for Json<T>`; `OptionalFromRequest for Json<T>` | `Self::from_bytes` inside `Json<T>` impls -> inherent fn `json.rs:164`. |
| `HandleError::new` | `axum/src/error_handling/mod.rs:65`; `service_ext.rs:43` | `HandleErrorLayer::layer`; trait default `ServiceExt::handle_error` | local inherent associated function -> `error_handling/mod.rs:80`; service extension chain also has user `.handle_error(...)` at `routing/tests/handle_error.rs:86`. |
| `Handler::call` | `axum/src/handler/service.rs:171` | `impl Service for HandlerService::call` | import `super::Handler` at `service.rs:1`; bound `H: Handler<T, S>` at `:148`; call `Handler::call(...)` -> trait method `handler/mod.rs:153`. |

## Receiver And Method Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `self.extract_with_state` | `axum-core/src/ext_traits/request.rs:268` | `RequestExt::extract` | impl `RequestExt for Request` at `request.rs:262`; `Request` alias at `extract/mod.rs:29`; same impl method `extract_with_state` at `request.rs:271`; trait declaration `:122`. |
| `self.inner.poll_ready` | `axum/src/extension.rs:180` | `AddExtension::poll_ready` | field `inner: S` at `extension.rs:164-166`; impl bound `S: Service<Request<ResBody>>` at `:171`; imported `tower_service::Service` at `:12`; external trait method. |
| `Router::new` / `router.clone` | `Router::new` at `axum/src/serve/mod.rs:561`; `router.clone` at `:574,575,577,581,585,590,596,602` | `if_it_compiles_it_works` | typed local `let router: Router = Router::new()` -> `Router` import `serve/mod.rs:525` -> re-export `lib.rs:470` -> struct `routing/mod.rs:86` -> `Router::new` `:162`; clone target `Clone for Router` at `routing/mod.rs:90`. |
| local `req.extensions_mut` | `axum-core/src/ext_traits/request.rs:302` | `RequestExt::extract_parts_with_state` | `let mut req = Request::new(())` at `:297`; `Request` alias to `http::Request` at `extract/mod.rs:29`; external `http::Request::extensions_mut`. |
| parameter `req.extensions_mut` | `axum/src/extension.rs:184` | `AddExtension::call` | parameter `mut req: Request<ResBody>` at `:183`; `Request` imported from `http` at `:7`; external method. |
| `self.0.size_hint` | `axum-core/src/body.rs:127` | `impl http_body::Body for Body::size_hint` | tuple field `Body(BoxBody)` at `:39`; `BoxBody` alias at `:13`; external `http_body::Body` trait method. |
| path-call result receiver chain | `axum/src/middleware/from_fn.rs:411` | test `basic` | `Request` alias from `axum_core::extract` at `:1` -> external `http::Request::builder` / builder chain; nested `Body::empty` is local at `axum-core/src/body.rs:52`. |
| method-call result receiver | `axum/src/routing/route.rs:51` | `Route::oneshot_inner` | `Route<E>(BoxCloneSyncService<...>)` at `:31`; imports `BoxCloneSyncService`, `Oneshot`, `ServiceExt` at `:20-22`; external tower trait methods. |
| await result receiver | `axum/src/test_helpers/test_client.rs:134` | `RequestBuilder::into_future` | field `builder: reqwest::RequestBuilder` at `:90-92`; `.send()` external reqwest method; `.await.unwrap()` external result handling. |
| turbofish method call | `axum-core/src/ext_traits/request_parts.rs:164` | test `extract_with_state` | `parts: http::request::Parts` from `Request::new(()).into_parts()` at `:159`; impl `RequestPartsExt for Parts` at `:117`; method impl at `:125`; trait decl `:108`. |
| shadowed callable `get` | `axum/src/routing/tests/mod.rs:423,424,425,426,427,429,430,431,432,433,434` | test `what_matches_wildcard` | module imports routing `get` at `:8-10`, but local `let get = |path| ...` at `:418` shadows it; calls target local closure, not `routing::get`. |

## Trait And Body-Owner Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `E::from_request` | `axum-core/src/ext_traits/request.rs:279` | `RequestExt::extract_with_state` | trait `FromRequest` at `extract/mod.rs:79`; method `:85`; bound `E: FromRequest<S, M>` at `request.rs:276`; concrete impl remains type-parameter dependent. |
| `E::from_request_parts` | `axum-core/src/ext_traits/request.rs:305`; `ext_traits/request_parts.rs:133` | request and request-parts extraction helpers | trait `FromRequestParts` at `extract/mod.rs:53`; method `:59`; bound at call owner -> trait-associated dispatch. |
| handler macro `$ty::from_request_parts` | `axum/src/handler/mod.rs:242` | generated `Handler::call`, async block starts `:240` | bound `$ty: FromRequestParts<S> + Send` at `:233` -> trait method. |
| handler macro `$last::from_request` | `axum/src/handler/mod.rs:250` | generated `Handler::call`, async block starts `:240` | bound `$last: FromRequest<S, M> + Send` at `:234` -> trait method. |
| `FromRequest` ViaParts blanket inner call | `axum-core/src/extract/mod.rs:103` | blanket impl method body, async block | marker `private::ViaParts` at `:31`; blanket impl `:91`; bound `T: FromRequestParts<S>` at `:94`; call `Self::from_request_parts`. |
| `FromRef::from_ref` bounded calls | `axum-core/src/ext_traits/mod.rs:25,45`; `axum/src/extract/state.rs:309`; `middleware/from_extractor.rs:328` | state extraction helpers and tests | trait `FromRef` at `extract/from_ref.rs:13`; method `:15`; blanket impl `:18`; user impl may apply unless output equals input type. |
| `ServiceExt::handle_error` user call | `axum/src/routing/tests/handle_error.rs:86` | `handler_service_ext` | `.handle_error(...)` -> trait default `service_ext.rs:42` -> `HandleError::new` call `:43` -> inherent fn `error_handling/mod.rs:80`. |
| `Self::accept(self).await` | `axum/src/serve/listener.rs:41,61` | `Listener for TcpListener::accept`; `Listener for UnixListener::accept` | trait item `listener.rs:29`; impl self types at `:35` and `:55`; `Self::accept` targets external tokio listener inherent method, not recursive trait call. |
| dyn `Future::poll` | `axum/src/error_handling/mod.rs:251` | `HandleErrorFuture::poll` | field type `Pin<Box<dyn Future<...>>>` at `:240`; dispatch to trait-object `Future::poll`; concrete runtime future unresolved. |
| `<dyn Any>::downcast_mut` | `axum-core/src/body.rs:29`; `axum/src/util.rs:105` | `try_downcast` helpers | external `std::any::Any` trait-object associated call. |
| const initializer call owner | `axum/src/extract/ws.rs:382,384`; `routing/route.rs:202` | const initializer bodies | enclosing functions are not call owners; `HeaderValue::from_static(...)` / response value construction should be `CallBodyOwnerId::Const`. |
| closure body boundary | `axum-macros/src/from_ref.rs:23` | closure inside `from_ref::expand` | closure call to `expand_field` should be nested-owner owned once closures are modeled; target fn `from_ref.rs:29`. |
| async block boundary | `axum/src/handler/mod.rs:217,240` | handler `call` async blocks | calls inside async blocks should not be flattened into outer function owner once nested async owners are modeled. |

## Dynamic And Unsupported Callable Oracles

| Case | Callsites | Owner | Evidence chain |
| --- | --- | --- | --- |
| `(self.into_route)(...)` handler | `axum/src/boxed.rs:85` | `MakeErasedHandler::into_route` | field `into_route: fn(H, S) -> Route` at `boxed.rs:72`; initialized in `BoxedIntoRoute::from_handler` at `:23-25` with non-capturing closure; structural dynamic call, no named callee without field/closure proof. |
| `(self.into_route)(...)` router | `axum/src/boxed.rs:120` | `MakeErasedRouter::into_route` | field `into_route: fn(Router<S>, S) -> Route` at `boxed.rs:108`; construction site not found in selected `axum/src`; unsupported/fail-closed. |
| `(self.layer)(...)` | `axum/src/boxed.rs:159,163` | `Map::into_route`; `Map::call_with_state` | field `layer: Box<dyn LayerFn<E, E2>>` at `boxed.rs:142`; boxed from `BoxedIntoRoute::map(self, f)` at `:31-40`; `LayerFn` blanket impl `:167-173`; dynamic trait-object callable. |
| `(self.tap_fn)(...)` | `axum/src/serve/listener.rs:236` | `TapIo<L, F>::accept` | field `tap_fn: F` at `listener.rs:212`; set by `ListenerExt::tap_io(self, tap_fn)` at `:116-123`; bound `F: FnMut(&mut L::Io)` at `:118,229`; example closure passed at `serve/mod.rs:566`. |
| `and_then(f)` | `axum-macros/src/lib.rs:724` | `expand_with` | parameter `f: F` at `:718`; bound `F: FnOnce(I) -> syn::Result<K>` at `:720`; callback passed into external `and_then`, no concrete callee in this owner. |
| `expand_with(item, from_ref::expand)` | `axum-macros/src/lib.rs:715` | `derive_from_ref` | function item `from_ref::expand` at `from_ref.rs:11` -> parameter `f` in `expand_with`; interprocedural callback proof needed before resolving `and_then(f)`. |
| other `expand_with(...)` callers | `axum-macros/src/lib.rs:377,426,665` | derive macro entrypoints | closure literals passed into `expand_with`; closure bodies call `from_request::expand` or `typed_path::expand`; invocation through `and_then(f)` remains unsupported. |
| direct callable parameter `f(attr,input)` | `axum-macros/src/lib.rs:737` | `expand_attr_with` | parameter `f: F` at `:727`; bound `F: FnOnce(A, I) -> K` at `:729`; structural dynamic call, target intentionally unknown. |
| `expand_attr_with(...)` callers | `axum-macros/src/lib.rs:581,637,655` | `debug_handler`; `debug_middleware`; `__private_axum_test` | first two pass closure literals; `:655` passes function item `axum_test::expand` at `axum_test.rs:5`; resolving `f(...)` requires interprocedural callback proof. |
| IIFE closure expression | `axum-macros/src/lib.rs:734-738`; `from_request/mod.rs:200-203` | `expand_attr_with`; `from_request::expand` | closure literal immediately invoked; structural dynamic call should persist without fake named target. |

## Fallback Fixture Oracle Matrix

These rows extend the axum matrix with registered source-pinned call-graph
fixtures for the fallback crates. The DB assertions live beside the axum cases
in `crates/ploke-db/tests/unit/call_graph_fixture_queries/real_target_matrix/fallback.rs`.

| Fixture crate | Target / case | Callsites | Definition or binding | Evidence chain |
| --- | --- | --- | --- | --- |
| chrono | `MappedLocalTime::Single` alias constructor | `src/offset/mod.rs:143,156,178,193,216,238,260,468,502,535` plus one additional projected alias constructor row | alias `src/offset/mod.rs:77`; enum `LocalResult` at `:81`; variant `Single(T)` at `:83` | `corpus_chrono_call_graph` currently projects 11 targetless `MappedLocalTime::Single` rows with `Unresolved` status; no `LocalResult::Single` traversal edge is fabricated. |
| chrono | `DateTime...?.naive_utc()` try receiver | `src/format/parsed.rs:836,953`; macro-equivalent `src/naive/datetime/mod.rs:140,157,174,195` | `DateTime<Tz>::naive_utc` at `src/datetime/mod.rs:563`; constructors return `Option<Self>` at `:768,:803` | `corpus_chrono_call_graph` currently projects two `TryResult` receiver method rows for `naive_utc` with `Unsupported` status and no traversal edge to the method. |
| chrono | guarded match arm method guard | `src/format/strftime.rs:635` | `StrftimeItems.queue` field `src/format/strftime.rs:198` | `corpus_chrono_call_graph` currently projects one targetless `SelfField(["queue"]).is_empty()` row with `Unsupported` status. |
| memchr | arbitrary-expression dynamic callee | macro source `src/arch/x86_64/memchr.rs:153`; macro instantiations at `:180,203,227,252,278,305,326` | macro-local aliases `type Fn = *mut ()`, `type RealFn = $fnty` at `:72-73`; static pointer `FN` at `:74` | `corpus_memchr_call_graph` currently does not project the `core::mem::transmute::<Fn, RealFn>(fun)(...)` path or outer dynamic call, so the test asserts absence rather than guessed callees. |
| memchr | function-pointer field call | `src/memmem/searcher.rs:222,718` | `Searcher.call` at `:34`; alias `SearcherKindFn` at `:273`; `Prefilter.call` at `:605`; alias `PrefilterKindFn` at `:774` | `corpus_memchr_call_graph` currently projects two targetless dynamic rows owned by methods named `find`, with argument counts 4 and 2. |
| memchr | callable trait object field | `src/tests/substring/mod.rs:94,110` | `Runner.fwd` at `:67-68`; `Runner.rev` at `:70-71`; setters box closures at `:137,153` | `corpus_memchr_call_graph` currently does not project these boxed `dyn FnMut` local-binding calls as dynamic rows under `Runner::run`. |
| generic-array | guarded match arm | `src/lib.rs:1241,1243,1278,1280` | `ArrayLength` bound at `src/lib.rs:245`; `LengthError` at `:1197` | `corpus_generic_array_call_graph` currently does not project `iter.size_hint()` for these guarded match-arm checks. |

## Not-Fully-Oracled Items

| Item | Current state | Required next step before strict DB test |
| --- | --- | --- |
| `TestClient::new` owner-per-callsite matrix | Exact 172 file/line callsites are listed. `ploke-db` now asserts the current 167 projected structural rows by module fanout from parser facts, with all rows unsupported and targetless. The five missing source rows are the three `extract/multipart.rs` rows plus one `routing/tests/mod.rs` row and one `routing/tests/nest.rs` row. | Generate exact owner/source rows before asserting full 172-callsite parity; keep the current 167-row module-fanout assertion as the fail-closed DB contract until projection coverage changes. |
| Markdown/doc-comment examples | Excluded from oracle rows. | Keep excluded unless parser fixture intentionally ingests docs as Rust examples. |
| Dynamic callback targets | Source binding chains are recorded, but concrete targets remain intentionally unresolved. | Tests should assert structural dynamic call plus `Unsupported`/fail-closed status, not guessed callees. |
