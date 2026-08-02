use super::super::*;
use super::common::*;
use super::source_lines::*;
use ploke_test_utils::CORPUS_AXUM_CALL_GRAPH;
use uuid::Uuid;

const fn fanout(file_suffix: &'static str, lines: &'static [u32]) -> SourceLineFanout {
    SourceLineFanout { file_suffix, lines }
}

fn assert_path_edge(
    db: &Database,
    context: &[CallContextRow],
    owner: Uuid,
    call_path: &[&str],
    target: Uuid,
    label: &'static str,
) -> Result<Uuid, DbError> {
    let row = row_by_path(context, call_path);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
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
    )?;
    Ok(row.site.id)
}

fn assert_generated_rows(
    db: &Database,
    context: &[CallContextRow],
    owner: Uuid,
    target: Uuid,
    label: &str,
) -> Result<Vec<Uuid>, DbError> {
    let target_sites = db.call_sites_for_target(target)?;
    let mut sites = Vec::new();
    for index in 1..=16 {
        let ty = format!("T{index}");
        let expected = path(&[ty.as_str(), "from_request_parts"]);
        let row = context
            .iter()
            .find(|row| {
                row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&expected)
            })
            .unwrap_or_else(|| {
                panic!("{label} should expose {ty}::from_request_parts: {context:#?}")
            });
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
        assert_eq!(
            relations_for_site(db, row.site.id)?.rows.len(),
            1,
            "{label} {ty}::from_request_parts should persist one call edge"
        );
        assert!(
            target_sites
                .iter()
                .any(|site| site.owner_id == owner && site.id == row.site.id),
            "target-centered sites should include {label} {ty}::from_request_parts: {target_sites:#?}"
        );
        sites.push(row.site.id);
    }
    Ok(sites)
}

#[test]
fn axum_real_target_trait_associated_paths_reach_trait_methods() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Trait bindings are extract/mod.rs:53/:59 and :79/:85; each labeled call below has one edge.
    let from_request = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let from_request_parts =
        method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    let cases = [
        (
            "axum-core/src/ext_traits/request.rs:279",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request(self, state)",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request"][..],
            from_request,
        ),
        (
            "axum-core/src/extract/mod.rs:127",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request",
                "T::from_request(req, state).await",
                "axum-core/src/extract/mod.rs",
            )?,
            &["T", "from_request"][..],
            from_request,
        ),
        (
            "axum-core/src/ext_traits/request.rs:305",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_parts_with_state",
                "E::from_request_parts(&mut parts, state).await",
                "axum-core/src/ext_traits/request.rs",
            )?,
            &["E", "from_request_parts"][..],
            from_request_parts,
        ),
        (
            "axum-core/src/extract/mod.rs:115",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "T::from_request_parts(parts, state).await",
                "axum-core/src/extract/mod.rs",
            )?,
            &["T", "from_request_parts"][..],
            from_request_parts,
        ),
        (
            "axum-core/src/ext_traits/request_parts.rs:133",
            method_id_by_name_body_and_file_suffix(
                &db,
                "extract_with_state",
                "E::from_request_parts(self, state)",
                "axum-core/src/ext_traits/request_parts.rs",
            )?,
            &["E", "from_request_parts"][..],
            from_request_parts,
        ),
        (
            "axum/src/middleware/from_extractor.rs:220",
            async_block_owner_for_method_parent(
                &db,
                method_id_by_name_body_and_file_suffix(
                    &db,
                    "call",
                    "E::from_request_parts(&mut parts, &state).await",
                    "axum/src/middleware/from_extractor.rs",
                )?,
            )?,
            &["E", "from_request_parts"][..],
            from_request_parts,
        ),
    ];
    for (label, owner, path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_path_edge(&db, &context, owner, path, target, label)?;
    }

    let from_request_callers = db.callers_for_target(from_request)?;
    assert_eq!(
        from_request_callers.len(),
        34,
        "FromRequest::from_request should expose hand-written bounded callers plus generated Handler and tuple extractor arity callers: {from_request_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        from_request,
        &from_request_callers,
        "FromRequest::from_request real-corpus callers",
    )?;

    let from_request_parts_callers = db.callers_for_target(from_request_parts)?;
    assert_eq!(
        from_request_parts_callers.len(),
        517,
        "FromRequestParts::from_request_parts should expose all inspected bounded callers plus generated Handler, HandleError service, and tuple extractor callers: {from_request_parts_callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        from_request_parts,
        &from_request_parts_callers,
        "FromRequestParts::from_request_parts real-corpus callers",
    )?;

    Ok(())
}

