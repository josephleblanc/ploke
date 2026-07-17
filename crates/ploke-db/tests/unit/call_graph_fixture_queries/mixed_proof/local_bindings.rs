use super::*;
use ploke_db::LocalBindingRelationKind;

#[test]
fn fixture_projection_stores_parameter_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_function_pointer_param")?;
    let caller = function_id_by_name(&db, "call_single_function_pointer_param_with_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1523-1528:
    // `call_single_function_pointer_param(f: fn() -> i32) { f() }` is resolved
    // from the private caller that supplies `local_target`; this test proves the
    // callee parameter itself is also queryable as a durable local binding, and
    // the caller's helper-call site is linked to the parameter it supplies.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["f"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_function_pointer_param"]);
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "single function-pointer helper should expose one parameter binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "Parameter");
    assert_eq!(binding.source_id, None);
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "parameter binding should retain the parameter pattern span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        3,
        "parameter binding should expose containment, caller argument, and source-function edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-parameter-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::ArgumentSuppliesParameter
            && edge.source_id == helper_call.site.id
            && edge.source_kind == "Path"
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing caller call-site to parameter-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceFunction
            && edge.source_id == binding.id
            && edge.source_kind == "LocalBinding"
            && edge.target_id == target
            && edge.target_kind == "Function"),
        "missing parameter-binding to source-function edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_targetless_callable_callee_evidence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    struct Case {
        owner: &'static str,
        kind: CallSiteKind,
        path: &'static [&'static str],
        callee: &'static str,
        source: &'static str,
    }

    let cases = [
        Case {
            owner: "call_function_pointer_param",
            kind: CallSiteKind::Path,
            path: &["f"],
            callee: "ValueBinding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:683-685 `f()`",
        },
        Case {
            owner: "call_parenthesized_function_pointer_param",
            kind: CallSiteKind::Dynamic,
            path: &["f"],
            callee: "LocalBinding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:687-689 `(f)()`",
        },
        Case {
            owner: "call_function_pointer_param_cast",
            kind: CallSiteKind::Dynamic,
            path: &["f"],
            callee: "FnPointerCastLocalBinding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:691-693 `(f as fn() -> i32)()`",
        },
        Case {
            owner: "call_field_function_param",
            kind: CallSiteKind::Dynamic,
            path: &["holder", "callback"],
            callee: "FieldLocalBinding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:709-711 `(holder.callback)()`",
        },
        Case {
            owner: "call_if_function_pointer_param_branch",
            kind: CallSiteKind::Dynamic,
            path: &["f"],
            callee: "IfBranchParameter",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:721-723 `(if flag { f } else { f })()`",
        },
        Case {
            owner: "call_match_function_pointer_param_arm",
            kind: CallSiteKind::Dynamic,
            path: &["f"],
            callee: "MatchArmParameter",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:725-729 `match` arms both yield `f`",
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, case.kind, case.path);

        assert_eq!(
            row.status.status,
            CallStatusKind::Unsupported,
            "{} remains targetless until caller binding proof identifies the concrete callable: {row:#?}",
            case.source
        );
        assert!(
            row.targets.is_empty(),
            "{} must not fabricate a resolved call edge from callee evidence alone: {row:#?}",
            case.source
        );

        let evidence = db.call_callee_evidence_for_owner(owner)?;
        assert_eq!(
            evidence.len(),
            1,
            "{} should expose one callee evidence row for the owner: {evidence:#?}",
            case.source
        );
        let evidence = &evidence[0];
        assert_eq!(evidence.site_id, row.site.id, "{}", case.source);
        assert_eq!(evidence.site_kind, case.kind, "{}", case.source);
        assert_eq!(evidence.closure_id, None, "{}", case.source);
        assert_eq!(
            evidence.callee_kind, case.callee,
            "{} should persist parser callee classification",
            case.source
        );
        assert_eq!(
            evidence.callee_path,
            path(case.path),
            "{} should persist the structural callee path",
            case.source
        );

        assert!(
            relations_for_site(&db, row.site.id)?.rows.is_empty(),
            "{} should remain evidence-only with no call_relation row",
            case.source
        );
        assert_eq!(
            db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?
                .len(),
            2,
            "{} should still project call_site and call_resolution proof facts",
            case.source
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_named_field_projection_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;

    struct ProjectionCase {
        owner: &'static str,
        source: &'static str,
        call_path: &'static [&'static str],
        base_name: &'static str,
        constructed_path: &'static [&'static str],
        projection_name: &'static str,
        projection_path: &'static [&'static str],
    }

    let cases = [
        ProjectionCase {
            owner: "call_named_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:870-874 `holder.callback`",
            call_path: &["holder", "callback"],
            base_name: "holder",
            constructed_path: &["NamedCallbackHolder"],
            projection_name: "holder.callback",
            projection_path: &["callback"],
        },
        ProjectionCase {
            owner: "call_aliased_named_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:877-882 `let alias = holder; (alias.callback)()`",
            call_path: &["alias", "callback"],
            base_name: "alias",
            constructed_path: &["NamedCallbackHolder"],
            projection_name: "alias.callback",
            projection_path: &["callback"],
        },
        ProjectionCase {
            owner: "call_indexed_named_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:889-893 `holder.callbacks[0]`",
            call_path: &["holder", "callbacks", "0"],
            base_name: "holder",
            constructed_path: &["CallbackArrayHolder"],
            projection_name: "holder.callbacks.0",
            projection_path: &["callbacks", "0"],
        },
        ProjectionCase {
            owner: "call_aliased_indexed_named_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:902-907 `let alias = holder; alias.callbacks[0]()`",
            call_path: &["alias", "callbacks", "0"],
            base_name: "alias",
            constructed_path: &["CallbackArrayHolder"],
            projection_name: "alias.callbacks.0",
            projection_path: &["callbacks", "0"],
        },
        ProjectionCase {
            owner: "call_indexed_tuple_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:914-916 `holder.0[0]`",
            call_path: &["holder", "0", "0"],
            base_name: "holder",
            constructed_path: &["TupleCallbackArrayHolder"],
            projection_name: "holder.0.0",
            projection_path: &["0", "0"],
        },
        ProjectionCase {
            owner: "call_aliased_indexed_tuple_field_function_binding",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:925-929 `let alias = holder; alias.0[0]()`",
            call_path: &["alias", "0", "0"],
            base_name: "alias",
            constructed_path: &["TupleCallbackArrayHolder"],
            projection_name: "alias.0.0",
            projection_path: &["0", "0"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;

        // Source oracle: each case constructs a local holder with exact
        // `local_target` field or element evidence, then calls through that
        // projected callable before the dynamic call resolves to `local_target`.
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.call_path);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );

        let bindings = db.local_bindings_for_owner(owner)?;
        let holder = bindings
            .iter()
            .find(|binding| binding.kind == "LetBinding" && binding.name == case.base_name)
            .unwrap_or_else(|| {
                panic!(
                    "{} constructed holder binding should be persisted: {}",
                    case.owner, case.source
                )
            });
        assert_eq!(holder.source_kind, "Constructed");
        assert_eq!(holder.source_id, None);
        assert_eq!(holder.source_call_kind, None);
        assert_eq!(
            holder.source_path.as_ref(),
            Some(&path(case.constructed_path))
        );
        assert_eq!(holder.callee_kind, None);
        assert_eq!(holder.callee_path, None);

        let projection = bindings
            .iter()
            .find(|binding| {
                binding.kind == "FieldProjection" && binding.name == case.projection_name
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} projection binding should be persisted: {}",
                    case.projection_name, case.source
                )
            });
        assert_eq!(projection.source_kind, "FieldProjection");
        assert_eq!(projection.source_id, Some(holder.id));
        assert_eq!(projection.source_call_kind, None);
        assert_eq!(
            projection.source_path.as_ref(),
            Some(&path(case.projection_path))
        );
        assert_eq!(projection.callee_kind.as_deref(), Some("Path"));
        assert_eq!(
            projection.callee_path.as_ref(),
            Some(&path(&["local_target"]))
        );

        let edges = db.local_binding_edges_for_owner(owner)?;
        let holder_edges = edges
            .iter()
            .filter(|edge| edge.source_id == holder.id || edge.target_id == holder.id)
            .collect::<Vec<_>>();
        assert_eq!(
            holder_edges.len(),
            2,
            "{} constructed holder should expose owner and projection edges: {edges:#?}",
            case.owner
        );
        assert!(
            holder_edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::OwnerContainsBinding
                && edge.source_id == owner
                && edge.target_id == holder.id
                && edge.target_kind == "LocalBinding"),
            "missing owner-to-holder binding edge for {}: {holder_edges:#?}",
            case.owner
        );
        assert!(
            holder_edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::BindingProjectsField
                && edge.source_id == projection.id
                && edge.target_id == holder.id
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"),
            "missing projection-to-holder field edge for {}: {holder_edges:#?}",
            case.owner
        );

        let projection_edges = edges
            .iter()
            .filter(|edge| edge.source_id == projection.id || edge.target_id == projection.id)
            .collect::<Vec<_>>();
        assert_eq!(
            projection_edges.len(),
            3,
            "{} projection should expose owner, base-binding, and source-function edges: {edges:#?}",
            case.projection_name
        );
        assert!(
            projection_edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::OwnerContainsBinding
                && edge.source_id == owner
                && edge.target_id == projection.id),
            "missing owner-to-projection binding edge for {}: {projection_edges:#?}",
            case.projection_name
        );
        assert!(
            projection_edges.iter().any(|edge| edge.relation
                == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_id == projection.id
                && edge.source_kind == "LocalBinding"
                && edge.target_id == target
                && edge.target_kind == "Function"),
            "missing projection-to-function source edge for {}: {projection_edges:#?}",
            case.projection_name
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_keeps_unproven_field_parameter_without_projection_edge() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    struct UnprovenCase {
        owner: &'static str,
        source: &'static str,
        call_path: &'static [&'static str],
    }

    let cases = [
        UnprovenCase {
            owner: "call_field_function_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:709-710 `(holder.callback)()`",
            call_path: &["holder", "callback"],
        },
        UnprovenCase {
            owner: "call_indexed_field_function_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:885-886 `holder.callbacks[0]()`",
            call_path: &["holder", "callbacks", "0"],
        },
        UnprovenCase {
            owner: "call_indexed_tuple_field_function_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:910-911 `holder.0[0]()`",
            call_path: &["holder", "0", "0"],
        },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;

        // Source oracle: these public parameter field/indexed calls have no
        // local constructed binding proving which callable value reaches the
        // projected slot, so the durable binding carrier must stay fail-closed.
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.call_path);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert!(
            row.targets.is_empty(),
            "unproven parameter projection call must stay targetless for {}: {row:#?}",
            case.source
        );

        let bindings = db.local_bindings_for_owner(owner)?;
        assert!(
            bindings
                .iter()
                .all(|binding| binding.kind != "FieldProjection"),
            "unproven parameter projection must not emit a projection binding for {}: {bindings:#?}",
            case.source
        );
        assert!(
            bindings
                .iter()
                .any(|binding| binding.kind == "ParameterBinding" && binding.name == "holder"),
            "the parameter itself should still be visible as a durable binding for {}: {bindings:#?}",
            case.source
        );

        let edges = db.local_binding_edges_for_owner(owner)?;
        assert!(
            edges
                .iter()
                .all(|edge| edge.relation != LocalBindingRelationKind::BindingProjectsField),
            "unproven parameter projection must not emit projection edges for {}: {edges:#?}",
            case.source
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_constructed_field_argument_parameter_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_named_field_function_param")?;
    let caller = function_id_by_name(
        &db,
        "call_single_named_field_function_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1610-1616:
    // `call_single_named_field_function_param(holder) { (holder.callback)() }`
    // is private and has one local caller that supplies
    // `CallbackHolder { callback: local_target }`. The dynamic field call is
    // already resolved; this test pins the durable argument-to-parameter proof
    // row that explains which callsite supplies the `holder` parameter.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["holder", "callback"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_named_field_function_param"]);
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let holder = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "holder")
        .expect("holder parameter binding should be persisted");
    assert_eq!(holder.source_kind, "Parameter");

    let edges = db.local_binding_edges_for_owner(owner)?;
    let holder_edges = edges
        .iter()
        .filter(|edge| edge.source_id == holder.id || edge.target_id == holder.id)
        .collect::<Vec<_>>();
    assert!(
        holder_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == holder.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-holder parameter edge: {holder_edges:#?}"
    );
    assert!(
        holder_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::ArgumentSuppliesParameter
            && edge.source_id == helper_call.site.id
            && edge.source_kind == "Path"
            && edge.target_id == holder.id
            && edge.target_kind == "LocalBinding"),
        "missing constructed argument-to-holder parameter edge: {holder_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_initialized_path_let_binding() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_local_function_item_binding")?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:185-187:
    // `let f = local_target; f()` proves a callable value binding sourced by a
    // local item path. The call already resolves through that binding; this
    // test pins the durable binding row and source-function edge that explain
    // the value flow.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["f"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LetBinding" && binding.name == "f")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "initialized callable binding should expose one let binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "InitializedPath");
    assert_eq!(binding.source_id, None);
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "initialized callable binding should retain a non-empty source span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "initialized callable binding should expose owner containment plus source function edge: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-initialized-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceFunction
            && edge.source_id == binding.id
            && edge.source_kind == "LocalBinding"
            && edge.target_id == target
            && edge.target_kind == "Function"),
        "missing initialized-binding to function-source edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_value_alias_binding_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_aliased_function_pointer_param")?;
    let caller = function_id_by_name(
        &db,
        "call_single_aliased_function_pointer_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1685-1687:
    // `let g = f; g()` is already resolved through private caller proof. This
    // test pins the durable alias carrier: `g` is a let binding sourced by the
    // callee-owned parameter binding `f`.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["g"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(
        &caller_context,
        &["call_single_aliased_function_pointer_param"],
    );
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let parameter = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
        .expect("parameter binding f should be persisted");
    assert_eq!(parameter.source_kind, "Parameter");

    let alias = bindings
        .iter()
        .find(|binding| binding.kind == "LetBinding" && binding.name == "g")
        .expect("alias binding g should be persisted");
    assert_eq!(alias.source_kind, "ValueAlias");
    assert_eq!(alias.source_id, None);
    assert_eq!(alias.source_call_kind, None);
    assert_eq!(alias.source_path.as_ref(), Some(&path(&["f"])));
    assert_eq!(alias.callee_kind, None);
    assert_eq!(alias.callee_path, None);
    assert!(
        alias.span.0 < alias.span.1,
        "alias binding should retain a non-empty source span: {alias:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let alias_edges = edges
        .iter()
        .filter(|edge| edge.source_id == alias.id || edge.target_id == alias.id)
        .collect::<Vec<_>>();
    assert_eq!(
        alias_edges.len(),
        3,
        "alias binding should expose owner containment, alias, and source-function edges: {edges:#?}"
    );
    assert!(
        alias_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == alias.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-alias-binding edge: {alias_edges:#?}"
    );
    assert!(
        alias_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingAliasesBinding
            && edge.source_id == alias.id
            && edge.target_id == parameter.id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "LocalBinding"),
        "missing alias-to-parameter-binding edge: {alias_edges:#?}"
    );
    assert!(
        alias_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceFunction
            && edge.source_id == alias.id
            && edge.source_kind == "LocalBinding"
            && edge.target_id == target
            && edge.target_kind == "Function"),
        "missing alias-to-source-function edge: {alias_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_aliased_field_parameter_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_aliased_named_field_function_param")?;
    let caller = function_id_by_name(
        &db,
        "call_single_aliased_named_field_function_param_with_local_target",
    )?;
    let target = function_id_by_name(&db, "local_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2410-2419:
    // The private helper aliases `holder` before calling `(alias.callback)()`.
    // Its only local caller supplies `CallbackHolder { callback: local_target }`,
    // so the dynamic field call can resolve while the DB still records the
    // local alias edge separately from the caller argument proof.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["alias", "callback"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(
        &caller_context,
        &["call_single_aliased_named_field_function_param"],
    );
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let holder = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "holder")
        .expect("holder parameter binding should be persisted");
    assert_eq!(holder.source_kind, "Parameter");

    let alias = bindings
        .iter()
        .find(|binding| binding.kind == "LetBinding" && binding.name == "alias")
        .expect("alias binding should be persisted");
    assert_eq!(alias.source_kind, "ValueAlias");
    assert_eq!(alias.source_id, None);
    assert_eq!(alias.source_call_kind, None);
    assert_eq!(alias.source_path.as_ref(), Some(&path(&["holder"])));
    assert_eq!(alias.callee_kind, None);
    assert_eq!(alias.callee_path, None);

    let edges = db.local_binding_edges_for_owner(owner)?;
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.source_id == owner
                && edge.target_id == holder.id
                && edge.target_kind == "LocalBinding"
        ),
        "missing owner-to-holder parameter edge: {edges:#?}"
    );
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::OwnerContainsBinding
                && edge.source_id == owner
                && edge.target_id == alias.id
                && edge.target_kind == "LocalBinding"
        ),
        "missing owner-to-alias binding edge: {edges:#?}"
    );
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::BindingAliasesBinding
                && edge.source_id == alias.id
                && edge.target_id == holder.id
                && edge.source_kind == "LocalBinding"
                && edge.target_kind == "LocalBinding"
        ),
        "missing alias-to-holder binding edge: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::ArgumentSuppliesParameter
            && edge.source_id == helper_call.site.id
            && edge.source_kind == "Path"
            && edge.target_id == holder.id
            && edge.target_kind == "LocalBinding"),
        "missing constructed argument-to-holder parameter edge: {edges:#?}"
    );
    assert!(
        edges
            .iter()
            .all(|edge| edge.relation != LocalBindingRelationKind::BindingProjectsField),
        "aliased parameter proof must not fabricate a same-owner field projection: {edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_result_callback_parameter_source_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_result_callback")?;
    let caller = function_id_by_name(&db, "call_single_result_callback_with_local_target")?;
    let target = function_id_by_name(&db, "local_result_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2301-2310:
    // private `call_single_result_callback(f)` calls
    // `Ok::<i32, ()>(1).and_then(f)`, and its only local caller supplies
    // `local_result_target`. The method-callback call edge is already exact;
    // this test pins the durable parameter binding proof behind that edge.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_method_receiver(
        &context,
        "and_then",
        &CallReceiver::PathCallResult {
            path: path(&["Ok"]),
        },
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::MethodCallbackFunction,
        CallSiteKind::Method,
        CallTargetKind::Function,
    );

    let caller_context = db.call_context_for_owner(caller)?;
    let helper_call = row_by_path(&caller_context, &["call_single_result_callback"]);
    assert_resolved_target(
        helper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let parameter = bindings
        .iter()
        .find(|binding| binding.kind == "ParameterBinding" && binding.name == "f")
        .unwrap_or_else(|| {
            panic!("result callback should persist parameter binding f: {bindings:#?}")
        });
    assert_eq!(parameter.source_kind, "Parameter");

    let edges = db.local_binding_edges_for_owner(owner)?;
    assert!(
        edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::ArgumentSuppliesParameter
            && edge.source_id == helper_call.site.id
            && edge.source_kind == "Path"
            && edge.target_id == parameter.id
            && edge.target_kind == "LocalBinding"),
        "missing helper callsite to callback parameter edge: {edges:#?}"
    );
    assert!(
        edges.iter().any(
            |edge| edge.relation == LocalBindingRelationKind::BindingSourceFunction
                && edge.source_id == parameter.id
                && edge.source_kind == "LocalBinding"
                && edge.target_id == target
                && edge.target_kind == "Function"
        ),
        "missing callback parameter to source-function edge: {edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_let_closure_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let module_target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed closure binding context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["local_target"]);
    let closure_id = row.targets[0].target_id;
    assert_ne!(
        closure_id, module_target,
        "local closure binding must not point at the module-level local_target function"
    );
    assert_resolved_target(
        row,
        closure_id,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LetBinding")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "shadowed closure owner should expose one let binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.kind, "LetBinding");
    assert_eq!(binding.name, "local_target");
    assert_eq!(binding.source_kind, "Closure");
    assert_eq!(binding.source_id, Some(closure_id));
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "let binding should retain a non-empty source span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "shadowed closure owner should expose owner and source binding edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-let-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceClosure
            && edge.source_id == binding.id
            && edge.target_id == closure_id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Closure"),
        "missing let-binding-to-closure edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_local_function_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "local_fn_body_call_is_not_outer_call_site")?;
    let local_item = local_item_owner_for_parent_with_label(&db, owner, "local_fn:inner")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1447-1453:
    // the outer body declares block-local `fn inner()`, then calls `inner()`.
    // The call edge already resolves to the executable local-item body; this
    // test pins the durable binding row and source-local-item edge that explain
    // the visible local function item binding.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["inner"]);
    assert_resolved_target(
        row,
        local_item,
        CallRelationKind::LocalFunction,
        CallSiteKind::Path,
        CallTargetKind::LocalItem,
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LocalFunctionBinding" && binding.name == "inner")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "outer owner should expose one local function binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "LocalFunction");
    assert_eq!(binding.source_id, Some(local_item));
    assert_eq!(binding.source_call_kind, None);
    assert_eq!(binding.source_path, None);
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);
    assert!(
        binding.span.0 < binding.span.1,
        "local function binding should retain the item span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "local function binding should expose owner and source-local-item edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-local-function-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceLocalItem
            && edge.source_id == binding.id
            && edge.target_id == local_item
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "LocalItem"),
        "missing local-function-binding-to-local-item edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_awaited_future_let_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_stored_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2375-2377:
    // `let future = make_returned_async_closure()(); future.await` proves the
    // stored future binding is sourced by the earlier dynamic call result.
    let context = db.call_context_for_owner(owner)?;
    let dynamic = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(
        dynamic.status.status,
        CallStatusKind::Resolved,
        "stored returned async closure dynamic row should resolve once awaited: {dynamic:#?}"
    );
    assert_eq!(
        dynamic.site.kind,
        CallSiteKind::Dynamic,
        "stored future source should be the dynamic returned-callable callsite"
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let bindings = bindings
        .into_iter()
        .filter(|binding| binding.kind == "LetBinding" && binding.name == "future")
        .collect::<Vec<_>>();
    assert_eq!(
        bindings.len(),
        1,
        "stored returned async closure owner should expose one future let binding: {bindings:#?}"
    );
    let binding = &bindings[0];
    assert_eq!(binding.source_kind, "DynamicCallResult");
    assert_eq!(
        binding.source_id,
        Some(dynamic.site.id),
        "future binding should point at the stored returned-callable dynamic site"
    );
    assert_eq!(binding.source_call_kind.as_deref(), Some("Dynamic"));
    assert_eq!(binding.source_path, None);
    assert_eq!(
        binding.callee_kind.as_deref(),
        Some("AwaitedReturnedPathCall")
    );
    assert_eq!(
        binding.callee_path.as_ref(),
        Some(&path(&["make_returned_async_closure"]))
    );
    assert!(
        binding.span.0 < binding.span.1,
        "future let binding should retain a non-empty source span: {binding:#?}"
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "stored future binding should expose owner and source edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-future-binding edge: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceCallResult
            && edge.source_id == binding.id
            && edge.target_id == dynamic.site.id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Dynamic"),
        "missing future-binding-to-dynamic-call edge: {binding_edges:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_aggregate_returned_future_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    struct FutureCase {
        owner: &'static str,
        binding: &'static str,
        source: &'static str,
    }

    let cases = [
        FutureCase {
            owner: "call_stored_returned_async_closure_tuple_field",
            binding: "futures.0",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2380-2382 `futures.0.await`",
        },
        FutureCase {
            owner: "call_stored_returned_async_closure_named_field",
            binding: "holder.future",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2385-2389 `holder.future.await`",
        },
        FutureCase {
            owner: "call_stored_returned_async_closure_indexed_array",
            binding: "futures.0",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2392-2394 `futures[0].await`",
        },
    ];

    for case in cases {
        // Source oracle: each aggregate stores the future returned by
        // `make_returned_async_closure()()` and later awaits that exact slot.
        // The durable binding row points back to the original dynamic
        // returned-callable site without promoting general poll/resume.
        assert_returned_future_storage_binding(&db, case.owner, case.binding, case.source)?;
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_aggregate_forwarded_future_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_stored_forwarded_returned_async_future_tuple_field",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:2405-2407 `futures.0.await`",
        ),
        (
            "call_aliased_stored_forwarded_returned_async_future_tuple_field",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:2410-2413 `alias.0.await`",
        ),
    ];

    for (owner_name, source) in cases {
        assert_forwarded_future_storage_binding(&db, owner_name, source)?;
    }

    Ok(())
}

