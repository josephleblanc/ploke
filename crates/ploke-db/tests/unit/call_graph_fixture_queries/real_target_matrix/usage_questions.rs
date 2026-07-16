use std::collections::BTreeMap;

use ploke_db::{
    CallContextRelation, CallContextSeed, CallNodeKind, CallPathOptions, CallRelationKind,
    CrateBoundaryPolicyRule, ModuleBoundaryPolicyRule, ProofGraphStore,
};
use ploke_test_utils::CORPUS_MEMCHR_CALL_GRAPH;
use serde_json::json;

use super::super::*;
use super::common::*;

fn effect_seed(
    call_site_id: impl ToString,
    effect_seed_id: &str,
    effect_class: &str,
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id.to_string(),
        "effect_class": effect_class,
        "confidence": "source-oracle",
        "blocker_if_unresolved": false,
        "evidence_use": "proof_only"
    })
}

fn spawn_effect_seed(call_site_id: impl ToString, effect_seed_id: &str) -> serde_json::Value {
    effect_seed(call_site_id, effect_seed_id, "async_task_spawn")
}

fn owner_effect_policy(
    owner_id: impl ToString,
    effect_policy_id: &str,
    allowed_effects: &[&str],
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_policy",
        "schema_version": "ploke-proof-facts.v1",
        "effect_policy_id": effect_policy_id,
        "build_domain_id": "bd:axum-call-graph",
        "definition_id": owner_id.to_string(),
        "proof_policy_version": "axum-real-corpus-call-graph-test",
        "review_method": "source-oracle",
        "scope_of_validity": "axum deserialize_error_status_codes call graph fixture",
        "allowed_effects": allowed_effects,
        "invalidation_conditions": "call graph fixture source or proof policy changes",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

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
    //   axum/src/handler/mod.rs:250 also contributes generated
    //     `Tn::from_request(req, &state)` direct callsites through
    //     `all_the_tuples!(impl_handler)`.
    //   axum-core/src/extract/tuple.rs:68 also contributes generated
    //     `Tn::from_request(req, state)` direct callsites through
    //     `all_the_tuples!(impl_from_request)`.
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
        34,
        "target-centered direct callsite query should still answer exact callsite fanout, including generated handler and tuple extractor rows: {direct_sites:#?}"
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
fn axum_usage_questions_classify_guarded_and_unguarded_call_paths() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis / Architecture review:
    //   "Are authorization checks always called before protected state
    //   mutations?"
    //   "Do any call chains bypass the intended abstraction layer?"
    //
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // `extract_with_state` is used here as the required guard/intermediate:
    // every resolved path from `extract` to `FromRequest::from_request` must
    // pass through it before reaching the trait method binding.
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

    let guarded = db.call_guard_report_between(start, target, intermediate, options)?;
    assert_eq!(guarded.source.id, start);
    assert_eq!(guarded.target.id, target);
    assert_eq!(guarded.guard.id, intermediate);
    assert!(
        guarded.guarded,
        "expected all paths to pass guard: {guarded:#?}"
    );
    assert!(
        guarded.violations.is_empty(),
        "guarded path report should not include violating paths: {guarded:#?}"
    );
    assert_path_depths(
        &guarded
            .paths
            .iter()
            .map(|path| (path.end_id, path.depth))
            .collect::<Vec<_>>(),
        &[(target, 2)],
        "guarded RequestExt::extract path report",
    );

    // Real unrelated node for the missing-guard case:
    //   axum-macros/src/from_request/mod.rs:230 defines
    //   `parse_single_generic_type_on_struct`, which is not on the
    //   RequestExt::extract -> FromRequest::from_request path.
    let unrelated = function_id_by_name_in_module(
        &db,
        &["crate", "from_request"],
        "parse_single_generic_type_on_struct",
    )?;
    let unguarded = db.call_guard_report_between(start, target, unrelated, options)?;
    assert!(
        !unguarded.guarded,
        "unrelated guard should not satisfy the path policy: {unguarded:#?}"
    );
    assert_eq!(
        unguarded.violations.len(),
        unguarded.paths.len(),
        "every reachable path should be reported as a violation when the required guard is absent: {unguarded:#?}"
    );
    assert!(
        unguarded
            .violations
            .iter()
            .all(|path| path.end_id == target),
        "violating paths should still be ordinary resolved source-to-target paths: {unguarded:#?}"
    );

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
fn axum_usage_questions_list_crate_boundary_edges_for_component_review() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Are lower-level crates depending on higher-level application code?"
    // Build or deployment optimization:
    //   "Which crates or binaries need rebuilding after this internal function changes?"
    //
    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 in the `axum` crate calls
    //     `Body::empty()`.
    //   axum-core/src/body.rs:52 defines `Body::empty` in the `axum-core`
    //     crate.
    // Expected contract: the resolved direct edge is visible as a cross-crate
    // edge, while same-crate resolved edges in the same owner are not reported
    // as crate-boundary crossings.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 64,
    };

    let edges = db.crate_boundary_edges_from_owner(owner, options)?;
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_crate != edge.callee_crate),
        "crate_boundary_edges should only report resolved cross-crate edges: {edges:#?}"
    );
    let boundary = edges
        .iter()
        .find(|edge| edge.edge.caller_id == owner && edge.edge.callee_id == target)
        .unwrap_or_else(|| {
            panic!(
                "crate-boundary query should expose axum::middleware::from_fn::tests::basic -> axum_core::Body::empty: {edges:#?}"
            )
        });
    assert_eq!(boundary.caller_crate, "axum");
    assert_eq!(boundary.callee_crate, "axum-core");
    assert_eq!(
        boundary.caller.module_path,
        path(&["crate", "middleware", "from_fn"]),
        "call node metadata uses the existing shortest stable module path for nested test owners"
    );
    assert_eq!(boundary.callee.module_path, path(&["crate", "body"]));
    assert_eq!(boundary.edge.relation, CallRelationKind::AssociatedFunction);
    assert_eq!(boundary.edge.source_kind, CallSiteKind::Path);
    assert_eq!(boundary.site.path.as_ref(), Some(&path(&["Body", "empty"])));
    assert!(
        db.call_paths_from_owner(owner, options)?
            .iter()
            .flat_map(|path| path.edges.iter())
            .any(|edge| edge.caller_id == owner && edge.callee_id != target),
        "test owner should have other resolved same-crate edges, proving crate-boundary filtering is not just returning every edge"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_crate_boundary_policy_violations() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Architecture review:
    //   "Are lower-level crates depending on higher-level application code?"
    //   "Which crate-boundary calls violate the intended dependency direction?"
    //
    // Source oracle:
    //   axum/src/middleware/from_fn.rs:411 in the `axum` crate calls
    //     `Body::empty()`.
    //   axum-core/src/body.rs:52 defines `Body::empty` in the `axum-core`
    //     crate.
    // Expected policy contract: a caller-supplied forbidden dependency rule
    // reports the resolved `axum -> axum-core` edge without inventing policy
    // evidence for same-crate calls or targetless dependency frontiers.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "middleware", "from_fn", "tests"], "basic")?;
    let target = method_id_by_name_and_body_substring(&db, "empty", "Empty::new()")?;
    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 64,
    };

    let violations = db.crate_boundary_policy_violations_from_owner(
        owner,
        options,
        &[CrateBoundaryPolicyRule {
            rule_id: "axum-must-not-call-axum-core".to_string(),
            caller_crate: "axum".to_string(),
            callee_crate: "axum-core".to_string(),
        }],
    )?;
    assert_eq!(
        violations.len(),
        1,
        "crate policy should flag exactly the inspected cross-crate edge: {violations:#?}"
    );
    let violation = &violations[0];
    assert_eq!(violation.rule_id, "axum-must-not-call-axum-core");
    assert_eq!(violation.edge.edge.caller_id, owner);
    assert_eq!(violation.edge.edge.callee_id, target);
    assert_eq!(violation.edge.caller_crate, "axum");
    assert_eq!(violation.edge.callee_crate, "axum-core");
    assert_eq!(
        violation.edge.site.path.as_ref(),
        Some(&path(&["Body", "empty"]))
    );

    let allowed = db.crate_boundary_policy_violations_from_owner(
        owner,
        options,
        &[CrateBoundaryPolicyRule {
            rule_id: "axum-core-must-not-call-axum".to_string(),
            caller_crate: "axum-core".to_string(),
            callee_crate: "axum".to_string(),
        }],
    )?;
    assert!(
        allowed.is_empty(),
        "nonmatching crate policy should not report violations: {allowed:#?}"
    );

    let empty_crate = db
        .crate_boundary_policy_violations_from_owner(
            owner,
            options,
            &[CrateBoundaryPolicyRule {
                rule_id: "invalid-empty-crate".to_string(),
                caller_crate: String::new(),
                callee_crate: "axum-core".to_string(),
            }],
        )
        .expect_err("empty crate names should fail closed");
    assert!(
        empty_crate.to_string().contains("non-empty caller_crate"),
        "empty-crate error should explain the invalid rule: {empty_crate}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_module_boundary_policy_violations() -> Result<(), DbError> {
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
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method in the
    //     separate `crate::extract` module.
    // Expected policy contract: a caller-supplied forbidden boundary rule
    // reports the resolved `ext_traits::request -> extract` edge without
    // inventing edges for same-module calls or targetless frontiers.
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

    let violations = db.module_boundary_policy_violations_from_owner(
        start,
        options,
        &[ModuleBoundaryPolicyRule {
            rule_id: "ext-traits-must-not-call-extract".to_string(),
            caller_module_prefix: path(&["crate", "ext_traits"]),
            callee_module_prefix: path(&["crate", "extract"]),
        }],
    )?;
    assert_eq!(
        violations.len(),
        1,
        "policy should flag exactly the inspected cross-module edge: {violations:#?}"
    );
    let violation = &violations[0];
    assert_eq!(violation.rule_id, "ext-traits-must-not-call-extract");
    assert_eq!(violation.edge.edge.caller_id, intermediate);
    assert_eq!(violation.edge.edge.callee_id, target);
    assert_eq!(
        violation.edge.caller.module_path,
        path(&["crate", "ext_traits", "request"])
    );
    assert_eq!(
        violation.edge.callee.module_path,
        path(&["crate", "extract"])
    );
    assert_eq!(
        violation.edge.site.path.as_ref(),
        Some(&path(&["E", "from_request"]))
    );

    let allowed = db.module_boundary_policy_violations_from_owner(
        start,
        options,
        &[ModuleBoundaryPolicyRule {
            rule_id: "extract-must-not-call-ext-traits".to_string(),
            caller_module_prefix: path(&["crate", "extract"]),
            callee_module_prefix: path(&["crate", "ext_traits"]),
        }],
    )?;
    assert!(
        allowed.is_empty(),
        "nonmatching boundary policy should not report violations: {allowed:#?}"
    );

    let empty_prefix = db
        .module_boundary_policy_violations_from_owner(
            start,
            options,
            &[ModuleBoundaryPolicyRule {
                rule_id: "invalid-empty-prefix".to_string(),
                caller_module_prefix: Vec::new(),
                callee_module_prefix: path(&["crate", "extract"]),
            }],
        )
        .expect_err("empty module prefixes should fail closed");
    assert!(
        empty_prefix
            .to_string()
            .contains("non-empty caller_module_prefix"),
        "empty-prefix error should explain the invalid rule: {empty_prefix}"
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
fn axum_usage_questions_report_admitted_external_summary_as_reachable_effect() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Which reviewed external frontiers are reachable from this owner?"
    //   "Which trusted boundary effects are covered by an admitted summary?"
    //
    // Source oracle:
    //   axum/src/response/sse.rs:445 defines `EventDataWriter::write_buf`.
    //   axum/src/response/sse.rs:449 calls
    //     `std::mem::replace(&mut self.data_written, true)`.
    // Expected contract: admitting an external summary for that targetless
    // frontier makes its allowed `external_summary_boundary` effect visible in
    // reach/effect queries without creating a local call edge.
    let owner = method_id_by_name_and_body_substring(&db, "write_buf", "std::mem::replace")?;
    let context = db.call_context_for_owner(owner)?;
    let replace = row_by_path(&context, &["std", "mem", "replace"]);
    assert_external_targetless(replace);
    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "EventDataWriter::write_buf should project call_site and call_resolution proof rows: {projected}"
    );

    let options = CallPathOptions {
        max_depth: 1,
        max_paths: 16,
    };
    let before = db.call_effects_reachable_from_owner(owner, options)?;
    assert!(
        before
            .iter()
            .all(|effect| effect.call_site.site.id != replace.site.id
                || !effect
                    .effect_seed_id
                    .contains(ploke_test_utils::AXUM_STD_MEM_REPLACE_SUMMARY_ID)),
        "std::mem::replace should not expose a summary-derived effect before admission: {before:#?}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_std_mem_replace_summary_records(
        replace.site.id,
    ))?;

    let effects = db.call_effects_reachable_from_owner(owner, options)?;
    let summary_id = ploke_test_utils::AXUM_STD_MEM_REPLACE_SUMMARY_ID;
    let effect_id = format!("summary-effect:{summary_id}:external_summary_boundary");
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == effect_id)
        .unwrap_or_else(|| {
            panic!(
                "reachable effects should include admitted std::mem::replace summary: {effects:#?}"
            )
        });
    assert_eq!(effect.effect_class, "external_summary_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle-review"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site.id, replace.site.id);
    assert_eq!(effect.call_site.status.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct std::mem::replace frontier should not need an intermediate path: {effect:#?}"
    );
    assert!(
        effect.blocker_reasons.is_empty(),
        "admitted summary should discharge the missing-summary blocker for the derived effect: {effect:#?}"
    );
    assert!(
        relations_for_site(&db, effect.call_site.site.id)?
            .rows
            .is_empty(),
        "summary-derived reach effects must not fabricate local call edges"
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
fn axum_usage_questions_reach_generated_constructor_directly() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging:
    //   "What source callsite corresponds to this persisted generated-item
    //   call edge?"
    // Documentation and RAG:
    //   "Which generated-source boundary was crossed to make this edge
    //   traversable?"
    //
    // Source oracle:
    //   axum/src/handler/future.rs:11-18 defines the generated future type.
    //   axum/src/macros.rs:19-20 contains the macro template that would
    //   generate the inherent `new` constructor after expansion.
    //   axum/src/handler/service.rs:155 binds
    //     `type Future = super::future::IntoServiceFuture<H::Future>`.
    //   axum/src/handler/service.rs:174 calls
    //     `super::future::IntoServiceFuture::new(future)`.
    // Current contract: the bounded `opaque_future!` item invocation is
    // modeled as a generated struct plus inherent `new` method, so the
    // source callsite traverses directly to the generated constructor.
    let owner =
        method_id_by_name_and_body_substring(&db, "call", "IntoServiceFuture::new(future)")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["super", "future", "IntoServiceFuture", "new"]);
    let target = row
        .targets
        .first()
        .map(|target| target.target_id)
        .expect("generated constructor call should expose one target");
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    let paths = db.call_paths_from_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    assert!(
        paths.iter().any(|path| path.end_id == target
            && path.edges.len() == 1
            && path.edges[0].call_site_id == row.site.id),
        "generated constructor should appear as a one-hop path: {paths:#?}"
    );

    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let direct = report
        .direct_call_sites
        .iter()
        .find(|direct| direct.site.id == row.site.id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should expose generated constructor in direct callsite rows: {report:#?}"
            )
        });
    assert_resolved_target(
        direct,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );
    assert!(
        report
            .unresolved_frontier_calls
            .iter()
            .all(|frontier| frontier.site.id != row.site.id),
        "resolved generated constructor should not remain in unresolved frontier rows: {report:#?}"
    );
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
            max_paths: 128,
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
        34,
        "FromRequest::from_request impact report should expose all direct caller-site rows, including generated handler and tuple extractor rows: {report:#?}"
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
                && bucket.count == 34
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
fn axum_usage_questions_select_tests_without_fabricating_generated_edges() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Test planning / build optimization:
    //   "Which source tests and generated harness entrypoints should be
    //   considered for a change to this code item?"
    //
    // Source-test oracle:
    //   axum/src/routing/mod.rs:162 defines `Router::new`.
    //   axum/src/serve/mod.rs:756 calls `Router::new()` from
    //   `serve::tests::if_it_compiles_it_works`.
    //
    // Generated-entrypoint oracle:
    //   axum/src/error_handling/mod.rs:257 defines private `#[test] fn traits()`.
    //   The Rust test harness is generated outside stored source, so it remains
    //   an admitted `entrypoint_summary` proof row, not a source call edge.
    let router_new = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let source_test = function_id_by_name_in_module(
        &db,
        &["crate", "serve", "tests"],
        "if_it_compiles_it_works",
    )?;

    let router_selection = db.call_test_selection_for_target(
        router_new,
        CallPathOptions {
            max_depth: 1,
            max_paths: 512,
        },
    )?;
    assert_eq!(router_selection.target.id, router_new);
    assert_node_names(
        &router_selection.source_test_callers,
        &[(source_test, "if_it_compiles_it_works")],
        "Router::new source test selection callers",
    );
    assert!(
        router_selection.source_test_paths.iter().any(|path| {
            path.start_id == source_test && path.end_id == router_new && path.depth == 1
        }),
        "Router::new test selection should preserve the resolved source-test path: {router_selection:#?}"
    );
    assert!(
        router_selection.generated_entrypoints.is_empty(),
        "source-test selection should not invent generated entrypoint metadata: {router_selection:#?}"
    );
    assert!(
        router_selection.build_domains.is_empty(),
        "Router::new has no admitted build-domain proof in this test: {router_selection:#?}"
    );

    let traits = function_id_by_name_in_module(&db, &["crate", "error_handling"], "traits")?;
    let before = db.call_test_selection_for_target(
        traits,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
    )?;
    assert!(before.source_test_callers.is_empty(), "{before:#?}");
    assert!(before.source_test_paths.is_empty(), "{before:#?}");
    assert!(before.generated_entrypoints.is_empty(), "{before:#?}");
    assert!(before.build_domains.is_empty(), "{before:#?}");

    let domain_id = "bd:corpus-axum-call-graph";
    let mut records = axum_domain_records(domain_id);
    records.push(ploke_test_utils::axum_entrypoint_record(domain_id, traits));
    records.push(ploke_test_utils::axum_entrypoint_effect_policy_record(
        domain_id,
        traits,
        &["ffi_boundary"],
    ));
    db.upsert_proof_fact_values(&records)?;

    let after = db.call_test_selection_for_target(
        traits,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
    )?;
    assert_eq!(after.target.id, traits);
    assert!(after.source_test_callers.is_empty(), "{after:#?}");
    assert!(after.source_test_paths.is_empty(), "{after:#?}");
    assert_eq!(
        after.generated_entrypoints.len(),
        1,
        "generated test-harness proof should be selected without source call edges: {after:#?}"
    );
    let entrypoint = &after.generated_entrypoints[0];
    assert_eq!(
        entrypoint.entrypoint_summary_id,
        "entrypoint-summary:axum-error-handling-traits-test"
    );
    assert_eq!(entrypoint.build_domain_id.as_deref(), Some(domain_id));
    assert_eq!(entrypoint.target_kind.as_deref(), Some("test"));
    assert_eq!(
        entrypoint.target_name.as_deref(),
        Some("generated-test-harness")
    );
    assert_eq!(entrypoint.status.as_deref(), Some("admitted"));
    assert_eq!(entrypoint.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(after.build_domains.len(), 1, "{after:#?}");
    assert_eq!(after.build_domains[0].build_domain_id, domain_id);
    assert!(
        after.build_domains[0].blocker_reasons.is_empty(),
        "admitted generated harness build domain should be unblocked: {after:#?}"
    );
    assert_no_incoming_traversal_to_target(
        &db,
        traits,
        "axum/src/error_handling/mod.rs:257 generated test selection",
    )?;

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
    records.push(ploke_test_utils::axum_entrypoint_effect_policy_record(
        domain_id,
        traits,
        &["ffi_boundary"],
    ));
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
    let domains = db.call_build_domains_for_node(traits)?;
    assert_eq!(domains.len(), 1, "{domains:#?}");
    let domain = &domains[0];
    assert_eq!(domain.build_domain_id, domain_id);
    assert_eq!(domain.target_kind.as_deref(), Some("library"));
    assert_eq!(domain.target_name.as_deref(), Some("axum"));
    assert_eq!(domain.target_root.as_deref(), Some("axum/src/lib.rs"));
    assert_eq!(domain.profile.as_deref(), Some("dev"));
    assert_eq!(domain.active_cfg_hash.as_deref(), Some("sha256:axum-cfg"));
    assert_eq!(domain.rustc_version.as_deref(), Some("rustc fixture"));
    assert_eq!(
        domain.proof_policy_version.as_deref(),
        Some("proof-policy-test")
    );
    assert!(
        domain.blocker_reasons.is_empty(),
        "admitted cfg/rustc evidence should leave the build domain unblocked: {domains:#?}"
    );
    let entrypoints = db.call_test_entrypoints_for_node(traits)?;
    assert_eq!(
        entrypoints.len(),
        1,
        "generated test-harness entrypoint should have one typed summary: {entrypoints:#?}"
    );
    let entrypoint = &entrypoints[0];
    assert_eq!(
        entrypoint.entrypoint_summary_id,
        "entrypoint-summary:axum-error-handling-traits-test"
    );
    assert_eq!(entrypoint.build_domain_id.as_deref(), Some(domain_id));
    assert_eq!(
        entrypoint.definition_id.as_deref(),
        Some(traits_id.as_str())
    );
    assert_eq!(entrypoint.target_kind.as_deref(), Some("test"));
    assert_eq!(
        entrypoint.target_name.as_deref(),
        Some("generated-test-harness")
    );
    assert_eq!(entrypoint.summary_class.as_deref(), Some("analyzed_source"));
    assert_eq!(
        entrypoint.required_containment.as_deref(),
        Some("rust-test-harness")
    );
    assert_eq!(entrypoint.status.as_deref(), Some("admitted"));
    assert_eq!(entrypoint.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert!(
        entrypoint.blocker_reasons.is_empty(),
        "admitted entrypoint proof should not add blockers: {entrypoints:#?}"
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

    db.upsert_proof_fact_values(&[spawn_effect_seed(
        spawn_row.site.id,
        "effect:axum-test-client-task-spawn",
    )])?;

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
fn axum_usage_questions_report_reachable_performance_seed_for_json_parse() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Performance work:
    //   "Which callers trigger repeated parsing, cloning, serialization, or
    //   database work?"
    //   "What call chains reach a function that is known to dominate runtime
    //   cost?"
    //
    // Source-oracle chain:
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)` from
    //     request extraction impls.
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    // Expected contract: a reviewed proof `effect_seed` can classify the
    // serde_json frontier as a closed-vocabulary performance/surface-measure
    // sink. Owner-scoped queries should find it through the resolved
    // `Self::from_bytes` edge, keep the original external callsite targetless,
    // and preserve the path to the owner that contains the parse frontier.
    let parse_owner = method_id_by_name_and_body_substring(
        &db,
        "from_bytes",
        "serde_json::Deserializer::from_slice(bytes)",
    )?;
    let parse_context = db.call_context_for_owner(parse_owner)?;
    let parse_row = row_by_path(
        &parse_context,
        &["serde_json", "Deserializer", "from_slice"],
    );
    assert_external_targetless(parse_row);
    assert!(
        parse_row
            .site
            .cfgs
            .iter()
            .any(|cfg| cfg == r#"feature = "json""#),
        "serde_json parse frontier should preserve the json feature cfg: {parse_row:#?}"
    );
    assert!(
        relations_for_site(&db, parse_row.site.id)?.rows.is_empty(),
        "performance proof seed must not fabricate a local serde_json edge"
    );

    db.upsert_proof_fact_values(&[effect_seed(
        parse_row.site.id,
        "effect:axum-json-parse-surface-measure",
        "surface_measure",
    )])?;

    let callers =
        method_ids_by_name_and_body_substring(&db, "from_request", "Self::from_bytes(&bytes)")?;
    assert_eq!(
        callers.len(),
        2,
        "axum/src/json.rs should expose two request extraction callers for Json::from_bytes"
    );

    for caller in callers {
        let effects = db.call_effects_reachable_from_owner(
            caller,
            CallPathOptions {
                max_depth: 2,
                max_paths: 16,
            },
        )?;
        let effect = effects
            .iter()
            .find(|effect| effect.effect_seed_id == "effect:axum-json-parse-surface-measure")
            .unwrap_or_else(|| {
                panic!(
                    "reachable performance query should report the serde_json parse sink for caller {caller}: {effects:#?}"
                )
            });
        assert_eq!(effect.effect_class, "surface_measure");
        assert_eq!(effect.confidence.as_deref(), Some("source-oracle"));
        assert_eq!(effect.call_site.site.id, parse_row.site.id);
        assert_eq!(effect.call_site.status.status, CallStatusKind::External);
        assert!(
            effect.call_site.targets.is_empty(),
            "external performance frontier should stay targetless: {effect:#?}"
        );
        let path = effect
            .paths_to_owner
            .iter()
            .find(|path| path.start_id == caller && path.end_id == parse_owner && path.depth == 1)
            .unwrap_or_else(|| {
                panic!(
                    "performance effect should preserve caller -> Json::from_bytes path for caller {caller}: {effect:#?}"
                )
            });
        assert_eq!(path.edges.len(), 1);
        assert_eq!(path.edges[0].caller_id, caller);
        assert_eq!(path.edges[0].callee_id, parse_owner);
        assert_eq!(path.edges[0].source_kind, CallSiteKind::Path);
        assert_eq!(path.edges[0].relation, CallRelationKind::AssociatedFunction);
    }

    Ok(())
}

#[test]
fn memchr_usage_questions_report_reachable_unsafe_block_calls() -> Result<(), DbError> {
    let db = setup_call_graph_db(&CORPUS_MEMCHR_CALL_GRAPH)?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach `unsafe` blocks or FFI boundaries?"
    // Debugging:
    //   "What source callsite corresponds to this persisted call edge or proof
    //   blocker?"
    //
    // Source oracle:
    //   memchr/src/arch/x86_64/memchr.rs:153 generates
    //   `core::mem::transmute::<Fn, RealFn>(fun)(...)` inside an unsafe
    //   block. The `memchr_raw` invocation at :180 expands to a targetless
    //   external path row plus a targetless returned-path dynamic row.
    //
    // Expected contract: the usage helper reports unsafe-block callsites
    // reachable from `memchr_raw` by reusing existing call context. It must
    // not turn either generated transmute row into a local traversal edge.
    let owner =
        function_id_by_name_in_module(&db, &["crate", "arch", "x86_64", "memchr"], "memchr_raw")?;
    let calls = db.unsafe_block_calls_reachable_from_owner(
        owner,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;

    assert_eq!(
        calls.len(),
        2,
        "memchr_raw should expose the generated unsafe transmute path and returned-path dynamic rows: {calls:#?}"
    );
    assert!(
        calls.iter().all(|call| call.paths_to_owner.is_empty()),
        "direct unsafe callsites in the seed owner should not invent intermediate paths: {calls:#?}"
    );
    for call in &calls {
        assert!(
            call.call_site.site.unsafe_block,
            "unsafe helper should only return unsafe-block callsites: {call:#?}"
        );
        assert_external_targetless(&call.call_site);
        assert!(
            relations_for_site(&db, call.call_site.site.id)?
                .rows
                .is_empty(),
            "unsafe-block reporting must not fabricate local edges: {call:#?}"
        );
    }
    assert!(
        calls.iter().any(|call| {
            call.call_site.site.kind == CallSiteKind::Path
                && call.call_site.site.path.as_ref() == Some(&path(&["core", "mem", "transmute"]))
                && call.call_site.site.generic_arg_count == Some(2)
        }),
        "unsafe report should include generated core::mem::transmute path call: {calls:#?}"
    );
    assert!(
        calls.iter().any(|call| {
            call.call_site.site.kind == CallSiteKind::Dynamic
                && call.call_site.site.path.as_ref() == Some(&path(&["core", "mem", "transmute"]))
                && call.call_site.site.arg_count == Some(3)
        }),
        "unsafe report should include generated returned-path dynamic call: {calls:#?}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_report_reachable_effect_policy_violation() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink that is outside the
    //   caller's reviewed effect policy?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: a caller-supplied policy allowlist is evaluated over
    // existing reachable effect seeds only. The disallowed `async_task_spawn`
    // effect remains attached to the external targetless `tokio::spawn`
    // frontier and does not become a local traversal edge.
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

    db.upsert_proof_fact_values(&[spawn_effect_seed(
        spawn_row.site.id,
        "effect:axum-test-client-task-spawn-policy",
    )])?;

    let violations = db.call_effect_policy_violations_for_owner(
        start,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
        &["ffi_boundary"],
    )?;
    let violation = violations
        .iter()
        .find(|violation| {
            violation.effect.effect_seed_id == "effect:axum-test-client-task-spawn-policy"
        })
        .unwrap_or_else(|| {
            panic!(
                "policy helper should report the reachable async_task_spawn violation: {violations:#?}"
            )
        });
    assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(violation.effect.effect_class, "async_task_spawn");
    assert_eq!(violation.effect.call_site.site.id, spawn_row.site.id);
    assert_eq!(
        violation.effect.call_site.status.status,
        CallStatusKind::External
    );
    assert!(
        violation
            .effect
            .paths_to_owner
            .iter()
            .any(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2),
        "policy violation should preserve the resolved path to the sink owner: {violation:#?}"
    );
    assert!(
        relations_for_site(&db, violation.effect.call_site.site.id)?
            .rows
            .is_empty(),
        "policy evaluation must not fabricate local call edges"
    );

    let allowed = db.call_effect_policy_violations_for_owner(
        start,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
        &["async_task_spawn"],
    )?;
    assert!(
        allowed.is_empty(),
        "allowing async_task_spawn should clear the policy violation: {allowed:#?}"
    );

    Ok(())
}

#[test]
fn axum_usage_questions_classify_reachable_effect_guard_paths() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Can this entrypoint reach a sensitive sink without passing through
    //   the reviewed guard?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: the guard report classifies the resolved path to the
    // owner that contains the external targetless `tokio::spawn` effect. It
    // must not fabricate a call edge to `tokio::spawn` itself.
    let start = function_id_by_name_in_module(
        &db,
        &["crate", "form", "tests"],
        "deserialize_error_status_codes",
    )?;
    let guard = method_id_by_name_and_body_substring(&db, "new", "spawn_service(svc)")?;
    let spawn_owner = function_id_by_name_in_module(
        &db,
        &["crate", "test_helpers", "test_client"],
        "spawn_service",
    )?;
    let spawn_context = db.call_context_for_owner(spawn_owner)?;
    let spawn_row = row_by_path(&spawn_context, &["tokio", "spawn"]);
    assert_external_targetless(spawn_row);

    db.upsert_proof_fact_values(&[spawn_effect_seed(
        spawn_row.site.id,
        "effect:axum-test-client-task-spawn-guard",
    )])?;

    let options = CallPathOptions {
        max_depth: 3,
        max_paths: 16,
    };
    let report =
        db.call_effect_guard_report_for_owner(start, guard, "async_task_spawn", options)?;
    assert_eq!(report.owner.id, start);
    assert_eq!(report.guard.id, guard);
    assert_eq!(report.effect_class, "async_task_spawn");
    assert!(
        report.guarded,
        "TestClient::new should guard every resolved path to the spawn effect owner: {report:#?}"
    );
    assert_eq!(report.effects.len(), 1);
    assert!(
        report.violations.is_empty(),
        "guarded report should not contain violations: {report:#?}"
    );
    let effect = &report.effects[0];
    assert_eq!(
        effect.effect_seed_id,
        "effect:axum-test-client-task-spawn-guard"
    );
    assert_eq!(effect.call_site.site.id, spawn_row.site.id);
    assert!(
        effect
            .paths_to_owner
            .iter()
            .any(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2),
        "guarded effect should preserve the resolved path to spawn_service: {effect:#?}"
    );
    assert!(
        relations_for_site(&db, effect.call_site.site.id)?
            .rows
            .is_empty(),
        "effect guard report must not fabricate local call edges"
    );

    let unrelated = method_id_by_name_and_body_substring(&db, "new", "default_fallback: true")?;
    let unguarded =
        db.call_effect_guard_report_for_owner(start, unrelated, "async_task_spawn", options)?;
    assert!(
        !unguarded.guarded,
        "unrelated MethodRouter::new should not guard the spawn effect: {unguarded:#?}"
    );
    assert_eq!(unguarded.effects.len(), 1);
    assert_eq!(
        unguarded.violations.len(),
        1,
        "unguarded report should return the reachable effect as a violation: {unguarded:#?}"
    );
    assert_eq!(unguarded.violations[0].call_site.site.id, spawn_row.site.id);

    Ok(())
}

#[test]
fn axum_usage_questions_report_stored_effect_policy_violation() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink that is outside its
    //   admitted stored effect policy?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    // Expected contract: the persisted `effect_policy` proof fact supplies the
    // allowlist for this owner. The disallowed `async_task_spawn` effect remains
    // attached to the external targetless `tokio::spawn` frontier and the query
    // preserves the resolved two-hop path to `spawn_service`.
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

    db.upsert_proof_fact_values(&[
        spawn_effect_seed(
            spawn_row.site.id,
            "effect:axum-test-client-task-spawn-stored-policy",
        ),
        owner_effect_policy(
            start,
            "effect-policy:axum-test-client:stored-policy",
            &["ffi_boundary"],
        ),
    ])?;

    let violations = db.call_effect_policy_violations_for_stored_owner_policy(
        start,
        CallPathOptions {
            max_depth: 3,
            max_paths: 16,
        },
    )?;
    let violation = violations
        .iter()
        .find(|violation| {
            violation.effect.effect_seed_id
                == "effect:axum-test-client-task-spawn-stored-policy"
        })
        .unwrap_or_else(|| {
            panic!(
                "stored policy helper should report the reachable async_task_spawn violation: {violations:#?}"
            )
        });
    assert_eq!(violation.allowed_effects, vec!["ffi_boundary".to_string()]);
    assert_eq!(violation.effect.effect_class, "async_task_spawn");
    assert_eq!(violation.effect.call_site.site.id, spawn_row.site.id);
    assert_eq!(
        violation.effect.call_site.status.status,
        CallStatusKind::External
    );
    assert!(
        violation
            .effect
            .paths_to_owner
            .iter()
            .any(|path| path.start_id == start && path.end_id == spawn_owner && path.depth == 2),
        "stored policy violation should preserve the resolved path to spawn_service: {violation:#?}"
    );
    assert!(
        relations_for_site(&db, violation.effect.call_site.site.id)?
            .rows
            .is_empty(),
        "stored policy evaluation must not fabricate local call edges"
    );

    db.upsert_proof_fact_values(&[owner_effect_policy(
        start,
        "effect-policy:axum-test-client:stored-policy-conflict",
        &["async_task_spawn"],
    )])?;
    let ambiguous = db
        .call_effect_policy_violations_for_stored_owner_policy(
            start,
            CallPathOptions {
                max_depth: 3,
                max_paths: 16,
            },
        )
        .expect_err("multiple admitted owner policies should reject stored policy lookup");
    assert!(
        ambiguous
            .to_string()
            .contains("multiple admitted effect_policy")
    );

    Ok(())
}

#[derive(Clone, Copy)]
struct FuturePollCase {
    label: &'static str,
    file_suffix: &'static str,
    body: &'static str,
    source: &'static str,
    query: &'static str,
    receiver: FuturePollReceiver,
}

#[derive(Clone, Copy)]
enum FuturePollReceiver {
    MethodResult(&'static str),
    Unsupported,
}

impl FuturePollCase {
    const AXUM: [Self; 2] = [
        Self {
            label: "axum/src/error_handling/mod.rs:251 boxed dyn Future poll",
            file_suffix: "axum/src/error_handling/mod.rs",
            body: "self.project().future.poll(cx)",
            source: "axum/src/error_handling/mod.rs:251 dyn Future::poll",
            query: "dyn Future::poll",
            receiver: FuturePollReceiver::Unsupported,
        },
        Self {
            label: "axum/src/middleware/from_fn.rs:375 BoxFuture as_mut poll",
            file_suffix: "axum/src/middleware/from_fn.rs",
            body: "self.inner.as_mut().poll(cx).map(Ok)",
            source: "axum/src/middleware/from_fn.rs:375 BoxFuture::as_mut().poll",
            query: "BoxFuture::as_mut().poll",
            receiver: FuturePollReceiver::MethodResult("as_mut"),
        },
    ];

    fn call_receiver(self) -> CallReceiver {
        match self.receiver {
            FuturePollReceiver::MethodResult(method) => CallReceiver::MethodCallResult {
                method_name: method.to_string(),
            },
            FuturePollReceiver::Unsupported => CallReceiver::Unsupported,
        }
    }

    fn owner(self, db: &Database) -> Result<Uuid, DbError> {
        method_id_by_name_body_and_file_suffix(db, "poll", self.body, self.file_suffix)
    }

    fn poll_row<'a>(self, context: &'a [CallContextRow]) -> &'a CallContextRow {
        let receiver = self.call_receiver();
        row_by_method_receiver(context, "poll", &receiver)
    }
}

#[test]
fn axum_usage_questions_report_future_poll_runtime_dispatch_blockers() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging / Security analysis:
    //   "Which reachable call frontiers are blocked by runtime dispatch?"
    //   "Can the call graph distinguish an unsupported frontier from a
    //   fabricated local edge?"
    //
    // Source oracles:
    //   axum/src/error_handling/mod.rs:240 stores
    //     `Pin<Box<dyn Future<Output = Result<Response, Infallible>>>>`.
    //   axum/src/error_handling/mod.rs:251 calls
    //     `self.project().future.poll(cx)`.
    //   axum/src/middleware/from_fn.rs:368 stores
    //     `BoxFuture<'static, Response>`.
    //   axum/src/middleware/from_fn.rs:375 calls
    //     `self.inner.as_mut().poll(cx).map(Ok)`.
    // Expected contract: each future poll callsite remains unsupported and
    // targetless, while proof facts can attach the runtime-dispatch blocker to
    // the same callsite identity for fail-closed traversal consumers.
    for case in FuturePollCase::AXUM {
        let owner = case.owner(&db)?;
        let context = db.call_context_for_owner(owner)?;
        let poll = case.poll_row(&context);
        assert_targetless_status(poll, CallStatusKind::Unsupported);
        assert!(
            relations_for_site(&db, poll.site.id)?.rows.is_empty(),
            "{} must not fabricate a local edge",
            case.label
        );
        assert_no_traversal_candidates_for_site(&db, owner, poll.site.id, case.label)?;

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
                    "reach report should expose {} as unsupported: {report:#?}",
                    case.label
                )
            });
        assert_eq!(frontier.site.owner_id, owner);
        assert!(
            frontier.targets.is_empty(),
            "{} unsupported frontier should remain targetless: {frontier:#?}",
            case.label
        );

        let site = poll.site.id.to_string();
        db.upsert_proof_fact_values(&[ploke_test_utils::axum_future_poll_blocker(
            poll.site.id,
            case.source,
        )])?;

        let blockers = db.proof_blockers()?;
        assert!(
            blockers.iter().any(|proof| {
                proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.reason == "dynamic_dispatch_unbounded"
                    && proof.status == "blocked"
            }),
            "{} should expose a runtime-dispatch proof blocker: {blockers:#?}",
            case.label
        );

        let proof_rows = db.proof_graphrag_context(case.query)?;
        assert!(
            proof_rows.iter().any(|proof| {
                proof.kind == "proof_blocker"
                    && proof.call_site_id.as_deref() == Some(site.as_str())
                    && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
            }),
            "RAG proof context lookup should retrieve the {} blocker: {proof_rows:#?}",
            case.label
        );
    }

    Ok(())
}