#[test]
fn axum_core_tuple_impl_from_request_projects_generated_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // tuple.rs:18-77 generates arities 1-16; calls at :29/:52/:57 retain exact owners and edges.
    let from_request = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let from_request_parts =
        method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    let parts_owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "T1 :: from_request_parts (parts , state) . await",
        "axum-core/src/extract/tuple.rs",
    )?;
    assert_eq!(
        parts_owners.len(),
        16,
        "tuple impl_from_request! should generate sixteen FromRequestParts owners"
    );
    let mut parts_owner = None;
    for owner in parts_owners {
        let context = db.call_context_for_owner(owner)?;
        let rows = context
            .iter()
            .filter(|row| {
                row.site.kind == CallSiteKind::Path
                    && row.site.path.as_ref().is_some_and(|path| {
                        path.last().is_some_and(|part| part == "from_request_parts")
                    })
            })
            .collect::<Vec<_>>();
        if rows.len() == 1
            && rows[0].site.path.as_ref() == Some(&path(&["T1", "from_request_parts"]))
        {
            parts_owner = Some(owner);
            break;
        }
    }
    let parts_owner = parts_owner.expect("expected generated arity-1 tuple FromRequestParts owner");
    assert_method_owner_impl_trait(
        &db,
        parts_owner,
        "FromRequestParts",
        "generated arity-1 tuple FromRequestParts::from_request_parts",
    )?;
    let parts_context = db.call_context_for_owner(parts_owner)?;
    assert_path_edge(
        &db,
        &parts_context,
        parts_owner,
        &["T1", "from_request_parts"],
        from_request_parts,
        "axum-core/src/extract/tuple.rs:29 generated arity-1 T1::from_request_parts",
    )?;

    let arity_sixteen = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "T16 :: from_request_parts (parts , state) . await",
        "axum-core/src/extract/tuple.rs",
    )?;
    assert_method_owner_impl_trait(
        &db,
        arity_sixteen,
        "FromRequestParts",
        "generated arity-16 tuple FromRequestParts::from_request_parts",
    )?;
    let arity_sixteen_context = db.call_context_for_owner(arity_sixteen)?;
    assert_generated_rows(
        &db,
        &arity_sixteen_context,
        arity_sixteen,
        from_request_parts,
        "axum-core/src/extract/tuple.rs:29/:52 generated arity-16 tuple",
    )?;

    let request_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request",
        "T2 :: from_request (req , state) . await",
        "axum-core/src/extract/tuple.rs",
    )?;
    assert_method_owner_impl_trait(
        &db,
        request_owner,
        "FromRequest",
        "generated arity-2 tuple FromRequest::from_request",
    )?;
    let request_body = async_block_owner_for_method_parent(&db, request_owner)?;
    let request_context = db.call_context_for_owner(request_body)?;
    assert_path_edge(
        &db,
        &request_context,
        request_body,
        &["T1", "from_request_parts"],
        from_request_parts,
        "axum-core/src/extract/tuple.rs:52 generated arity-2 T1::from_request_parts",
    )?;
    assert_path_edge(
        &db,
        &request_context,
        request_body,
        &["T2", "from_request"],
        from_request,
        "axum-core/src/extract/tuple.rs:57 generated arity-2 T2::from_request",
    )?;

    assert_no_path_rows(&db, &["ty", "from_request_parts"])?;
    assert_no_path_rows(&db, &["last", "from_request"])
}

