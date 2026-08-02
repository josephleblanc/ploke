use super::super::*;
use super::common::*;
use super::source_lines::*;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;
use serde_json::json;

#[derive(Clone, Copy)]
struct PathExpectation {
    label: &'static str,
    owner: Uuid,
    target: Uuid,
    path: &'static [&'static str],
    relation: CallRelationKind,
    kind: CallTargetKind,
}

#[derive(Clone, Copy)]
struct OwnerPathCase {
    label: &'static str,
    owner: Uuid,
    path: &'static [&'static str],
    count: usize,
}

const fn fanout(file_suffix: &'static str, lines: &'static [u32]) -> SourceLineFanout {
    SourceLineFanout { file_suffix, lines }
}

fn assert_path_target(db: &Database, expected: PathExpectation) -> Result<Uuid, DbError> {
    let context = db.call_context_for_owner(expected.owner)?;
    let row = row_by_path(&context, expected.path);
    assert_resolved_target(
        row,
        expected.target,
        expected.relation,
        CallSiteKind::Path,
        expected.kind,
    );
    Ok(row.site.id)
}

fn assert_path_edge(db: &Database, expected: PathExpectation) -> Result<Uuid, DbError> {
    let site_id = assert_path_target(db, expected)?;
    assert_one_edge_traversal(
        db,
        TraversalExpectation {
            label: expected.label,
            owner: expected.owner,
            target: expected.target,
            site_id,
            expected_edge_count: 1,
        },
    )?;
    Ok(site_id)
}

fn assert_function_edge(
    db: &Database,
    owner: Uuid,
    target: Uuid,
    path: &'static [&'static str],
    label: &'static str,
) -> Result<Uuid, DbError> {
    assert_path_edge(
        db,
        PathExpectation {
            label,
            owner,
            target,
            path,
            relation: CallRelationKind::Function,
            kind: CallTargetKind::Function,
        },
    )
}

fn assert_path_callers(
    db: &Database,
    callers: &[ploke_db::CallCallerRow],
    target: Uuid,
    case: OwnerPathCase,
) -> Result<(), DbError> {
    let context = db.call_context_for_owner(case.owner)?;
    let expected = path(case.path);
    let rows = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&expected)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        case.count,
        "{} should expose the expected resolved path rows: {context:#?}",
        case.label
    );
    for row in rows {
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert!(
            callers
                .iter()
                .any(|caller| caller.site.id == row.site.id && caller.target.target_id == target),
            "{} should be present in target-centered callers: {callers:#?}",
            case.label
        );
    }
    Ok(())
}

fn assert_method_edge(
    db: &Database,
    owner: Uuid,
    target: Uuid,
    method: &'static str,
    label: &'static str,
) -> Result<(), DbError> {
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(&context, method, &CallReceiver::SelfValue);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    assert_eq!(
        relations_for_site(db, row.site.id)?.rows.len(),
        1,
        "{label} should persist one self.{method} edge"
    );
    assert_one_edge_traversal(
        db,
        TraversalExpectation {
            label,
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )
}

fn exact_callers(
    db: &Database,
    target: Uuid,
    count: usize,
    label: &str,
) -> Result<Vec<ploke_db::CallCallerRow>, DbError> {
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        count,
        "{label} should expose exactly {count} callers: {callers:#?}"
    );
    assert_sites_match_callers(db, target, &callers, label)?;
    Ok(callers)
}

fn assert_caller_shape(
    caller: &ploke_db::CallCallerRow,
    target: Uuid,
    relation: CallRelationKind,
    kind: CallTargetKind,
    label: &str,
) {
    assert_eq!(caller.status.status, CallStatusKind::Resolved, "{label}");
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact),
        "{label}"
    );
    assert_eq!(caller.target.target_id, target, "{label}");
    assert_eq!(caller.target.relation, relation, "{label}");
    assert_eq!(caller.target.source_kind, CallSiteKind::Path, "{label}");
    assert_eq!(caller.target.target_kind, kind, "{label}");
}

fn incoming_candidates(
    db: &Database,
    target: Uuid,
) -> Result<Vec<ploke_db::CallContextCandidate>, DbError> {
    db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )
}

fn assert_macro_blocker(
    blockers: &[ploke_db::ProofBlockerRow],
    context: &[ProofGraphContextRow],
    site_id: Uuid,
    label: &str,
) {
    let site = site_id.to_string();
    assert!(
        blockers.iter().any(|proof| {
            proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.reason == "macro_expansion_not_available"
                && proof.status == "blocked"
        }),
        "{label} should expose a macro-expansion blocker for {site}: {blockers:#?}"
    );
    assert!(
        context.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("macro_expansion_not_available")
        }),
        "{label} should be retrievable as macro-expansion proof context for {site}: {context:#?}"
    );
}

#[test]
fn axum_macros_expand_helpers_reach_root_expand() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "expand")?;

    // Ground truth:
    //   axum-macros/src/lib.rs:724 expand(syn::parse(input).and_then(f))
    //   axum-macros/src/lib.rs:739 expand(expand_result)
    let cases = [
        ExpectedCaller {
            owner_name: "expand_with",
            path: &["expand"],
        },
        ExpectedCaller {
            owner_name: "expand_attr_with",
            path: &["expand"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name_in_module(&db, &["crate"], case.owner_name)?;
        assert_function_edge(&db, owner, target, case.path, case.owner_name)?;
    }

    let callers = exact_callers(&db, target, cases.len(), "root expand helper callers")?;
    for case in cases {
        let owner = function_id_by_name_in_module(&db, &["crate"], case.owner_name)?;
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, case.path);
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
    }

    Ok(())
}

