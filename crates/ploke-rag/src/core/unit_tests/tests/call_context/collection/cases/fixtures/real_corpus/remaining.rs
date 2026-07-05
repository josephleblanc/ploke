use super::*;

struct ExactShapeCase {
    label: &'static str,
    target: Uuid,
    expected: Vec<ExpectedExactShape>,
}

#[derive(Clone)]
struct ExpectedExactShape {
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    relation: CallTargetKind,
    count: usize,
}

#[tokio::test]
async fn call_context_exact_reads_remaining_axum_supported_matrix_targets() -> Result<(), Error> {
    init_tracing_once();
    let (db, rag) = setup_axum_call_graph_rag()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // These are the supported DB real-corpus rows not already pinned by the
    // dedicated RAG exact-context tests in the parent module. Each case asserts
    // that RAG exact call context preserves the same one-hop caller-site
    // identities and call-shape fanout exposed by `Database::callers_for_target`.
    let cases = vec![
        ExactShapeCase {
            // axum-macros/src/lib.rs:797 defines `run_ui_tests`.
            // debug_handler.rs:{885,890}, typed_path.rs:443, from_ref.rs:104,
            // and from_request/mod.rs:1050 call `crate::run_ui_tests(...)`.
            label: "axum-macros run_ui_tests helper callers",
            target: function_id_by_name_in_module(&db, &["crate"], "run_ui_tests")?,
            expected: vec![path_shape(
                &["crate", "run_ui_tests"],
                CallTargetKind::Function,
                5,
            )],
        },
        ExactShapeCase {
            // axum-core/src/body.rs:26 defines try_downcast; body.rs currently
            // resolves two same-module non-macro caller rows to it.
            label: "axum-core try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
            expected: vec![path_shape(&["try_downcast"], CallTargetKind::Function, 2)],
        },
        ExactShapeCase {
            // axum/src/util.rs:99 defines try_downcast; routing/mod.rs:205
            // calls the imported helper.
            label: "axum try_downcast current resolved subset",
            target: function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
            expected: vec![path_shape(&["try_downcast"], CallTargetKind::Function, 1)],
        },
        ExactShapeCase {
            // axum-macros/src/with_position.rs:66 defines Position::First.
            // with_position.rs:92 calls `Position::First(item)`.
            label: "axum-macros Position::First enum variant constructor",
            target: variant_id_by_enum_and_variant_names(&db, "Position", "First")?,
            expected: vec![path_shape(
                &["Position", "First"],
                CallTargetKind::EnumVariantConstructor,
                1,
            )],
        },
        ExactShapeCase {
            // axum/src/error_handling/mod.rs:80 defines HandleError::new.
            // error_handling/mod.rs:65 and service_ext.rs:43 call it.
            label: "axum HandleError::new constructor callers",
            target: method_id_by_name_and_body_substring(
                &db,
                "new",
                "Self { inner, f, _extractor",
            )?,
            expected: vec![path_shape(
                &["HandleError", "new"],
                CallTargetKind::AssociatedFunction,
                2,
            )],
        },
        ExactShapeCase {
            // axum-core/src/ext_traits/request.rs:268 calls
            // `self.extract_with_state(&())`; the callee is the same impl
            // method whose body calls `E::from_request(self, state)`.
            label: "axum-core RequestExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
            )?,
            expected: vec![method_shape(
                "extract_with_state",
                Some(CallReceiverInfo::SelfValue),
                CallTargetKind::Method,
                1,
            )],
        },
        ExactShapeCase {
            // axum-core/src/ext_traits/request_parts.rs:122 calls
            // `self.extract_with_state(&())`; the callee is the same impl
            // method whose body calls `E::from_request_parts(self, state)`.
            label: "axum-core RequestPartsExt extract self-call",
            target: method_id_by_name_and_body_substring(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
            )?,
            expected: vec![method_shape(
                "extract_with_state",
                Some(CallReceiverInfo::SelfValue),
                CallTargetKind::Method,
                1,
            )],
        },
        ExactShapeCase {
            // axum-core/src/extract/mod.rs:85 declares
            // FromRequest::from_request. ext_traits/request.rs:279 calls
            // `E::from_request(...)`; extract/mod.rs:127 calls
            // `T::from_request(...)`.
            label: "axum-core FromRequest::from_request trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequest", "from_request")?,
            expected: vec![
                path_shape(
                    &["E", "from_request"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
                path_shape(
                    &["T", "from_request"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
            ],
        },
        ExactShapeCase {
            // axum-core/src/extract/mod.rs:59 declares
            // FromRequestParts::from_request_parts. ext_traits/request.rs:305
            // and ext_traits/request_parts.rs:133 call `E::...`; extract/mod.rs:115
            // calls `T::from_request_parts(...)`.
            label: "axum-core FromRequestParts::from_request_parts trait-associated paths",
            target: method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?,
            expected: vec![
                path_shape(
                    &["E", "from_request_parts"],
                    CallTargetKind::AssociatedFunction,
                    2,
                ),
                path_shape(
                    &["T", "from_request_parts"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
            ],
        },
        ExactShapeCase {
            // axum-core/src/extract/from_ref.rs:15 declares FromRef::from_ref.
            // ext_traits/mod.rs:25 calls `InnerState::from_ref(state)` and
            // ext_traits/mod.rs:45 calls `String::from_ref(state)`.
            label: "axum-core FromRef::from_ref same-crate bounded associated paths",
            target: method_id_by_trait_name(&db, "FromRef", "from_ref")?,
            expected: vec![
                path_shape(
                    &["InnerState", "from_ref"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
                path_shape(
                    &["String", "from_ref"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
            ],
        },
        ExactShapeCase {
            // axum/src/routing/mod.rs:162 defines Router::new. The current
            // fixture resolves 307 `Router::new` rows, plus
            // routing/mod.rs:109 `Self::new()` and
            // routing/method_routing.rs:1494 `crate::Router::new()`.
            label: "axum Router::new current resolved fanout",
            target: method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?,
            expected: vec![
                path_shape(&["Router", "new"], CallTargetKind::AssociatedFunction, 307),
                path_shape(&["Self", "new"], CallTargetKind::AssociatedFunction, 1),
                path_shape(
                    &["crate", "Router", "new"],
                    CallTargetKind::AssociatedFunction,
                    1,
                ),
            ],
        },
        ExactShapeCase {
            // axum/src/test_helpers/test_client.rs:19 defines
            // TestClient::new. The regenerated fixture resolves nested/direct
            // re-export import rows and routing child-module inherited glob
            // rows; the axum-core request_parts boundary row remains pinned
            // targetless in the DB tests.
            label: "axum TestClient::new resolved re-export subset",
            target: resolved_path_target_count(
                &db,
                &["TestClient", "new"],
                167,
                "TestClient::new",
            )?,
            expected: vec![path_shape(
                &["TestClient", "new"],
                CallTargetKind::AssociatedFunction,
                167,
            )],
        },
    ];

    for case in cases {
        assert_exact_shape_case(&db, &rag, case)?;
    }

    Ok(())
}

fn variant_id_by_enum_and_variant_names(
    db: &Database,
    enum_name: &str,
    variant_name: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *enum { id: enum_id, name: $enum_name @ 'NOW' },
            *variant { id, name: $variant_name, owner_id: enum_id @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one enum variant {enum_name}::{variant_name}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn resolved_path_target_count(
    db: &Database,
    path_parts: &[&str],
    expected_count: usize,
    label: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert(
        "path".to_string(),
        DataValue::List(
            path_parts
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let rows = db.raw_query_params(
        r#"?[target_id, count(site_id)] :=
            *call_site {
                id: site_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: "Resolved",
                resolution_kind: "LocalExact" @ 'NOW'
            },
            *call_relation {
                source_id: site_id,
                source_kind: "Path",
                relation_kind: "AssociatedFunction",
                target_id,
                target_kind: "Method" @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{label} should resolve to exactly one associated-function target: {:#?}",
        rows.rows
    );

    let DataValue::Num(cozo::Num::Int(count)) = &rows.rows[0][1] else {
        panic!(
            "{label} resolved row count should be an integer: {:#?}",
            rows.rows
        );
    };
    assert_eq!(
        *count as usize, expected_count,
        "{label} should expose exactly {expected_count} resolved path rows"
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

fn path_shape(segments: &[&str], relation: CallTargetKind, count: usize) -> ExpectedExactShape {
    ExpectedExactShape {
        kind: CallSiteKind::Path,
        callee: CallCalleeInfo::Path {
            path: path(segments),
        },
        relation,
        count,
    }
}

fn method_shape(
    name: &str,
    receiver: Option<CallReceiverInfo>,
    relation: CallTargetKind,
    count: usize,
) -> ExpectedExactShape {
    ExpectedExactShape {
        kind: CallSiteKind::Method,
        callee: CallCalleeInfo::Method {
            name: name.to_string(),
            receiver,
        },
        relation,
        count,
    }
}

fn assert_exact_shape_case(
    db: &Database,
    rag: &RagService,
    case: ExactShapeCase,
) -> Result<(), Error> {
    let callers = db.callers_for_target(case.target)?;
    let expected_count = case
        .expected
        .iter()
        .map(|expected| expected.count)
        .sum::<usize>();
    assert_eq!(
        callers.len(),
        expected_count,
        "{} should expose the expected DB caller count: {callers:#?}",
        case.label
    );

    let context = rag.exact_call_context(case.target)?;
    let incoming = context
        .iter()
        .filter(|call| {
            call.targets
                .iter()
                .any(|candidate| candidate.target_id == case.target)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        incoming.len(),
        expected_count,
        "{} should expose the expected RAG exact incoming caller count: {context:#?}",
        case.label
    );

    let expected_site_ids = callers
        .iter()
        .map(|caller| caller.site.id)
        .collect::<BTreeSet<_>>();
    let incoming_site_ids = incoming
        .iter()
        .map(|call| call.site_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        incoming_site_ids, expected_site_ids,
        "{} should preserve DB caller site identities",
        case.label
    );

    let mut actual = BTreeMap::<(CallSiteKind, CallCalleeInfo, CallTargetKind), usize>::new();
    for call in incoming {
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, case.target);
        *actual
            .entry((
                call.kind.clone(),
                call.callee.clone(),
                call.targets[0].relation.clone(),
            ))
            .or_default() += 1;
    }

    let expected = case
        .expected
        .into_iter()
        .map(|shape| ((shape.kind, shape.callee, shape.relation), shape.count))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "{} should preserve the oracle matrix callee shape fanout",
        case.label
    );

    Ok(())
}
