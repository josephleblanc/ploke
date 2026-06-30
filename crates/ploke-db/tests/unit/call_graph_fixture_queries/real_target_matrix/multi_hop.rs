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
