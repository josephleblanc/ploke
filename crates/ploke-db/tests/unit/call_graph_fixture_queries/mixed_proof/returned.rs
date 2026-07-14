use super::*;

#[test]
fn fixture_projection_stores_real_returned_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let maker = function_id_by_name(&db, "make_fn")?;
    let returned = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic row: {context:#?}"
    );
    let dynamic_row = dynamic_rows[0];
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.site.path.as_ref(), Some(&path(&["make_fn"])));
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 7);
    assert_returned_callable_binding_evidence(
        &db,
        dynamic_row,
        "resolved",
        "returned function dynamic call",
    )?;

    assert_owner_proof_edges(
        &db,
        "returned-function resolved calls",
        &[
            OwnerProofEdge {
                owner,
                site: path_site,
                span: path_span,
                target: maker,
            },
            OwnerProofEdge {
                owner,
                site: dynamic_site,
                span: dynamic_span,
                target: returned,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_returned_parameter_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(
        &db,
        "call_returned_forwarded_function_pointer_param_with_local_target",
    )?;
    let helper = function_id_by_name(&db, "return_forwarded_function_pointer")?;
    let returned = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned parameter function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["return_forwarded_function_pointer"]);
    let dynamic_row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["return_forwarded_function_pointer"],
    );
    assert_resolved_target(
        path_row,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 7);
    assert_returned_callable_binding_evidence(
        &db,
        dynamic_row,
        "resolved",
        "returned parameter function dynamic call",
    )?;

    assert_owner_proof_edges(
        &db,
        "returned parameter function resolved calls",
        &[
            OwnerProofEdge {
                owner,
                site: path_row.site.id,
                span: path_row.site.span,
                target: helper,
            },
            OwnerProofEdge {
                owner,
                site: dynamic_row.site.id,
                span: dynamic_row.site.span,
                target: returned,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_returned_closure_dynamic_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1431:
        // The outer dynamic call invokes the closure returned directly by
        // `make_closure` at lines 1426-1428.
        ReturnedClosureProofCase {
            owner: "call_returned_closure",
            maker: "make_closure",
            path: &["make_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1453:
        // The outer dynamic call invokes the closure binding returned by
        // `make_bound_closure` at lines 1447-1449.
        ReturnedClosureProofCase {
            owner: "call_returned_bound_closure",
            maker: "make_bound_closure",
            path: &["make_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1500:
        // The outer dynamic call invokes the returned local alias of the
        // closure binding in `make_alias_bound_closure` at lines 1493-1496.
        ReturnedClosureProofCase {
            owner: "call_returned_alias_bound_closure",
            maker: "make_alias_bound_closure",
            path: &["make_alias_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
        // The outer dynamic call invokes the sync closure returned by
        // `make_target_closure` through the forwarding producer
        // `make_forwarded_returned_closure`.
        ReturnedClosureProofCase {
            owner: "call_forwarded_returned_closure",
            maker: "make_forwarded_returned_closure",
            path: &["make_forwarded_returned_closure"],
        },
    ];

    let mut expected = Vec::new();
    for case in cases {
        expected.extend(project_returned_closure_proof(&db, case)?);
    }

    assert_owner_proof_edges(
        &db,
        "returned-closure resolved calls",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_resolves_forwarded_returned_closure_value_flow() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
    // `make_forwarded_returned_closure()` returns the closure produced by
    // `make_target_closure()`, while `call_forwarded_returned_closure()`
    // immediately invokes the producer result. This is the bounded sync
    // returned-callable forwarding case: the caller reaches the returned
    // closure owner and can then traverse the closure body to `local_target`.
    let owner = function_id_by_name(&db, "call_forwarded_returned_closure")?;
    let producer = function_id_by_name(&db, "make_forwarded_returned_closure")?;
    let maker = function_id_by_name(&db, "make_target_closure")?;
    let local_target = function_id_by_name(&db, "local_target")?;

    let owner_context = db.call_context_for_owner(owner)?;
    assert_eq!(
        owner_context.len(),
        2,
        "forwarded returned closure caller should expose the producer path and outer dynamic call: {owner_context:#?}"
    );
    let producer_row = row_by_path(&owner_context, &["make_forwarded_returned_closure"]);
    assert_resolved_target(
        producer_row,
        producer,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(
        &owner_context,
        CallSiteKind::Dynamic,
        &["make_forwarded_returned_closure"],
    );
    assert_resolved_target(
        dynamic,
        dynamic.targets[0].target_id,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let closure = dynamic.targets[0].target_id;
    assert!(
        !relations_for_site(&db, dynamic.site.id)?.rows.is_empty(),
        "forwarded returned closure flow should persist a dynamic closure edge"
    );
    assert_return_closure_binding(&db, maker, closure, "make_target_closure")?;

    let producer_context = db.call_context_for_owner(producer)?;
    assert_eq!(
        producer_context.len(),
        1,
        "forwarded returned closure producer should only expose the maker path: {producer_context:#?}"
    );
    let maker_row = row_by_path(&producer_context, &["make_target_closure"]);
    assert_resolved_target(
        maker_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_return_path_call_binding(
        &db,
        producer,
        maker_row.site.id,
        &["make_target_closure"],
        "make_forwarded_returned_closure",
    )?;
    let producer_binding =
        only_return_binding(&db, producer, "make_forwarded_returned_closure flow")?;

    let flows = db.returned_call_binding_flows_for_owner(owner)?;
    assert_eq!(
        flows.len(),
        1,
        "forwarded returned closure should expose one dynamic-call-to-return-binding proof path: {flows:#?}"
    );
    let flow = &flows[0];
    assert_eq!(flow.caller_id, owner);
    assert_eq!(flow.dynamic.id, dynamic.site.id);
    assert_eq!(flow.dynamic.span, dynamic.site.span);
    assert_eq!(
        flow.dynamic.path,
        path(&["make_forwarded_returned_closure"])
    );
    assert_eq!(flow.dynamic.target_id, closure);
    assert_eq!(flow.dynamic.relation, CallRelationKind::DynamicClosure);
    assert_eq!(flow.dynamic.target_kind, CallTargetKind::Closure);
    assert_eq!(flow.producer.id, producer);
    assert_eq!(flow.producer.site_id, producer_row.site.id);
    assert_eq!(flow.producer.span, producer_row.site.span);
    assert_eq!(flow.producer.path, flow.dynamic.path);
    assert_eq!(flow.binding.id, producer_binding.id);
    assert_eq!(
        flow.binding.source.id, maker_row.site.id,
        "producer return binding should be sourced by the maker callsite"
    );
    assert_eq!(
        flow.binding.source.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.binding.source.kind, "Path");

    let producer_paths = db.call_paths_between(
        owner,
        producer,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        producer_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the closure producer: {producer_paths:#?}"
    );
    assert_eq!(producer_paths[0].depth, 1);
    assert_eq!(producer_paths[0].edges[0].caller_id, owner);
    assert_eq!(producer_paths[0].edges[0].callee_id, producer);

    let closure_paths = db.call_paths_between(
        owner,
        closure,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        closure_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the returned closure owner: {closure_paths:#?}"
    );
    assert_eq!(closure_paths[0].depth, 1);
    assert_eq!(closure_paths[0].edges[0].caller_id, owner);
    assert_eq!(closure_paths[0].edges[0].callee_id, closure);
    assert_eq!(
        closure_paths[0].edges[0].relation,
        CallRelationKind::DynamicClosure
    );

    let local_target_paths = db.call_paths_between(
        owner,
        local_target,
        ploke_db::CallPathOptions {
            max_depth: 4,
            max_paths: 16,
        },
    )?;
    assert!(
        local_target_paths.iter().any(|path| {
            path.depth == 2
                && path.edges[0].caller_id == owner
                && path.edges[0].callee_id == closure
                && path.edges[0].relation == CallRelationKind::DynamicClosure
                && path.edges[1].caller_id == closure
                && path.edges[1].callee_id == local_target
                && path.edges[1].relation == CallRelationKind::Function
        }),
        "forwarded returned closure flow should traverse caller -> closure -> local_target: {local_target_paths:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_awaited_returned_async_closure_edge_only_when_polled()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let maker = function_id_by_name(&db, "make_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2355-2365:
    // `make_returned_async_closure()()` is targetless until the returned async
    // closure future is immediately polled by `.await`, either directly or
    // through a same-block local future binding.
    let no_await_owner = function_id_by_name(&db, "call_returned_async_closure_without_await")?;
    let no_await_context = db.call_context_for_owner(no_await_owner)?;
    assert_eq!(
        no_await_context.len(),
        2,
        "un-awaited returned async closure proof context rows: {no_await_context:#?}"
    );
    let no_await_path = row_by_path(&no_await_context, &["make_returned_async_closure"]);
    assert_resolved_target(
        no_await_path,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let no_await_dynamic = row_by_kind_path(
        &no_await_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(no_await_dynamic.status.status, CallStatusKind::Unsupported);
    assert_eq!(no_await_dynamic.status.resolution, None);
    assert!(
        no_await_dynamic.targets.is_empty(),
        "un-awaited returned async closure proof row must stay targetless: {no_await_dynamic:#?}"
    );
    let no_await_count =
        db.project_call_proof_facts_for_owner(no_await_owner, "bd:fixture-call-graph")?;
    assert_eq!(no_await_count, 7);
    assert_returned_callable_binding_evidence(
        &db,
        no_await_dynamic,
        "blocked",
        "un-awaited returned async closure",
    )?;
    assert_returned_path_poll_resume_blocker(
        &db,
        no_await_dynamic,
        "un-awaited returned async closure",
    )?;

    let mut expected = vec![OwnerProofEdge {
        owner: no_await_owner,
        site: no_await_path.site.id,
        span: no_await_path.site.span,
        target: maker,
    }];
    for (owner, label) in [
        (
            "call_awaited_returned_async_closure",
            "awaited returned async closure",
        ),
        (
            "call_stored_returned_async_closure",
            "stored returned async closure future",
        ),
    ] {
        expected.extend(project_polled_returned_async_proof(
            &db, owner, label, maker,
        )?);
    }

    assert_owner_proof_edges(
        &db,
        "returned async closure resolved calls",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_keeps_forwarded_returned_async_future_fail_closed() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2368-2373:
    // `make_forwarded_returned_async_future()` returns the future produced by
    // `make_returned_async_closure()()`, while
    // `call_forwarded_returned_async_future()` awaits only the producer
    // function result. Current call-graph proof does not carry future value
    // flow across that function boundary, so only the producer call traverses.
    let owner = function_id_by_name(&db, "call_forwarded_returned_async_future")?;
    let producer = function_id_by_name(&db, "make_forwarded_returned_async_future")?;
    let returned_maker = function_id_by_name(&db, "make_returned_async_closure")?;
    let local_target = function_id_by_name(&db, "local_target")?;

    let owner_context = db.call_context_for_owner(owner)?;
    assert_eq!(
        owner_context.len(),
        1,
        "forwarded returned async future caller should only expose the awaited producer call: {owner_context:#?}"
    );
    let producer_row = row_by_path(&owner_context, &["make_forwarded_returned_async_future"]);
    assert_resolved_target(
        producer_row,
        producer,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let awaited_owner_sites = db.awaited_call_sites_for_owner(owner)?;
    assert_eq!(
        awaited_owner_sites.len(),
        1,
        "forwarded returned async future caller should record exactly one awaited producer callsite: {awaited_owner_sites:#?}"
    );
    assert_eq!(awaited_owner_sites[0].id, producer_row.site.id);
    assert_eq!(awaited_owner_sites[0].kind, CallSiteKind::Path);
    let producer_path = path(&["make_forwarded_returned_async_future"]);
    assert_eq!(
        awaited_owner_sites[0].path.as_deref(),
        Some(producer_path.as_slice())
    );
    let owner_flows = db.returned_call_binding_flows_for_owner(owner)?;
    assert!(
        owner_flows.is_empty(),
        "awaiting a forwarded future producer must not create a returned-callable binding flow in the caller: {owner_flows:#?}"
    );

    let producer_context = db.call_context_for_owner(producer)?;
    assert_eq!(
        producer_context.len(),
        2,
        "producer should expose the returned async closure maker path plus targetless outer call: {producer_context:#?}"
    );
    let returned_maker_row = row_by_path(&producer_context, &["make_returned_async_closure"]);
    assert_resolved_target(
        returned_maker_row,
        returned_maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let dynamic = row_by_kind_path(
        &producer_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(dynamic.status.status, CallStatusKind::Unsupported);
    assert_eq!(dynamic.status.resolution, None);
    assert!(
        dynamic.targets.is_empty(),
        "non-local returned async future flow must not fabricate a closure edge: {dynamic:#?}"
    );
    assert!(
        relations_for_site(&db, dynamic.site.id)?.rows.is_empty(),
        "non-local returned async future flow must not persist a call edge"
    );
    assert_return_dynamic_binding(
        &db,
        producer,
        dynamic.site.id,
        "ReturnedPathCall",
        &["make_returned_async_closure"],
        "make_forwarded_returned_async_future",
    )?;
    let future_flows = db.returned_future_flows_for_owner(owner)?;
    assert_eq!(
        future_flows.len(),
        1,
        "awaited producer should expose one returned future proof flow without admitting a traversal edge: {future_flows:#?}"
    );
    let flow = &future_flows[0];
    assert_eq!(flow.caller_id, owner);
    assert_eq!(flow.producer.id, producer);
    assert_eq!(flow.producer.site_id, producer_row.site.id);
    assert_eq!(
        flow.producer.path,
        path(&["make_forwarded_returned_async_future"])
    );
    assert_eq!(flow.future.id, dynamic.site.id);
    assert_eq!(flow.future.path, path(&["make_returned_async_closure"]));
    assert_eq!(flow.future.callee_kind, "ReturnedPathCall");
    assert_eq!(flow.binding.source.id, dynamic.site.id);
    assert_eq!(
        flow.binding.source.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.binding.source.kind, "Dynamic");
    let returned_maker_binding =
        only_return_binding(&db, returned_maker, "make_returned_async_closure")?;
    assert_eq!(
        returned_maker_binding.source_kind, "AsyncClosure",
        "returned async closure maker should return an async closure owner"
    );
    let returned_async_closure = returned_maker_binding
        .source_id
        .expect("async closure maker return binding should point at the closure owner");

    let execution_flows = db.returned_future_execution_flows_for_owner(owner)?;
    assert_eq!(
        execution_flows.len(),
        1,
        "awaiting caller should expose one contextual returned-future execution proof flow: {execution_flows:#?}"
    );
    let execution = &execution_flows[0];
    assert_eq!(execution.caller_id, owner);
    assert_eq!(execution.producer.id, producer);
    assert_eq!(execution.producer.site_id, producer_row.site.id);
    assert_eq!(execution.producer_binding.id, flow.binding.id);
    assert_eq!(execution.producer_binding.source.id, dynamic.site.id);
    assert_eq!(
        execution.producer_binding.source.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(execution.producer_binding.source.kind, "Dynamic");
    assert_eq!(execution.future.id, dynamic.site.id);
    assert_eq!(
        execution.future.path,
        path(&["make_returned_async_closure"])
    );
    assert_eq!(execution.future.callee_kind, "ReturnedPathCall");
    assert_eq!(execution.maker.id, returned_maker);
    assert_eq!(execution.maker.site_id, returned_maker_row.site.id);
    assert_eq!(execution.maker.span, returned_maker_row.site.span);
    assert_eq!(execution.maker.path, path(&["make_returned_async_closure"]));
    assert_eq!(execution.callable_binding.id, returned_maker_binding.id);
    assert_eq!(execution.callable_binding.source.id, returned_async_closure);
    assert_eq!(
        execution.callable_binding.source.relation,
        LocalBindingRelationKind::BindingSourceClosure
    );
    assert_eq!(execution.callable_binding.source.kind, "Closure");
    assert_eq!(execution.body_edge.caller_id, returned_async_closure);
    assert_eq!(execution.body_edge.callee_id, local_target);
    assert_eq!(execution.body_edge.relation, CallRelationKind::Function);
    assert_eq!(execution.body_edge.source_kind, CallSiteKind::Path);
    assert_eq!(execution.body_edge.target_kind, CallTargetKind::Function);

    let awaited_producer_sites = db.awaited_call_sites_for_owner(producer)?;
    assert!(
        awaited_producer_sites.is_empty(),
        "the producer returns its async future without polling the inner returned callable: {awaited_producer_sites:#?}"
    );
    let producer_flows = db.returned_call_binding_flows_for_owner(producer)?;
    assert!(
        producer_flows.is_empty(),
        "targetless returned async future dynamic call must not expose a returned-call binding flow: {producer_flows:#?}"
    );
    let producer_future_flows = db.returned_future_flows_for_owner(producer)?;
    assert!(
        producer_future_flows.is_empty(),
        "returned future proof flow belongs to the awaiting caller, not to the producer that only returns the future: {producer_future_flows:#?}"
    );
    let producer_execution_flows = db.returned_future_execution_flows_for_owner(producer)?;
    assert!(
        producer_execution_flows.is_empty(),
        "contextual returned-future execution proof belongs to the awaiting caller, not the producer: {producer_execution_flows:#?}"
    );
    let count = db.project_call_proof_facts_for_owner(producer, "bd:fixture-call-graph")?;
    assert_eq!(count, 7, "forwarded returned async future proof facts");
    assert_returned_callable_binding_evidence(
        &db,
        dynamic,
        "blocked",
        "forwarded returned async future",
    )?;
    assert_returned_path_poll_resume_blocker(&db, dynamic, "forwarded returned async future")?;

    let producer_paths = db.call_paths_between(
        owner,
        producer,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        producer_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the future producer: {producer_paths:#?}"
    );
    assert_eq!(producer_paths[0].depth, 1);
    assert_eq!(producer_paths[0].edges[0].caller_id, owner);
    assert_eq!(producer_paths[0].edges[0].callee_id, producer);

    let local_target_paths = db.call_paths_between(
        owner,
        local_target,
        ploke_db::CallPathOptions {
            max_depth: 4,
            max_paths: 16,
        },
    )?;
    assert!(
        local_target_paths.is_empty(),
        "non-local returned async future flow should remain fail-closed until a typed future-flow carrier exists: {local_target_paths:#?}"
    );

    Ok(())
}

fn assert_returned_path_poll_resume_blocker(
    db: &ploke_db::Database,
    row: &CallContextRow,
    label: &str,
) -> Result<(), DbError> {
    let site = row.site.id.to_string();
    let blockers = db.proof_blockers()?;
    assert!(
        blockers.iter().any(|proof| {
            proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.reason == "dynamic_dispatch_unbounded"
                && proof.status == "blocked"
                && proof.detail.contains("returned future")
                && proof.detail.contains("async poll/resume proof")
        }),
        "{label} should expose a returned-future poll/resume proof blocker: {blockers:#?}"
    );

    let proof_rows = db.proof_graphrag_context("dynamic_dispatch_unbounded")?;
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "proof_blocker"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.blocker_reason.as_deref() == Some("dynamic_dispatch_unbounded")
        }),
        "{label} GraphRAG proof context should expose the returned-future blocker: {proof_rows:#?}"
    );

    Ok(())
}

fn assert_returned_callable_binding_evidence(
    db: &ploke_db::Database,
    row: &CallContextRow,
    state: &str,
    label: &str,
) -> Result<(), DbError> {
    let site = row.site.id.to_string();
    let owner = row.site.owner_id.to_string();
    let typed_rows = db.proof_binding_evidence_for_call_site(&site)?;
    assert!(
        typed_rows.iter().any(|proof| {
            proof.binding_evidence_kind == "returned_callable"
                && matches!(
                    proof.callee_kind.as_str(),
                    "ReturnedPathCall" | "AwaitedReturnedPathCall"
                )
                && proof.callee_path == row.site.path.clone().unwrap_or_default()
                && proof.call_site_id == site
                && proof.caller_def_id == owner
                && proof.resolution_state == state
                && proof.source_file.ends_with("fixture_call_graph/src/lib.rs")
                && proof.detail.contains("callable returned by")
        }),
        "{label} should expose typed returned-callable binding evidence for {site}: {typed_rows:#?}"
    );

    let proof_rows = db.proof_graphrag_context("returned_callable")?;
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "binding_evidence"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some(state)
                && proof
                    .source_file
                    .as_deref()
                    .is_some_and(|file| file.ends_with("fixture_call_graph/src/lib.rs"))
                && proof.detail.as_deref().is_some_and(|detail| {
                    detail.contains("callable returned by") && detail.contains("binding evidence")
                })
        }),
        "{label} should expose returned-callable binding evidence for {site}: {proof_rows:#?}"
    );

    Ok(())
}

fn project_polled_returned_async_proof(
    db: &ploke_db::Database,
    owner_name: &str,
    label: &str,
    maker: Uuid,
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "{label} proof context rows: {context:#?}");

    let path_row = row_by_path(&context, &["make_returned_async_closure"]);
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(dynamic_row.targets.len(), 1);
    let closure = dynamic_row.targets[0].target_id;
    assert_resolved_target(
        dynamic_row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 7, "{label} proof fact count");
    assert_returned_callable_binding_evidence(db, dynamic_row, "resolved", label)?;

    Ok(vec![
        OwnerProofEdge {
            owner,
            site: path_row.site.id,
            span: path_row.site.span,
            target: maker,
        },
        OwnerProofEdge {
            owner,
            site: dynamic_row.site.id,
            span: dynamic_row.site.span,
            target: closure,
        },
    ])
}

struct ReturnedClosureProofCase {
    owner: &'static str,
    maker: &'static str,
    path: &'static [&'static str],
}

fn project_returned_closure_proof(
    db: &ploke_db::Database,
    case: ReturnedClosureProofCase,
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let owner = function_id_by_name(db, case.owner)?;
    let maker = function_id_by_name(db, case.maker)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "{} returned closure proof context rows: {context:#?}",
        case.owner
    );

    let path_row = row_by_path(&context, case.path);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(
        dynamic_row.targets.len(),
        1,
        "{} returned closure dynamic row: {dynamic_row:#?}",
        case.owner
    );
    let closure = dynamic_row.targets[0].target_id;
    assert_resolved_target(
        dynamic_row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 7);
    assert_returned_callable_binding_evidence(db, dynamic_row, "resolved", case.owner)?;

    Ok(vec![
        OwnerProofEdge {
            owner,
            site: path_site,
            span: path_span,
            target: maker,
        },
        OwnerProofEdge {
            owner,
            site: dynamic_site,
            span: dynamic_span,
            target: closure,
        },
    ])
}

fn assert_return_closure_binding(
    db: &ploke_db::Database,
    owner: Uuid,
    closure: Uuid,
    label: &str,
) -> Result<(), DbError> {
    let binding = only_return_binding(db, owner, label)?;
    assert_eq!(binding.source_kind, "Closure", "{label} binding source");
    assert_eq!(
        binding.source_id,
        Some(closure),
        "{label} return binding should point at the returned closure owner"
    );
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert_return_binding_edges(db, owner, binding.id, closure, "Closure", label)?;
    Ok(())
}

fn assert_return_path_call_binding(
    db: &ploke_db::Database,
    owner: Uuid,
    call_site: Uuid,
    expected_path: &[&str],
    label: &str,
) -> Result<(), DbError> {
    let binding = only_return_binding(db, owner, label)?;
    assert_eq!(
        binding.source_kind, "PathCallResult",
        "{label} binding source"
    );
    assert_eq!(
        binding.source_id,
        Some(call_site),
        "{label} return binding should point at the forwarded path-call site"
    );
    assert_eq!(binding.source_call_kind.as_deref(), Some("Path"));
    assert_eq!(binding.source_path.as_ref(), Some(&path(expected_path)));
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert_return_binding_edges(db, owner, binding.id, call_site, "Path", label)?;
    Ok(())
}

fn assert_return_dynamic_binding(
    db: &ploke_db::Database,
    owner: Uuid,
    call_site: Uuid,
    callee_kind: &str,
    expected_path: &[&str],
    label: &str,
) -> Result<(), DbError> {
    let binding = only_return_binding(db, owner, label)?;
    assert_eq!(
        binding.source_kind, "DynamicCallResult",
        "{label} binding source"
    );
    assert_eq!(
        binding.source_id,
        Some(call_site),
        "{label} return binding should point at the dynamic call site"
    );
    assert_eq!(binding.source_call_kind.as_deref(), Some("Dynamic"));
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind.as_deref(), Some(callee_kind));
    assert_eq!(binding.callee_path.as_ref(), Some(&path(expected_path)));
    assert_return_binding_edges(db, owner, binding.id, call_site, "Dynamic", label)?;
    Ok(())
}

fn assert_return_binding_edges(
    db: &ploke_db::Database,
    owner: Uuid,
    binding: Uuid,
    source: Uuid,
    source_kind: &str,
    label: &str,
) -> Result<(), DbError> {
    let edges = db.local_binding_edges_for_owner(owner)?;
    assert_eq!(
        edges.len(),
        2,
        "{label} should expose owner->binding and binding->source edges: {edges:#?}"
    );

    let owner_edge = edges
        .iter()
        .find(|edge| edge.relation == LocalBindingRelationKind::OwnerContainsBinding)
        .unwrap_or_else(|| panic!("{label} missing OwnerContainsBinding edge: {edges:#?}"));
    assert_eq!(owner_edge.source_id, owner, "{label} owner edge source");
    assert_eq!(
        owner_edge.target_id, binding,
        "{label} owner edge target should be the return binding"
    );
    assert_eq!(
        owner_edge.target_kind, "LocalBinding",
        "{label} owner edge target kind"
    );

    let source_edge = edges
        .iter()
        .find(|edge| edge.relation != LocalBindingRelationKind::OwnerContainsBinding)
        .unwrap_or_else(|| panic!("{label} missing binding source edge: {edges:#?}"));
    assert_eq!(
        source_edge.source_id, binding,
        "{label} source edge should start at the return binding"
    );
    assert_eq!(
        source_edge.target_id, source,
        "{label} source edge should target the binding source"
    );
    assert_eq!(
        source_edge.source_kind, "LocalBinding",
        "{label} source edge source kind"
    );
    assert_eq!(
        source_edge.target_kind, source_kind,
        "{label} source edge target kind"
    );
    Ok(())
}

fn only_return_binding(
    db: &ploke_db::Database,
    owner: Uuid,
    label: &str,
) -> Result<ploke_db::LocalBindingRow, DbError> {
    let bindings = db.local_bindings_for_owner(owner)?;
    assert_eq!(
        bindings.len(),
        1,
        "{label} should expose exactly one local return binding: {bindings:#?}"
    );
    let binding = bindings.into_iter().next().expect("binding length checked");
    assert_eq!(binding.kind, "ReturnExpression", "{label} binding kind");
    assert_eq!(binding.name, "return", "{label} binding name");
    assert!(
        binding.span.0 < binding.span.1,
        "{label} return binding should retain a non-empty source span: {binding:#?}"
    );
    Ok(binding)
}
