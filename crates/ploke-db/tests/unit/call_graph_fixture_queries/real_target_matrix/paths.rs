use super::super::*;
use super::common::*;

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
    // Expected traversal: eight call-site edges reach `parse_attrs` in one
    // step; target expansion de-duplicates those to six owner candidates.
    // The three closure-body rows at :471, :1029, and :1039 are asserted below
    // as absent until nested closure call-body ownership lands.
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
            "from_request/mod.rs:592 extract_fields -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?,
            &["parse_attrs"][..],
            1,
        ),
        (
            "from_request/mod.rs:715 impl_struct_by_extracting_all_at_once -> parse_attrs",
            function_id_by_name_in_module(
                &db,
                &["crate", "from_request"],
                "impl_struct_by_extracting_all_at_once",
            )?,
            &["parse_attrs"][..],
            1,
        ),
        (
            "from_request/mod.rs:{880,896} impl_enum_by_extracting_all_at_once -> parse_attrs",
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
        8,
        "parse_attrs should expose the eight currently resolved real-corpus callers: {callers:#?}"
    );
    assert_sites_match_callers(&db, target, &callers, "parse_attrs real-corpus callers")?;
    let mut actual_by_owner_path = std::collections::BTreeMap::<_, usize>::new();
    for caller in &callers {
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::Function);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Function);
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
        actual_by_owner_path, expected_by_owner_path,
        "target-centered parse_attrs callers should match the source-oracle owner fanout"
    );

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
        expected_owners.len(),
        "parse_attrs target expansion should return one one-hop candidate per owner"
    );
    for owner in expected_owners {
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

    let inferred_state = function_id_by_name_in_module(
        &db,
        &["crate", "from_request"],
        "infer_state_type_from_field_attributes",
    )?;
    let inferred_state_context = db.call_context_for_owner(inferred_state)?;
    assert!(
        inferred_state_context.iter().all(|row| {
            row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&path(&["parse_attrs"]))
        }),
        "from_request::infer_state_type_from_field_attributes should not flatten closure-body parse_attrs rows from axum-macros/src/from_request/mod.rs:1029 and :1039: {inferred_state_context:#?}"
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
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,592,715,880,896}
    //   call imported `parse_attrs(...)`.
    // Expected proof traversal: all eight current caller sites project
    // call_site, call_resolution, call_edge, and per-source provenance facts.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        8,
        "parse_attrs proof setup should use the current eight corpus callers: {callers:#?}"
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
            ("axum-macros/src/from_request/mod.rs", 6),
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
fn axum_real_target_take_route_helper_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `take_route_or_internal_error` path row.
    // Source chain:
    //   axum/src/routing/mod.rs:63 defines `take_route_or_internal_error`.
    //   routing/mod.rs:410,430 and routing/tests/mod.rs:56,59 call it.
    // Current model gap: the debug-only `super::...` test owner is not present
    // in this fixture, and the same-module callsites in `fallback_endpoint`
    // live inside closure bodies that are not projected yet. No local
    // traversal edge should be invented.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing"], "take_route_or_internal_error")?;
    assert_no_path_rows(&db, &["super", "take_route_or_internal_error"])?;
    assert_no_path_rows(&db, &["take_route_or_internal_error"])?;

    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "take_route_or_internal_error should remain targetless until routing same-module/super paths resolve: {callers:#?}"
    );
    let sites = db.call_sites_for_target(target)?;
    assert!(
        sites.is_empty(),
        "call_sites_for_target should mirror the absent routing helper rows: {sites:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        target,
        "axum/src/routing/mod.rs:410,430 and routing/tests/mod.rs:56,59 take_route_or_internal_error",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_turbofish_calls_are_documented_gaps() -> Result<(), DbError> {
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
    // Current model gap: closure bodies do not yet receive nested call owners,
    // so the `parse_args::<T>()` method call is not projected under
    // `parse_attrs`; no traversal edge should be invented.
    let owner = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let context = db.call_context_for_owner(owner)?;
    assert!(
        context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("parse_args")),
        "closure-body parse_args::<T>() should remain absent until closure owners are modeled: {context:#?}"
    );

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
    // two std-root rows as external and leaves the wrapper-body rows listed
    // below absent rather than inventing local traversal edges.
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

    assert_targetless_path_rows(&db, &["std", "mem", "replace"], CallStatusKind::External, 2)?;

    assert_no_method_owner_by_body_and_file_suffix(
        &db,
        "call",
        "let (mut parts, body) = req.into_parts();",
        "axum/src/error_handling/mod.rs",
        "axum/src/error_handling/mod.rs:181 macro-template std::mem::replace",
    )?;

    for (label, file_suffix) in [
        (
            "axum/src/middleware/map_request.rs:281",
            "axum/src/middleware/map_request.rs",
        ),
        (
            "axum/src/middleware/from_fn.rs:285",
            "axum/src/middleware/from_fn.rs",
        ),
        (
            "axum/src/middleware/map_response.rs:260",
            "axum/src/middleware/map_response.rs",
        ),
    ] {
        assert_no_method_owner_by_body_and_file_suffix(
            &db,
            "call",
            "std::mem::replace(&mut self.inner, not_ready_inner)",
            file_suffix,
            label,
        )?;
    }

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
    //   axum/src/extract/raw_form.rs:65 calls `Body::empty()` through a
    //   direct `axum_core::body::Body` import inside a test helper.
    // Expected traversal for the current fixture: two `Body::empty` rows and
    // two `Self::empty` rows reach the same target in one edge. The raw_form
    // helper row is structurally present but targetless and classified external
    // because the owner imports `axum_core::body::Body` across the axum member
    // boundary.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        4,
        "current axum fixture should resolve exactly the four axum-core Body::empty callers: {callers:#?}"
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
        4,
        "Body::empty target expansion should traverse the four current resolved edges: {incoming:#?}"
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
            (path(&["Body", "empty"]), 2),
            (path(&["Self", "empty"]), 2),
        ]),
        "Body::empty callers should split into literal Body::empty and trait-impl Self::empty rows"
    );

    let raw_form_owner = function_id_by_name_in_module(
        &db,
        &["crate", "extract", "raw_form", "tests"],
        "check_query",
    )?;
    assert_owner_path_targetless(
        &db,
        raw_form_owner,
        &["Body", "empty"],
        CallStatusKind::External,
        "axum/src/extract/raw_form.rs:65",
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
        4,
        "Body::empty proof setup should use the current four resolved corpus callers: {callers:#?}"
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
            (path(&["Body", "empty"]), 2),
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
        ],
        "type_resolution_missing",
    )
}

