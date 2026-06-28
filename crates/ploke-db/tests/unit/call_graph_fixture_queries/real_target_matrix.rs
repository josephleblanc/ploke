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
//! | Import/path function call | `axum-macros/src/typed_path.rs:23` calls `crate::attr_parsing::parse_attrs(...)` | currently unresolved in the corpus fixture; the structural row is asserted as a targetless gap. |
//! | Routing helper paths | `axum/src/routing/mod.rs:{410,430}` and `routing/tests/mod.rs:{56,59}` call `take_route_or_internal_error` | currently targetless in the corpus fixture. |
//! | External roots | `axum/src/json.rs:184` and `axum/src/response/sse.rs:449` call dependency/std roots | external rows remain targetless and do not become traversal edges. |
//! | Re-exported body constructor | `axum-core/src/response/into_response.rs:{128,163}` call `Body::empty()` | current resolved subset traverses two one-hop edges; broader fanout remains an import/re-export gap. |
//! | Inherent associated function | `axum/src/json.rs:{112,128}` call `Self::from_bytes(...)` | currently unsupported: the structural rows are visible but do not yet traverse to `Json::from_bytes`. |
//! | Trait associated function | `axum/src/handler/service.rs:171` calls `Handler::call(...)` | currently unresolved: the structural row is visible but does not yet traverse to the trait method. |
//! | Tuple-struct constructor | `axum/src/boxed.rs:38` calls `BoxedIntoRoute(...)` | explicit tuple-struct constructor resolves to the `BoxedIntoRoute` struct in one call edge. |
//! | Inherent constructor | `axum/src/error_handling/mod.rs:65` calls `HandleError::new(...)` | extension methods resolve to the local inherent constructor in one call edge. |
//! | Generated constructor | `axum/src/handler/service.rs:174` calls `IntoServiceFuture::new(...)` | currently unresolved in the corpus fixture. |
//! | Same-impl self methods | `axum-core/src/ext_traits/{request.rs:268,request_parts.rs:122}` call `self.extract_with_state(&())` | owner traversal resolves both method calls to their same-impl `extract_with_state`. |
//! | Proc-macro body calls | `axum-macros/src/lib.rs:{377,426,665,715}` call `expand_with(...)` | currently unsupported: proc-macro function bodies are not visited for call sites. |
//! | Closure body call | `axum-macros/src/from_ref.rs:23` calls `expand_field(...)` inside a closure | currently unsupported: no call site targets `expand_field`. |
//! | Dynamic callable fields | `axum/src/boxed.rs:{85,159}` call function-pointer / trait-object fields | currently unsupported: visible dynamic call sites remain targetless blockers. |

mod associated;
mod common;
mod paths;
mod receivers;
mod unsupported;