#[test]
fn axum_real_target_from_ref_bounded_paths_reach_trait_method() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // from_ref.rs:13/:15 binds the trait method; ext_traits/mod.rs:25/:45 traverse once.
    let target = method_id_by_trait_name(&db, "FromRef", "from_ref")?;

    let cases = [
        (
            "axum-core/src/ext_traits/mod.rs:25",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "InnerState::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["InnerState", "from_ref"][..],
            target,
        ),
        (
            "axum-core/src/ext_traits/mod.rs:45",
            method_id_by_name_body_and_file_suffix(
                &db,
                "from_request_parts",
                "String::from_ref(state)",
                "axum-core/src/ext_traits/mod.rs",
            )?,
            &["String", "from_ref"][..],
            target,
        ),
    ];
    for (label, owner, path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_path_edge(&db, &context, owner, path, target, label)?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(
        callers.len(),
        4,
        "FromRef::from_ref should expose the two same-crate callers plus both axum dependency-root callers: {callers:#?}"
    );
    assert_sites_match_callers(
        &db,
        target,
        &callers,
        "FromRef::from_ref real-corpus callers",
    )?;

    Ok(())
}

#[test]
fn axum_real_target_from_ref_dependency_root_bound_reaches_workspace_trait_method()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // state.rs:1/:309 and from_extractor.rs:306/:328 import FromRef and traverse one edge.
    let target = method_id_by_trait_name(&db, "FromRef", "from_ref")?;
    let state_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_request_parts",
        "InnerState::from_ref(state)",
        "axum/src/extract/state.rs",
    )?;
    let state_context = db.call_context_for_owner(state_owner)?;
    let state_site = assert_path_edge(
        &db,
        &state_context,
        state_owner,
        &["InnerState", "from_ref"],
        target,
        "axum/src/extract/state.rs:309",
    )?;

    let middleware_test = function_id_by_name(&db, "test_from_extractor")?;
    let middleware_owner = local_item_owner_for_parent_with_label(
        &db,
        middleware_test,
        "local_impl_method:from_request_parts",
    )?;
    let middleware_context = db.call_context_for_owner(middleware_owner)?;
    let middleware_site = assert_path_edge(
        &db,
        &middleware_context,
        middleware_owner,
        &["Secret", "from_ref"],
        target,
        "axum/src/middleware/from_extractor.rs:328",
    )?;

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    let cases = [
        (
            state_site,
            state_owner,
            "axum/src/extract/state.rs:309 dependency-root proof",
        ),
        (
            middleware_site,
            middleware_owner,
            "axum/src/middleware/from_extractor.rs:328 dependency-root proof",
        ),
    ];
    records.extend(cases.map(|(site, owner, _)| {
        ploke_test_utils::axum_dependency_record(domain_id, site, owner, target)
    }));
    db.upsert_proof_fact_values(&records)?;

    let target_id = target.to_string();
    let proof_rows = db.proof_symbol_lookup(&target_id)?;
    for (site, owner, label) in cases {
        assert_root_proof(&proof_rows, site, owner, target, label);
    }

    Ok(())
}

fn assert_root_proof(
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
                && row.target_kind.as_deref() == Some("workspace_trait_method")
                && row.target_name.as_deref() == Some("axum_core::extract::FromRef::from_ref")
                && row.target_root.as_deref() == Some("axum-core/src/extract/from_ref.rs")
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "{label} should expose an admitted dependency-root proof row: {rows:#?}"
    );
}

