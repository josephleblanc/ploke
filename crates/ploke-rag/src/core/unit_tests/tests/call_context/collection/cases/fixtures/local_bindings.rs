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
async fn local_bindings_exact_expose_local_function_binding_evidence() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1447-1453:
    // `local_fn_body_call_is_not_outer_call_site` declares block-local
    // `fn inner()` before calling `inner()`. Exact RAG should expose the
    // durable local-binding proof that connects the name `inner` to the
    // executable LocalItem owner.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "local_fn_body_call_is_not_outer_call_site"),
    )?;
    let local_item = local_item_owner_for_parent_with_label(&db, owner, "local_fn:inner")?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let binding = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LocalFunctionBinding"
                && binding.name == "inner"
                && binding.source_kind == "LocalFunction"
                && binding.source_id == Some(local_item)
                && binding.source_call_kind.is_none()
                && binding.source_path.is_none()
                && binding.callee_kind.is_none()
                && binding.callee_path.is_none()
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose local function binding `inner`: {bindings:#?}")
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
        "RAG should expose owner-to-local-function binding containment: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.source_id == binding.id
                && edge.target_id == local_item
                && edge.relation == LocalBindingRelationKind::BindingSourceLocalItem
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalItem"
        }),
        "RAG should expose local-function binding-to-local-item proof: {edges:#?}"
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

#[tokio::test]
async fn local_bindings_exact_expose_direct_aliased_parameter_source_evidence() -> Result<(), Error>
{
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1685-1691:
    // `call_single_aliased_function_pointer_param(f) { let g = f; g() }`
    // has one local caller that supplies `local_target`. Exact RAG should
    // expose both the alias carrier and the derived source-function proof for
    // the parameter and its alias.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_aliased_function_pointer_param"),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "f"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("RAG should expose parameter `f`: {bindings:#?}"));
    let alias = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "LetBinding"
                && binding.name == "g"
                && binding.source_kind == "ValueAlias"
                && matches!(binding.source_path.as_deref(), Some([segment]) if segment == "f")
        })
        .unwrap_or_else(|| panic!("RAG should expose alias `g = f`: {bindings:#?}"));

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    let has_edge = |source_id, target_id, relation, source_kind: Option<&str>, target_kind| {
        edges.iter().any(|edge| {
            edge.source_id == source_id
                && edge.target_id == target_id
                && edge.relation == relation
                && source_kind.is_none_or(|kind| edge.source_kind == kind)
                && edge.target_kind == target_kind
        })
    };

    assert!(
        has_edge(
            owner,
            parameter.id,
            LocalBindingRelationKind::OwnerContainsBinding,
            None,
            "LocalBinding",
        ),
        "RAG should expose owner-to-parameter containment: {edges:#?}"
    );
    assert!(
        has_edge(
            owner,
            alias.id,
            LocalBindingRelationKind::OwnerContainsBinding,
            None,
            "LocalBinding",
        ),
        "RAG should expose owner-to-alias containment: {edges:#?}"
    );
    assert!(
        has_edge(
            alias.id,
            parameter.id,
            LocalBindingRelationKind::BindingAliasesBinding,
            Some("LocalBinding"),
            "LocalBinding",
        ),
        "RAG should expose alias-to-parameter proof: {edges:#?}"
    );
    assert!(
        has_edge(
            parameter.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            Some("LocalBinding"),
            "Function",
        ),
        "RAG should expose parameter-to-function source proof: {edges:#?}"
    );
    assert!(
        has_edge(
            alias.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            Some("LocalBinding"),
            "Function",
        ),
        "RAG should expose alias-to-function source proof: {edges:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn local_bindings_exact_expose_method_argument_parameter_evidence() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2422-2429:
    // private `LocalAssoc::call_function_pointer_param(&self, f)` calls
    // `f()`, while `call_method_function_pointer_param_with_local_target()`
    // supplies `local_target` through a resolved method call.
    let owner = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "call_function_pointer_param"),
    )?;
    let caller = one_uuid(
        &db,
        &function_in_module_query(
            &["crate"],
            "call_method_function_pointer_param_with_local_target",
        ),
    )?;
    let target = one_uuid(&db, &function_in_module_query(&["crate"], "local_target"))?;
    let receiver = ploke_db::CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: vec!["LocalAssoc".to_string()],
    };
    let caller_context = db.call_context_for_owner(caller)?;
    let method_call = caller_context
        .iter()
        .find(|row| {
            row.site.kind == ploke_db::CallSiteKind::Method
                && row.site.method.as_deref() == Some("call_function_pointer_param")
                && row.site.receiver.as_ref() == Some(&receiver)
        })
        .unwrap_or_else(|| {
            panic!("expected resolved method callsite in caller context: {caller_context:#?}")
        });

    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "f"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| panic!("RAG should expose method parameter `f`: {bindings:#?}"));

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    let has_edge = |source_id, target_id, relation, source_kind: Option<&str>, target_kind| {
        edges.iter().any(|edge| {
            edge.source_id == source_id
                && edge.target_id == target_id
                && edge.relation == relation
                && source_kind.is_none_or(|kind| edge.source_kind == kind)
                && edge.target_kind == target_kind
        })
    };

    assert!(
        has_edge(
            owner,
            parameter.id,
            LocalBindingRelationKind::OwnerContainsBinding,
            None,
            "LocalBinding",
        ),
        "RAG should expose owner-to-method-parameter containment: {edges:#?}"
    );
    assert!(
        has_edge(
            method_call.site.id,
            parameter.id,
            LocalBindingRelationKind::ArgumentSuppliesParameter,
            Some("Method"),
            "LocalBinding",
        ),
        "RAG should expose method-call argument to method-parameter proof: {edges:#?}"
    );
    assert!(
        has_edge(
            parameter.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            Some("LocalBinding"),
            "Function",
        ),
        "RAG should expose method parameter-to-function source proof: {edges:#?}"
    );

    Ok(())
}