fn assert_forwarded_future_storage_binding(
    db: &ploke_db::Database,
    owner_name: &str,
    source: &str,
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let producer = function_id_by_name(db, "make_forwarded_returned_async_future")?;

    // Source oracle: the awaited tuple slot is produced by
    // `make_forwarded_returned_async_future()`. For the aliased case the poll
    // point is visible only through `alias.0.await`, so this assertion proves
    // one-hop aggregate alias tracking without admitting a traversal edge to
    // the async closure body.
    let context = db.call_context_for_owner(owner)?;
    let producer_row = row_by_path(&context, &["make_forwarded_returned_async_future"]);
    assert_resolved_target(
        producer_row,
        producer,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let awaited_sites = db.awaited_call_sites_for_owner(owner)?;
    assert_eq!(
        awaited_sites.len(),
        1,
        "{owner_name} should record exactly one awaited producer call: {source}; {awaited_sites:#?}"
    );
    assert_eq!(awaited_sites[0].id, producer_row.site.id);
    assert_eq!(awaited_sites[0].kind, CallSiteKind::Path);

    let bindings = db.local_bindings_for_owner(owner)?;
    let matches = bindings
        .iter()
        .filter(|binding| binding.kind == "LetBinding" && binding.name == "futures.0")
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{owner_name} should expose one aggregate future binding: {source}; {bindings:#?}"
    );
    let binding = matches[0];
    assert_eq!(binding.source_kind, "PathCallResult");
    assert_eq!(binding.source_id, Some(producer_row.site.id));
    assert_eq!(binding.source_call_kind.as_deref(), Some("Path"));
    assert_eq!(
        binding.source_path.as_ref(),
        Some(&path(&["make_forwarded_returned_async_future"]))
    );
    assert_eq!(binding.callee_kind, None);
    assert_eq!(binding.callee_path, None);

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "{owner_name} forwarded future binding should expose owner and source edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-forwarded-future binding edge for {owner_name}: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceCallResult
            && edge.source_id == binding.id
            && edge.target_id == producer_row.site.id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Path"),
        "missing forwarded-future binding-to-path-call edge for {owner_name}: {binding_edges:#?}"
    );

    Ok(())
}

