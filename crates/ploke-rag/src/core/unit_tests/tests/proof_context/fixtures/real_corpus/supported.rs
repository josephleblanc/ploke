use super::super::super::super::*;
use super::helpers::{
    ProofCase, assert_case_rows, axum_db, function_id, method_id_by_name_and_body, project_case,
    struct_id, trait_method_id, variant_id,
};
use ploke_db::ProofGraphStore;

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
            // axum-core/src/ext_traits/request_parts.rs:164 calls
            // `parts.extract_with_state::<State<String>, String>(&state)`
            // through the `http::Request::into_parts` tuple-return summary.
            // axum-core/src/ext_traits/request_parts.rs:186 calls
            // `parts.extract_with_state(state)` through direct `&mut Parts`
            // receiver proof.
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            edges: 3,
        },
        ProofCase {
            // axum-core/src/extract/mod.rs:85 declares FromRequest.
            // ext_traits/request.rs:279 calls `E::from_request(...)`;
            // extract/mod.rs:127 calls `T::from_request(...)`; generated
            // handler arity rows in axum/src/handler/mod.rs and tuple
            // extractor rows in axum-core/src/extract/tuple.rs call
            // `Tn::from_request(...)` for arities 1 through 16.
            label: "axum-core FromRequest::from_request trait paths",
            target: trait_method_id(&db, "FromRequest", "from_request")?,
            edges: 34,
        },
        ProofCase {
            // axum-core/src/extract/mod.rs:59 declares FromRequestParts.
            // ext_traits/request.rs:305 and request_parts.rs:133 call `E::`;
            // extract/mod.rs:115 calls `T::from_request_parts(...)`;
            // extract/mod.rs:103 calls `Self::from_request_parts(...)` from a
            // nested async-block owner. Generated Handler and HandleError
            // service impls plus generated tuple extractor impls add
            // extractor-prefix rows for `Tn::`.
            label: "axum-core FromRequestParts::from_request_parts trait paths",
            target: trait_method_id(&db, "FromRequestParts", "from_request_parts")?,
            edges: 517,
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
        ProofCase {
            // axum/src/test_helpers/test_client.rs:36 defines
            // `TestClient::new`. This high-fanout target includes the
            // axum-core/src/extract/request_parts.rs:193 workspace dependency
            // glob import through `axum::{test_helpers::*, Router}`.
            label: "axum TestClient::new resolved import fanout",
            target: method_id_by_name_and_body(&db, "new", "spawn_service(svc)")?,
            edges: 168,
        },
    ];

    let projected = cases
        .into_iter()
        .map(|case| project_case(&db, case))
        .collect::<Result<Vec<_>, _>>()?;
    attach_dependency_roots(&db, &projected)?;

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.proof_context_degraded(),
        "projected axum target proof facts should enable RAG proof context"
    );
    for case in &projected {
        assert_case_rows(&rag, case)?;
    }
    let from_ref = projected
        .iter()
        .find(|case| case.case.label == "axum-core FromRef::from_ref bounded paths")
        .expect("FromRef projected case");
    let rows = rag.exact_proof_context(from_ref.case.target)?;
    assert_dependency_roots(&rows, from_ref);
    let test_client = projected
        .iter()
        .find(|case| case.case.label == "axum TestClient::new resolved import fanout")
        .expect("TestClient::new projected case");
    let rows = rag.exact_proof_context(test_client.case.target)?;
    assert_test_client_dependency_root(&rows, test_client);
    let body_empty = projected
        .iter()
        .find(|case| case.case.label == "axum-core Body::empty current resolved subset")
        .expect("Body::empty projected case");
    let rows = rag.exact_proof_context(body_empty.case.target)?;
    assert_body_empty_dependency_root(&rows, body_empty);
    let router_new = projected
        .iter()
        .find(|case| case.case.label == "axum Router::new resolved fanout")
        .expect("Router::new projected case");
    let rows = rag.exact_proof_context(router_new.case.target)?;
    assert_router_new_dependency_root(&rows, router_new);

    Ok(())
}