#[test]
fn axum_real_target_explicit_crate_path_parse_attrs_reaches_helper() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // typed_path.rs:23: the explicit crate path reaches attr_parsing::parse_attrs in one edge.
    let owner = function_id_by_name_in_module(&db, &["crate", "typed_path"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    assert_function_edge(
        &db,
        owner,
        target,
        &["crate", "attr_parsing", "parse_attrs"],
        "axum-macros/src/typed_path.rs:23 crate::attr_parsing::parse_attrs",
    )?;

    let callers = db.callers_for_target(target)?;
    caller_by_owner_kind_path(
        &callers,
        owner,
        CallSiteKind::Path,
        &["crate", "attr_parsing", "parse_attrs"],
    );

    Ok(())
}

#[test]
fn axum_real_target_parse_attrs_reaches_helper_current_fanout() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;

    // Eleven imported/explicit path rows reach parse_attrs; three from_request rows retain
    // their nested closure owners. See the real-corpus call-site oracle matrix.
    let cases = [
        (
            "typed_path.rs:23 expand -> crate::attr_parsing::parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "typed_path"], "expand")?,
            &["crate", "attr_parsing", "parse_attrs"][..],
            1,
        ),
        (
            "from_ref.rs:30 expand_field -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?,
            &["parse_attrs"][..],
            1,
        ),
        (
            "from_request/mod.rs:{112,196} expand -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?,
            &["parse_attrs"][..],
            2,
        ),
        (
            "from_request/mod.rs:598 extract_fields -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?,
            &["parse_attrs"][..],
            1,
        ),
        (
            "from_request/mod.rs:727 impl_struct_by_extracting_all_at_once -> parse_attrs",
            function_id_by_name_in_module(
                &db,
                &["crate", "from_request"],
                "impl_struct_by_extracting_all_at_once",
            )?,
            &["parse_attrs"][..],
            1,
        ),
        (
            "from_request/mod.rs:{892,908} impl_enum_by_extracting_all_at_once -> parse_attrs",
            function_id_by_name_in_module(
                &db,
                &["crate", "from_request"],
                "impl_enum_by_extracting_all_at_once",
            )?,
            &["parse_attrs"][..],
            2,
        ),
    ];

    let callers = exact_callers(&db, target, 11, "parse_attrs real-corpus callers")?;
    let mut expected_owners = std::collections::BTreeSet::new();
    let mut expected_by_owner_path = std::collections::BTreeMap::new();
    for (label, owner, call_path, expected_edges) in cases {
        assert_path_callers(
            &db,
            &callers,
            target,
            OwnerPathCase {
                label,
                owner,
                path: call_path,
                count: expected_edges,
            },
        )?;
        expected_owners.insert(owner);
        expected_by_owner_path.insert((owner, path(call_path)), expected_edges);
    }

    let mut actual_by_owner_path = std::collections::BTreeMap::<_, usize>::new();
    let mut closure_owners = std::collections::BTreeSet::new();
    let mut closure_rows = 0;
    for caller in &callers {
        assert_caller_shape(
            caller,
            target,
            CallRelationKind::Function,
            CallTargetKind::Function,
            "parse_attrs caller",
        );
        if owner_kind_for_call_body_owner(&db, caller.site.owner_id)? == "Closure" {
            assert_eq!(
                caller.site.path.as_ref(),
                Some(&path(&["parse_attrs"])),
                "closure-owned parse_attrs rows should preserve the imported helper path"
            );
            closure_owners.insert(caller.site.owner_id);
            closure_rows += 1;
            continue;
        }
        *actual_by_owner_path
            .entry((
                caller.site.owner_id,
                caller
                    .site
                    .path
                    .clone()
                    .expect("parse_attrs caller should carry a path"),
            ))
            .or_default() += 1;
    }
    assert_eq!(
        closure_rows, 3,
        "parse_attrs should expose the three nested closure-body source rows"
    );
    assert_eq!(
        closure_owners.len(),
        3,
        "parse_attrs closure-body rows should remain owned by distinct closure executable owners"
    );
    assert_eq!(
        actual_by_owner_path, expected_by_owner_path,
        "target-centered non-closure parse_attrs callers should match the source-oracle owner fanout"
    );

    let mut expected_incoming_owners = expected_owners;
    expected_incoming_owners.extend(closure_owners.iter().copied());
    let incoming = incoming_candidates(&db, target)?;
    assert_eq!(
        incoming.len(),
        expected_incoming_owners.len(),
        "parse_attrs target expansion should return one one-hop candidate per owner"
    );
    for owner in expected_incoming_owners {
        assert!(
            incoming.iter().any(|candidate| {
                candidate.node_id == owner
                    && candidate.target_id == target
                    && candidate.relation == ploke_db::CallContextRelation::IncomingCaller
                    && candidate.distance == 1
            }),
            "parse_attrs expansion should include owner {owner}: {incoming:#?}"
        );
    }

    let extract_fields =
        function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?;
    let extract_fields_rows = db
        .call_context_for_owner(extract_fields)?
        .into_iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path
                && row.site.path.as_ref() == Some(&path(&["parse_attrs"]))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        extract_fields_rows.len(),
        1,
        "from_request::extract_fields should keep only the current non-closure parse_attrs row; the matrix source at axum-macros/src/from_request/mod.rs:471 remains a nested-owner gap"
    );

    Ok(())
}

#[test]
fn axum_real_target_parse_attrs_projects_proof_facts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;

    // All eleven current caller sites project resolution, edge, and source-provenance facts.
    let callers = exact_callers(&db, target, 11, "parse_attrs proof setup")?;
    for caller in &callers {
        assert_caller_shape(
            caller,
            target,
            CallRelationKind::Function,
            CallTargetKind::Function,
            "parse_attrs proof caller",
        );
    }
    let expected = callers
        .iter()
        .map(|caller| TargetProofSite {
            owner: caller.site.owner_id,
            site: caller.site.id,
        })
        .collect::<Vec<_>>();

    assert_target_proof_projection_source_counts(
        &db,
        "real corpus parse_attrs",
        "bd:corpus-axum-call-graph",
        target,
        &callers,
        &expected,
        &[
            ("axum-macros/src/typed_path.rs", 1),
            ("axum-macros/src/from_ref.rs", 1),
            ("axum-macros/src/from_request/mod.rs", 9),
        ],
        "type_resolution_missing",
    )
}

