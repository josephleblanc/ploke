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
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "root expand should have exactly the inspected helper callers: {callers:#?}"
    );
    for case in cases {
        let owner = function_id_by_name_in_module(&db, &["crate"], case.owner_name)?;
        let caller = caller_by_owner_kind_path(&callers, owner, CallSiteKind::Path, case.path);
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
    }

    Ok(())
}

#[test]
fn axum_real_target_explicit_crate_path_parse_attrs_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `parse_attrs` path/import row.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(&input.attrs, "typed_path")`.
    // Current model gap: the callsite is structurally present, but this corpus
    // fixture does not yet resolve the explicit crate path to the helper.
    let owner = function_id_by_name_in_module(&db, &["crate", "typed_path"], "expand")?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["crate", "attr_parsing", "parse_attrs"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unresolved parse_attrs row should not expose traversal targets: {row:#?}"
    );
    assert!(
        db.callers_for_target(target)?.iter().all(|caller| {
            caller.site.owner_id != owner
                || caller.site.path.as_ref()
                    != Some(&path(&["crate", "attr_parsing", "parse_attrs"]))
        }),
        "the typed_path::expand parse_attrs row should remain targetless until explicit crate-path resolution handles this corpus row"
    );

    Ok(())
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
            },
        )?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        cases.len(),
        "run_ui_tests should have exactly the five inspected real-corpus callers"
    );

    Ok(())
}

#[test]
fn axum_real_target_take_route_helper_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `take_route_or_internal_error` path row.
    // Source chain:
    //   axum/src/routing/mod.rs:63 defines `take_route_or_internal_error`.
    //   routing/mod.rs:410,430 and routing/tests/mod.rs:56,59 call it.
    // Current model gap: this axum fixture has no resolved target-centered
    // callers for these same-module / super-path rows yet.
    let target =
        function_id_by_name_in_module(&db, &["crate", "routing"], "take_route_or_internal_error")?;
    let callers = db.callers_for_target(target)?;
    assert!(
        callers.is_empty(),
        "take_route_or_internal_error should remain targetless until routing same-module/super paths resolve: {callers:#?}"
    );

    Ok(())
}

#[test]
fn axum_real_target_external_path_rows_remain_targetless() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: external path rows.
    // Source chain:
    //   axum/src/json.rs:184 calls `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/response/sse.rs:449 calls `std::mem::replace(...)`.
    // Expected traversal: zero local call edges; both rows classify as external.
    let json_owner = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;
    let json_context = db.call_context_for_owner(json_owner)?;
    let json_row = row_by_path(&json_context, &["serde_json", "Deserializer", "from_slice"]);
    assert_external_targetless(json_row);

    let replace_owner =
        method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let replace_context = db.call_context_for_owner(replace_owner)?;
    let replace_row = row_by_path(&replace_context, &["std", "mem", "replace"]);
    assert_external_targetless(replace_row);

    Ok(())
}

#[test]
fn axum_real_target_body_empty_reaches_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    // Matrix: `Body::empty` re-exported constructor row.
    // Source chain:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/response/into_response.rs:128,163 call `Body::empty()`.
    // Expected traversal for the current fixture: two `into_response` callers,
    // each with one edge to `Body::empty`. The broader matrix fanout is a
    // remaining import/re-export completeness gap.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "current axum fixture should resolve exactly the two axum-core into_response Body::empty callers: {callers:#?}"
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
        2,
        "Body::empty target expansion should traverse the two current resolved edges: {incoming:#?}"
    );

    for caller in callers {
        assert_eq!(caller.site.path, Some(path(&["Body", "empty"])));
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

    Ok(())
}
