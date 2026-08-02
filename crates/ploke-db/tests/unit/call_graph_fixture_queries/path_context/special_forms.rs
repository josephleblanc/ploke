use ploke_db::{CallPathOptions, ProofGraphStore};
use serde_json::json;

use super::*;

fn process_create_effect_seed(
    call_site_id: impl ToString,
    effect_seed_id: &str,
) -> serde_json::Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id.to_string(),
        "effect_class": "operating_system_process_create",
        "confidence": "fixture-source-oracle",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })
}

#[test]
fn fixture_context_reads_projected_generic_unsafe_extern_and_chained_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_generic_identity_turbofish")?;
    let target = function_id_by_name(&db, "generic_identity")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["generic_identity"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_method_turbofish")?;
    let target = method_id_by_impl_self_type_name(&db, "GenericMethodTarget", "generic_instance")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic method context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("generic_instance"));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_unsafe_function")?;
    let target = function_id_by_name(&db, "unsafe_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "unsafe function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["unsafe_target"])));
    assert!(
        row.site.unsafe_block,
        "unsafe_target() call should preserve unsafe-block occurrence metadata: {row:#?}"
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let owner_info = db
        .call_node_info(owner)?
        .expect("call_unsafe_function should have call node metadata");
    let target_info = db
        .call_node_info(target)?
        .expect("unsafe_target should have call node metadata");
    assert!(
        !owner_info.is_unsafe,
        "safe wrapper should not inherit unsafe metadata from its callee: {owner_info:#?}"
    );
    assert!(
        target_info.is_unsafe,
        "unsafe function item metadata should be visible on the call target: {target_info:#?}"
    );

    let unsafe_impact = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        unsafe_impact.target.is_unsafe,
        "impact summaries should mark unsafe function item targets: {unsafe_impact:#?}"
    );
    assert!(
        unsafe_impact
            .direct_callers
            .iter()
            .any(|caller| caller.id == owner && !caller.is_unsafe),
        "unsafe target impact should preserve the safe direct caller without marking it unsafe: {unsafe_impact:#?}"
    );

    let owner = function_id_by_name(&db, "call_extern_c_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "extern C context rows: {context:#?}");
    let row = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    assert!(
        row.site.unsafe_block,
        "abs(value) external frontier should preserve unsafe-block occurrence metadata: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_imported_external_type_alias_constructor")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "imported external type alias constructor context rows: {context:#?}"
    );
    let row = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["ImportedExternalVec", "new"],
            0,
            CallStatusKind::External,
            "imported external type alias constructor",
        ),
    );
    assert!(
        relations_for_site(&db, row.site.id)?.rows.is_empty(),
        "imported external type alias constructor must not fabricate local call_relation targets"
    );

    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let maker = function_id_by_name(&db, "make_unary_fn")?;
    let returned = function_id_by_name(&db, "unary_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "chained call context rows: {context:#?}");

    let row = row_by_path(&context, &["make_unary_fn"]);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(&context, CallSiteKind::Dynamic, &["make_unary_fn"]);
    assert_eq!(dynamic.site.arg_count, Some(1));
    assert_resolved_target(
        dynamic,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_qualified_dyn_any_downcast_mut")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "qualified dyn Any path context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["std", "any", "Any", "downcast_mut"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "qualified dyn Any downcast_mut must stay targetless: {row:#?}"
    );
    assert!(
        relations_for_site(&db, row.site.id)?.rows.is_empty(),
        "qualified dyn Any downcast_mut must stay an external frontier without local targets"
    );

    Ok(())
}

#[test]
fn fixture_context_marks_async_function_node_metadata() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let sync_target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["make_ready_local_assoc"]);

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:581 defines
    //   `pub async fn make_ready_local_assoc()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:585 defines
    //   `pub async fn call_await_result_instance_method()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:586 calls
    //   `make_ready_local_assoc().await.instance_value()`.
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner_info = db
        .call_node_info(owner)?
        .expect("call_await_result_instance_method should have call node metadata");
    let target_info = db
        .call_node_info(target)?
        .expect("make_ready_local_assoc should have call node metadata");
    let sync_info = db
        .call_node_info(sync_target)?
        .expect("local_target should have call node metadata");
    assert!(
        owner_info.is_async,
        "async caller metadata should reflect its own signature: {owner_info:#?}"
    );
    assert!(
        target_info.is_async,
        "async function item metadata should be visible on the call target: {target_info:#?}"
    );
    assert!(
        !sync_info.is_async,
        "sync function item metadata should remain false: {sync_info:#?}"
    );

    let impact = db.call_impact_for_target(
        target,
        CallPathOptions {
            max_depth: 1,
            max_paths: 16,
        },
    )?;
    assert!(
        impact.target.is_async,
        "impact summaries should mark async function item targets: {impact:#?}"
    );
    assert!(
        impact
            .direct_callers
            .iter()
            .any(|caller| caller.id == owner && caller.is_async),
        "async target impact should preserve the async direct caller metadata: {impact:#?}"
    );

    Ok(())
}