#[test]
fn axum_real_target_generated_post_function_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: generated `routing::post` handler fanout.
    // Source chain:
    //   axum/src/routing/method_routing.rs:165 template; macro invocation :445.
    //   JSON, multipart, method_routing, and routing tests call `post(...)`.
    // Current model gap: this corpus fixture does not expose a generated
    // top-level function named `post`, so target-centered traversal cannot bind
    // these real callsites yet.
    let rows = db.raw_query(
        r#"?[id] :=
            *function { id, name: "post" @ 'NOW' }"#,
    )?;
    assert!(
        rows.rows.is_empty(),
        "generated routing::post should remain absent until macro-generated handler functions are modeled: {rows:#?}"
    );

    // Matrix immediate candidate:
    //   axum/src/json.rs:237 imports `routing::post` in a grouped import.
    //   axum/src/json.rs:248 calls `post(echo_json)` from `deserialize_body`.
    // Current DB contract: the grouped-import callsite is visible, but it
    // remains unsupported and targetless because the generated `post` function
    // binding itself is absent.
    let json_owner =
        function_id_by_name_in_module(&db, &["crate", "json", "tests"], "deserialize_body")?;
    assert_owner_path_targetless(
        &db,
        json_owner,
        &["post"],
        CallStatusKind::Unsupported,
        "axum/src/json.rs:248 grouped-import post",
    )?;
    for (owner_name, label) in [
        (
            "consume_body_to_json_requires_json_content_type",
            "json.rs:264",
        ),
        ("json_content_types", "json.rs:279"),
        ("invalid_json_syntax", "json.rs:299"),
        ("extra_chars_after_valid_json_syntax", "json.rs:318"),
        ("invalid_json_data", "json.rs:353"),
    ] {
        let owner = function_id_by_name_in_module(&db, &["crate", "json", "tests"], owner_name)?;
        assert_owner_path_targetless(&db, owner, &["post"], CallStatusKind::Unsupported, label)?;
    }

    for (owner_name, label) in [
        (
            "content_type_with_encoding",
            "axum/src/extract/multipart.rs:381",
        ),
        ("body_too_large", "axum/src/extract/multipart.rs:420"),
        ("optional_multipart", "axum/src/extract/multipart.rs:448"),
    ] {
        let rows = db.raw_query_params(
            r#"?[id] :=
                *function { id, name: $name @ 'NOW' }"#,
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
        assert_owner_path_targetless(&db, owner, &["post"], CallStatusKind::Unsupported, label)?;
    }

    struct RoutingPostCase {
        owner: &'static str,
        expected_count: usize,
        label: &'static str,
    }

    // Matrix routing-test rows:
    //   routing/tests/mod.rs:88,768,810,888-890,916,936,956,
    //   982,986,988,1043,1306 call generated `post(...)`.
    // These 14 projected rows are grouped by owner because the DB fixture does
    // not persist source line numbers for individual call sites.
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
        assert_owner_path_targetless_count(
            &db,
            owner,
            &["post"],
            CallStatusKind::Unsupported,
            case.expected_count,
            case.label,
        )?;
    }

    let logging_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "tests"], "logging_rejections")?;
    let logging_context = db.call_context_for_owner(logging_owner)?;
    assert!(
        logging_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&path(&["post"]))),
        "axum/src/routing/tests/mod.rs:1215 closure-body post(...) should remain absent until closure ownership is modeled: {logging_context:#?}"
    );

    assert_targetless_path_rows(&db, &["post"], CallStatusKind::Unsupported, 22)?;

    Ok(())
}