#[test]
fn axum_real_target_run_ui_tests_crate_paths_reach_helper() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate"], "run_ui_tests")?;

    // Five UI helpers call crate::run_ui_tests in one edge.
    let cases = [
        (&["crate", "debug_handler"][..], "ui_debug_handler"),
        (&["crate", "debug_handler"][..], "ui_debug_middleware"),
        (&["crate", "typed_path"][..], "ui"),
        (&["crate", "from_ref"][..], "ui"),
        (&["crate", "from_request"][..], "ui"),
    ];

    for (module_path, owner_name) in cases.iter().copied() {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        assert_function_edge(&db, owner, target, &["crate", "run_ui_tests"], owner_name)?;
    }

    exact_callers(&db, target, cases.len(), "run_ui_tests real-corpus callers")?;

    Ok(())
}

#[test]
fn axum_real_target_take_route_helper_resolves_tap_inner_closure_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // tap_inner! contributes two closure-owned routing/mod.rs:{410,430} calls; debug-only
    // super::take_route_or_internal_error rows remain absent from this normal-build fixture.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing"], "take_route_or_internal_error")?;
    assert_no_path_rows(&db, &["super", "take_route_or_internal_error"])?;
    let line_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["take_route_or_internal_error"],
        CallRelationKind::Function,
        CallTargetKind::Function,
        &[fanout("axum/src/routing/mod.rs", &[410, 430])],
    )?;
    assert_eq!(
        line_target, target,
        "tap_inner closure call rows should resolve to the routing helper definition"
    );

    exact_callers(
        &db,
        target,
        2,
        "axum/src/routing/mod.rs:410,430 take_route_or_internal_error",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_calls_preserve_generic_counts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // attr_parsing.rs:{22,45}: external type_name::<K> rows preserve one generic argument
    // without local relations or traversal candidates.
    for (label, owner_name) in [
        ("attr_parsing.rs:22", "parse_parenthesized_attribute"),
        ("attr_parsing.rs:45", "parse_assignment_attribute"),
    ] {
        let owner = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, &["std", "any", "type_name"]);
        assert_external_targetless(row);
        assert_eq!(
            row.site.generic_arg_count,
            Some(1),
            "{label} should preserve the `<K>` turbofish arity"
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "{label} should not have raw call_relation targets"
        );
        assert_no_traversal_candidates_for_site(
            &db,
            owner,
            row.site.id,
            &format!("{label} std::any::type_name::<K>"),
        )?;
    }

    // attr_parsing.rs:66: closure-owned attr.parse_args::<T> preserves its generic arity but
    // remains an unsupported external-binding receiver with no fabricated edge.
    let owner = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let context = db.call_context_for_owner(owner)?;
    assert!(
        context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("parse_args")),
        "closure-body parse_args::<T>() should remain owned by the nested closure, not by parse_attrs: {context:#?}"
    );

    assert_targetless_method_owner_kind_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "parse_args",
        "LocalBinding",
        Some(&["attr"]),
        CallStatusKind::Unsupported,
        "Closure",
        &[fanout("axum-macros/src/attr_parsing.rs", &[66])],
    )?;

    let mut params = std::collections::BTreeMap::new();
    params.insert("method".to_string(), cozo::DataValue::from("parse_args"));
    params.insert(
        "receiver_path".to_string(),
        cozo::DataValue::List(vec![cozo::DataValue::from("attr")]),
    );
    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, generic_arg_count] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Method",
                method_name: $method,
                receiver_kind: "LocalBinding",
                receiver_path: $receiver_path,
                generic_arg_count @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: "Unsupported",
                resolution_kind @ 'NOW'
            },
            *call_body_owner {
                id: owner_id,
                owner_kind: "Closure" @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "attr_parsing.rs:66 should project one closure-owned parse_args::<T>() row"
    );
    assert_eq!(
        rows.rows[0][2],
        cozo::DataValue::Num(cozo::Num::Int(1)),
        "attr_parsing.rs:66 should preserve the `<T>` method generic arity"
    );
    let site_id = to_uuid(&rows.rows[0][0])?;
    let closure_owner = to_uuid(&rows.rows[0][1])?;
    assert!(
        relations_for_site(&db, site_id)?.rows.is_empty(),
        "attr_parsing.rs:66 parse_args::<T>() should not have raw call_relation targets"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        closure_owner,
        site_id,
        "axum-macros/src/attr_parsing.rs:66 attr.parse_args::<T>",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_external_path_rows_remain_targetless() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Real-corpus serde_json and std::mem::replace dependency paths remain targetless and
    // edge-free, including all bounded generated middleware wrapper owners.
    let json_owner = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;
    let json_context = db.call_context_for_owner(json_owner)?;
    let json_row = row_by_path(&json_context, &["serde_json", "Deserializer", "from_slice"]);
    assert_external_targetless(json_row);
    assert_no_traversal_candidates_for_site(
        &db,
        json_owner,
        json_row.site.id,
        "axum/src/json.rs:184 serde_json::Deserializer::from_slice",
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["serde_json", "Deserializer", "from_slice"],
        CallStatusKind::External,
        &[fanout("axum/src/json.rs", &[184])],
    )?;

    let replace_owner =
        method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let replace_context = db.call_context_for_owner(replace_owner)?;
    let replace_row = row_by_path(&replace_context, &["std", "mem", "replace"]);
    assert_external_targetless(replace_row);
    assert_no_traversal_candidates_for_site(
        &db,
        replace_owner,
        replace_row.site.id,
        "axum/src/response/sse.rs:449 std::mem::replace",
    )?;

    let error_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "let inner = std::mem::replace(&mut self.inner, clone)",
        "axum/src/error_handling/mod.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        error_owner,
        &["std", "mem", "replace"],
        CallStatusKind::External,
        "axum/src/error_handling/mod.rs:138 std::mem::replace",
    )?;

    assert_targetless_path_rows(
        &db,
        &["std", "mem", "replace"],
        CallStatusKind::External,
        51,
    )?;
    assert_no_method_owner_by_body_and_file_suffix(
        &db,
        "call",
        "let (mut parts, body) = req.into_parts();",
        "axum/src/error_handling/mod.rs",
        "axum/src/error_handling/mod.rs:181 macro-template std::mem::replace",
    )?;

    for (label, file_suffix, count) in [
        (
            "axum/src/middleware/map_request.rs:281",
            "axum/src/middleware/map_request.rs",
            16,
        ),
        (
            "axum/src/middleware/from_fn.rs:285",
            "axum/src/middleware/from_fn.rs",
            16,
        ),
        (
            "axum/src/middleware/map_response.rs:260",
            "axum/src/middleware/map_response.rs",
            17,
        ),
    ] {
        let owners = method_ids_by_name_body_and_file_suffix(
            &db,
            "call",
            "std::mem::replace(&mut self.inner, not_ready_inner)",
            file_suffix,
        )?;
        assert_eq!(
            owners.len(),
            count,
            "{label} should project one generated Service::call owner per supported tuple arity"
        );
        for owner in owners {
            assert_owner_path_targetless(
                &db,
                owner,
                &["std", "mem", "replace"],
                CallStatusKind::External,
                label,
            )?;
        }
    }

    Ok(())
}

