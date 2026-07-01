use std::collections::BTreeMap;

use ploke_db::{
    CallContextRelation, CallContextSeed, CallNodeKind, CallPathOptions, CallRelationKind,
};

use super::super::*;
use super::common::*;

#[test]
fn axum_usage_questions_have_multi_hop_navigation_and_impact_answers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Navigation:
    //   "From this owner function, which local callees can I traverse to in
    //   the persisted graph?"
    // Impact analysis:
    //   "Which callers eventually reach this function?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_name_body_and_file_suffix(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;
    let options = CallPathOptions {
        max_depth: 2,
        max_paths: 128,
    };

    let outgoing = db.call_paths_from_owner(start, options)?;
    assert_path_depths(
        &outgoing
            .iter()
            .map(|path| (path.end_id, path.depth))
            .collect::<Vec<_>>(),
        &[(intermediate, 1), (target, 2)],
        "RequestExt::extract outgoing traversal",
    );

    let incoming = db.call_paths_to_target(target, options)?;
    assert_path_depths(
        &incoming
            .iter()
            .map(|path| (path.start_id, path.depth))
            .collect::<Vec<_>>(),
        &[(intermediate, 1), (start, 2)],
        "FromRequest::from_request incoming traversal",
    );

    let direct_sites = db.call_sites_for_target(target)?;
    assert_eq!(
        direct_sites.len(),
        2,
        "target-centered direct callsite query should still answer exact callsite fanout: {direct_sites:#?}"
    );
    assert!(
        direct_sites
            .iter()
            .any(|site| site.owner_id == intermediate
                && site.path == Some(path(&["E", "from_request"]))),
        "direct callsite query should include the exact E::from_request source row: {direct_sites:#?}"
    );

    let expanded = db.expand_call_path_context(CallContextSeed::Target(target), options)?;
    let by_node = expanded
        .iter()
        .map(|candidate| (candidate.node_id, candidate.distance, candidate.relation))
        .collect::<Vec<_>>();
    assert!(
        by_node.contains(&(start, 2, CallContextRelation::IncomingCaller))
            && by_node.contains(&(intermediate, 1, CallContextRelation::IncomingCaller)),
        "path expansion should expose direct and eventual incoming callers for impact-style traversal: {expanded:#?}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_answer_direct_reachability_between_known_symbols() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Performance work / Debugging:
    //   "Can this entrypoint reach this sink/helper/error-producing function?"
    //   "What ordered call path connects the two known symbols?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_name_body_and_file_suffix(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let one_hop = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        one_hop.is_empty(),
        "direct reachability must not collapse the intermediate method: {one_hop:#?}"
    );

    let paths = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "expected a two-hop RequestExt::extract -> FromRequest::from_request path: {paths:#?}"
            )
        });
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
    assert_path_edge_span_matches_site(
        &db,
        start,
        path.edges[0].call_site_id,
        path.edges[0].span,
        "RequestExt::extract -> extract_with_state",
    )?;
    assert_path_edge_span_matches_site(
        &db,
        intermediate,
        path.edges[1].call_site_id,
        path.edges[1].span,
        "RequestExt::extract_with_state -> FromRequest::from_request",
    )?;

    Ok(())
}

