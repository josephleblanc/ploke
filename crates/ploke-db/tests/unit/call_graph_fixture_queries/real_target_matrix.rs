//! Real-target call graph contracts over the pinned axum corpus fixture.
//!
//! These tests use the immutable `corpus_axum_call_graph` backup, not parser
//! fixture crates. Each row starts from source inspected in the pinned checkout:
//!
//! ```text
//! github:tokio-rs/axum@a3446d68bc03d61fb8e7513052bad2825d0c0db1
//! selected members: axum, axum-core, axum-macros
//! ```
//!
//! Source-oracle references:
//! - `docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-case-matrix.md`
//! - `docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md`
//!
//! Coverage table for this consolidated batch:
//!
//! | Bucket | Source ground truth | DB contract |
//! | --- | --- | --- |
//! | Regular helper callers | `axum-macros/src/lib.rs:{724,739}` call root `expand(...)` | `callers_for_target`, `call_sites_for_target`, and owner traversal resolve both helper callers. |
//! | UI test helper callers | `axum-macros/src/{debug_handler.rs,typed_path.rs,from_ref.rs,from_request/mod.rs}` call `crate::run_ui_tests(...)` | target-centered callers and context expansion traverse the five real helper edges. |
//! | Import/path function call | `axum-macros/src/typed_path.rs:23` calls `crate::attr_parsing::parse_attrs(...)` | explicit file-module paths and imported `parse_attrs` rows traverse to the local helper. |
//! | Routing helper paths | `axum/src/routing/mod.rs:{410,430}` and `routing/tests/mod.rs:{56,59}` call `take_route_or_internal_error` | currently targetless in the corpus fixture. |
//! | External roots | `axum/src/json.rs:184` and `axum/src/response/sse.rs:449` call dependency/std roots | external rows remain targetless and do not become traversal edges. |
//! | Re-exported body constructor | `axum-core/src/body.rs:{110,116}` call `Self::empty()` and response conversion rows call `Body::empty()` | current resolved subset traverses four one-hop edges; projected targetless fanout is pinned by source file and remains non-traversable. |
//! | Inherent associated function | `axum/src/json.rs:{112,128}` call `Self::from_bytes(...)` | both trait-impl `Self::from_bytes` rows traverse to the inherent `Json::from_bytes` method. |
//! | Trait method path call | `axum/src/handler/service.rs:171` calls `Handler::call(...)` | path-style trait method dispatch resolves to the trait method binding in one call edge. |
//! | Tuple-struct constructor | `axum/src/boxed.rs:{23,38,51}` calls `BoxedIntoRoute(...)` / `Self(...)` | explicit tuple-struct constructor resolves to the `BoxedIntoRoute` struct in one call edge; `Self(...)` rows remain unsupported and targetless. |
//! | Enum variant constructor | `axum-macros/src/with_position.rs:92` calls `Position::First(item)` | local enum-variant constructor resolves to the `Position::First` variant in one call edge. |
//! | Inherent constructor | `axum/src/error_handling/mod.rs:65` calls `HandleError::new(...)` | extension methods resolve to the local inherent constructor in one call edge. |
//! | Generated constructor | `axum/src/handler/service.rs:174` calls `IntoServiceFuture::new(...)` | currently unresolved: the row is visible, but macro-generated concrete `opaque_future!` items are not modeled as traversal targets. |
//! | High-fanout test helper | `axum/src/test_helpers/test_client.rs:36` defines `TestClient::new`; the oracle lists selected-member callsites | 168 structural `TestClient::new` rows are projected; 167 traverse through nested/direct re-export imports, direct imports, and inherited parent glob imports; the remaining axum-core row is unsupported and targetless. |
//! | Same-impl self methods | `axum-core/src/ext_traits/{request.rs:268,request_parts.rs:122}` call `self.extract_with_state(&())` | owner traversal resolves both method calls to their same-impl `extract_with_state`. |
//! | Trait associated extraction paths | `axum-core/src/ext_traits/{request.rs:279,305}`, `request_parts.rs:133`, and `extract/mod.rs:{115,127}` call `E::from_request*` / `T::from_request*` | bounded type-parameter associated paths traverse to the `FromRequest` / `FromRequestParts` trait method bindings in one call edge each. |
//! | Bounded `FromRef` paths | `axum-core/src/ext_traits/mod.rs:{25,45}`, `axum/src/extract/state.rs:309`, and `middleware/from_extractor.rs:328` call `*::from_ref(...)` | same-crate axum-core bounds traverse to `FromRef::from_ref`; the top-level axum dependency-root bound also traverses through parsed workspace dependency proof; the nested middleware local-impl row remains targetless under a `local_impl_method:from_request_parts` `LocalItem` owner. |
//! | Listener `Self::accept` paths | `axum/src/serve/listener.rs:{41,61}` call `Self::accept(self).await` | external and targetless for the projected line-41 row; it must not be modeled as recursive trait dispatch, while line 61 remains a body-owner completeness gap. |
//! | `HeaderValue::from_static` external paths | response conversion bodies plus local const initializer examples | response-body rows are external and targetless; the route local const initializer row is an external targetless `LocalItem` owner and does not leak into its enclosing owner; websocket local const rows remain absent in this fixture. |
//! | Generated handler functions | `routing::post` template/invocation plus JSON/multipart/routing tests | no generated `post` function is exposed yet; 22 structural `post` rows are unsupported, targetless, and file-bucketed. |
//! | Handler macro extraction and async body paths | `axum/src/handler/mod.rs:{217,240,242,250}` has nested async-body calls plus `$ty::from_request_parts` / `$last::from_request` | concrete `Handler::call` async-block body rows at `:217` are projected on an `AsyncBlock` owner and remain targetless; macro-template associated paths are not projected yet. |
//! | Receiver forwarding gaps and exact receiver positives | `axum/src/extension.rs:180`, `routing/route.rs:51`, `boxed.rs:134`, `routing/mod.rs:673`, `serve/mod.rs` router clones, and `middleware/from_fn.rs:411` exercise field/result/typed receivers | unsupported receiver shapes remain visible and targetless; exact local `Router::clone` receiver rows traverse; `Router::new` currently has 309 caller rows and 203 incoming expansion candidates. |
//! | Await and trait-object receivers | `test_helpers/test_client.rs:134`, `serve/listener.rs:143`, `error_handling/mod.rs:251`, and `try_downcast` helpers use await/dyn receiver calls | awaited/dyn Future receiver rows remain targetless; qualified `<dyn Any>::downcast_mut` rows project as external targetless path frontiers. |
//! | Proc-macro body calls | `axum-macros/src/lib.rs:{377,426,665,715}` call `expand_with(...)` | proc-macro owners traverse to `expand_with` through one resolved edge each. |
//! | Closure body call | `axum-macros/src/from_ref.rs:23` calls `expand_field(...)` inside a closure | nested closure-owner row traverses to `expand_field` without flattening into `from_ref::expand`. |
//! | Dynamic callable fields | `axum/src/boxed.rs:{85,120,159}` and `serve/listener.rs:236` call function-pointer / trait-object fields | currently unsupported: visible dynamic call sites remain targetless blockers. |
//! | Macro callback/IIFE calls | `axum-macros/src/lib.rs:{581,637,655,715,724,734-738}` and `from_request/mod.rs:200-203` cover callback arguments, `and_then(f)`, and IIFEs | active proc-macro callback helper rows traverse; callback receiver and IIFE dynamic rows stay targetless; inner closure-body callback invocation remains absent. |
//! | Shadowed local callable | `axum/src/routing/tests/mod.rs:{418,423-434}` shadows imported `get` with a closure | only the two setup `routing::get` rows are projected; closure calls inside assertion macros are not fabricated as routing edges. |
//! | Fallback chrono corpus | `MappedLocalTime::Single`, try receiver `.naive_utc()`, and `self.queue.is_empty()` in chrono | registered `corpus_chrono_call_graph` fixture asserts resolved alias constructor rows and targetless receiver rows. |
//! | Fallback memchr corpus | arbitrary-expression dynamic callee, function-pointer field calls, and boxed callable fields in memchr | registered `corpus_memchr_call_graph` fixture asserts dynamic field rows and absent unsupported shapes. |
//! | Fallback generic-array corpus | guarded `iter.size_hint()` match-arm checks in generic-array | registered `corpus_generic_array_call_graph` fixture asserts the current absent projection gap. |

mod associated;
mod common;
mod fallback;
mod multi_hop;
mod paths;
mod proc_macros;
mod receivers;
mod source_lines;
mod trait_body;
mod traversal;
mod unsupported;
mod usage_questions;
mod workspace;