#[test]
fn axum_real_target_self_accept_rows_are_external_frontiers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // listener.rs:41/:61 Self::accept rows are external and never recursive edges.
    let owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "accept",
        "Self::accept(self).await",
        "axum/src/serve/listener.rs",
    )?;
    assert_eq!(
        owners.len(),
        2,
        "expected TcpListener and UnixListener accept method bodies to be visible under linux cfg"
    );
    for owner in owners {
        assert_owner_path_targetless(
            &db,
            owner,
            &["Self", "accept"],
            CallStatusKind::External,
            "axum/src/serve/listener.rs Self::accept",
        )?;
    }
    assert_targetless_path_rows(&db, &["Self", "accept"], CallStatusKind::External, 2)?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["Self", "accept"],
        CallStatusKind::External,
        &[fanout("axum/src/serve/listener.rs", &[41, 61])],
    )
}

#[test]
fn axum_real_target_header_value_from_static_external_paths_are_targetless() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // ws.rs:382/:384 stay absent; route.rs:202, json.rs:208/:217, and response rows stay external.
    assert_no_method_owner_by_body_and_file_suffix(
        &db,
        "on_upgrade",
        "const UPGRADE: HeaderValue = HeaderValue::from_static(\"upgrade\")",
        "axum/src/extract/ws.rs",
        "axum/src/extract/ws.rs:382 and :384",
    )?;

    let route_owner =
        function_id_by_name_in_module(&db, &["crate", "routing", "route"], "set_content_length")?;
    let route_context = db.call_context_for_owner(route_owner)?;
    let json_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_response",
        "serde_json::to_writer(&mut buf, &self.0)",
        "axum/src/json.rs",
    )?;
    let json_context = db.call_context_for_owner(json_owner)?;
    for (label, context) in [
        ("axum/src/routing/route.rs:202 local const", &route_context),
        (
            "axum/src/json.rs:208/:217 nested local function",
            &json_context,
        ),
    ] {
        assert!(
            context.iter().all(|row| {
                row.site
                    .path
                    .as_ref()
                    .is_none_or(|path| path != &["HeaderValue", "from_static"])
            }),
            "{label} HeaderValue::from_static rows should remain absent under the outer owner: {context:#?}"
        );
    }
    assert_targetless_path_owner_kind_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "LocalItem",
        3,
        "axum local const HeaderValue::from_static rows",
    )?;
    assert_targetless_path_owner_kind_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "LocalItem",
        &[
            fanout("axum/src/json.rs", &[208, 217]),
            fanout("axum/src/routing/route.rs", &[202]),
        ],
    )?;

    let html_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "into_response",
        "mime::TEXT_HTML_UTF_8.as_ref()",
        "axum/src/response/mod.rs",
    )?;
    assert_owner_path_targetless(
        &db,
        html_owner,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        "axum/src/response/mod.rs:47 HeaderValue::from_static",
    )?;

    #[rustfmt::skip]
    let core_cases = [
        ("axum-core/src/response/into_response.rs:196 HeaderValue::from_static", "mime::TEXT_PLAIN_UTF_8.as_ref()", 1),
        ("axum-core/src/response/into_response.rs:207 and :320 HeaderValue::from_static", "Body::from(self).into_response();res.headers_mut().insert(header::CONTENT_TYPE,HeaderValue::from_static(mime::APPLICATION_OCTET_STREAM.as_ref())", 2),
        ("axum-core/src/response/into_response.rs:232 HeaderValue::from_static", "BytesChainBody", 1),
    ];
    for (label, body_marker, expected_owners) in core_cases {
        let owners = method_ids_by_name_body_and_file_suffix(
            &db,
            "into_response",
            body_marker,
            "axum-core/src/response/into_response.rs",
        )?;
        assert_eq!(
            owners.len(),
            expected_owners,
            "{label} should resolve the expected into_response owner set"
        );
        for owner in owners {
            assert_owner_path_targetless(
                &db,
                owner,
                &["HeaderValue", "from_static"],
                CallStatusKind::External,
                label,
            )?;
        }
    }

    let const_rows = db.raw_query(
        r#"?[site_id] :=
            *const { id: owner_id @ 'NOW' },
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: ["HeaderValue", "from_static"] @ 'NOW'
            }"#,
    )?;
    assert!(
        const_rows.rows.is_empty(),
        "HeaderValue::from_static should not be modeled as item-level const-owned rows: {const_rows:#?}"
    );

    assert_targetless_path_rows(
        &db,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        8,
    )?;
    assert_targetless_path_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        &["HeaderValue", "from_static"],
        CallStatusKind::External,
        &[
            fanout(
                "axum-core/src/response/into_response.rs",
                &[196, 207, 232, 320],
            ),
            fanout("axum/src/response/mod.rs", &[47]),
        ],
    )
}