fn attach_dependency_roots(
    db: &Database,
    projected: &[super::helpers::ProjectedCase],
) -> Result<(), Error> {
    let Some(case) = projected
        .iter()
        .find(|case| case.case.label == "axum-core FromRef::from_ref bounded paths")
    else {
        return Ok(());
    };

    let mut records = Vec::new();
    for caller in &case.callers {
        let site = caller.site.id.to_string();
        let source = db
            .proof_source_provenance(&site)?
            .unwrap_or_else(|| panic!("FromRef caller site {site} should have proof provenance"));
        if source.source_file.ends_with("axum/src/extract/state.rs")
            || source
                .source_file
                .ends_with("axum/src/middleware/from_extractor.rs")
        {
            records.push(ploke_test_utils::axum_dependency_record(
                super::helpers::AXUM_DOMAIN,
                caller.site.id,
                caller.site.owner_id,
                case.case.target,
            ));
        }
    }
    assert_eq!(
        records.len(),
        2,
        "FromRef proof context should admit both axum dependency-root callsites"
    );
    if let Some(case) = projected
        .iter()
        .find(|case| case.case.label == "axum TestClient::new resolved import fanout")
    {
        for caller in &case.callers {
            let site = caller.site.id.to_string();
            let source = db.proof_source_provenance(&site)?.unwrap_or_else(|| {
                panic!("TestClient::new caller site {site} should have proof provenance")
            });
            if source
                .source_file
                .ends_with("axum-core/src/extract/request_parts.rs")
            {
                records.push(ploke_test_utils::axum_test_client_dependency_record(
                    super::helpers::AXUM_DOMAIN,
                    caller.site.id,
                    caller.site.owner_id,
                    case.case.target,
                ));
            }
        }
    }
    if let Some(case) = projected
        .iter()
        .find(|case| case.case.label == "axum Router::new resolved fanout")
    {
        for caller in &case.callers {
            let site = caller.site.id.to_string();
            let source = db.proof_source_provenance(&site)?.unwrap_or_else(|| {
                panic!("Router::new caller site {site} should have proof provenance")
            });
            if source
                .source_file
                .ends_with("axum-core/src/extract/request_parts.rs")
            {
                records.push(ploke_test_utils::axum_router_new_dependency_record(
                    super::helpers::AXUM_DOMAIN,
                    caller.site.id,
                    caller.site.owner_id,
                    case.case.target,
                ));
            }
        }
    }
    if let Some(case) = projected
        .iter()
        .find(|case| case.case.label == "axum-core Body::empty current resolved subset")
    {
        for caller in &case.callers {
            let site = caller.site.id.to_string();
            let source = db.proof_source_provenance(&site)?.unwrap_or_else(|| {
                panic!("Body::empty caller site {site} should have proof provenance")
            });
            if source.source_file.ends_with("axum/src/form.rs") {
                records.push(ploke_test_utils::axum_body_empty_dependency_record(
                    super::helpers::AXUM_DOMAIN,
                    caller.site.id,
                    caller.site.owner_id,
                    case.case.target,
                ));
            } else if source.source_file.ends_with("axum/src/extract/raw_form.rs") {
                records.push(
                    ploke_test_utils::axum_body_empty_reexport_dependency_record(
                        super::helpers::AXUM_DOMAIN,
                        caller.site.id,
                        caller.site.owner_id,
                        case.case.target,
                    ),
                );
            }
        }
    }
    db.upsert_proof_fact_values(&records)?;

    Ok(())
}

