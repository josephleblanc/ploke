use super::super::super::super::super::*;
use super::super::super::helpers::*;
use ploke_core::rag_types::LocalBindingRelationKind;

#[tokio::test]
async fn local_bindings_exact_expose_initialized_path_evidence() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:185-187:
    // `let f = local_target; f()` already resolves as a function
    // edge. Exact RAG should also expose the durable local-binding proof rows
    // that explain the callable value source.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_local_function_item_binding"),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "f"
                && binding.source_kind == "InitializedPath"
                && matches!(
                    binding.source_path.as_deref(),
                    Some([segment]) if segment == "local_target"
                )
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose initialized local binding `f`: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == binding.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == binding.id
                && edge.target_id == target
                && edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "Function"
        }),
        "RAG should expose binding-to-function source proof: {edges:#?}"
    );

    Ok(())
}
