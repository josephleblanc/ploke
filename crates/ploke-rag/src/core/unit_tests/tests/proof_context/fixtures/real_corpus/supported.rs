use super::super::super::super::*;
use super::helpers::{
    ProofCase, assert_case_rows, axum_db, function_id, method_id_by_name_and_body, project_case,
    struct_id, trait_method_id, variant_id,
};

#[tokio::test]
async fn proof_context_exact_preserves_axum_supported_target_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = axum_db()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // This proof batch mirrors the supported real-corpus RAG exact-context
    // matrix. Each case pins the expected one-hop edge count and then verifies
    // that target-centered proof projection exposes the call_site, call_edge,
    // and call_resolution fact family for every DB caller-site identity.
    let cases = vec![
        ProofCase {
            // axum-core/src/body.rs:52 defines `Body::empty`.
            // axum-core/src/body.rs:{110,116} call `Self::empty()`, and
            // axum-core/src/response/into_response.rs plus
            // ext_traits/request.rs call `Body::empty()`. Four axum
            // direct parsed-workspace import rows and eleven local
            // re-exported, inherited, closure, and local-item workspace import
            // rows also reach the target.
            label: "axum-core Body::empty current resolved subset",
            target: method_id_by_name_and_body(&db, "empty", "Empty::new()")?,
            edges: 23,
        },
        ProofCase {
            // axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
            // typed_path.rs:23 calls the crate-qualified path; from_ref.rs:30
            // and from_request/mod.rs call the imported helper, including
            // three closure-owned executable rows.
            label: "axum-macros parse_attrs path/import callers",
            target: function_id(&db, &["crate", "attr_parsing"], "parse_attrs")?,
            edges: 11,
        },
        ProofCase {
            // axum-macros/src/from_ref.rs:29 defines `expand_field`.
            // from_ref.rs:23 calls it from a closure body owned by a nested
            // closure executable, not by the enclosing `from_ref::expand`.
            label: "axum-macros closure-owned expand_field caller",
            target: function_id(&db, &["crate", "from_ref"], "expand_field")?,
            edges: 1,
        },
        ProofCase {
            // axum/src/json.rs:164 defines `Json::from_bytes`.
            // axum/src/json.rs:{112,128} call `Self::from_bytes(&bytes)`.
            label: "axum Json::from_bytes associated callers",
            target: method_id_by_name_and_body(
                &db,
                "from_bytes",
                "serde_json::Deserializer::from_slice(bytes)",
            )?,
            edges: 2,
        },
        ProofCase {
            // axum/src/boxed.rs:12 defines the tuple struct constructor.
            // axum/src/boxed.rs:{23,38,51} call `Self(...)`,
            // `BoxedIntoRoute(...)`, and `Self(...)`.
            label: "axum BoxedIntoRoute explicit tuple constructor",
            target: struct_id(&db, "BoxedIntoRoute")?,
            edges: 3,
        },
        ProofCase {
            // axum/src/handler/mod.rs:153 declares `Handler::call`.
            // axum/src/handler/service.rs:171 calls `Handler::call(...)`
            // through the `H: Handler<T, S>` trait-method binding.
            label: "axum Handler::call trait-method binding",
            target: trait_method_id(&db, "Handler", "call")?,
            edges: 1,
        },
        ProofCase {
            // axum-macros/src/lib.rs:797 defines `run_ui_tests`.
            // debug_handler.rs:{885,890}, typed_path.rs:443, from_ref.rs:104,
            // and from_request/mod.rs:1050 call `crate::run_ui_tests(...)`.
            label: "axum-macros run_ui_tests helper callers",
            target: function_id(&db, &["crate"], "run_ui_tests")?,
            edges: 5,
        },
        ProofCase {
            // axum-core/src/body.rs:26 defines `try_downcast`; body.rs
            // currently resolves the two same-module non-macro caller rows.
            label: "axum-core try_downcast resolved subset",
            target: function_id(&db, &["crate", "body"], "try_downcast")?,
            edges: 2,
        },
        ProofCase {
            // axum/src/util.rs:99 defines `try_downcast`; routing/mod.rs:205
            // calls the imported helper.
            label: "axum try_downcast resolved subset",
            target: function_id(&db, &["crate", "util"], "try_downcast")?,
            edges: 1,
        },
        ProofCase {
            // axum-macros/src/with_position.rs:66 defines `Position::First`.
            // with_position.rs:92 calls `Position::First(item)`.
            label: "axum-macros Position::First variant constructor",
            target: variant_id(&db, "Position", "First")?,
            edges: 1,
        },
        ProofCase {
            // axum/src/error_handling/mod.rs:80 defines `HandleError::new`.
            // error_handling/mod.rs:65 and service_ext.rs:43 call it.
            label: "axum HandleError::new constructor callers",
            target: method_id_by_name_and_body(&db, "new", "Self { inner, f, _extractor")?,
            edges: 2,
        },
        ProofCase {
            // axum-core/src/ext_traits/request.rs:268 calls
            // `self.extract_with_state(&())`; the target body calls
            // `E::from_request(self, state)`.
            label: "axum-core RequestExt extract self-call",
            target: method_id_by_name_and_body(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
            )?,
            edges: 1,
        },
        ProofCase {
            // axum-core/src/ext_traits/request_parts.rs:122 calls
            // `self.extract_with_state(&())`; the target body calls
            // `E::from_request_parts(self, state)`.
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            edges: 1,
        },
        ProofCase {
            // axum-core/src/extract/mod.rs:85 declares FromRequest.
            // ext_traits/request.rs:279 calls `E::from_request(...)`;
            // extract/mod.rs:127 calls `T::from_request(...)`.
            label: "axum-core FromRequest::from_request trait paths",
            target: trait_method_id(&db, "FromRequest", "from_request")?,
            edges: 2,
        },
        ProofCase {
            // axum-core/src/extract/mod.rs:59 declares FromRequestParts.
            // ext_traits/request.rs:305 and request_parts.rs:133 call `E::`;
            // extract/mod.rs:115 calls `T::from_request_parts(...)`;
            // extract/mod.rs:103 calls `Self::from_request_parts(...)` from a
            // nested async-block owner.
            label: "axum-core FromRequestParts::from_request_parts trait paths",
            target: trait_method_id(&db, "FromRequestParts", "from_request_parts")?,
            edges: 4,
        },
        ProofCase {
            // axum-core/src/extract/from_ref.rs:15 declares FromRef.
            // ext_traits/mod.rs:{25,45} call same-crate bounded associated
            // paths `InnerState::from_ref` and `String::from_ref`; axum
            // extract/state.rs:309 reaches the same target through parsed
            // workspace dependency proof for `axum_core::extract::FromRef`;
            // middleware/from_extractor.rs:328 reaches it from a nested local
            // impl method through `Secret: FromRef<S>`.
            label: "axum-core FromRef::from_ref bounded paths",
            target: trait_method_id(&db, "FromRef", "from_ref")?,
            edges: 4,
        },
        ProofCase {
            // axum/src/routing/mod.rs:162 defines `Router::new`.
            // The current fixture resolves 308 `Router::new` rows plus
            // routing/mod.rs:109 `Self::new()`,
            // axum-core/src/extract/request_parts.rs:193 `Router::new()`, and
            // method_routing.rs:1494 `crate::Router::new()`.
            label: "axum Router::new resolved fanout",
            target: method_id_by_name_and_body(&db, "new", "default_fallback: true")?,
            edges: 310,
        },
    ];

    let projected = cases
        .into_iter()
        .map(|case| project_case(&db, case))
        .collect::<Result<Vec<_>, _>>()?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum target proof facts should enable RAG proof context"
    );
    for case in &projected {
        assert_case_rows(&rag, case)?;
    }

    Ok(())
}
