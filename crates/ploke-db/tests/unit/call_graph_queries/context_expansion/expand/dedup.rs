use super::*;

use ploke_db::CallContextCandidate;

#[test]
fn expand_call_context_deduplicates_candidates_before_applying_limit() -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let owner = Uuid::from_u128(0x501);
    let other_owner = Uuid::from_u128(0x502);
    let target = Uuid::from_u128(0x503);
    let other_target = Uuid::from_u128(0x504);
    let module = Uuid::from_u128(0x505);

    let first_duplicate_site = Uuid::from_u128(0x511);
    let second_duplicate_site = Uuid::from_u128(0x512);
    let distinct_site = Uuid::from_u128(0x513);
    let other_owner_site = Uuid::from_u128(0x514);

    for (site, target, span) in [
        (first_duplicate_site, target, (10, 20)),
        (second_duplicate_site, target, (30, 40)),
        (distinct_site, other_target, (50, 60)),
    ] {
        insert_resolved_graph(
            &db,
            ResolvedGraphSeed {
                owner,
                module,
                site,
                target,
                file: None,
                span,
                path: &["crate", "target"],
            },
        )?;
    }

    insert_resolved_graph(
        &db,
        ResolvedGraphSeed {
            owner: other_owner,
            module,
            site: other_owner_site,
            target,
            file: None,
            span: (70, 80),
            path: &["crate", "target"],
        },
    )?;

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 2,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(outgoing.len(), 2, "outgoing candidates: {outgoing:#?}");
    assert_unique_nodes(&outgoing);
    assert!(
        outgoing.iter().any(|candidate| candidate.node_id == target
            && candidate.relation == CallContextRelation::OutgoingTarget),
        "deduped outgoing expansion should keep the repeated target once: {outgoing:#?}"
    );
    assert!(
        outgoing
            .iter()
            .any(|candidate| candidate.node_id == other_target
                && candidate.relation == CallContextRelation::OutgoingTarget),
        "deduping before truncation should leave room for the distinct target: {outgoing:#?}"
    );

    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 2,
            ..CallContextOptions::default()
        },
    )?;
    assert_eq!(incoming.len(), 2, "incoming candidates: {incoming:#?}");
    assert_unique_nodes(&incoming);
    assert!(
        incoming.iter().any(|candidate| candidate.node_id == owner
            && candidate.relation == CallContextRelation::IncomingCaller),
        "deduped incoming expansion should keep the repeated caller once: {incoming:#?}"
    );
    assert!(
        incoming
            .iter()
            .any(|candidate| candidate.node_id == other_owner
                && candidate.relation == CallContextRelation::IncomingCaller),
        "deduping before truncation should leave room for the distinct caller: {incoming:#?}"
    );

    Ok(())
}

fn assert_unique_nodes(candidates: &[CallContextCandidate]) {
    for (index, candidate) in candidates.iter().enumerate() {
        assert!(
            !candidates[index + 1..].iter().any(|other| {
                other.node_id == candidate.node_id && other.relation == candidate.relation
            }),
            "duplicate graphRAG candidate for node {} and relation {:?}: {candidates:#?}",
            candidate.node_id,
            candidate.relation
        );
    }
}