#[test]
fn axum_external_frontier_accepts_admitted_summary_proof() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";
    let summary_id = "external-summary:axum-std-mem-replace";

    // Admitting a summary for response/sse.rs:449 discharges its external blocker without
    // changing the targetless std::mem::replace call graph row.
    let owner = method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["std", "mem", "replace"]);
    assert_external_targetless(row);
    assert!(relations_for_site(&db, row.site.id)?.rows.is_empty());

    let site = row.site.id.to_string();
    let count = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        count >= 2,
        "owner proof projection should include call_site and call_resolution rows: {count}"
    );

    let missing_before = db.proof_graphrag_context("external_dependency_summary_missing")?;
    assert!(
        missing_before.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "projected external frontier should start with a missing-summary blocker: {missing_before:#?}"
    );

    let mut records = axum_domain_records(domain_id);
    records.extend([
        json!({
            "fact_kind": "call_resolution",
            "schema_version": "ploke-proof-facts.v1",
            "call_site_id": site.clone(),
            "resolution_state": "externally_summarized",
            "external_summary_id": summary_id,
            "evidence_use": "proof_and_navigation"
        }),
        admitted_summary(AdmittedSummary {
            id: summary_id,
            domain_id,
            artifact_hash: "sha256:axum-std-mem-replace-summary",
            version: "axum-call-graph-summary-v1",
            scope: "axum std::mem::replace frontier in corpus_axum_call_graph",
            effect: "external_summary_boundary",
        }),
    ]);
    db.upsert_proof_fact_values(&records)?;

    let blockers = db.proof_blockers()?;
    assert!(
        !blockers.iter().any(|proof| {
            proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.reason == "external_dependency_summary_missing"
        }),
        "linked admitted external summary should discharge the real frontier blocker: {blockers:#?}"
    );

    let summary_rows = db.proof_graphrag_context(summary_id)?;
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.summary_class.as_deref() == Some("audited_no_process_effects")
                && proof.status.as_deref() == Some("admitted")
        }),
        "summary-id lookup should expose the admitted summary artifact: {summary_rows:#?}"
    );
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.resolution_state.as_deref() == Some("externally_summarized")
                && proof.blocker_reason.is_none()
        }),
        "summary-id lookup should expose the externally summarized call resolution without a blocker: {summary_rows:#?}"
    );

    let context_after = db.call_context_for_owner(owner)?;
    let row_after = row_by_path(&context_after, &["std", "mem", "replace"]);
    assert_external_targetless(row_after);
    assert!(
        relations_for_site(&db, row_after.site.id)?.rows.is_empty(),
        "proof summary admission must not create a call graph edge"
    );

    Ok(())
}

#[test]
fn axum_real_target_body_empty_reaches_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    // Twenty-one Body::empty paths and two Self::empty paths reach axum-core's constructor,
    // covering direct imports, re-exports, inherited imports, and nested executable owners.
    let body_path_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Body", "empty"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        &[
            fanout("axum-core/src/ext_traits/request.rs", &[346, 364, 377, 390]),
            fanout("axum-core/src/response/into_response.rs", &[128, 163]),
            fanout("axum/src/extract/query.rs", &[106]),
            fanout("axum/src/extract/raw_form.rs", &[65]),
            fanout("axum/src/form.rs", &[158]),
            fanout("axum/src/middleware/from_fn.rs", &[411]),
            fanout("axum/src/routing/method_routing.rs", &[1700]),
            fanout("axum/src/routing/route.rs", &[161, 174]),
            fanout("axum/src/routing/tests/get_to_head.rs", &[25, 59]),
            fanout("axum/src/routing/tests/merge.rs", &[198, 204]),
            fanout("axum/src/routing/tests/mod.rs", &[228, 1133, 1151]),
            fanout("axum/src/serve/mod.rs", &[799]),
        ],
    )?;
    let self_path_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Self", "empty"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        &[fanout("axum-core/src/body.rs", &[83, 89])],
    )?;
    assert_eq!(body_path_target, target);
    assert_eq!(self_path_target, target);

    let callers = exact_callers(&db, target, 23, "Body::empty current resolved subset")?;
    let incoming = incoming_candidates(&db, target)?;
    assert_eq!(
        incoming.len(),
        23,
        "Body::empty target expansion should traverse the twenty-three current resolved edges: {incoming:#?}"
    );

    let mut path_counts = std::collections::BTreeMap::new();
    for caller in callers {
        *path_counts
            .entry(
                caller
                    .site
                    .path
                    .clone()
                    .expect("Body::empty caller should carry a path"),
            )
            .or_insert(0usize) += 1;
        assert_caller_shape(
            &caller,
            target,
            CallRelationKind::AssociatedFunction,
            CallTargetKind::Method,
            "Body::empty caller",
        );
        assert_incoming_candidate(
            &incoming,
            caller.site.owner_id,
            caller.site.id,
            target,
            "Body::empty incoming caller missing",
        );
    }
    assert_eq!(
        path_counts,
        std::collections::BTreeMap::from([
            (path(&["Body", "empty"]), 21),
            (path(&["Self", "empty"]), 2),
        ]),
        "Body::empty callers should split into literal Body::empty and trait-impl Self::empty rows"
    );

    // Every projected Body::empty row is resolved; neither stale external nor unsupported
    // source/file evidence may remain for this local target shape.
    for status in [CallStatusKind::External, CallStatusKind::Unsupported] {
        assert_path_file_fanout(&db, &["Body", "empty"], status, &[])?;
        assert_targetless_path_line_fanout(
            &db,
            &CORPUS_AXUM_CALL_GRAPH,
            &["Body", "empty"],
            status,
            &[],
        )?;
    }

    Ok(())
}

