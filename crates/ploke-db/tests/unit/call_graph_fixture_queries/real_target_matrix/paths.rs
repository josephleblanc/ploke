use super::super::*;
use super::common::*;
use super::source_lines::*;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;
use serde_json::json;

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
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, case.path);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: case.owner_name,
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "root expand should have exactly the inspected helper callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "root expand helper callers")?;
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

    // Matrix: `parse_attrs` path/import row.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(&input.attrs, "typed_path")`.
    // Expected traversal: `typed_path::expand` reaches `parse_attrs` in one
    // edge through the file-module declaration at axum-macros/src/lib.rs:9.
    let owner = function_id_by_name_in_module(&db, &["crate", "typed_path"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["crate", "attr_parsing", "parse_attrs"]);

    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/typed_path.rs:23 crate::attr_parsing::parse_attrs",
            owner,
            target,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
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

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   `parse_attrs` path/import row.
    //
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls it with an explicit
    //   `crate::attr_parsing::parse_attrs(...)` path.
    //   axum-macros/src/from_ref.rs:9 imports it and from_ref.rs:30 calls it
    //   from `expand_field`.
    //   axum-macros/src/from_request/mod.rs:3 imports it and source callsites
    //   are :112, :196, :471, :598, :727, :892, :908, :1029, and :1039.
    // Expected traversal: eleven call-site edges reach `parse_attrs` in one
    // step. The regenerated fixture now owns the nested closure-body rows at
    // :471, :1029, and :1039 under closure executable owners instead of
    // flattening them into their parent function owners.
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

    let mut expected_owners = std::collections::BTreeSet::new();
    let mut expected_by_owner_path = std::collections::BTreeMap::new();
    for (label, owner, call_path, expected_edges) in cases {
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&path(call_path))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            expected_edges,
            "{label} should expose the expected parse_attrs call rows: {context:#?}"
        );
        for row in rows {
            assert_resolved_target(
                row,
                target,
                CallRelationKind::Function,
                CallSiteKind::Path,
                CallTargetKind::Function,
            );
        }
        expected_owners.insert(owner);
        expected_by_owner_path.insert((owner, path(call_path)), expected_edges);
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        11,
        "parse_attrs should expose the eleven currently resolved real-corpus callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "parse_attrs real-corpus callers")?;
    let mut actual_by_owner_path = std::collections::BTreeMap::<_, usize>::new();
    let mut closure_owners = std::collections::BTreeSet::new();
    let mut closure_rows = 0;
    for caller in &callers {
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::Function);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Function);
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
    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
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

    // Matrix proof bridge:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(...)`.
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,471,598,727,892,908,
    //   1029,1039} call imported `parse_attrs(...)`; :471, :1029, and :1039
    //   are owned by nested closure executable owners.
    // Expected proof traversal: all eleven current caller sites project
    // call_site, call_resolution, call_edge, and per-source provenance facts.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        11,
        "parse_attrs proof setup should use the current eleven corpus callers: {callers:#?}"
    );
    for caller in &callers {
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::Function);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Function);
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

    // Matrix: `run_ui_tests` path/import row.
    // Source chain:
    //   axum-macros/src/lib.rs:797 defines `run_ui_tests`.
    //   debug_handler.rs:885,890; typed_path.rs:443; from_ref.rs:104;
    //   from_request/mod.rs:1050 call `crate::run_ui_tests(...)`.
    // Expected traversal: each UI helper -> `run_ui_tests`, one call edge.
    let cases = [
        (&["crate", "debug_handler"][..], "ui_debug_handler"),
        (&["crate", "debug_handler"][..], "ui_debug_middleware"),
        (&["crate", "typed_path"][..], "ui"),
        (&["crate", "from_ref"][..], "ui"),
        (&["crate", "from_request"][..], "ui"),
    ];

    for (module_path, owner_name) in cases.iter().copied() {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, &["crate", "run_ui_tests"]);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: owner_name,
                owner,
                target,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "run_ui_tests should have exactly the five inspected real-corpus callers"
    );
    assert_sites_match_callers(&db, target, &callers, "run_ui_tests real-corpus callers")?;

    Ok(())
}

