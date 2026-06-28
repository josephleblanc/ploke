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
fn axum_real_target_imported_parse_attrs_reaches_helper_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;

    // Matrix: imported `parse_attrs` path rows.
    // Source chain:
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   from_ref.rs:9 imports it and from_ref.rs:30 calls it from
    //   `expand_field`.
    //   from_request/mod.rs:3 imports it and current resolved rows are at
    //   :112, :196, :592, :715, :880, and :896.
    // Expected traversal: seven call-site edges reach `parse_attrs` in one
    // step; target expansion de-duplicates those to five owner candidates.
    let cases = [
        (
            "from_ref.rs:30 expand_field -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_ref"], "expand_field")?,
            1,
        ),
        (
            "from_request/mod.rs:{112,196} expand -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?,
            2,
        ),
        (
            "from_request/mod.rs:592 extract_fields -> parse_attrs",
            function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?,
            1,
        ),
        (
            "from_request/mod.rs:715 impl_struct_by_extracting_all_at_once -> parse_attrs",
            function_id_by_name_in_module(
                &db,
                &["crate", "from_request"],
                "impl_struct_by_extracting_all_at_once",
            )?,
            1,
        ),
        (
            "from_request/mod.rs:{880,896} impl_enum_by_extracting_all_at_once -> parse_attrs",
            function_id_by_name_in_module(
                &db,
                &["crate", "from_request"],
                "impl_enum_by_extracting_all_at_once",
            )?,
            2,
        ),
    ];

    let mut expected_by_owner = std::collections::BTreeMap::new();
    for (label, owner, expected_edges) in cases {
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref() == Some(&path(&["parse_attrs"]))
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
        expected_by_owner.insert(owner, expected_edges);
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        7,
        "parse_attrs should expose the seven currently resolved imported callers: {callers:#?}"
    );
    let mut actual_by_owner = std::collections::BTreeMap::<_, usize>::new();
    for caller in &callers {
        assert_eq!(caller.site.path, Some(path(&["parse_attrs"])));
        assert_eq!(caller.status.status, CallStatusKind::Resolved);
        assert_eq!(
            caller.status.resolution,
            Some(CallResolutionKind::LocalExact)
        );
        assert_eq!(caller.target.relation, CallRelationKind::Function);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Function);
        *actual_by_owner.entry(caller.site.owner_id).or_default() += 1;
    }
    assert_eq!(
        actual_by_owner, expected_by_owner,
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
        expected_by_owner.len(),
        "parse_attrs target expansion should return one one-hop candidate per owner"
    );
    for owner in expected_by_owner.keys().copied() {
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
    assert_targetless_path_rows(&db, &["std", "mem", "replace"], CallStatusKind::External, 2)?;

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

#[test]
fn axum_real_target_body_empty_projects_proof_facts() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;

    // Matrix proof bridge:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/response/into_response.rs:128 and :163 call
    //   `Body::empty()`.
    // Expected proof traversal: both current resolved caller sites project
    // call_site, call_resolution, and call_edge facts for the same target.
    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        2,
        "Body::empty proof setup should use the current two resolved corpus callers: {callers:#?}"
    );
    for caller in &callers {
        assert_eq!(caller.site.path, Some(path(&["Body", "empty"])));
        assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
        assert_eq!(caller.target.source_kind, CallSiteKind::Path);
        assert_eq!(caller.target.target_kind, CallTargetKind::Method);
    }
    let expected = callers
        .iter()
        .map(|caller| TargetProofSite {
            owner: caller.site.owner_id,
            site: caller.site.id,
        })
        .collect::<Vec<_>>();

    assert_target_proof_projection(
        &db,
        "real corpus Body::empty",
        "bd:corpus-axum-call-graph",
        target,
        &callers,
        &expected,
        "axum-core/src/response/into_response.rs",
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
    assert_targetless_path_rows(&db, &["post"], CallStatusKind::Unsupported, 22)?;

    Ok(())
}

#[test]
fn axum_real_target_try_downcast_helpers_reach_current_resolved_subset() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: same-named `try_downcast` helpers.
    // Source chain:
    //   axum-core/src/body.rs:23 defines the axum-core helper; body.rs callsites
    //   at :20, :48, :224, and :225 are the source oracle.
    //   axum/src/util.rs:99 defines the axum helper; routing/mod.rs:205 and
    //   util.rs:114,115 are the source oracle.
    // Expected traversal for the current fixture: two resolved callers for the
    // axum-core helper and one resolved caller for the axum helper.
    let cases = [
        (
            "axum-core try_downcast",
            function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
            2,
        ),
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

    Ok(())
}

#[test]
fn axum_real_target_position_first_variant_is_documented_gap() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Matrix: `Position::First` enum variant constructor row.
    // Source chain:
    //   axum-macros/src/with_position.rs:65 defines `Position`.
    //   with_position.rs:66 defines variant `First`.
    //   with_position.rs:92 calls `Position::First(item)`.
    // Current model gap: the constructor call is structurally present, but it
    // does not produce a resolved target-centered edge yet.
    let position = variant_id_by_enum_and_variant_names(&db, "Position", "First")?;
    let owner = method_id_by_name_and_body_substring(&db, "next", "Position::First(item)")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["Position", "First"]);

    assert_eq!(row.status.status, CallStatusKind::Unresolved);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Position::First constructor row should remain targetless: {row:#?}"
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing
            .iter()
            .all(|candidate| candidate.target_id != position),
        "targetless Position::First row should not traverse to the enum variant: {outgoing:#?}"
    );

    let callers = db.callers_for_target(position)?;
    assert!(
        callers.is_empty(),
        "Position::First should remain targetless until enum variant constructor resolution covers this corpus row: {callers:#?}"
    );

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
    }

    Ok(())
}