#[test]
fn axum_real_target_body_empty_projects_proof_facts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    // All 23 constructor caller sites project resolution and edge facts for the same target.
    let callers = exact_callers(&db, target, 23, "Body::empty proof setup")?;
    let mut path_counts = std::collections::BTreeMap::new();
    for caller in &callers {
        *path_counts
            .entry(
                caller
                    .site
                    .path
                    .clone()
                    .expect("Body::empty caller should carry a path"),
            )
            .or_insert(0usize) += 1;
        assert_caller_shape(
            caller,
            target,
            CallRelationKind::AssociatedFunction,
            CallTargetKind::Method,
            "Body::empty proof caller",
        );
    }
    assert_eq!(
        path_counts,
        std::collections::BTreeMap::from([
            (path(&["Body", "empty"]), 21),
            (path(&["Self", "empty"]), 2),
        ]),
        "Body::empty proof callers should split into literal Body::empty and Self::empty rows"
    );
    let expected = callers
        .iter()
        .map(|caller| TargetProofSite {
            owner: caller.site.owner_id,
            site: caller.site.id,
        })
        .collect::<Vec<_>>();

    assert_target_proof_projection_source_counts(
        &db,
        "real corpus Body::empty",
        "bd:corpus-axum-call-graph",
        target,
        &callers,
        &expected,
        &[
            ("axum-core/src/body.rs", 2),
            ("axum-core/src/response/into_response.rs", 2),
            ("axum-core/src/ext_traits/request.rs", 4),
            ("axum/src/extract/query.rs", 1),
            ("axum/src/extract/raw_form.rs", 1),
            ("axum/src/form.rs", 1),
            ("axum/src/middleware/from_fn.rs", 1),
            ("axum/src/routing/method_routing.rs", 1),
            ("axum/src/routing/route.rs", 2),
            ("axum/src/routing/tests/get_to_head.rs", 2),
            ("axum/src/routing/tests/merge.rs", 2),
            ("axum/src/routing/tests/mod.rs", 3),
            ("axum/src/serve/mod.rs", 1),
        ],
        "type_resolution_missing",
    )?;

    let mut records = axum_domain_records("bd:corpus-axum-call-graph");
    let mut form_sites = Vec::new();
    let mut raw_form_sites = Vec::new();
    for caller in &callers {
        let provenance = db
            .proof_source_provenance(&caller.site.id.to_string())?
            .unwrap_or_else(|| panic!("Body::empty caller should have proof provenance"));
        if provenance.source_file.ends_with("axum/src/form.rs") {
            form_sites.push(caller);
            records.push(ploke_test_utils::axum_body_empty_dependency_record(
                "bd:corpus-axum-call-graph",
                caller.site.id,
                caller.site.owner_id,
                target,
            ));
        } else if provenance
            .source_file
            .ends_with("axum/src/extract/raw_form.rs")
        {
            raw_form_sites.push(caller);
            records.push(
                ploke_test_utils::axum_body_empty_reexport_dependency_record(
                    "bd:corpus-axum-call-graph",
                    caller.site.id,
                    caller.site.owner_id,
                    target,
                ),
            );
        }
    }
    assert_eq!(
        form_sites.len(),
        1,
        "Body::empty dependency-root proof should use the single direct axum/src/form.rs callsite"
    );
    assert_eq!(
        raw_form_sites.len(),
        1,
        "Body::empty dependency-root proof should use the single axum/src/extract/raw_form.rs re-export callsite"
    );
    db.upsert_proof_fact_values(&records)?;

    let proof_rows = db.proof_symbol_lookup(&target.to_string())?;
    assert_body_empty_dependency_root_proof(
        &proof_rows,
        form_sites[0].site.id,
        form_sites[0].site.owner_id,
        target,
        "axum/src/form.rs:158 direct Body::empty import",
    );
    assert_body_empty_dependency_root_proof(
        &proof_rows,
        raw_form_sites[0].site.id,
        raw_form_sites[0].site.owner_id,
        target,
        "axum/src/extract/raw_form.rs:65 crate::body::Body re-export import",
    );

    Ok(())
}

fn assert_body_empty_dependency_root_proof(
    rows: &[ProofGraphContextRow],
    site_id: Uuid,
    caller_id: Uuid,
    target_id: Uuid,
    label: &str,
) {
    let site_id = site_id.to_string();
    let caller_id = caller_id.to_string();
    let target_id = target_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "dependency_root"
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.caller_def_id.as_deref() == Some(caller_id.as_str())
                && row.resolved_def_id.as_deref() == Some(target_id.as_str())
                && row.target_kind.as_deref() == Some("workspace_inherent_method")
                && row.target_name.as_deref() == Some("axum_core::body::Body::empty")
                && row.target_root.as_deref() == Some("axum-core/src/body.rs")
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "{label} should expose an admitted Body::empty dependency-root proof row: {rows:#?}"
    );
}