#[test]
fn axum_real_target_try_downcast_helpers_reach_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

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

    let turbofish_owner =
        function_id_by_name_in_module(&db, &["crate", "body"], "test_try_downcast")?;
    let turbofish_context = db.call_context_for_owner(turbofish_owner)?;
    let turbofish_path_rows = turbofish_context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path
                && row.site.path.as_ref() == Some(&path(&["try_downcast"]))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        turbofish_path_rows.len(),
        0,
        "axum-core/src/body.rs:251 and :252 are inside assert_eq! macro invocations and should not be flattened into try_downcast path rows: {turbofish_context:#?}"
    );
    assert_eq!(
        turbofish_context.len(),
        2,
        "axum-core/src/body.rs:251 and :252 should project two unsupported macro-bound rows: {turbofish_context:#?}"
    );
    for row in &turbofish_context {
        assert_targetless_status(row, CallStatusKind::Unsupported);
        assert_eq!(row.site.generic_arg_count, None);
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "macro-bound try_downcast rows should not fabricate local targets"
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
    // Current model gap: the function owner only projects the two setup
    // `routing::get` path calls at routing/tests/mod.rs:412-413 plus macro
    // rows for the assertions; it must not fabricate edges from the shadowed
    // local closure calls to the imported routing helper.
    let owner = function_id_by_name(&db, "what_matches_wildcard")?;
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
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert_eq!(row.site.arg_count, Some(1));
        assert!(
            row.targets.is_empty(),
            "shadowed get/routing get rows should remain targetless in current fixture: {row:#?}"
        );
        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "shadowed get/routing get setup row should have zero persisted call edges"
        );
        assert_no_traversal_candidates_for_site(
            &db,
            owner,
            row.site.id,
            "axum/src/routing/tests/mod.rs:412-413 setup routing::get rows",
        )?;
    }

    Ok(())
}