#[test]
fn axum_real_target_trait_object_dispatch_rows_are_documented_gaps() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Future::poll at error_handling.rs:251 and middleware/serve fanouts stays targetless.
    assert_targetless_method_result_field_rows(
        &db,
        "poll",
        "as_mut",
        &["inner"],
        CallStatusKind::Unsupported,
        3,
    )?;
    assert_targetless_method_result_field_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll",
        "as_mut",
        &["inner"],
        CallStatusKind::Unsupported,
        &[
            fanout("axum/src/middleware/from_fn.rs", &[375]),
            fanout("axum/src/middleware/map_request.rs", &[345]),
            fanout("axum/src/middleware/map_response.rs", &[333]),
        ],
    )?;
    assert_targetless_method_result_field_rows(
        &db,
        "poll",
        "as_mut",
        &["0"],
        CallStatusKind::Unsupported,
        1,
    )?;
    assert_targetless_method_result_field_line_fanout(
        &db,
        &CORPUS_AXUM_CALL_GRAPH,
        "poll",
        "as_mut",
        &["0"],
        CallStatusKind::Unsupported,
        &[fanout("axum/src/serve/mod.rs", &[485])],
    )?;
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "poll",
        "self.project().future.poll(cx)",
        "axum/src/error_handling/mod.rs",
    )?;
    assert_owner_method_targetless(
        &db,
        owner,
        "project",
        &CallReceiver::SelfValue,
        CallStatusKind::Unresolved,
        "axum/src/error_handling/mod.rs:251 self.project() receiver setup",
    )?;
    assert_owner_method_result_field_targetless(
        &db,
        owner,
        "poll",
        "project",
        &["future"],
        CallStatusKind::Unsupported,
        "axum/src/error_handling/mod.rs:251 dyn Future::poll receiver dispatch",
    )?;
    for (label, owner) in [
        (
            "axum-core/src/body.rs:32 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "body"], "try_downcast")?,
        ),
        (
            "axum/src/util.rs:105 <dyn Any>::downcast_mut",
            function_id_by_name_in_module(&db, &["crate", "util"], "try_downcast")?,
        ),
    ] {
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_path(&context, &["std", "any", "Any", "downcast_mut"]);
        assert_external_targetless(row);
        assert_eq!(row.site.arg_count, Some(1), "{label} arg count");
        assert_eq!(
            row.site.generic_arg_count,
            Some(1),
            "{label} generic arg count"
        );
        assert_no_traversal_candidates_for_site(&db, owner, row.site.id, label)?;
    }

    assert_targetless_path_rows(
        &db,
        &["std", "any", "Any", "downcast_mut"],
        CallStatusKind::External,
        2,
    )?;
    Ok(())
}

#[test]
fn axum_real_target_blanket_via_parts_self_path_reaches_trait_method() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let target = method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    // extract/mod.rs:103 resolves async-block-owned Self::from_request_parts through ViaParts.
    let callers = db.callers_for_target(target)?;
    let caller = callers
        .iter()
        .find(|caller| {
            caller.site.kind == CallSiteKind::Path
                && caller.site.path.as_deref() == Some(&path(&["Self", "from_request_parts"]))
                && matches!(
                    owner_kind_for_call_body_owner(&db, caller.site.owner_id),
                    Ok(kind) if kind == "AsyncBlock"
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "FromRequestParts::from_request_parts should include the async-block Self::from_request_parts caller: {callers:#?}"
            )
        });
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.target_id, target);
    assert_eq!(caller.target.relation, CallRelationKind::AssociatedFunction);
    assert_one_edge_traversal(
        &db,
        TraversalExpectation {
            label: "axum-core/src/extract/mod.rs:103 async-block Self::from_request_parts",
            owner: caller.site.owner_id,
            target,
            site_id: caller.site.id,
            expected_edge_count: 1,
        },
    )?;
    Ok(())
}