#[test]
fn axum_usage_questions_summarize_owner_reach_for_navigation() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Navigation:
    //   "What functions does this request handler call directly?"
    //   "From this owner function, which local callees can I traverse to in
    //   the persisted graph?"
    // Security analysis / Performance work:
    //   "Which external dependency calls are made from this user-facing
    //   entrypoint?" and "What call chains reach a known sink/helper?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_name_body_and_file_suffix(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let report = db.call_reach_for_owner(
        start,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    assert_eq!(report.owner.id, start);
    assert_eq!(report.owner.kind, CallNodeKind::Method);
    assert_eq!(report.owner.name, "extract");

    assert_path_depths(
        &report
            .paths
            .iter()
            .map(|path| (path.end_id, path.depth))
            .collect::<Vec<_>>(),
        &[(intermediate, 1), (target, 2)],
        "RequestExt::extract reach report outgoing paths",
    );
    assert_node_names(
        &report.callees,
        &[
            (intermediate, "extract_with_state"),
            (target, "from_request"),
        ],
        "RequestExt::extract reach report eventual callees",
    );
    assert_node_names(
        &report.direct_callees,
        &[(intermediate, "extract_with_state")],
        "RequestExt::extract reach report direct callees",
    );
    assert_eq!(
        report.direct_call_sites.len(),
        1,
        "RequestExt::extract reach report should expose its exact resolved direct callsite row: {report:#?}"
    );
    let direct_site = report
        .direct_call_sites
        .iter()
        .find(|row| {
            row.site.owner_id == start
                && row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some("extract_with_state")
                && row.targets.iter().any(|target_row| target_row.target_id == intermediate)
        })
        .unwrap_or_else(|| {
            panic!(
                "RequestExt::extract reach report should include the extract_with_state callsite row: {report:#?}"
            )
    });
    assert_eq!(direct_site.status.status, CallStatusKind::Resolved);
    assert_eq!(
        direct_site.site.arg_count,
        Some(1),
        "RequestExt::extract should preserve the one explicit argument to extract_with_state: {direct_site:#?}"
    );
    assert!(
        report.boundary_call_sites.is_empty(),
        "RequestExt::extract direct call stays inside ext_traits::request and should not be a module-boundary row: {report:#?}"
    );
    assert_eq!(
        report.boundary_edges.len(),
        1,
        "RequestExt::extract reach should expose the transitive cross-module edge to FromRequest: {report:#?}"
    );
    let boundary_edge = report.boundary_edges[0];
    assert_eq!(boundary_edge.caller_id, intermediate);
    assert_eq!(boundary_edge.callee_id, target);
    assert_eq!(boundary_edge.source_kind, CallSiteKind::Path);
    assert_node_names(
        &report.public_callees,
        &[(target, "from_request")],
        "RequestExt::extract reach report public callees",
    );
    assert_source_file(
        &report.source_files,
        "axum-core/src/ext_traits/request.rs",
        "RequestExt::extract reach report source files",
    );
    assert_source_file(
        &report.source_files,
        "axum-core/src/extract/mod.rs",
        "RequestExt::extract reach report source files",
    );
    assert_source_module(
        &report.source_modules,
        &["crate", "ext_traits", "request"],
        "RequestExt::extract reach report source modules",
    );
    assert_source_module(
        &report.source_modules,
        &["crate", "extract"],
        "RequestExt::extract reach report source modules",
    );

    let boundary_report = db.call_reach_for_owner(
        intermediate,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert_eq!(
        boundary_report.boundary_call_sites.len(),
        1,
        "RequestExt::extract_with_state should expose its direct cross-module FromRequest callsite: {boundary_report:#?}"
    );
    let boundary = &boundary_report.boundary_call_sites[0];
    assert_eq!(boundary.site.owner_id, intermediate);
    assert_eq!(
        boundary.site.path.as_ref(),
        Some(&path(&["E", "from_request"]))
    );
    assert_eq!(
        boundary.site.arg_count,
        Some(2),
        "E::from_request should preserve the two explicit source arguments: {boundary:#?}"
    );
    assert!(
        boundary
            .targets
            .iter()
            .any(|target_row| target_row.target_id == target),
        "module-boundary row should target FromRequest::from_request: {boundary_report:#?}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_surface_external_frontier_for_dependency_calls() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which external dependency calls are made from this user-facing
    //   entrypoint?"
    // Performance work:
    //   "Which callers trigger repeated parsing, cloning, serialization, or
    //   database work?"
    // Debugging:
    //   "What source callsite corresponds to this persisted call edge or proof
    //   blocker?"
    //
    // Source oracle:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    // Current contract: dependency-root path calls are visible as external
    // frontier rows but do not become local traversal edges.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
        "axum/src/json.rs",
    )?;
    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;

    assert_eq!(report.owner.id, owner);
    assert_eq!(report.owner.name, "from_bytes");
    assert!(
        report.paths.is_empty() && report.callees.is_empty(),
        "external dependency calls should not fabricate local reach edges: {report:#?}"
    );

    let frontier = report
        .frontier_calls
        .iter()
        .find(|row| {
            row.site.path.as_ref() == Some(&path(&["serde_json", "Deserializer", "from_slice"]))
        })
        .unwrap_or_else(|| {
            panic!(
                "Json::from_bytes reach report should include serde_json frontier row: {report:#?}"
            )
        });
    assert_external_targetless(frontier);
    assert_eq!(frontier.site.owner_id, owner);
    let external_frontier = report
        .external_frontier_calls
        .iter()
        .find(|row| {
            row.site.path.as_ref() == Some(&path(&["serde_json", "Deserializer", "from_slice"]))
        })
        .unwrap_or_else(|| {
            panic!(
                "Json::from_bytes reach report should include serde_json in external frontier rows: {report:#?}"
            )
        });
    assert_external_targetless(external_frontier);
    assert_eq!(external_frontier.site.owner_id, owner);
    assert_source_file(
        &report.source_files,
        "axum/src/json.rs",
        "Json::from_bytes reach report source files",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_summarize_eventual_callers_for_impact() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Impact analysis:
    //   "Which callers eventually reach this function?"
    // Dead code detection:
    //   "Is this implementation truly unused, or is it only called through trait dispatch?"
    // Refactoring support:
    //   "Which callers need migration before this helper can be split or removed?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let start = method_id_by_name_body_and_file_suffix(
        &db,
        "extract",
        "self.extract_with_state(&())",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let intermediate = method_id_by_name_body_and_file_suffix(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let report = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    assert_eq!(report.target.id, target);
    assert_eq!(report.target.kind, CallNodeKind::Method);
    assert_eq!(report.target.name, "from_request");

    assert_path_depths(
        &report
            .paths
            .iter()
            .map(|path| (path.start_id, path.depth))
            .collect::<Vec<_>>(),
        &[(intermediate, 1), (start, 2)],
        "FromRequest::from_request impact report incoming paths",
    );
    assert_node_names(
        &report.callers,
        &[(intermediate, "extract_with_state"), (start, "extract")],
        "FromRequest::from_request impact report eventual callers",
    );
    assert_node_names(
        &report.direct_callers,
        &[(intermediate, "extract_with_state")],
        "FromRequest::from_request impact report direct callers",
    );
    assert_eq!(
        report.direct_call_sites.len(),
        2,
        "FromRequest::from_request impact report should expose both direct caller-site rows: {report:#?}"
    );
    assert!(
        report.direct_call_sites.iter().any(|row| {
            row.site.owner_id == intermediate
                && row.site.path.as_ref() == Some(&path(&["E", "from_request"]))
                && row.site.arg_count == Some(2)
                && row
                    .targets
                    .iter()
                    .any(|target_row| target_row.target_id == target)
        }),
        "FromRequest::from_request impact report should include the E::from_request callsite row: {report:#?}"
    );
    assert!(
        report.callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallRelationKind::AssociatedFunction
                && bucket.count == 2
        }),
        "FromRequest::from_request impact report should summarize direct path/associated-function callsites: {report:#?}"
    );
    assert!(
        report.public_callers.is_empty(),
        "direct stored-public filtering should not infer trait-effective visibility from inherited method rows: {report:#?}"
    );
    assert_source_file(
        &report.source_files,
        "axum-core/src/ext_traits/request.rs",
        "FromRequest::from_request impact report source files",
    );
    assert_source_file(
        &report.source_files,
        "axum-core/src/extract/mod.rs",
        "FromRequest::from_request impact report source files",
    );
    assert_source_module(
        &report.source_modules,
        &["crate", "ext_traits", "request"],
        "FromRequest::from_request impact report source modules",
    );
    assert_source_module(
        &report.source_modules,
        &["crate", "extract"],
        "FromRequest::from_request impact report source modules",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_summarize_public_api_callers_for_impact() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Impact analysis:
    //   "Which public APIs eventually call this helper?"
    // API understanding:
    //   "How is this library function used in real target code?"
    //
    // Source oracle:
    //   axum/src/routing/method_routing.rs:799 defines
    //     `MethodRouter::new`.
    //   axum/src/routing/method_routing.rs:375,434,472,514 call
    //     `MethodRouter::new()` from public top-level routing functions.
    let target = method_id_by_name_body_and_file_suffix(
        &db,
        "new",
        "let fallback = Route::new",
        "axum/src/routing/method_routing.rs",
    )?;
    let on_service =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on_service")?;
    let any_service =
        function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "any_service")?;
    let on = function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "on")?;
    let any = function_id_by_name_in_module(&db, &["crate", "routing", "method_routing"], "any")?;

    let report = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 64,
        },
    )?;
    assert_eq!(report.target.id, target);
    assert_eq!(report.target.name, "new");
    assert_node_names(
        &report.public_callers,
        &[
            (on_service, "on_service"),
            (any_service, "any_service"),
            (on, "on"),
            (any, "any"),
        ],
        "MethodRouter::new public API impact callers",
    );
    assert!(
        report.public_callers.iter().all(|caller| caller.is_public),
        "public_callers should only include stored-public nodes: {report:#?}"
    );
    assert!(
        report.direct_call_sites.iter().any(|row| {
            row.site.owner_id == on_service
                && row.site.path.as_ref() == Some(&path(&["MethodRouter", "new"]))
                && row.site.arg_count == Some(0)
        }),
        "MethodRouter::new impact report should preserve zero-argument public caller sites: {report:#?}"
    );
    assert_source_file(
        &report.source_files,
        "axum/src/routing/method_routing.rs",
        "MethodRouter::new impact report source files",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_bucket_impact_callers_by_test_source() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection / Test planning:
    //   "Is this function only used by tests, or is it reachable from
    //   production entrypoints?"
    //   "Which tests should cover a change to this function?"
    //
    // Source oracle:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/routing/mod.rs:109 calls `Self::new()` from
    //   `Default for Router`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()` from the
    //   `serve::tests::if_it_compiles_it_works` test helper.
    let target = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let non_test_owner = method_id_by_name_body_and_file_suffix(
        &db,
        "default",
        "Self::new()",
        "axum/src/routing/mod.rs",
    )?;
    let test_owner = function_id_by_name_in_module(
        &db,
        &["crate", "serve", "tests"],
        "if_it_compiles_it_works",
    )?;

    let report = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 256,
        },
    )?;
    assert_eq!(report.target.id, target);
    assert!(
        !report.test_callers.is_empty() && !report.non_test_callers.is_empty(),
        "Router::new should prove both test and non-test impact buckets: {report:#?}"
    );
    assert_eq!(
        report.test_callers.len() + report.non_test_callers.len(),
        report.callers.len(),
        "test/non-test impact buckets should partition eventual callers: {report:#?}"
    );
    assert_node_names(
        &report.test_callers,
        &[(test_owner, "if_it_compiles_it_works")],
        "Router::new test impact callers",
    );
    assert_node_names(
        &report.non_test_callers,
        &[(non_test_owner, "default")],
        "Router::new non-test impact callers",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_proc_macro_public_entrypoint_impact() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    // Impact analysis:
    //   "Which public macro entrypoints reach this helper?"
    //
    // Source oracle:
    //   axum-macros/src/lib.rs:377,426,665,715 call `expand_with(...)` from
    //   public proc-macro entrypoints.
    // Expected traversal: each public proc-macro entrypoint reaches
    // `expand_with` in one path-call edge.
    let target = function_id_by_name_in_module(&db, &["crate"], "expand_with")?;
    let expected = [
        (
            macro_id_by_name(&db, "derive_from_request")?,
            "derive_from_request",
        ),
        (
            macro_id_by_name(&db, "derive_from_request_parts")?,
            "derive_from_request_parts",
        ),
        (
            macro_id_by_name(&db, "derive_typed_path")?,
            "derive_typed_path",
        ),
        (macro_id_by_name(&db, "derive_from_ref")?, "derive_from_ref"),
    ];
    let report = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;

    assert_eq!(report.target.id, target);
    assert_eq!(report.target.kind, CallNodeKind::Function);
    assert_eq!(report.target.name, "expand_with");

    assert_eq!(report.paths.len(), expected.len(), "{report:#?}");
    assert_eq!(report.callers.len(), expected.len(), "{report:#?}");
    assert_eq!(report.direct_callers.len(), expected.len(), "{report:#?}");
    assert_eq!(
        report.direct_call_sites.len(),
        expected.len(),
        "{report:#?}"
    );
    assert_eq!(report.public_callers.len(), expected.len(), "{report:#?}");
    assert_eq!(report.test_callers.len(), 0, "{report:#?}");
    assert_eq!(report.non_test_callers.len(), expected.len(), "{report:#?}");

    assert_node_names(&report.callers, &expected, "expand_with impact callers");
    assert_node_names(
        &report.public_callers,
        &expected,
        "expand_with public macro callers",
    );
    assert_path_depths(
        &report
            .paths
            .iter()
            .map(|path| (path.start_id, path.depth))
            .collect::<Vec<_>>(),
        &expected
            .iter()
            .map(|(id, _name)| (*id, 1))
            .collect::<Vec<_>>(),
        "expand_with macro-entrypoint paths",
    );

    for row in &report.direct_call_sites {
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path, Some(path(&["expand_with"])));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1, "{row:#?}");
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Function);
    }
    assert_source_file(
        &report.source_files,
        "axum-macros/src/lib.rs",
        "expand_with proc-macro impact report source files",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_surface_fail_closed_debugging_context() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging:
    //   "Which callers can pass this unsupported receiver shape into the
    //   resolver?"
    // Documentation and RAG:
    //   "What fail-closed blocker should be shown when a callsite is visible
    //   but targetless?"
    //
    // Source oracle:
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //     `self.sem.clone().acquire_owned().await.unwrap()`.
    // Current contract: the awaited-result `unwrap()` callsite is visible as
    // unsupported call context, but it has no fabricated callee and cannot
    // become a traversal edge.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "accept",
        "self.sem.clone().acquire_owned().await.unwrap()",
        "axum/src/serve/listener.rs",
    )?;
    let site_id = assert_owner_method_targetless(
        &db,
        owner,
        "unwrap",
        &CallReceiver::AwaitResult,
        CallStatusKind::Unsupported,
        "axum/src/serve/listener.rs:143 awaited-result unwrap",
    )?;
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        site_id,
        "axum/src/serve/listener.rs:143 awaited-result unwrap",
    )?;

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    assert!(
        paths
            .iter()
            .all(|path| path.edges.iter().all(|edge| edge.call_site_id != site_id)),
        "targetless unsupported receiver rows must not appear in call paths: {paths:#?}"
    );
    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let unsupported = report
        .unsupported_frontier_calls
        .iter()
        .find(|row| row.site.id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should expose awaited-result unwrap in unsupported frontier rows: {report:#?}"
            )
        });
    assert_eq!(unsupported.site.owner_id, owner);
    assert_eq!(unsupported.status.status, CallStatusKind::Unsupported);
    assert!(
        unsupported.targets.is_empty(),
        "unsupported frontier call should remain targetless: {unsupported:#?}"
    );

    Ok(())
}