#[test]
fn axum_real_target_generated_post_function_resolves() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // The generated method_routing::post function calls on; all 23 visible JSON and routing
    // callsites bind to that generated item without inventing handler::post rows.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "post")?;
    let on_target =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on")?;
    assert_function_edge(
        &db,
        target,
        on_target,
        &["on"],
        "generated routing::post body on(...)",
    )?;

    let line_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["post"],
        CallRelationKind::Function,
        CallTargetKind::Function,
        &[
            fanout("axum/src/json.rs", &[248, 264, 279, 299, 318, 353]),
            fanout("axum/src/routing/method_routing.rs", &[1448, 1660]),
            fanout(
                "axum/src/routing/tests/mod.rs",
                &[
                    88, 624, 666, 744, 745, 746, 772, 792, 812, 838, 842, 844, 899, 1071, 1162,
                ],
            ),
        ],
    );
    assert_eq!(line_target?, target);

    let callers = exact_callers(&db, target, 23, "generated routing::post callers")?;

    // json.rs:248: grouped-import post remains the proof-summary anchor.
    let json_owner =
        function_id_by_name_in_module(&db, &["crate", "json", "tests"], "deserialize_body")?;
    let post_site = assert_function_edge(
        &db,
        json_owner,
        target,
        &["post"],
        "axum/src/json.rs:248 grouped-import post",
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let projected = db.project_call_proof_facts_for_owner(json_owner, domain_id)?;
    assert!(
        projected >= 3,
        "deserialize_body should project call_site, call_resolution, and call_relation rows for generated post: {projected}"
    );
    let site_id = post_site.to_string();
    db.upsert_proof_fact_values(&ploke_test_utils::axum_routing_post_macro_summary_records(
        post_site,
    ))?;

    let summary_id = ploke_test_utils::AXUM_ROUTING_POST_SUMMARY_ID;
    let boundary_id = ploke_test_utils::axum_routing_post_boundary_id(post_site);
    let summary_rows = db.proof_graphrag_context(summary_id)?;
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "external_summary"
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.status.as_deref() == Some("admitted")
                && proof.allowed_effects == ["external_summary_boundary".to_string()]
        }),
        "summary-id lookup should expose the admitted routing::post macro-boundary summary: {summary_rows:#?}"
    );
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "expansion_boundary"
                && proof.call_site_id.as_deref() == Some(site_id.as_str())
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.external_summary_id.as_deref() == Some(summary_id)
                && proof.blocker_reason.is_none()
        }),
        "summary-id lookup should expose the summarized routing::post macro boundary without a blocker: {summary_rows:#?}"
    );
    assert!(
        summary_rows.iter().any(|proof| {
            proof.kind == "expanded_item"
                && proof.expanded_item_id.as_deref() == Some("expanded:item:axum-routing-post")
                && proof.boundary_id.as_deref() == Some(boundary_id.as_str())
                && proof.definition_id.as_deref() == Some("def:axum::routing::method_routing::post")
        }),
        "summary-id lookup should expose the generated routing::post item linkage: {summary_rows:#?}"
    );
    let post_after = assert_path_target(
        &db,
        PathExpectation {
            label: "axum/src/json.rs:248 post after summary",
            owner: json_owner,
            target,
            path: &["post"],
            relation: CallRelationKind::Function,
            kind: CallTargetKind::Function,
        },
    )?;
    assert_eq!(
        post_after, post_site,
        "summary admission changed the post site"
    );
    assert_eq!(
        relations_for_site(&db, post_after)?.rows.len(),
        1,
        "routing::post macro boundary summary should preserve the single local generated-function edge"
    );

    for (owner_name, label) in [
        (
            "consume_body_to_json_requires_json_content_type",
            "json.rs:264",
        ),
        ("invalid_json_syntax", "json.rs:299"),
        ("extra_chars_after_valid_json_syntax", "json.rs:318"),
        ("invalid_json_data", "json.rs:353"),
    ] {
        let owner = function_id_by_name_in_module(&db, &["crate", "json", "tests"], owner_name)?;
        assert_path_callers(
            &db,
            &callers,
            target,
            OwnerPathCase {
                label,
                owner,
                path: &["post"],
                count: 1,
            },
        )?;
    }

    for (owner_name, label) in [
        (
            "content_type_with_encoding",
            "axum/src/extract/multipart.rs:381",
        ),
        (
            "_multipart_from_request_limited",
            "axum/src/extract/multipart.rs:404",
        ),
        ("body_too_large", "axum/src/extract/multipart.rs:420"),
        ("optional_multipart", "axum/src/extract/multipart.rs:448"),
    ] {
        let rows = db.raw_query_params(
            r#"?[id] :=
                *function { id: id, name: $name @ 'NOW' }"#,
            std::collections::BTreeMap::from([(
                "name".to_string(),
                cozo::DataValue::from(owner_name),
            )]),
        )?;
        assert!(
            rows.rows.is_empty(),
            "{label} multipart owner should remain absent in the current fixture: {rows:#?}"
        );
    }
    assert_no_path_rows(&db, &["handler", "post"])?;

    for (owner_name, label) in [
        ("merge", "axum/src/routing/method_routing.rs:1448"),
        (
            "merge_accessing_state",
            "axum/src/routing/method_routing.rs:1660",
        ),
    ] {
        let owner = function_id_by_name_in_module(
            &db,
            &["crate", "routing", "method_routing", "tests"],
            owner_name,
        )?;
        assert_path_callers(
            &db,
            &callers,
            target,
            OwnerPathCase {
                label,
                owner,
                path: &["post"],
                count: 1,
            },
        )?;
    }

    // Group module-anchored rows by owner; the fanout above separately proves the nested
    // async-block-owned row at :1071.
    let routing_cases = [
        ("hello_world", 1, "axum/src/routing/tests/mod.rs:88"),
        (
            "different_methods_added_in_different_routes",
            1,
            "axum/src/routing/tests/mod.rs:768",
        ),
        (
            "merging_routers_with_same_paths_but_different_methods",
            1,
            "axum/src/routing/tests/mod.rs:810",
        ),
        (
            "body_limited_by_default",
            3,
            "axum/src/routing/tests/mod.rs:888-890",
        ),
        (
            "disabling_the_default_limit",
            1,
            "axum/src/routing/tests/mod.rs:916",
        ),
        (
            "limited_body_with_content_length",
            1,
            "axum/src/routing/tests/mod.rs:936",
        ),
        (
            "changing_the_default_limit",
            1,
            "axum/src/routing/tests/mod.rs:956",
        ),
        (
            "changing_the_default_limit_differently_on_different_routes",
            3,
            "axum/src/routing/tests/mod.rs:982,986,988",
        ),
        (
            "limited_body_with_streaming_body",
            1,
            "axum/src/routing/tests/mod.rs:1043",
        ),
        (
            "impl_handler_for_into_response",
            1,
            "axum/src/routing/tests/mod.rs:1306",
        ),
    ];

    for (owner_name, count, label) in routing_cases {
        let owner = function_id_by_name_in_module(&db, &["crate", "routing", "tests"], owner_name)?;
        assert_path_callers(
            &db,
            &callers,
            target,
            OwnerPathCase {
                label,
                owner,
                path: &["post"],
                count,
            },
        )?;
    }

    let logging_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "tests"], "logging_rejections")?;
    let logging_context = db.call_context_for_owner(logging_owner)?;
    assert!(
        logging_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&path(&["post"]))),
        "axum/src/routing/tests/mod.rs:1215 closure-body post(...) should remain absent under the parent function owner: {logging_context:#?}"
    );

    Ok(())
}

