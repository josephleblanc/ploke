use std::collections::BTreeMap;

use ploke_db::{CallContextRelation, CallContextSeed, CallPathOptions};

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

    Ok(())
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