#[test]
fn axum_real_target_take_route_helper_resolves_tap_inner_closure_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `take_route_or_internal_error` path row.
    // Source chain:
    //   axum/src/routing/mod.rs:63 defines `take_route_or_internal_error`.
    //   axum/src/routing/mod.rs:398 invokes `tap_inner!`.
    //   The transparent macro input block contains two closure-owned calls at
    //   routing/mod.rs:410,430.
    //   routing/tests/mod.rs:56,59 are debug-only `super::...` calls and remain
    //   absent from the normal-build corpus fixture.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing"], "take_route_or_internal_error")?;
    assert_no_path_rows(&db, &["super", "take_route_or_internal_error"])?;
    let line_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["take_route_or_internal_error"],
        CallRelationKind::Function,
        CallTargetKind::Function,
        &[SourceLineFanout {
            file_suffix: "axum/src/routing/mod.rs",
            lines: &[410, 430],
        }],
    )?;
    assert_eq!(
        line_target, target,
        "tap_inner closure call rows should resolve to the routing helper definition"
    );

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "take_route_or_internal_error should expose the two inspected tap_inner closure callers"
    );
    let sites = db.call_sites_for_target(target)?;
    assert_eq!(
        sites.len(),
        2,
        "call_sites_for_target should mirror the two tap_inner closure rows"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "axum/src/routing/mod.rs:410,430 take_route_or_internal_error",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_calls_preserve_generic_counts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: external turbofish path calls.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:22 and :45 call
    //   `std::any::type_name::<K>()` while building duplicate-attribute
    //   errors.
    // Expected traversal: zero local call edges; both rows classify as
    // external and preserve one generic argument.
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

    // Matrix: turbofish method call.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:66 calls
    //   `attr.parse_args::<T>()` inside an iterator closure.
    // Expected traversal: zero local call edges. The row is projected on the
    // nested closure owner, preserves one generic argument, and remains
    // unsupported because the receiver is a local binding to external `syn`.
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
        &[SourceLineFanout {
            file_suffix: "axum-macros/src/attr_parsing.rs",
            lines: &[66],
        }],
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

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   external path rows.
    //
    // Source chain:
    //   axum/src/json.rs:184 calls `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/error_handling/mod.rs:138,181;
    //   middleware/map_request.rs:281; middleware/from_fn.rs:285;
    //   middleware/map_response.rs:260; and response/sse.rs:449 call
    //   `std::mem::replace(...)`.
    // Expected traversal: zero local call edges. The current fixture projects
    // std-root rows as external, including the bounded generated middleware
    // `from_fn`, `map_request`, and `map_response` wrapper-body rows.
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
        &[SourceLineFanout {
            file_suffix: "axum/src/json.rs",
            lines: &[184],
        }],
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

    // Source oracle:
    //   axum/src/response/sse.rs:449 calls
    //   `std::mem::replace(&mut self.data_written, true)`.
    // Current call-graph contract: the std-root row remains an external
    // targetless frontier with zero local traversal edges. This proof-layer
    // check adds an admitted external summary for the same real call-site and
    // proves the derived `external_dependency_summary_missing` blocker is
    // discharged without inventing a callee edge.
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

    // Matrix: `Body::empty` re-exported constructor row.
    // Source chain:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()` from body
    //   conversion impls.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    //   axum/src/{extract/query.rs,extract/raw_form.rs,form.rs,serve/mod.rs}
    //   call `Body::empty()` through direct parsed-workspace imports.
    //   axum routing and middleware tests call `Body::empty()` through local
    //   re-export imports and inherited `super::*` imports, including the
    //   route.rs closure body and routing/tests/mod.rs local handler rows.
    // Expected traversal for the current fixture: twenty-one `Body::empty` rows
    // and two `Self::empty` rows reach the same target in one edge.
    let body_path_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Body", "empty"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        &[
            SourceLineFanout {
                file_suffix: "axum-core/src/ext_traits/request.rs",
                lines: &[346, 364, 377, 390],
            },
            SourceLineFanout {
                file_suffix: "axum-core/src/response/into_response.rs",
                lines: &[128, 163],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/query.rs",
                lines: &[106],
            },
            SourceLineFanout {
                file_suffix: "axum/src/extract/raw_form.rs",
                lines: &[65],
            },
            SourceLineFanout {
                file_suffix: "axum/src/form.rs",
                lines: &[158],
            },
            SourceLineFanout {
                file_suffix: "axum/src/middleware/from_fn.rs",
                lines: &[411],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1700],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/route.rs",
                lines: &[161, 174],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/get_to_head.rs",
                lines: &[25, 59],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/merge.rs",
                lines: &[198, 204],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[228, 1133, 1151],
            },
            SourceLineFanout {
                file_suffix: "axum/src/serve/mod.rs",
                lines: &[799],
            },
        ],
    )?;
    let self_path_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Self", "empty"],
        CallRelationKind::AssociatedFunction,
        CallTargetKind::Method,
        &[SourceLineFanout {
            file_suffix: "axum-core/src/body.rs",
            lines: &[83, 89],
        }],
    )?;
    assert_eq!(body_path_target, target);
    assert_eq!(self_path_target, target);

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        23,
        "current axum fixture should resolve exactly the twenty-three Body::empty callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "Body::empty current resolved subset")?;

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
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
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
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

    // Matrix source frontier:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Expected traversal: all currently projected `Body::empty` rows resolve to
    // axum-core/src/body.rs:52. Do not leave stale external or unsupported
    // targetless rows for this local target shape.
    //   resolved: axum-core/src/ext_traits/request.rs:{346,364,377,390}.
    //   resolved via parsed workspace imports:
    //   extract/query.rs:106; raw_form.rs:65; form.rs:158; serve/mod.rs:799.
    //   resolved via local re-exported workspace imports:
    //   middleware/from_fn.rs:411; routing/method_routing.rs:1700;
    //   route.rs:{161,174}; routing/tests/mod.rs:{228,1133,1151};
    //   routing/tests/get_to_head.rs:{25,59}; routing/tests/merge.rs:{198,204}.
    assert_path_file_fanout(&db, &["Body", "empty"], CallStatusKind::External, &[])?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Body", "empty"],
        CallStatusKind::External,
        &[],
    )?;
    assert_path_file_fanout(&db, &["Body", "empty"], CallStatusKind::Unsupported, &[])?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Body", "empty"],
        CallStatusKind::Unsupported,
        &[],
    )?;

    Ok(())
}