#[test]
fn axum_real_target_handler_macro_extraction_paths_project_generated_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // handler/mod.rs:242/:250 projects concrete generated paths under their async owners.
    let one_param = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "T1 :: from_request (req , & state) . await",
        "axum/src/handler/mod.rs",
    )?;
    assert_method_owner_impl_trait(&db, one_param, "Handler", "generated arity-1 Handler::call")?;
    let one_param_body = async_block_owner_for_method_parent(&db, one_param)?;
    let one_param_context = db.call_context_for_owner(one_param_body)?;
    let from_request = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let from_request_parts =
        method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;
    assert_path_edge(
        &db,
        &one_param_context,
        one_param_body,
        &["T1", "from_request"],
        from_request,
        "axum/src/handler/mod.rs:250 generated arity-1 T1::from_request",
    )?;

    let two_param = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "T2 :: from_request (req , & state) . await",
        "axum/src/handler/mod.rs",
    )?;
    assert_method_owner_impl_trait(&db, two_param, "Handler", "generated arity-2 Handler::call")?;
    let two_param_body = async_block_owner_for_method_parent(&db, two_param)?;
    let two_param_context = db.call_context_for_owner(two_param_body)?;
    assert_path_edge(
        &db,
        &two_param_context,
        two_param_body,
        &["T1", "from_request_parts"],
        from_request_parts,
        "axum/src/handler/mod.rs:242 generated arity-2 T1::from_request_parts",
    )?;
    assert_path_edge(
        &db,
        &two_param_context,
        two_param_body,
        &["T2", "from_request"],
        from_request,
        "axum/src/handler/mod.rs:250 generated arity-2 T2::from_request",
    )?;
    assert_no_path_rows(&db, &["ty", "from_request_parts"])?;
    assert_no_path_rows(&db, &["last", "from_request"])
}

#[test]
fn axum_error_handling_impl_service_paths_project_generated_rows() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // error_handling/mod.rs:152-222 generates 16 Service::call extractor owner/path sets.
    let owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "call",
        "from_request_parts(&mut parts, &()).await",
        "axum/src/error_handling/mod.rs",
    )?;
    assert_eq!(
        owners.len(),
        16,
        "error_handling::impl_service! should generate exactly sixteen Service::call methods"
    );

    let arity_sixteen = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "T16::from_request_parts(&mut parts, &()).await",
        "axum/src/error_handling/mod.rs",
    )?;
    let async_owner = async_block_owner_for_method_parent(&db, arity_sixteen)?;
    let context = db.call_context_for_owner(async_owner)?;
    let target = method_id_by_trait_name(&db, "FromRequestParts", "from_request_parts")?;

    let site_ids = assert_generated_rows(
        &db,
        &context,
        async_owner,
        target,
        "axum/src/error_handling/mod.rs:152-222 generated arity-16",
    )?;

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(async_owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing.iter().any(|candidate| {
            candidate.node_id == target
                && candidate.target_id == target
                && candidate.relation == ploke_db::CallContextRelation::OutgoingTarget
                && candidate.distance == 1
                && site_ids.contains(&candidate.call_site_id)
        }),
        "generated arity-16 async owner should traverse to FromRequestParts::from_request_parts through one of its extractor rows: {outgoing:#?}"
    );

    Ok(())
}