#[test]
fn fixture_reach_surfaces_extern_c_call_as_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_extern_c_function")?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach unsafe blocks or FFI boundaries?"
    //   "Which external dependency calls are made from this user-facing entrypoint?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 declares
    //   `abs(value)` inside an `unsafe extern "C"` block and calls it from
    //   `call_extern_c_function`. The call is visible as an external frontier,
    //   but no local traversal edge is fabricated for the foreign function.
    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;

    assert_eq!(report.owner.id, owner);
    assert_eq!(report.owner.name, "call_extern_c_function");
    assert!(
        report.paths.is_empty() && report.callees.is_empty(),
        "extern C calls should not fabricate local reach edges: {report:#?}"
    );
    let frontier = assert_targetless_row(
        &report.frontier_calls,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    let external_frontier = assert_targetless_row(
        &report.external_frontier_calls,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    assert_eq!(
        external_frontier.site.id, frontier.site.id,
        "external frontier subset should preserve the same extern C callsite"
    );
    assert!(
        report.unsupported_frontier_calls.is_empty()
            && report.unresolved_frontier_calls.is_empty()
            && report.ambiguous_frontier_calls.is_empty(),
        "extern C reach should classify the FFI boundary as external only: {report:#?}"
    );
    assert!(
        report
            .source_files
            .iter()
            .any(|file| file.ends_with("fixture_call_graph/src/lib.rs")),
        "extern C reach should point back to the fixture source file: {report:#?}"
    );
    assert!(
        relations_for_site(&db, frontier.site.id)?.rows.is_empty(),
        "extern C frontier must not fabricate call edges: {frontier:#?}"
    );

    assert!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")? >= 2,
        "extern C owner should project targetless proof rows"
    );
    db.upsert_proof_fact_values(&[ploke_test_utils::fixture_extern_c_abs_effect_record(
        frontier.site.id,
    )])?;
    let effects = db.call_effects_reachable_from_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let effect = effects
        .iter()
        .find(|effect| effect.effect_seed_id == "effect:fixture-extern-c-abs")
        .unwrap_or_else(|| {
            panic!("extern C reach should expose the FFI boundary effect seed: {effects:#?}")
        });
    assert_eq!(effect.effect_class, "ffi_boundary");
    assert_eq!(effect.confidence.as_deref(), Some("fixture-source-oracle"));
    assert_eq!(effect.blocker_if_unresolved, Some(true));
    assert_eq!(effect.call_site.site.id, frontier.site.id);
    assert_eq!(effect.call_site.status.status, CallStatusKind::External);
    assert!(
        effect.paths_to_owner.is_empty(),
        "direct frontier effects should not invent a self path to the owner: {effect:#?}"
    );
    assert!(
        effect
            .blocker_reasons
            .iter()
            .any(|reason| reason == "external_dependency_summary_missing"),
        "extern C effect should preserve the external-summary blocker reason: {effect:#?}"
    );

    Ok(())
}

#[test]
fn fixture_reach_scopes_proof_invariant_findings_to_reachable_process_effects()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_extern_c_function")?;
    let context = db.call_context_for_owner(owner)?;
    let frontier = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    assert!(
        relations_for_site(&db, frontier.site.id)?.rows.is_empty(),
        "extern C frontier must not fabricate call edges: {frontier:#?}"
    );
    assert!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")? >= 2,
        "extern C owner should project targetless proof rows"
    );

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/process lifetime review:
    //   "Which reachable process-spawn proof obligations are still blocked?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 declares
    //   `abs(value)` inside an `unsafe extern "C"` block and calls it from
    //   `call_extern_c_function`. This test deliberately marks the external
    //   callsite with an OS process-create proof seed so the detached-process
    //   invariant has a scoped finding to expose; the callsite itself remains
    //   targetless.
    db.upsert_proof_fact_values(&[process_create_effect_seed(
        frontier.site.id,
        "effect:fixture-extern-c-process-create",
    )])?;

    let findings = db.call_proof_invariant_findings_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;
    let finding = findings
        .iter()
        .find(|finding| {
            finding.invariant == "detached_process_successor_handoff"
                && finding.call_site_id.as_deref() == Some(frontier.site.id.to_string().as_str())
        })
        .unwrap_or_else(|| {
            panic!(
                "owner-scoped invariant query should report the reachable process effect: {findings:#?}"
            )
        });
    assert_eq!(finding.status, "blocked");
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "external process frontier should remain blocked on summary evidence: {finding:#?}"
    );
    let Some(call_site) = finding.call_site.as_ref() else {
        panic!("scoped invariant finding should preserve the source callsite: {finding:#?}");
    };
    assert_eq!(call_site.site.id, frontier.site.id);
    assert_eq!(call_site.status.status, CallStatusKind::External);
    assert!(
        call_site.targets.is_empty(),
        "proof invariant findings must not fabricate local targets: {finding:#?}"
    );
    assert!(
        relations_for_site(&db, frontier.site.id)?.rows.is_empty(),
        "invariant projection must not fabricate local call edges"
    );

    Ok(())
}