#[test]
fn axum_real_target_body_empty_projects_proof_facts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    // Matrix proof bridge:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    // Expected proof traversal: all current resolved caller sites project
    // call_site, call_resolution, and call_edge facts for the same target.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        23,
        "Body::empty proof setup should use the current twenty-three resolved corpus callers: {callers:#?}"
    );
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
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
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

    // Matrix: generated `routing::post` handler fanout.
    // Source chain:
    //   axum/src/routing/method_routing.rs:165 template; macro invocation :445.
    //   JSON, multipart, method_routing, and routing tests call `post(...)`.
    // Expected traversal: the item-position `top_level_handler_fn!(post, POST)`
    // expansion creates a generated function node in `method_routing`, visible
    // callsites bind to that target, and the generated body itself calls `on`.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "post")?;
    let on_target =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on")?;
    let generated_context = db.call_context_for_owner(target)?;
    let generated_on = row_by_path(&generated_context, &["on"]);
    assert_resolved_target(
        generated_on,
        on_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "generated routing::post body on(...)",
            owner: target,
            target: on_target,
            site_id: generated_on.site.id,
            expected_edge_count: 1,
        },
    )?;

    let line_target = assert_resolved_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["post"],
        CallRelationKind::Function,
        CallTargetKind::Function,
        &[
            SourceLineFanout {
                file_suffix: "axum/src/json.rs",
                lines: &[248, 264, 279, 299, 318, 353],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1448, 1660],
            },
            SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[
                    88, 624, 666, 744, 745, 746, 772, 792, 812, 838, 842, 844, 899, 1071, 1162,
                ],
            },
        ],
    );
    assert_eq!(line_target?, target);

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        23,
        "generated routing::post should expose the inspected real-corpus callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "generated routing::post callers")?;

    // Matrix immediate candidate:
    //   axum/src/json.rs:237 imports `routing::post` in a grouped import.
    //   axum/src/json.rs:248 calls `post(echo_json)` from `deserialize_body`.
    let json_owner =
        function_id_by_name_in_module(&db, &["crate", "json", "tests"], "deserialize_body")?;
    let post_context = db.call_context_for_owner(json_owner)?;
    let post_row = row_by_path(&post_context, &["post"]);
    assert_resolved_target(
        post_row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum/src/json.rs:248 grouped-import post",
            owner: json_owner,
            target,
            site_id: post_row.site.id,
            expected_edge_count: 1,
        },
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let projected = db.project_call_proof_facts_for_owner(json_owner, domain_id)?;
    assert!(
        projected >= 3,
        "deserialize_body should project call_site, call_resolution, and call_relation rows for generated post: {projected}"
    );
    let site_id = post_row.site.id.to_string();
    db.upsert_proof_fact_values(&ploke_test_utils::axum_routing_post_macro_summary_records(
        post_row.site.id,
    ))?;

    let summary_id = ploke_test_utils::AXUM_ROUTING_POST_SUMMARY_ID;
    let boundary_id = ploke_test_utils::axum_routing_post_boundary_id(post_row.site.id);
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
    let context_after = db.call_context_for_owner(json_owner)?;
    let post_after = row_by_path(&context_after, &["post"]);
    assert_resolved_target(
        post_after,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_eq!(
        relations_for_site(&db, post_after.site.id)?.rows.len(),
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
        let context = db.call_context_for_owner(owner)?;
        assert_resolved_target(
            row_by_path(&context, &["post"]),
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert!(
            callers
                .iter()
                .any(|caller| caller.site.owner_id == owner && caller.target.target_id == target),
            "{label} should be present in generated routing::post callers: {callers:#?}"
        );
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
        let context = db.call_context_for_owner(owner)?;
        assert_resolved_target(
            row_by_path(&context, &["post"]),
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert!(
            callers
                .iter()
                .any(|caller| caller.site.owner_id == owner && caller.target.target_id == target),
            "{label} should be present in generated routing::post callers: {callers:#?}"
        );
    }

    struct RoutingPostCase {
        owner: &'static str,
        expected_count: usize,
        label: &'static str,
    }

    // Matrix routing-test rows:
    //   routing/tests/mod.rs:88,624,666,744,745,746,772,792,812,
    //   838,842,844,899,1071,1162 call generated `post(...)`.
    // These module-anchored rows are grouped by owner here; line fanout above
    // also proves the nested async-block-owned row at :1071.
    let routing_cases = [
        RoutingPostCase {
            owner: "hello_world",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:88",
        },
        RoutingPostCase {
            owner: "different_methods_added_in_different_routes",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:768",
        },
        RoutingPostCase {
            owner: "merging_routers_with_same_paths_but_different_methods",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:810",
        },
        RoutingPostCase {
            owner: "body_limited_by_default",
            expected_count: 3,
            label: "axum/src/routing/tests/mod.rs:888-890",
        },
        RoutingPostCase {
            owner: "disabling_the_default_limit",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:916",
        },
        RoutingPostCase {
            owner: "limited_body_with_content_length",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:936",
        },
        RoutingPostCase {
            owner: "changing_the_default_limit",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:956",
        },
        RoutingPostCase {
            owner: "changing_the_default_limit_differently_on_different_routes",
            expected_count: 3,
            label: "axum/src/routing/tests/mod.rs:982,986,988",
        },
        RoutingPostCase {
            owner: "limited_body_with_streaming_body",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:1043",
        },
        RoutingPostCase {
            owner: "impl_handler_for_into_response",
            expected_count: 1,
            label: "axum/src/routing/tests/mod.rs:1306",
        },
    ];

    for case in routing_cases {
        let owner = function_id_by_name_in_module(&db, &["crate", "routing", "tests"], case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&path(&["post"]))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rows.len(),
            case.expected_count,
            "{} should expose the expected generated routing::post row count: {context:#?}",
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
                callers.iter().any(|caller| caller.site.id == row.site.id),
                "{} should be present in target-centered generated routing::post callers: {callers:#?}",
                case.label
            );
        }
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

    // Matrix: generated `*_service` top-level handler fanout.
    // Source chain:
    //   axum/src/routing/method_routing.rs:31-91 template; macro invocations
    //   :335-343. The template creates free functions whose generated bodies
    //   call `on_service(MethodFilter::<METHOD>, svc)`.
    // Expected traversal: every generated service function exists and reaches
    // `on_service`; visible real-corpus `get_service`, `delete_service`,
    // `patch_service`, and `post_service` path callsites bind to those generated
    // functions without treating chained `.post_service(...)` methods as free
    // function rows.
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
        let generated_context = db.call_context_for_owner(target)?;
        let generated_on_service = row_by_path(&generated_context, &["on_service"]);
        assert_resolved_target(
            generated_on_service,
            on_service,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: name,
                owner: target,
                target: on_service,
                site_id: generated_on_service.site.id,
                expected_edge_count: 1,
            },
        )?;
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
                SourceLineFanout {
                    file_suffix: "axum/src/routing/method_routing.rs",
                    lines: &[1415],
                },
                SourceLineFanout {
                    file_suffix: "axum/src/routing/tests/get_to_head.rs",
                    lines: &[46],
                },
                SourceLineFanout {
                    file_suffix: "axum/src/routing/tests/handle_error.rs",
                    lines: &[88],
                },
                SourceLineFanout {
                    file_suffix: "axum/src/routing/tests/merge.rs",
                    lines: &[197, 203],
                },
                SourceLineFanout {
                    file_suffix: "axum/src/routing/tests/mod.rs",
                    lines: &[173, 231, 279],
                },
            ],
        },
        ServicePathCase {
            target: "get_service",
            path: &["crate", "routing", "get_service"],
            expected_callers: 9,
            lines: &[SourceLineFanout {
                file_suffix: "axum/src/routing/tests/fallback.rs",
                lines: &[203],
            }],
        },
        ServicePathCase {
            target: "delete_service",
            path: &["delete_service"],
            expected_callers: 1,
            lines: &[SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1500],
            }],
        },
        ServicePathCase {
            target: "patch_service",
            path: &["patch_service"],
            expected_callers: 1,
            lines: &[SourceLineFanout {
                file_suffix: "axum/src/routing/tests/mod.rs",
                lines: &[280],
            }],
        },
        ServicePathCase {
            target: "post_service",
            path: &["post_service"],
            expected_callers: 1,
            lines: &[SourceLineFanout {
                file_suffix: "axum/src/routing/method_routing.rs",
                lines: &[1620],
            }],
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
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            case.expected_callers,
            "{} should expose expected generated service callers: {callers:#?}",
            case.target
        );
        assert_sites_match_callers(&db, target, &callers, &format!("generated {}", case.target))?;
    }

    Ok(())
}