fn assert_node_names(nodes: &[ploke_db::CallNodeInfo], expected: &[(Uuid, &str)], label: &str) {
    for (id, name) in expected {
        let node = nodes
            .iter()
            .find(|node| node.id == *id)
            .unwrap_or_else(|| panic!("{label} should include node {id}: {nodes:#?}"));
        assert_eq!(
            node.name, *name,
            "{label} should preserve node metadata for {id}"
        );
    }
}

fn assert_path_depths(paths: &[(Uuid, u32)], expected: &[(Uuid, u32)], label: &str) {
    let actual = paths.iter().copied().fold(
        BTreeMap::<Uuid, Vec<u32>>::new(),
        |mut acc, (node, depth)| {
            acc.entry(node).or_default().push(depth);
            acc
        },
    );
    for (node, depth) in expected {
        assert!(
            actual
                .get(node)
                .is_some_and(|depths| depths.contains(depth)),
            "{label} should include node {node} at depth {depth}: {actual:#?}"
        );
    }
}

fn assert_source_file(files: &[String], suffix: &str, label: &str) {
    assert!(
        files.iter().any(|path| path.ends_with(suffix)),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}

fn assert_source_module(modules: &[Vec<String>], expected: &[&str], label: &str) {
    let expected = path(expected);
    assert!(
        modules.iter().any(|module| module == &expected),
        "{label} should include source module {expected:?}: {modules:#?}"
    );
}

fn assert_path_edge_span_matches_site(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    span: (u32, u32),
    label: &str,
) -> Result<(), DbError> {
    let context = db.call_context_for_owner(owner)?;
    let row = context
        .iter()
        .find(|row| row.site.id == site)
        .unwrap_or_else(|| panic!("{label} should have callsite {site}: {context:#?}"));
    assert_eq!(
        span, row.site.span,
        "{label} path edge should preserve the persisted callsite span"
    );
    Ok(())
}
