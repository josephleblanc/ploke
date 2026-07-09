use std::collections::BTreeMap;

use ploke_db::{
    CallContextRelation, CallContextSeed, CallNodeKind, CallPathOptions, CallRelationKind,
    ProofGraphStore,
};
use serde_json::json;

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
fn axum_usage_questions_list_module_boundary_edges_for_architecture_review() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Which modules call across a boundary that should be one-way?"
    //   "Do any call chains bypass the intended abstraction layer?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())` inside
    //     the same `crate::ext_traits::request` module.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding in the
    //     separate `crate::extract` module.
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
        max_paths: 16,
    };
    let boundaries = db.module_boundary_edges_from_owner(start, options)?;
    assert!(
        boundaries
            .iter()
            .all(|edge| edge.caller.module_path != edge.callee.module_path),
        "module_boundary_edges should only report resolved cross-module edges: {boundaries:#?}"
    );
    let boundary = boundaries
        .iter()
        .find(|edge| edge.edge.caller_id == intermediate && edge.edge.callee_id == target)
        .unwrap_or_else(|| {
            panic!(
                "architecture boundary query should expose RequestExt::extract_with_state -> FromRequest::from_request: {boundaries:#?}"
            )
        });
    assert_eq!(
        boundary.caller.module_path,
        path(&["crate", "ext_traits", "request"])
    );
    assert_eq!(boundary.callee.module_path, path(&["crate", "extract"]));
    assert_eq!(boundary.edge.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(boundary.edge.source_kind, CallSiteKind::Path);
    assert_eq!(
        boundary.site.path.as_ref(),
        Some(&path(&["E", "from_request"]))
    );
    assert_eq!(boundary.site.arg_count, Some(2));
    assert!(
        !boundaries
            .iter()
            .any(|edge| edge.edge.caller_id == start && edge.edge.callee_id == intermediate),
        "same-module RequestExt::extract -> extract_with_state should not be a module-boundary edge"
    );

    let reach = db.call_reach_for_owner(start, options)?;
    assert!(
        reach
            .boundary_edges
            .iter()
            .any(|edge| edge.call_site_id == boundary.edge.call_site_id),
        "owner-centered reach boundary edges should agree with the owner-scoped module-boundary helper: {reach:#?}"
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
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Current contract: dependency-root path calls are visible as external
    // frontier rows but do not become local traversal edges. The owner reach
    // query must still preserve the feature gate so build/deployment reviews
    // can separate feature-specific call paths.
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
    assert!(
        report
            .source_cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "Json::from_bytes reach report should preserve the json feature cfg: {report:#?}"
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
    assert!(
        frontier
            .site
            .cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "feature-gated reach summary should be backed by callsite cfg metadata: {frontier:#?}"
    );
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
    assert!(
        external_frontier
            .site
            .cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "external frontier row should preserve the json feature cfg: {external_frontier:#?}"
    );
    assert_source_file(
        &report.source_files,
        "axum/src/json.rs",
        "Json::from_bytes reach report source files",
    );

    Ok(())
}

#[test]
fn axum_usage_questions_list_external_summary_needs_for_owner() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which external dependency frontiers still need an audited summary?"
    // Performance work:
    //   "Which external calls need review before their effects can be trusted?"
    // Documentation and RAG:
    //   "What proof input is missing for this targetless frontier?"
    //
    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 calls `Request::builder()`.
    // Current contract: this alias leaves the selected local workspace as an
    // external targetless frontier. The owner-scoped proof queue should list
    // it while the `external_dependency_summary_missing` blocker is active,
    // and should drop it once an admitted external summary is linked. The
    // call graph must not create a local edge in either state.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let site_id = assert_owner_path_targetless(
        &db,
        owner,
        &["Request", "builder"],
        CallStatusKind::External,
        "axum/src/middleware/from_fn.rs:411 Request::builder",
    )?;
    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "from_fn::tests::basic should project call_site and call_resolution facts: {projected}"
    );

    let options = CallPathOptions {
        max_depth: 3,
        max_paths: 64,
    };
    let needs = db.external_summary_needs_for_owner(owner, options)?;
    let need = needs
        .iter()
        .find(|need| need.call_site.site.id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "Request::builder should be listed as an external-summary need before admission: {needs:#?}"
            )
        });
    assert_external_targetless(&need.call_site);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct frontier from the selected owner should not need an intermediate path: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "summary need should retain the active missing-summary blocker: {need:#?}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_request_builder_summary_records(
        site_id,
    ))?;
    let after = db.external_summary_needs_for_owner(owner, options)?;
    assert!(
        after.iter().all(|need| need.call_site.site.id != site_id),
        "admitted Request::builder summary should discharge this owner-scoped need without adding a local edge: {after:#?}"
    );
    assert!(
        relations_for_site(&db, site_id)?.rows.is_empty(),
        "summary admission must not fabricate a local Request::builder edge"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_list_external_summary_need_for_feature_gated_json_frontier()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Performance work:
    //   "Which feature-gated external dependency frontiers still need an
    //   audited summary before their effects can be trusted?"
    //
    // Source oracle:
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Expected contract: proof projection lists the serde_json frontier as an
    // owner-scoped external-summary need while the blocker is active, preserves
    // the inherited feature cfg, and drops the need after an admitted summary
    // is linked without creating a local traversal edge.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
        "axum/src/json.rs",
    )?;
    let site_id = assert_owner_path_targetless(
        &db,
        owner,
        &["serde_json", "Deserializer", "from_slice"],
        CallStatusKind::External,
        "axum/src/json.rs:184 serde_json::Deserializer::from_slice",
    )?;
    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "Json::from_bytes should project call_site and call_resolution facts: {projected}"
    );

    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };
    let needs = db.external_summary_needs_for_owner(owner, options)?;
    let need = needs
        .iter()
        .find(|need| need.call_site.site.id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "serde_json::Deserializer::from_slice should be listed as an external-summary need before admission: {needs:#?}"
            )
        });
    assert_external_targetless(&need.call_site);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct serde_json frontier should not need an intermediate path: {need:#?}"
    );
    assert!(
        need.call_site
            .site
            .cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "serde_json summary need should preserve the inherited json cfg: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "serde_json summary need should retain the active missing-summary blocker: {need:#?}"
    );

    db.upsert_proof_fact_values(
        &ploke_test_utils::axum_serde_json_from_slice_summary_records(site_id),
    )?;
    let after = db.external_summary_needs_for_owner(owner, options)?;
    assert!(
        after.iter().all(|need| need.call_site.site.id != site_id),
        "admitted serde_json summary should discharge this owner-scoped need without adding a local edge: {after:#?}"
    );
    assert!(
        relations_for_site(&db, site_id)?.rows.is_empty(),
        "summary admission must not fabricate a local serde_json::Deserializer::from_slice edge"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_preserve_argument_shape_for_external_frontier() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // API understanding:
    //   "What argument shapes do existing callers pass?"
    // Build or deployment optimization:
    //   "Are there feature-gated or platform-specific call paths that should
    //   be checked separately?"
    //
    // Source oracle:
    //   axum-macros/src/attr_parsing.rs:7 defines
    //     `parse_parenthesized_attribute<K, T>(...)`.
    //   axum-macros/src/attr_parsing.rs:22 calls
    //     `std::any::type_name::<K>()`.
    // Current contract: dependency-root calls stay external and targetless,
    // but DB call context preserves the explicit source argument shape. This
    // proves the persisted graph can answer the turbofish arity question
    // without inventing a local traversal edge for `std::any::type_name`.
    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "attr_parsing"],
        "parse_parenthesized_attribute",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["std", "any", "type_name"]);
    assert_external_targetless(row);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(
        row.site.arg_count,
        Some(0),
        "type_name::<K>() should preserve zero value arguments: {row:#?}"
    );
    assert_eq!(
        row.site.generic_arg_count,
        Some(1),
        "type_name::<K>() should preserve one turbofish argument: {row:#?}"
    );

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    let frontier = report
        .external_frontier_calls
        .iter()
        .find(|frontier| frontier.site.id == row.site.id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should retain type_name::<K>() as an external frontier row: {report:#?}"
            )
        });
    assert_external_targetless(frontier);
    assert_eq!(frontier.site.arg_count, Some(0));
    assert_eq!(frontier.site.generic_arg_count, Some(1));

    Ok(())
}