#[test]
fn axum_real_target_generated_chained_method_functions_resolve() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated chained `MethodRouter` methods.
    // Source chain:
    //   axum/src/routing/method_routing.rs:176-259 templates
    //   `chained_service_fn!`; invocations at :992-1000 generate inherent
    //   methods whose bodies call `self.on_service(MethodFilter::<METHOD>, svc)`.
    //   axum/src/routing/method_routing.rs:263-326 templates
    //   `chained_handler_fn!`; invocations at :642-650 generate inherent
    //   methods whose bodies call `self.on(MethodFilter::<METHOD>, handler)`.
    // Expected traversal: generated impl-item macro methods are stored under
    // the surrounding `MethodRouter` impls and resolve to the local inherent
    // `on` / `on_service` methods without claiming arbitrary macro expansion.
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
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "on", &CallReceiver::SelfValue);
        assert_resolved_target(
            row,
            on,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "generated chained handler method {name} should persist one self.on edge"
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: "generated chained handler method self.on(...)",
                owner,
                target: on,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
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
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_method_receiver(&context, "on_service", &CallReceiver::SelfValue);
        assert_resolved_target(
            row,
            on_service,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(&db, row.site.id)?.rows.len(),
            1,
            "generated chained service method {name} should persist one self.on_service edge"
        );
        assert_one_edge_traversal(
            &db,
            TraversalExpectation {
                label: "generated chained service method self.on_service(...)",
                owner,
                target: on_service,
                site_id: row.site.id,
                expected_edge_count: 1,
            },
        )?;
    }

    Ok(())
}