#[test]
fn axum_real_target_generated_service_functions_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Each generated *_service free function reaches on_service; real path callers bind to
    // those functions without conflating chained methods with free-function rows.
    let on_service =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on_service")?;
    for name in [
        "connect_service",
        "delete_service",
        "get_service",
        "head_service",
        "options_service",
        "patch_service",
        "post_service",
        "put_service",
        "trace_service",
    ] {
        let target =
            function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], name)?;
        assert_function_edge(&db, target, on_service, &["on_service"], name)?;
    }

    struct ServicePathCase<'a> {
        target: &'static str,
        path: &'a [&'a str],
        expected_callers: usize,
        lines: &'a [SourceLineFanout],
    }

    let cases = [
        ServicePathCase {
            target: "get_service",
            path: &["get_service"],
            expected_callers: 9,
            lines: &[
                fanout("axum/src/routing/method_routing.rs", &[1415]),
                fanout("axum/src/routing/tests/get_to_head.rs", &[46]),
                fanout("axum/src/routing/tests/handle_error.rs", &[88]),
                fanout("axum/src/routing/tests/merge.rs", &[197, 203]),
                fanout("axum/src/routing/tests/mod.rs", &[173, 231, 279]),
            ],
        },
        ServicePathCase {
            target: "get_service",
            path: &["crate", "routing", "get_service"],
            expected_callers: 9,
            lines: &[fanout("axum/src/routing/tests/fallback.rs", &[203])],
        },
        ServicePathCase {
            target: "delete_service",
            path: &["delete_service"],
            expected_callers: 1,
            lines: &[fanout("axum/src/routing/method_routing.rs", &[1500])],
        },
        ServicePathCase {
            target: "patch_service",
            path: &["patch_service"],
            expected_callers: 1,
            lines: &[fanout("axum/src/routing/tests/mod.rs", &[280])],
        },
        ServicePathCase {
            target: "post_service",
            path: &["post_service"],
            expected_callers: 1,
            lines: &[fanout("axum/src/routing/method_routing.rs", &[1620])],
        },
    ];

    for case in cases {
        let target = function_id_by_name_in_module(
            &db,
            &["crate", "routing", "method_routing"],
            case.target,
        )?;
        let line_target = assert_resolved_path_line_fanout(
            &db,
            &CORPUS_AXUM_CALL_GRAPH,
            case.path,
            CallRelationKind::Function,
            CallTargetKind::Function,
            case.lines,
        )?;
        assert_eq!(
            line_target, target,
            "{:?} should resolve to generated {}",
            case.path, case.target
        );
        exact_callers(
            &db,
            target,
            case.expected_callers,
            &format!("generated {}", case.target),
        )?;
    }

    Ok(())
}

#[test]
fn axum_real_target_generated_chained_method_functions_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // All generated MethodRouter handler/service methods resolve their self.on or
    // self.on_service body call to the matching local inherent method in one edge.
    let on = method_id_by_name_body_and_file_suffix(
        &db,
        "on",
        "self.on_endpoint(filter, &MethodEndpoint::BoxedHandler",
        "axum/src/routing/method_routing.rs",
    )?;
    let on_service = method_id_by_name_body_and_file_suffix(
        &db,
        "on_service",
        "self.on_endpoint(filter, &MethodEndpoint::Route",
        "axum/src/routing/method_routing.rs",
    )?;

    for name in [
        "connect", "delete", "get", "head", "options", "patch", "post", "put", "trace",
    ] {
        let owner = method_id_by_name_body_and_file_suffix(
            &db,
            name,
            "self.on(MethodFilter::",
            "axum/src/routing/method_routing.rs",
        )?;
        assert_method_edge(&db, owner, on, "on", name)?;
    }

    for name in [
        "connect_service",
        "delete_service",
        "get_service",
        "head_service",
        "options_service",
        "patch_service",
        "post_service",
        "put_service",
        "trace_service",
    ] {
        let owner = method_id_by_name_body_and_file_suffix(
            &db,
            name,
            "self.on_service(MethodFilter::",
            "axum/src/routing/method_routing.rs",
        )?;
        assert_method_edge(&db, owner, on_service, "on_service", name)?;
    }

    Ok(())
}