#[test]
fn axum_real_target_handler_async_block_body_calls_are_async_block_owned() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // handler/mod.rs:217/:240 keeps self()/into_response() under the concrete async owner.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "call",
        "self().await.into_response()",
        "axum/src/handler/mod.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;

    assert!(
        context
            .iter()
            .all(|row| row.site.kind != CallSiteKind::Dynamic),
        "axum/src/handler/mod.rs:217 inner async-block `self()` must not be flattened into Handler::call: {context:#?}"
    );
    assert!(
        context
            .iter()
            .all(|row| row.site.method.as_deref() != Some("into_response")),
        "axum/src/handler/mod.rs:217 inner async-block `into_response()` must not be flattened into Handler::call: {context:#?}"
    );

    let async_owner = async_block_owner_for_method_parent(&db, owner)?;
    let async_context = db.call_context_for_owner(async_owner)?;
    let self_row = row_by_path(&async_context, &["self"]);
    assert_targetless_status(self_row, CallStatusKind::Unsupported);
    let into_response_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["self"]),
    };
    let into_response =
        row_by_method_receiver(&async_context, "into_response", &into_response_receiver);
    assert_targetless_status(into_response, CallStatusKind::Unsupported);
    assert_no_traversal_candidates_for_site(
        &db,
        async_owner,
        self_row.site.id,
        "axum/src/handler/mod.rs:217 Handler::call async-block self()",
    )?;
    assert_no_traversal_candidates_for_site(
        &db,
        async_owner,
        into_response.site.id,
        "axum/src/handler/mod.rs:217 Handler::call async-block into_response()",
    )?;

    assert!(
        db.project_call_proof_facts_for_owner(async_owner, "bd:corpus-axum-call-graph")? >= 4,
        "Handler::call async block should project targetless self()/into_response() proof rows"
    );
    db.upsert_proof_fact_values(&[
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(self_row.site.id, "self"),
        ploke_test_utils::axum_handler_async_block_poll_resume_blocker(
            into_response.site.id,
            "into_response",
        ),
    ])?;

    let sites = [
        (
            self_row.site.id.to_string(),
            "Handler::call async-block self()",
        ),
        (
            into_response.site.id.to_string(),
            "Handler::call async-block into_response()",
        ),
    ];
    let blockers = db.proof_blockers()?;
    for (site, label) in &sites {
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
            }),
            "{label} should expose the async poll/resume blocker: {blockers:#?}"
        );
    }

    let proof_rows = db.proof_graphrag_context("async poll/resume")?;
    for (site, label) in &sites {
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "{label} proof context should expose the async poll/resume blocker: {proof_rows:#?}"
        );
    }

    Ok(())
}

fn async_block_owner_for_method_parent(db: &Database, parent: Uuid) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id] :=
            parent = to_uuid("{parent}"),
            *call_body_owner {{
                id,
                owner_kind: "AsyncBlock",
                parent_id: parent,
                parent_kind: "Method",
                label: "async_block" @ 'NOW'
            }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one async_block call_body_owner row for method parent {parent}: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0])
}

fn assert_method_owner_impl_trait(
    db: &Database,
    method: Uuid,
    trait_name: &str,
    label: &str,
) -> Result<(), DbError> {
    let rows = db.raw_query(&format!(
        r#"?[impl_id, trait_type_id, trait_target_id] :=
            method = to_uuid("{method}"),
            *method {{ id: method, owner_id: impl_id @ 'NOW' }},
            *impl {{ id: impl_id, trait_type: trait_type_id @ 'NOW' }},
            *type_use {{
                owner_id: impl_id,
                root_type_id: trait_type_id,
                role: "ImplTrait" @ 'NOW'
            }},
            *type_relation {{
                source_id: trait_type_id,
                target_id: trait_target_id,
                relation_kind: "Trait" @ 'NOW'
            }},
            *trait {{ id: trait_target_id, name: "{trait_name}" @ 'NOW' }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{label} should be recorded as an impl of trait {trait_name}: {:#?}",
        rows.rows
    );
    Ok(())
}