#[test]
fn axum_real_target_try_downcast_helpers_reach_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Matrix: same-named `try_downcast` helpers.
    // Source chain:
    //   axum-core/src/body.rs:23 defines the axum-core helper; body.rs callsites
    //   at :20, :48, :251, and :252 are the source oracle. The test function
    //   rows at :251 and :252 use `try_downcast::<i32, _>(...)`.
    //   axum/src/util.rs:99 defines the axum helper; routing/mod.rs:205 and
    //   util.rs:114,115 are the source oracle.
    // Expected traversal for the current fixture: two resolved non-macro
    // callers for the axum-core helper and one resolved caller for the axum
    // helper. The turbofish test calls are inside `assert_eq!` macro
    // invocations, so they remain unsupported macro rows rather than path
    // traversal edges.
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
        let callers = db.callers_for_target(target)?;
        assert_eq!(
            callers.len(),
            expected_edges,
            "{label} should expose the current resolved subset of callers: {callers:#?}"
        );
        assert_sites_match_callers(&db, target, &callers, label)?;
        for caller in &callers {
            assert_eq!(caller.status.status, CallStatusKind::Resolved);
            assert_eq!(
                caller.status.resolution,
                Some(CallResolutionKind::LocalExact)
            );
            assert_eq!(caller.target.relation, CallRelationKind::Function);
            assert_eq!(caller.target.source_kind, CallSiteKind::Path);
            assert_eq!(caller.target.target_kind, CallTargetKind::Function);
        }

        let incoming = db.expand_call_context(
            CallContextSeed::Target(target),
            CallContextOptions {
                include_outgoing_targets: false,
                max_candidates: 512,
                ..CallContextOptions::default()
            },
        )?;
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
        let site = site_id.to_string();
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "macro_expansion_not_available"
                    && proof.status == "blocked"
            }),
            "{label} should expose a macro-expansion blocker for unsupported assert_eq! row {site}: {blockers:#?}"
        );
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "call_resolution"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("macro_expansion_not_available")
            }),
            "{label} should be retrievable as macro-expansion proof context for unsupported assert_eq! row {site}: {proof_rows:#?}"
        );
    }

    Ok(())
}

