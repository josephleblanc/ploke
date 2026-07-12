use ploke_db::CallPathOptions;

use super::super::*;
use super::common::*;

#[test]
fn axum_body_size_hint_external_summary_covers_method_frontier() -> Result<(), DbError> {
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
    //   axum-core/src/body.rs defines `impl http_body::Body for Body`.
    //   Its `size_hint` method calls `self.0.size_hint()`, where `Body` stores
    //   a `BoxBody` tuple field backed by the external http-body-util body.
    // Current contract: the method call remains an external targetless
    // frontier, but an admitted summary can discharge the missing-summary
    // proof need and expose a proof-derived effect without adding a call edge.
    let owner = method_id_by_name_and_body_substring(&db, "size_hint", "self.0.size_hint()")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(
        &context,
        "size_hint",
        &CallReceiver::SelfField {
            path: vec!["0".to_string()],
        },
    );
    assert_external_targetless(row);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.arg_count, Some(0));
    let site_id = row.site.id;

    let projected = db.project_call_proof_facts_for_owner(owner, domain_id)?;
    assert!(
        projected >= 2,
        "Body::size_hint should project call_site and call_resolution proof rows: {projected}"
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
            panic!("Body::size_hint should be listed as an external-summary need before admission: {needs:#?}")
        });
    assert_external_targetless(&need.call_site);
    assert!(
        need.paths_to_owner.is_empty(),
        "direct Body::size_hint frontier should not need an intermediate path: {need:#?}"
    );
    assert!(
        need.blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "Body::size_hint summary need should retain the active missing-summary blocker: {need:#?}"
    );

    let before = db.call_effects_reachable_from_owner(owner, options)?;
    assert!(
        before
            .iter()
            .all(|effect| effect.call_site.site.id != site_id
                || !effect
                    .effect_seed_id
                    .contains(ploke_test_utils::AXUM_BODY_SIZE_HINT_SUMMARY_ID)),
        "Body::size_hint should not expose a summary-derived effect before admission: {before:#?}"
    );

    db.upsert_proof_fact_values(&ploke_test_utils::axum_body_size_hint_summary_records(
        site_id,
    ))?;

    let after = db.external_summary_needs_for_owner(owner, options)?;
    assert!(
        after.iter().all(|need| need.call_site.site.id != site_id),
        "admitted Body::size_hint summary should discharge this owner-scoped need: {after:#?}"
    );

    let summary_id = ploke_test_utils::AXUM_BODY_SIZE_HINT_SUMMARY_ID;
    let effect_id = format!("summary-effect:{summary_id}:external_summary_boundary");
    let effects = db.call_effects_reachable_from_owner(owner, options)?;
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == effect_id)
        .unwrap_or_else(|| {
            panic!(
                "reachable effects should include admitted Body::size_hint summary: {effects:#?}"
            )
        });
    assert_eq!(effect.effect_class, "external_summary_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("source-oracle-review"));
    assert_eq!(effect.blocker_if_unresolved, Some(false));
    assert_eq!(effect.call_site.site.id, site_id);
    assert_eq!(effect.call_site.status.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct Body::size_hint frontier should not need an intermediate path: {effect:#?}"
    );
    assert!(
        effect.blocker_reasons.is_empty(),
        "admitted summary should discharge the missing-summary blocker: {effect:#?}"
    );
    assert!(
        relations_for_site(&db, site_id)?.rows.is_empty(),
        "summary admission must not fabricate a local Body::size_hint edge"
    );

    Ok(())
}