#[test]
fn axum_usage_questions_surface_platform_cfgs_for_listener_reach() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Build or deployment optimization:
    //   "Are there feature-gated or platform-specific call paths that should
    //   be checked separately?"
    //
    // Source oracle:
    //   axum/src/serve/listener.rs:40-43 contains the non-gated
    //   `TcpListener` impl.
    //   axum/src/serve/listener.rs:55-63 contains the `#[cfg(unix)]`
    //   `UnixListener` impl.
    // Current contract: both `Self::accept(self).await` rows remain external
    // and targetless, but owner reach summaries preserve the cfg metadata for
    // the platform-gated row.
    let owners = method_ids_by_name_body_and_file_suffix(
        &db,
        "accept",
        "Self::accept(self).await",
        "axum/src/serve/listener.rs",
    )?;
    assert_eq!(owners.len(), 2, "listener accept owner fanout changed");

    let mut gated_reports = 0;
    for owner in owners {
        let report = db.call_reach_for_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?;
        let frontier = report
            .external_frontier_calls
            .iter()
            .find(|row| row.site.path.as_ref() == Some(&path(&["Self", "accept"])))
            .unwrap_or_else(|| {
                panic!("listener accept reach should expose Self::accept frontier: {report:#?}")
            });
        assert_external_targetless(frontier);

        if report.source_cfgs.iter().any(|cfg| cfg == "unix") {
            gated_reports += 1;
            assert!(
                frontier.site.cfgs.iter().any(|cfg| cfg == "unix"),
                "platform-gated reach summary should be backed by callsite cfg metadata: {frontier:#?}"
            );
        }
    }
    assert_eq!(
        gated_reports, 1,
        "exactly one listener accept reach summary should carry the unix cfg"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_surface_unresolved_frontier_for_generated_constructor()
-> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging:
    //   "What source callsite corresponds to this persisted call edge or proof
    //   blocker?"
    // Documentation and RAG:
    //   "What fail-closed blocker should be shown when a callsite is visible
    //   but targetless?"
    //
    // Source oracle:
    //   axum/src/handler/future.rs:11-18 defines the generated future type.
    //   axum/src/macros.rs:19-20 contains the macro template that would
    //   generate the inherent `new` constructor after expansion.
    //   axum/src/handler/service.rs:155 binds
    //     `type Future = super::future::IntoServiceFuture<H::Future>`.
    //   axum/src/handler/service.rs:174 calls
    //     `super::future::IntoServiceFuture::new(future)`.
    // Current contract: the source callsite is visible as an unresolved
    // frontier row, but it has no fabricated callee and cannot become a local
    // traversal edge until macro-expanded inherent items are modeled.
    let owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["super", "future", "IntoServiceFuture", "new"]);
    assert_targetless_status(row, CallStatusKind::Unresolved);

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    assert!(
        paths.iter().all(|path| path
            .edges
            .iter()
            .all(|edge| edge.call_site_id != row.site.id)),
        "targetless unresolved rows must not appear in call paths: {paths:#?}"
    );

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let unresolved = report
        .unresolved_frontier_calls
        .iter()
        .find(|frontier| frontier.site.id == row.site.id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should expose generated constructor in unresolved frontier rows: {report:#?}"
            )
        });
    assert_eq!(unresolved.site.owner_id, owner);
    assert_targetless_status(unresolved, CallStatusKind::Unresolved);
    assert!(
        report.ambiguous_frontier_calls.is_empty(),
        "this axum owner should not report ambiguous frontier rows: {report:#?}"
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
            max_paths: 512,
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
fn axum_usage_questions_list_private_nodes_without_incoming_callers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Which private helpers have no incoming callers?"
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:257 defines `#[test] fn traits()`.
    //   No checked-in axum source row calls `traits(...)`; any test-harness
    //   entrypoint is generated outside the stored source call graph.
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`, which has
    //   multiple real source callers and must not appear in this list.
    //
    // Current contract: this is a direct resolved-call graph query, not a full
    // semantic reachability proof across generated harnesses or value-flow.
    let uncalled = db.private_uncalled_nodes()?;
    let traits = function_id_by_name_in_module(&db, &["crate", "error_handling"], "traits")?;
    let parse_attrs =
        function_id_by_name_in_module(&db, &["crate", "attr_parsing"], "parse_attrs")?;

    let traits_row = uncalled
        .iter()
        .find(|node| node.id == traits)
        .unwrap_or_else(|| {
            panic!(
                "private uncalled-node query should include error_handling::traits: {uncalled:#?}"
            )
        });
    assert_eq!(traits_row.kind, CallNodeKind::Function);
    assert_eq!(traits_row.name, "traits");
    assert!(!traits_row.is_public);
    assert_source_file(
        std::slice::from_ref(&traits_row.file_path),
        "axum/src/error_handling/mod.rs",
        "private uncalled-node source file",
    );

    assert!(
        uncalled.iter().all(|node| node.id != parse_attrs),
        "private uncalled-node query must exclude called helper parse_attrs: {uncalled:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        traits,
        "axum/src/error_handling/mod.rs:257 traits",
    )?;

    let report = db.call_impact_for_target(
        traits,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
    )?;
    assert_eq!(report.target.id, traits);
    assert!(report.callers.is_empty(), "{report:#?}");
    assert!(report.direct_callers.is_empty(), "{report:#?}");
    assert!(report.direct_call_sites.is_empty(), "{report:#?}");
    assert!(report.public_callers.is_empty(), "{report:#?}");
    assert!(report.test_callers.is_empty(), "{report:#?}");
    assert!(report.non_test_callers.is_empty(), "{report:#?}");

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    records.push(ploke_test_utils::axum_entrypoint_record(domain_id, traits));
    db.upsert_proof_fact_values(&records)?;
    let traits_id = traits.to_string();
    let proof_rows = db.proof_symbol_lookup(&traits_id)?;
    assert!(
        proof_rows.iter().any(|row| {
            row.kind == "entrypoint_summary"
                && row.definition_id.as_deref() == Some(traits_id.as_str())
                && row.target_kind.as_deref() == Some("test")
                && row.target_name.as_deref() == Some("generated-test-harness")
                && row.summary_class.as_deref() == Some("analyzed_source")
                && row.status.as_deref() == Some("admitted")
        }),
        "generated test-harness reachability should be represented as proof context, not source call edges: {proof_rows:#?}"
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
fn axum_usage_questions_report_reachable_effect_seed_for_task_spawn() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink?"
    //   "Which call chain reaches a task-spawn point?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: resolved edges make `spawn_service` reachable from the
    // test owner, while `tokio::spawn` remains an external targetless frontier.
    // A proof `effect_seed` attached to that frontier is queryable as a
    // reachable `async_task_spawn` effect without turning the frontier into a
    // local callee edge.
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let spawn_owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let spawn_context = db.call_context_for_owner(spawn_owner)?;
    let spawn_row = row_by_path(&spawn_context, &["tokio", "spawn"]);
    assert_external_targetless(spawn_row);
    assert!(
        relations_for_site(&db, spawn_row.site.id)?.rows.is_empty(),
        "tokio::spawn should remain an external frontier, not a local edge"
    );

    db.upsert_proof_fact_values(&[json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:axum-test-client-task-spawn",
        "call_site_id": spawn_row.site.id.to_string(),
        "effect_class": "async_task_spawn",
        "confidence": "source-oracle",
        "blocker_if_unresolved": false,
        "evidence_use": "proof_only"
    })])?;

    let effects = db.call_effects_reachable_from_owner(
        start,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
    )?;
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:axum-test-client-task-spawn")
        .unwrap_or_else(|| {
            panic!("reachable effect query should report the axum tokio::spawn sink: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "async_task_spawn");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
    assert_eq!(effect.call_site.site.id, spawn_row.site.id);
    assert_eq!(effect.call_site.status.status, CallStatusKind::External);
    assert!(
        effect.blocker_reasons.is_empty(),
        "the effect marker itself should not add a blocker when blocker_if_unresolved=false: {effect:#?}"
    );
    let effect_path = effect
        .paths_to_owner
        .iter()
        .find(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "reachable effect should include the resolved path to spawn_service: {effect:#?}"
            )
        });
    assert_eq!(effect_path.edges.len(), 2);
    assert_eq!(effect_path.edges[0].caller_id, start);
    assert_eq!(effect_path.edges[1].callee_id, spawn_owner);
    assert!(
        relations_for_site(&db, effect.call_site.site.id)?
            .rows
            .is_empty(),
        "reachable effect annotations must not fabricate call edges"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_dyn_future_poll_runtime_dispatch_blocker() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging / Security analysis:
    //   "Which reachable call frontiers are blocked by runtime dispatch?"
    //   "Can the call graph distinguish an unsupported frontier from a
    //   fabricated local edge?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:240 stores
    //     `Pin<Box<dyn Future<Output = Result<Response, Infallible>>>>`.
    //   axum/src/error_handling/mod.rs:251
    //     `HandleErrorFuture::poll` calls `self.project().future.poll(cx)`.
    // Expected contract: the dyn `Future::poll` callsite remains unsupported
    // and targetless, while proof facts can attach the runtime dispatch blocker
    // to the same callsite identity for fail-closed traversal consumers.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "poll",
        "self.project().future.poll(cx)",
        "axum/src/error_handling/mod.rs",
    )?;
    let context = db.call_context_for_owner(owner)?;
    let poll = row_by_method_receiver(&context, "poll", &CallReceiver::Unsupported);
    assert_targetless_status(poll, CallStatusKind::Unsupported);
    assert!(
        relations_for_site(&db, poll.site.id)?.rows.is_empty(),
        "axum/src/error_handling/mod.rs:251 dyn Future::poll must not fabricate a local edge"
    );
    assert_no_traversal_candidates_for_site(
        &db,
        owner,
        poll.site.id,
        "axum/src/error_handling/mod.rs:251 dyn Future::poll receiver dispatch",
    )?;

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    let frontier = report
        .unsupported_frontier_calls
        .iter()
        .find(|row| row.site.id == poll.site.id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should expose dyn Future::poll as an unsupported frontier: {report:#?}"
            )
        });
    assert_eq!(frontier.site.owner_id, owner);
    assert!(
        frontier.targets.is_empty(),
        "unsupported dyn Future::poll frontier should remain targetless: {frontier:#?}"
    );

    let site = poll.site.id.to_string();
    db.upsert_proof_fact_values(&[ploke_test_utils::axum_dyn_future_poll_blocker(poll.site.id)])?;

    let blockers = db.proof_blockers()?;
    assert!(
        blockers.iter().any(|proof| {
            proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.reason == "dynamic_dispatch_unbounded"
                && proof.status == "blocked"
        }),
        "dyn Future::poll callsite should expose a runtime dispatch proof blocker: {blockers:#?}"
    );

    let proof_rows = db.proof_graphrag_context("dyn Future::poll")?;
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "proof_blocker"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "RAG proof context lookup should retrieve the dyn Future::poll blocker: {proof_rows:#?}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_architecture_boundary_edges() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Which modules call across a boundary that should be one-way?"
    //   "Do any call chains bypass the intended abstraction layer?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    let owner = method_id_by_name_body_and_file_suffix(
        &db,
        "extract_with_state",
        "E::from_request(self, state)",
        "axum-core/src/ext_traits/request.rs",
    )?;
    let target = method_id_by_trait_name(&db, "FromRequest", "from_request")?;

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;

    assert_eq!(
        report.boundary_call_sites.len(),
        1,
        "architecture review should expose the direct cross-module callsite: {report:#?}"
    );
    let boundary_site = &report.boundary_call_sites[0];
    assert_eq!(boundary_site.site.owner_id, owner);
    assert_eq!(
        boundary_site.site.path.as_ref(),
        Some(&path(&["E", "from_request"]))
    );
    assert_eq!(
        boundary_site.site.arg_count,
        Some(2),
        "architecture review should preserve the source argument shape for the boundary call: {boundary_site:#?}"
    );
    assert!(
        boundary_site
            .targets
            .iter()
            .any(|target_row| target_row.target_id == target),
        "architecture review boundary callsite should target FromRequest::from_request: {report:#?}"
    );

    assert_eq!(
        report.boundary_edges.len(),
        1,
        "architecture review should expose the same crossing as a path edge: {report:#?}"
    );
    let edge = report.boundary_edges[0];
    assert_eq!(edge.caller_id, owner);
    assert_eq!(edge.callee_id, target);
    assert_eq!(edge.source_kind, CallSiteKind::Path);
    assert_eq!(edge.relation, CallRelationKind::AssociatedFunction);

    let caller = db
        .call_node_info(edge.caller_id)?
        .unwrap_or_else(|| panic!("missing caller metadata for boundary edge {edge:#?}"));
    let callee = db
        .call_node_info(edge.callee_id)?
        .unwrap_or_else(|| panic!("missing callee metadata for boundary edge {edge:#?}"));
    assert_eq!(
        caller.module_path,
        path(&["crate", "ext_traits", "request"])
    );
    assert_eq!(callee.module_path, path(&["crate", "extract"]));
    assert_ne!(
        caller.module_path, callee.module_path,
        "boundary edge should represent a real module crossing"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_body_empty_component_impact() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Build or deployment optimization:
    //   "Which components are affected by a change to this API?"
    //   "Which crates or binaries need rebuilding after this internal function changes?"
    // API understanding:
    //   "How is this library function used in real target code?"
    //   "Which constructors are used directly, and which are only reached
    //   through re-exports or aliases?"
    //
    // Source oracle:
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:{110,116} calls `Self::empty()`.
    //   axum-core, axum, closure-owned, and local-item rows call
    //   `Body::empty()` through direct imports, re-exports, and inherited
    //   glob imports.
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;
    let report = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 64,
        },
    )?;

    assert_eq!(report.target.id, target);
    assert_eq!(report.target.kind, CallNodeKind::Method);
    assert_eq!(report.target.name, "empty");
    assert_eq!(
        report.paths.len(),
        23,
        "component impact should traverse every current Body::empty direct caller: {report:#?}"
    );
    assert_eq!(
        report.direct_call_sites.len(),
        23,
        "component impact should expose every current Body::empty callsite: {report:#?}"
    );
    assert!(
        !report.test_callers.is_empty() && !report.non_test_callers.is_empty(),
        "component impact should show that Body::empty is reached from both test and non-test owners: {report:#?}"
    );
    assert_eq!(
        report.test_callers.len() + report.non_test_callers.len(),
        report.callers.len(),
        "test/non-test component buckets should partition the eventual caller set: {report:#?}"
    );
    assert!(
        report.callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallRelationKind::AssociatedFunction
                && bucket.count == 23
        }),
        "API understanding should summarize Body::empty as associated-function path usage: {report:#?}"
    );

    let path_counts = report.direct_call_sites.iter().fold(
        BTreeMap::<Vec<String>, usize>::new(),
        |mut counts, row| {
            let call_path = row
                .site
                .path
                .clone()
                .expect("Body::empty caller should carry a path");
            *counts.entry(call_path).or_default() += 1;
            counts
        },
    );
    assert_eq!(
        path_counts,
        BTreeMap::from([
            (path(&["Body", "empty"]), 21),
            (path(&["Self", "empty"]), 2)
        ]),
        "API understanding should distinguish direct Body::empty use from Self::empty wrapper rows"
    );

    for suffix in [
        "axum-core/src/body.rs",
        "axum-core/src/ext_traits/request.rs",
        "axum/src/middleware/from_fn.rs",
        "axum/src/routing/tests/mod.rs",
    ] {
        assert_source_file(
            &report.source_files,
            suffix,
            "Body::empty component impact source files",
        );
    }
    assert_source_crate(
        &report.source_crates,
        "axum-core",
        "Body::empty component impact source crates",
    );
    assert_source_crate(
        &report.source_crates,
        "axum",
        "Body::empty component impact source crates",
    );
    for module in [
        &["crate", "body"][..],
        &["crate", "ext_traits", "request"][..],
        &["crate", "middleware", "from_fn"][..],
        &["crate", "routing", "tests"][..],
    ] {
        assert_source_module(
            &report.source_modules,
            module,
            "Body::empty component impact source modules",
        );
    }

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

fn assert_source_crate(crates: &[String], expected: &str, label: &str) {
    assert!(
        crates.iter().any(|name| name == expected),
        "{label} should include crate {expected:?}: {crates:#?}"
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
