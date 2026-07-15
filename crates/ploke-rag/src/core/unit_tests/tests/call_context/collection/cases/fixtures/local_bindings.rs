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

#[tokio::test]
async fn local_bindings_exact_expose_aliased_parameter_field_evidence() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2410-2419:
    // The helper aliases `holder` before calling `(alias.callback)()`. Exact
    // RAG should expose the durable alias edge that explains the admitted
    // private parameter-field proof.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_aliased_named_field_function_param"),
    )?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let holder = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "holder"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("RAG should expose `holder` parameter: {bindings:#?}"));
    let alias = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "alias"
                && binding.source_kind == "ValueAlias"
                && matches!(
                    binding.source_path.as_deref(),
                    Some([segment]) if segment == "holder"
                )
        })
        .unwrap_or_else(|| panic!("RAG should expose `alias = holder`: {bindings:#?}"));

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == holder.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-holder containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == owner
                && edge.target_id == alias.id
                && edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose owner-to-alias containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == alias.id
                && edge.target_id == holder.id
                && edge.relation == LocalBindingRelationKind::BindingAliasesBinding
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        }),
        "RAG should expose alias-to-holder proof: {edges:#?}"
    );

    Ok(())
}