#[test]
fn axum_usage_questions_list_future_poll_runtime_dispatch_needs() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;
    let domain_id = "bd:corpus-axum-call-graph";

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Debugging / RAG:
    //   "Which runtime-dispatch frontiers are blocking a complete local
    //   traversal from this owner?"
    //   "What proof input is missing for an async poll/resume boundary?"
    //
    // Expected contract: once the proof layer records runtime-dispatch blockers
    // for reviewed targetless future-poll callsites, the owner-scoped proof
    // queue lists them as dynamic-dispatch needs without adding traversal
    // edges. An admitted runtime summary discharges the authoring need only.
    for case in FuturePollCase::AXUM {
        let owner = case.owner(&db)?;
        let context = db.call_context_for_owner(owner)?;
        let poll = case.poll_row(&context);
        assert_targetless_status(poll, CallStatusKind::Unsupported);
        assert!(
            relations_for_site(&db, poll.site.id)?.rows.is_empty(),
            "{} must not start with a local edge",
            case.label
        );

        db.project_call_proof_facts_for_owner(owner, domain_id)?;
        db.upsert_proof_fact_values(&[ploke_test_utils::axum_future_poll_blocker(
            poll.site.id,
            case.source,
        )])?;

        let needs = db.runtime_dispatch_needs_for_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?;
        let need = needs
            .iter()
            .find(|need| need.call_site.site.id == poll.site.id)
            .unwrap_or_else(|| {
                panic!(
                    "{} should be listed as an owner-scoped runtime-dispatch need: {needs:#?}",
                    case.label
                )
            });
        assert_targetless_status(&need.call_site, CallStatusKind::Unsupported);
        assert!(
            need.paths_to_owner.is_empty(),
            "{} direct frontier should not need an intermediate path: {need:#?}",
            case.label
        );
        assert!(
            need.blocker_reasons
                .iter()
                .any(|reason| reason == "dynamic_dispatch_unbounded"),
            "{} should retain the dynamic-dispatch blocker: {need:#?}",
            case.label
        );
        assert!(
            relations_for_site(&db, poll.site.id)?.rows.is_empty(),
            "{} runtime-dispatch proof queue must not fabricate an edge",
            case.label
        );

        db.upsert_proof_fact_values(&[
            ploke_test_utils::axum_future_poll_runtime_dispatch_summary(poll.site.id, case.source),
        ])?;
        let after = db.runtime_dispatch_needs_for_owner(
            owner,
            CallPathOptions {
                max_depth: 1,
                max_paths: 16,
            },
        )?;
        assert!(
            after
                .iter()
                .all(|need| need.call_site.site.id != poll.site.id),
            "{} admitted summary should discharge the proof-authoring need: {after:#?}",
            case.label
        );
        assert!(
            relations_for_site(&db, poll.site.id)?.rows.is_empty(),
            "{} admitted summary must not fabricate a local edge",
            case.label
        );
    }

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
    // an external frontier because the direct inner call chain is externally
    // proven, but it has no fabricated callee and cannot become a local
    // traversal edge.
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
        &CallReceiver::AwaitMethodCallResult {
            method_name: "acquire_owned".to_string(),
        },
        CallStatusKind::External,
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
        "targetless awaited receiver rows must not appear in call paths: {paths:#?}"
    );
    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let external = report
        .external_frontier_calls
        .iter()
        .find(|row| row.site.id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "reach report should expose awaited-result unwrap in external frontier rows: {report:#?}"
            )
        });
    assert_eq!(external.site.owner_id, owner);
    assert_eq!(external.status.status, CallStatusKind::External);
    assert!(
        external.targets.is_empty(),
        "external frontier call should remain targetless: {external:#?}"
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
