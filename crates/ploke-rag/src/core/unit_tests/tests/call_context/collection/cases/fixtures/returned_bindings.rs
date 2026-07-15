use super::super::super::super::super::*;
use super::super::super::helpers::*;
use ploke_core::rag_types::{
    CallCalleeInfo, CallEndpointKind, CallSiteKind, CallTargetKind, LocalBindingRelationKind,
    ReturnedCallSourceKind,
};

#[tokio::test]
async fn returned_call_binding_flows_exact_expose_forwarded_closure_proof() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
    // `call_forwarded_returned_closure()` invokes a sync closure returned
    // through `make_forwarded_returned_closure()`. The DB helper proves the
    // dynamic call through the producer's persisted return binding instead of
    // relying only on the admitted call edge.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_forwarded_returned_closure"),
    )?;
    let producer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_forwarded_returned_closure"),
    )?;
    let maker = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_target_closure"),
    )?;
    let flows = rag
        .exact_returned_call_binding_flows_for_owner(owner)?
        .expect("call context is enabled");
    assert_eq!(
        flows.len(),
        1,
        "RAG should expose one returned-call binding flow for forwarded sync closure: {flows:#?}"
    );
    let flow = &flows[0];
    let sync_path = vec!["make_forwarded_returned_closure".to_string()];
    assert_eq!(flow.caller_id, owner);
    assert_eq!(flow.dynamic.path, sync_path);
    assert_eq!(flow.dynamic.relation, CallTargetKind::DynamicClosure);
    assert_eq!(flow.dynamic.target_kind, CallEndpointKind::Closure);
    assert_eq!(flow.producer.id, producer);
    assert_eq!(flow.producer.path, flow.dynamic.path);
    assert_eq!(
        flow.binding.source.relation,
        LocalBindingRelationKind::BindingSourceCallResult
    );
    assert_eq!(flow.binding.source.kind, ReturnedCallSourceKind::Path);

    let db_flows = db.returned_call_binding_flows_for_owner(owner)?;
    assert_eq!(db_flows.len(), 1);
    assert_eq!(
        flow.binding.source.id, db_flows[0].binding.source.id,
        "RAG should preserve the exact producer return-binding source callsite"
    );
    assert_eq!(
        db_flows[0].producer.id, producer,
        "DB producer endpoint should be the forwarded closure producer"
    );
    assert_ne!(
        db_flows[0].binding.source.id, db_flows[0].producer.site_id,
        "producer return binding source should be the inner maker callsite, not the caller's producer callsite"
    );
    assert_ne!(
        db_flows[0].binding.source.id, maker,
        "binding source is a callsite ID, not the maker function ID"
    );

    let future_producer = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_forwarded_returned_async_future"),
    )?;
    let future_maker = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_returned_async_closure"),
    )?;
    let local_target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;

    for (owner_name, label) in [
        (
            "call_forwarded_returned_async_future",
            "direct forwarded async future",
        ),
        (
            "call_stored_forwarded_returned_async_future_tuple_field",
            "stored forwarded async future",
        ),
    ] {
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:2397-2407:
        // The direct caller awaits `make_forwarded_returned_async_future()`;
        // the stored caller awaits the same producer after storing it in
        // `futures.0`. Both expose returned-future proof rows without
        // promoting returned-call binding flow or ordinary traversal to the
        // async closure body.
        let future_owner = one_uuid(&db, &function_in_module_query(&["crate"], owner_name))?;
        let awaited_sites = rag
            .exact_awaited_call_sites_for_owner(future_owner)?
            .expect("call context is enabled");
        assert_eq!(
            awaited_sites.len(),
            1,
            "RAG should expose the awaited producer source-site proof for {label}: {awaited_sites:#?}"
        );
        let awaited = &awaited_sites[0];
        assert_eq!(awaited.owner_id, future_owner);
        assert_eq!(awaited.kind, CallSiteKind::Path);
        assert_eq!(
            awaited.callee,
            CallCalleeInfo::Path {
                path: vec!["make_forwarded_returned_async_future".to_string()]
            }
        );
        let awaited_path = vec!["make_forwarded_returned_async_future".to_string()];
        assert_eq!(awaited.path.as_deref(), Some(awaited_path.as_slice()));
        let owner_flows = rag
            .exact_returned_call_binding_flows_for_owner(future_owner)?
            .expect("call context is enabled");
        assert!(
            owner_flows.is_empty(),
            "awaiting the producer result must not expose a returned-call flow without future proof for {label}: {owner_flows:#?}"
        );
        let future_flows = rag
            .exact_returned_future_flows_for_owner(future_owner)?
            .expect("call context is enabled");
        assert_eq!(
            future_flows.len(),
            1,
            "RAG should expose the returned future proof flow without promoting a returned-call binding flow for {label}: {future_flows:#?}"
        );
        let future_flow = &future_flows[0];
        assert_eq!(future_flow.caller_id, future_owner);
        assert_eq!(future_flow.producer.id, future_producer);
        assert_eq!(future_flow.producer.path, awaited_path);
        assert_eq!(
            future_flow.future.path,
            vec!["make_returned_async_closure".to_string()]
        );
        assert_eq!(future_flow.future.callee_kind, "ReturnedPathCall");
        assert_eq!(
            future_flow.binding.source.relation,
            LocalBindingRelationKind::BindingSourceCallResult
        );
        assert_eq!(
            future_flow.binding.source.kind,
            ReturnedCallSourceKind::Dynamic
        );
        let execution_flows = rag
            .exact_returned_future_execution_flows_for_owner(future_owner)?
            .expect("call context is enabled");
        assert_eq!(
            execution_flows.len(),
            1,
            "RAG should expose the contextual returned future execution proof without adding a call-path edge for {label}: {execution_flows:#?}"
        );
        let execution = &execution_flows[0];
        assert_eq!(execution.caller_id, future_owner);
        assert_eq!(execution.producer.id, future_producer);
        assert_eq!(execution.producer.path, awaited_path);
        assert_eq!(
            execution.producer_binding.source.relation,
            LocalBindingRelationKind::BindingSourceCallResult
        );
        assert_eq!(
            execution.producer_binding.source.kind,
            ReturnedCallSourceKind::Dynamic
        );
        assert_eq!(
            execution.future.path,
            vec!["make_returned_async_closure".to_string()]
        );
        assert_eq!(execution.future.callee_kind, "ReturnedPathCall");
        assert_eq!(execution.maker.id, future_maker);
        assert_eq!(
            execution.maker.path,
            vec!["make_returned_async_closure".to_string()]
        );
        assert_eq!(
            execution.callable_binding.source.relation,
            LocalBindingRelationKind::BindingSourceClosure
        );
        assert_eq!(
            execution.callable_binding.source.kind,
            ReturnedCallSourceKind::Closure
        );
        assert_eq!(execution.body_edge.callee_id, local_target);
        assert_eq!(execution.body_edge.relation, CallTargetKind::Function);
        assert_eq!(execution.body_edge.source_kind, CallSiteKind::Path);
        assert_eq!(execution.body_edge.target_kind, CallEndpointKind::Function);
    }

    let producer_flows = rag
        .exact_returned_call_binding_flows_for_owner(future_producer)?
        .expect("call context is enabled");
    assert!(
        producer_flows.is_empty(),
        "targetless returned async future producer must remain absent from exact RAG binding flows: {producer_flows:#?}"
    );
    let producer_future_flows = rag
        .exact_returned_future_flows_for_owner(future_producer)?
        .expect("call context is enabled");
    assert!(
        producer_future_flows.is_empty(),
        "returned future proof flow belongs to the awaiting caller, not the producer: {producer_future_flows:#?}"
    );
    let producer_execution_flows = rag
        .exact_returned_future_execution_flows_for_owner(future_producer)?
        .expect("call context is enabled");
    assert!(
        producer_execution_flows.is_empty(),
        "contextual returned future execution proof belongs to the awaiting caller, not the producer: {producer_execution_flows:#?}"
    );
    let producer_awaited_sites = rag
        .exact_awaited_call_sites_for_owner(future_producer)?
        .expect("call context is enabled");
    assert!(
        producer_awaited_sites.is_empty(),
        "producer returns its async future without awaiting the inner returned callable: {producer_awaited_sites:#?}"
    );

    Ok(())
}