fn assert_router_new_dependency_root(
    rows: &[ProofContextInfo],
    case: &super::helpers::ProjectedCase,
) {
    let target = case.case.target.to_string();
    let sites = case
        .callers
        .iter()
        .filter(|caller| {
            let path = caller
                .site
                .path
                .as_ref()
                .map(|path| path.iter().map(String::as_str).collect::<Vec<_>>());
            path.as_deref() == Some(&["Router", "new"][..])
        })
        .filter(|caller| {
            let site = caller.site.id.to_string();
            let owner = caller.site.owner_id.to_string();
            rows.iter().any(|row| {
                row.kind == "dependency_root"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.resolved_def_id.as_deref() == Some(target.as_str())
                    && row.target_name.as_deref() == Some("axum::routing::Router::new")
                    && row.status.as_deref() == Some("admitted")
            })
        })
        .count();
    assert_eq!(
        sites, 1,
        "RAG exact proof context should expose the axum-core Router::new dependency-root proof row: {rows:#?}"
    );
}

fn assert_body_empty_dependency_root(
    rows: &[ProofContextInfo],
    case: &super::helpers::ProjectedCase,
) {
    let target = case.case.target.to_string();
    let sites = case
        .callers
        .iter()
        .filter(|caller| {
            let path = caller
                .site
                .path
                .as_ref()
                .map(|path| path.iter().map(String::as_str).collect::<Vec<_>>());
            path.as_deref() == Some(&["Body", "empty"][..])
        })
        .filter(|caller| {
            let site = caller.site.id.to_string();
            let owner = caller.site.owner_id.to_string();
            rows.iter().any(|row| {
                row.kind == "dependency_root"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.resolved_def_id.as_deref() == Some(target.as_str())
                    && row.target_name.as_deref() == Some("axum_core::body::Body::empty")
                    && row.status.as_deref() == Some("admitted")
            })
        })
        .count();
    assert_eq!(
        sites, 2,
        "RAG exact proof context should expose the direct and re-exported Body::empty dependency-root proof rows: {rows:#?}"
    );
}

fn assert_test_client_dependency_root(
    rows: &[ProofContextInfo],
    case: &super::helpers::ProjectedCase,
) {
    let target = case.case.target.to_string();
    let sites = case
        .callers
        .iter()
        .filter(|caller| {
            let path = caller
                .site
                .path
                .as_ref()
                .map(|path| path.iter().map(String::as_str).collect::<Vec<_>>());
            path.as_deref() == Some(&["TestClient", "new"][..])
        })
        .filter(|caller| {
            let site = caller.site.id.to_string();
            let owner = caller.site.owner_id.to_string();
            rows.iter().any(|row| {
                row.kind == "dependency_root"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.resolved_def_id.as_deref() == Some(target.as_str())
                    && row.target_name.as_deref() == Some("axum::test_helpers::TestClient::new")
                    && row.status.as_deref() == Some("admitted")
            })
        })
        .count();
    assert_eq!(
        sites, 1,
        "RAG exact proof context should expose the axum-core TestClient::new dependency-root proof row: {rows:#?}"
    );
}

fn assert_dependency_roots(rows: &[ProofContextInfo], case: &super::helpers::ProjectedCase) {
    let target = case.case.target.to_string();
    let sites = case
        .callers
        .iter()
        .filter(|caller| {
            let path = caller
                .site
                .path
                .as_ref()
                .map(|path| path.iter().map(String::as_str).collect::<Vec<_>>());
            path.as_deref() == Some(&["InnerState", "from_ref"][..])
                || path.as_deref() == Some(&["Secret", "from_ref"][..])
        })
        .filter(|caller| {
            let site = caller.site.id.to_string();
            let owner = caller.site.owner_id.to_string();
            rows.iter().any(|row| {
                row.kind == "dependency_root"
                    && row.call_site_id.as_deref() == Some(site.as_str())
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.resolved_def_id.as_deref() == Some(target.as_str())
                    && row.target_name.as_deref() == Some("axum_core::extract::FromRef::from_ref")
                    && row.status.as_deref() == Some("admitted")
            })
        })
        .count();
    assert_eq!(
        sites, 2,
        "RAG exact proof context should expose both FromRef dependency-root proof rows: {rows:#?}"
    );
}