fn assert_returned_future_storage_binding(
    db: &ploke_db::Database,
    owner_name: &str,
    binding_name: &str,
    source: &str,
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    let dynamic = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(
        dynamic.status.status,
        CallStatusKind::Resolved,
        "{owner_name} should resolve the awaited returned async-closure future: {source}; row: {dynamic:#?}"
    );

    let bindings = db.local_bindings_for_owner(owner)?;
    let matches = bindings
        .iter()
        .filter(|binding| binding.kind == "LetBinding" && binding.name == binding_name)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{owner_name} should expose one aggregate future binding {binding_name}: {source}; bindings: {bindings:#?}"
    );
    let binding = matches[0];
    assert_eq!(binding.source_kind, "DynamicCallResult");
    assert_eq!(
        binding.source_id,
        Some(dynamic.site.id),
        "{owner_name} aggregate future binding should point at the returned-callable dynamic site"
    );
    assert_eq!(binding.source_call_kind.as_deref(), Some("Dynamic"));
    assert_eq!(binding.source_path, None);
    assert_eq!(
        binding.callee_kind.as_deref(),
        Some("AwaitedReturnedPathCall")
    );
    assert_eq!(
        binding.callee_path.as_ref(),
        Some(&path(&["make_returned_async_closure"]))
    );

    let edges = db.local_binding_edges_for_owner(owner)?;
    let binding_edges = edges
        .iter()
        .filter(|edge| edge.source_id == binding.id || edge.target_id == binding.id)
        .collect::<Vec<_>>();
    assert_eq!(
        binding_edges.len(),
        2,
        "{owner_name} aggregate future binding should expose owner and source edges: {edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::OwnerContainsBinding
            && edge.source_id == owner
            && edge.target_id == binding.id
            && edge.target_kind == "LocalBinding"),
        "missing owner-to-aggregate-future binding edge for {owner_name}: {binding_edges:#?}"
    );
    assert!(
        binding_edges.iter().any(|edge| edge.relation
            == LocalBindingRelationKind::BindingSourceCallResult
            && edge.source_id == binding.id
            && edge.target_id == dynamic.site.id
            && edge.source_kind == "LocalBinding"
            && edge.target_kind == "Dynamic"),
        "missing aggregate future binding-to-dynamic-call edge for {owner_name}: {binding_edges:#?}"
    );

    Ok(())
}