#[tokio::test]
async fn local_bindings_exact_expose_result_callback_source() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let rag = init_test_rag_mock(Arc::clone(&db));

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2301-2310:
    // private `call_single_result_callback(f)` calls
    // `Ok::<i32, ()>(1).and_then(f)`, and its only local caller supplies
    // `local_result_target`. Exact RAG should expose the durable parameter
    // binding proof behind that resolved callback edge.
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_result_callback"),
    )?;
    let caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_result_callback_with_local_target"),
    )?;
    let target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "local_result_target"),
    )?;
    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = caller_context
        .iter()
        .find(|row| {
            row.site.kind == ploke_db::CallSiteKind::Path
                && row.site.path.as_deref().is_some_and(|path| {
                    path.iter()
                        .map(String::as_str)
                        .eq(["call_single_result_callback"])
                })
        })
        .unwrap_or_else(|| {
            panic!("expected caller path callsite supplying callback: {caller_context:#?}")
        });

    let bindings = rag
        .exact_local_bindings_for_owner(owner)?
        .expect("call context is enabled");
    let parameter = bindings
        .iter()
        .find(|binding| {
            binding.owner_id == owner
                && binding.kind == "ParameterBinding"
                && binding.name == "f"
                && binding.source_kind == "Parameter"
        })
        .unwrap_or_else(|| {
            panic!("RAG should expose result callback parameter `f`: {bindings:#?}")
        });

    let edges = rag
        .exact_local_binding_edges_for_owner(owner)?
        .expect("call context is enabled");
    let has_edge = |source_id, target_id, relation, source_kind: Option<&str>, target_kind| {
        edges.iter().any(|edge| {
            edge.source_id == source_id
                && edge.target_id == target_id
                && edge.relation == relation
                && source_kind.is_none_or(|kind| edge.source_kind == kind)
                && edge.target_kind == target_kind
        })
    };

    assert!(
        has_edge(
            owner,
            parameter.id,
            LocalBindingRelationKind::OwnerContainsBinding,
            None,
            "LocalBinding",
        ),
        "RAG should expose result callback parameter containment: {edges:#?}"
    );
    assert!(
        has_edge(
            helper_call.site.id,
            parameter.id,
            LocalBindingRelationKind::ArgumentSuppliesParameter,
            Some("Path"),
            "LocalBinding",
        ),
        "RAG should expose caller path argument to callback parameter proof: {edges:#?}"
    );
    assert!(
        has_edge(
            parameter.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            Some("LocalBinding"),
            "Function",
        ),
        "RAG should expose callback parameter-to-function source proof: {edges:#?}"
    );

    Ok(())
}