#[test]
fn axum_real_target_try_downcast_helpers_reach_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // The same-named axum-core/axum helpers retain exact 2/1 resolved fanout; turbofish calls
    // inside assert_eq! remain unsupported macro rows with proof blockers and no local edges.
    let core_target = function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?;
    let cases = [
        ("axum-core try_downcast", core_target, 2),
        (
            "axum try_downcast",
            function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
            1,
        ),
    ];

    for (label, target, expected_edges) in cases {
        let callers = exact_callers(&db, target, expected_edges, label)?;
        for caller in &callers {
            assert_caller_shape(
                caller,
                target,
                CallRelationKind::Function,
                CallTargetKind::Function,
                label,
            );
        }

        let incoming = incoming_candidates(&db, target)?;
        assert_eq!(
            incoming.len(),
            expected_edges,
            "{label} should traverse the same number of one-hop incoming edges: {incoming:#?}"
        );
        for caller in callers {
            assert_incoming_candidate(
                &incoming,
                caller.site.owner_id,
                caller.site.id,
                target,
                label,
            );
        }
    }

    let macro_cases = [
        ("axum-core/src/body.rs:251 and :252", &["crate", "body"][..]),
        ("axum/src/util.rs:114 and :115", &["crate", "util"][..]),
    ];
    let mut macro_sites = Vec::new();
    for (label, module_path) in macro_cases {
        let owner = function_id_by_name_in_module(&db, module_path, "test_try_downcast")?;
        let context = db.call_context_for_owner(owner)?;
        let path_rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&path(&["try_downcast"]))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            path_rows.len(),
            0,
            "{label} are inside assert_eq! macro invocations and should not be flattened into try_downcast path rows: {context:#?}"
        );
        assert_eq!(
            context.len(),
            2,
            "{label} should project two unsupported macro-bound rows: {context:#?}"
        );
        for row in &context {
            assert_eq!(
                row.site.kind,
                CallSiteKind::Macro,
                "{label} should only project macro call rows for assert_eq! wrappers: {context:#?}"
            );
            assert_targetless_status(row, CallStatusKind::Unsupported);
            assert_eq!(row.site.generic_arg_count, None);
            assert!(
                relations_for_site(&db, row.site.id)?.rows.is_empty(),
                "{label} macro-bound try_downcast rows should not fabricate local targets"
            );
            assert_no_traversal_candidates_for_site(&db, owner, row.site.id, label)?;
            macro_sites.push((label, row.site.id));
        }
        let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
        assert!(
            projected >= 4,
            "{label} should project call_site and blocked call_resolution facts for both assert_eq! macro rows: {projected}"
        );
    }

    let blockers = db.proof_blockers()?;
    let proof_rows = db.proof_graphrag_context("macro_expansion_not_available")?;
    for (label, site_id) in macro_sites {
        assert_macro_blocker(&blockers, &proof_rows, site_id, label);
    }

    Ok(())
}

#[test]
fn axum_real_target_position_first_variant_reaches_enum_variant() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // with_position.rs:92 reaches the local Position::First constructor in one edge.
    let position = variant_id_by_enum_and_variant_names(&db, "Position", "First")?;
    let owner = method_id_by_name_and_body_substring(&db, "next", "Position::First(item)")?;
    assert_path_edge(
        &db,
        PathExpectation {
            label: "axum-macros/src/with_position.rs:92 Position::First",
            owner,
            target: position,
            path: &["Position", "First"],
            relation: CallRelationKind::EnumVariantConstructor,
            kind: CallTargetKind::Variant,
        },
    )?;

    let callers = exact_callers(&db, position, 1, "Position::First real-corpus caller")?;
    caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["Position", "First"]);

    Ok(())
}

#[test]
fn axum_real_target_shadowed_get_closure_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // The two setup routing::get paths resolve; eleven assert_eq! boundaries containing calls
    // to the shadowing local closure remain blocked and must not fabricate routing edges.
    let owner = function_id_by_name(&db, "what_matches_wildcard")?;
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "get")?;
    let context = db.call_context_for_owner(owner)?;
    let get_path = path(&["get"]);
    let get_rows = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path == Some(get_path.clone())
        })
        .collect::<Vec<_>>();
    let macro_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Macro)
        .collect::<Vec<_>>();
    assert_eq!(
        get_rows.len(),
        2,
        "what_matches_wildcard should only project the two setup routing::get rows until macro/closure body calls are modeled: {context:#?}"
    );
    assert_eq!(
        macro_rows.len(),
        11,
        "what_matches_wildcard should preserve the eleven assert_eq! macro boundaries that contain shadowed local closure calls: {context:#?}"
    );
    for row in get_rows {
        assert_eq!(row.site.arg_count, Some(1));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "each setup routing::get source callsite should preserve one raw edge"
        );
    }
    for row in &macro_rows {
        assert_targetless_status(row, CallStatusKind::Unsupported);
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "shadowed get assert_eq! macro rows should not fabricate local targets"
        );
        assert_no_traversal_candidates_for_site(
            &db,
            owner,
            row.site.id,
            "shadowed get assert_eq! macro boundary",
        )?;
    }

    let domain_id = "bd:corpus-axum-call-graph";
    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= macro_rows.len() * 2,
        "shadowed get owner should project call-site and blocked resolution facts for the assert_eq! macro rows: {projected}"
    );
    let blockers = db.proof_blockers()?;
    let proof_rows = db.proof_graphrag_context("macro_expansion_not_available")?;
    for row in macro_rows {
        assert_macro_blocker(
            &blockers,
            &proof_rows,
            row.site.id,
            "shadowed get assert_eq! row",
        );
    }

    Ok(())
}
