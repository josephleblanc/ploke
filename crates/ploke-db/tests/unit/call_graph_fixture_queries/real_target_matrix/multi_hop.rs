use ploke_db::{CallContextRelation, CallContextSeed, CallPathOptions};

use super::super::*;
use super::common::*;

#[test]
fn axum_request_extract_reaches_from_request_trait_method_in_two_hops() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // This is the first real-corpus multi-hop call-chain contract: the direct
    // edge tests already prove each one-hop edge separately; this test requires
    // the DB API to preserve the ordered two-edge path.
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

    let one_hop = db.call_paths_from_owner(
        start,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        one_hop.iter().all(|path| path.end_id != target),
        "one-hop traversal should not skip over the intermediate method: {one_hop:#?}"
    );

    let outgoing = db.call_paths_from_owner(
        start,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let path = outgoing
        .iter()
        .find(|path| path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!(
                "expected two-hop path from RequestExt::extract to FromRequest::from_request: {outgoing:#?}"
            )
        });
    assert_eq!(path.start_id, start);
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);

    let incoming = db.call_paths_to_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let reverse_path = incoming
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("expected reverse lookup to find the same two-hop source chain: {incoming:#?}")
        });
    assert_eq!(reverse_path.edges, path.edges);

    Ok(())
}

#[test]
fn axum_from_request_expand_reaches_extract_fields_in_two_free_function_hops() -> Result<(), DbError>
{
    let db = setup_axum_call_graph_db()?;

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source-oracle chain:
    //   axum-macros/src/from_request/mod.rs:93
    //     defines `from_request::expand`.
    //   axum-macros/src/from_request/mod.rs:145
    //     `expand` calls `impl_struct_by_extracting_each_field(...)`.
    //   axum-macros/src/from_request/mod.rs:330
    //     defines `impl_struct_by_extracting_each_field`.
    //   axum-macros/src/from_request/mod.rs:342
    //     `impl_struct_by_extracting_each_field` calls `extract_fields(...)`.
    //   axum-macros/src/from_request/mod.rs:412
    //     defines `extract_fields`.
    //
    // This covers the regular free-function multi-hop bucket: both edges are
    // unqualified local path calls between normal functions, with no method,
    // trait, closure, dynamic-call, or proc-macro body involvement.
    let start = function_id_by_name_in_module(&db, &["crate", "from_request"], "expand")?;
    let intermediate = function_id_by_name_in_module(
        &db,
        &["crate", "from_request"],
        "impl_struct_by_extracting_each_field",
    )?;
    let target = function_id_by_name_in_module(&db, &["crate", "from_request"], "extract_fields")?;

    let start_context = db.call_context_for_owner(start)?;
    let first_edge = row_by_path(&start_context, &["impl_struct_by_extracting_each_field"]);
    assert_resolved_target(
        first_edge,
        intermediate,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let intermediate_context = db.call_context_for_owner(intermediate)?;
    let second_edge = row_by_path(&intermediate_context, &["extract_fields"]);
    assert_resolved_target(
        second_edge,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let one_hop = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 128,
        },
    )?;
    assert!(
        one_hop.is_empty(),
        "one-hop traversal should not skip over impl_struct_by_extracting_each_field: {one_hop:#?}"
    );

    let paths = db.call_paths_between(
        start,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == start && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("expected two-hop path from from_request::expand to extract_fields: {paths:#?}")
        });
    assert_eq!(path.edges.len(), 2);
    assert_eq!(path.edges[0].caller_id, start);
    assert_eq!(path.edges[0].callee_id, intermediate);
    assert_eq!(path.edges[0].call_site_id, first_edge.site.id);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Function);
    assert_eq!(path.edges[1].caller_id, intermediate);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].call_site_id, second_edge.site.id);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    let outgoing = db.call_paths_from_owner(
        start,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    assert!(
        outgoing
            .iter()
            .any(|candidate| candidate.edges == path.edges),
        "owner traversal should expose the same ordered free-function path: {outgoing:#?}"
    );

    let incoming = db.call_paths_to_target(
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 128,
        },
    )?;
    assert!(
        incoming
            .iter()
            .any(|candidate| candidate.edges == path.edges),
        "target traversal should expose the same ordered free-function path: {incoming:#?}"
    );

    Ok(())
}

#[test]
fn axum_request_extract_expands_call_path_context_in_two_hops() -> Result<(), DbError> {
    let db = setup_axum_call_graph_db()?;

    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    //
    // This is the DB query primitive for usage questions such as "from this
    // function, which local callees can I reach within N hops?" and "which
    // callers can reach this target within N hops?" It returns the same
    // candidate shape as one-hop `expand_call_context`, but with real path
    // distance from the persisted call graph.
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

    let outgoing = db.expand_call_path_context(
        CallContextSeed::Owner(start),
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let outgoing_target = outgoing
        .iter()
        .find(|candidate| {
            candidate.node_id == target
                && candidate.relation == CallContextRelation::OutgoingTarget
                && candidate.distance == 2
        })
        .unwrap_or_else(|| {
            panic!("expected two-hop outgoing target candidate for FromRequest::from_request: {outgoing:#?}")
        });
    assert_eq!(outgoing_target.target_id, target);

    let direct_target = outgoing
        .iter()
        .find(|candidate| {
            candidate.node_id == intermediate
                && candidate.relation == CallContextRelation::OutgoingTarget
                && candidate.distance == 1
        })
        .unwrap_or_else(|| {
            panic!(
                "expected one-hop outgoing target candidate for extract_with_state: {outgoing:#?}"
            )
        });
    assert_eq!(direct_target.target_id, intermediate);
    assert_ne!(
        outgoing_target.call_site_id, direct_target.call_site_id,
        "multi-hop candidate should retain the terminal edge call-site, not collapse to the first edge"
    );

    let incoming = db.expand_call_path_context(
        CallContextSeed::Target(target),
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let incoming_start = incoming
        .iter()
        .find(|candidate| {
            candidate.node_id == start
                && candidate.relation == CallContextRelation::IncomingCaller
                && candidate.distance == 2
        })
        .unwrap_or_else(|| {
            panic!(
                "expected two-hop incoming caller candidate for RequestExt::extract: {incoming:#?}"
            )
        });
    assert_eq!(incoming_start.target_id, target);

    Ok(())
}