#[test]
fn axum_real_target_position_first_variant_reaches_enum_variant() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Position::First` enum variant constructor row.
    // Source-oracle reference:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source chain:
    //   axum-macros/src/with_position.rs:65 defines `Position`.
    //   with_position.rs:66 defines variant `First`.
    //   with_position.rs:92 calls `Position::First(item)` from
    //   `WithPosition<I>::next`.
    // Expected traversal: `WithPosition<I>::next` reaches the local
    // `Position::First` variant constructor in one persisted call edge.
    let position = variant_id_by_enum_and_variant_names(&db, "Position", "First")?;
    let owner = method_id_by_name_and_body_substring(&db, "next", "Position::First(item)")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["Position", "First"]);

    assert_resolved_target(
        row,
        position,
        CallRelationKind::EnumVariantConstructor,
        CallSiteKind::Path,
        CallTargetKind::Variant,
    );
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-macros/src/with_position.rs:92 Position::First",
            owner,
            target: position,
            site_id: row.site.id,
            expected_edge_count: 1,
        },
    )?;

    let callers = db.callers_for_target(position)?;
    assert_eq!(
        callers.len(),
        1,
        "Position::First should expose the inspected real-corpus caller: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        position,
        &callers,
        "Position::First real-corpus caller",
    )?;
    caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, &["Position", "First"]);

    Ok(())
}

#[test]
fn axum_real_target_shadowed_get_closure_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: shadowed callable `get`.
    // Source chain:
    //   axum/src/routing/tests/mod.rs:418 binds `let get = |path| ...`.
    //   routing/tests/mod.rs:423-434 call that local closure inside
    //   `assert_eq!(get(...).await, ...)`.
    // Current model gap: the function owner projects and resolves the two setup
    // `routing::get` path calls at routing/tests/mod.rs:412-413 plus macro
    // rows for the assertions; it must not fabricate edges from the shadowed
    // local closure calls to the imported routing helper.
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
    assert_eq!(
        get_rows.len(),
        2,
        "what_matches_wildcard should only project the two setup routing::get rows until macro/closure body calls are modeled: {context:#?}"
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

    Ok(())
}
