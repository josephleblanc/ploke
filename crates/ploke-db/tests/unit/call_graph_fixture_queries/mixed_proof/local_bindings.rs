use std::collections::BTreeSet;

use super::*;
use ploke_db::LocalBindingRelationKind;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct BindingExpectation<'a> {
    kind: &'a str,
    name: &'a str,
    source: &'a str,
    id: Option<Uuid>,
    call: Option<&'a str>,
    path: Option<&'a [&'a str]>,
    callee: Option<&'a str>,
    callee_path: Option<&'a [&'a str]>,
}

impl<'a> BindingExpectation<'a> {
    fn new(kind: &'a str, name: &'a str, source: &'a str) -> Self {
        Self {
            kind,
            name,
            source,
            id: None,
            call: None,
            path: None,
            callee: None,
            callee_path: None,
        }
    }

    fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    fn call(mut self, call: &'a str) -> Self {
        self.call = Some(call);
        self
    }

    fn path(mut self, path: &'a [&'a str]) -> Self {
        self.path = Some(path);
        self
    }

    fn callee(mut self, callee: &'a str) -> Self {
        self.callee = Some(callee);
        self
    }

    fn callee_path(mut self, path: &'a [&'a str]) -> Self {
        self.callee_path = Some(path);
        self
    }
}

fn assert_binding<'a>(
    bindings: &'a [ploke_db::LocalBindingRow],
    owner: Uuid,
    expected: BindingExpectation<'_>,
    label: &str,
) -> &'a ploke_db::LocalBindingRow {
    let matches = bindings
        .iter()
        .filter(|binding| binding.kind == expected.kind && binding.name == expected.name)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{label} should select exactly one binding: {bindings:#?}"
    );
    let binding = matches[0];
    assert_eq!(binding.owner_id, owner, "{label} owner");
    assert_eq!(binding.source_kind, expected.source, "{label} source kind");
    assert_eq!(binding.source_id, expected.id, "{label} source id");
    assert_eq!(
        binding.source_call_kind.as_deref(),
        expected.call,
        "{label} call kind"
    );
    let source_path = expected.path.map(path);
    assert_eq!(
        binding.source_path.as_ref(),
        source_path.as_ref(),
        "{label} source path"
    );
    assert_eq!(
        binding.callee_kind.as_deref(),
        expected.callee,
        "{label} callee kind"
    );
    let callee_path = expected.callee_path.map(path);
    assert_eq!(
        binding.callee_path.as_ref(),
        callee_path.as_ref(),
        "{label} callee path"
    );
    assert!(
        binding.span.0 < binding.span.1,
        "{label} should retain a non-empty source span: {binding:#?}"
    );
    binding
}

type EdgeFact = (Uuid, Uuid, String, String, String);

fn edge(
    source: Uuid,
    target: Uuid,
    relation: LocalBindingRelationKind,
    source_kind: &str,
    target_kind: &str,
) -> EdgeFact {
    (
        source,
        target,
        format!("{relation:?}"),
        source_kind.to_string(),
        target_kind.to_string(),
    )
}

fn owner_edge(owner: Uuid, binding: Uuid) -> EdgeFact {
    edge(
        owner,
        binding,
        LocalBindingRelationKind::OwnerContainsBinding,
        "Function",
        "LocalBinding",
    )
}

fn argument_edge(site: Uuid, binding: Uuid) -> EdgeFact {
    edge(
        site,
        binding,
        LocalBindingRelationKind::ArgumentSuppliesParameter,
        "Path",
        "LocalBinding",
    )
}

fn source_edge(
    binding: Uuid,
    target: Uuid,
    relation: LocalBindingRelationKind,
    target_kind: &str,
) -> EdgeFact {
    edge(binding, target, relation, "LocalBinding", target_kind)
}

fn alias_edge(alias: Uuid, source: Uuid) -> EdgeFact {
    edge(
        alias,
        source,
        LocalBindingRelationKind::BindingAliasesBinding,
        "LocalBinding",
        "LocalBinding",
    )
}

fn projection_edge(projection: Uuid, source: Uuid) -> EdgeFact {
    edge(
        projection,
        source,
        LocalBindingRelationKind::BindingProjectsField,
        "LocalBinding",
        "LocalBinding",
    )
}

fn assert_edges(
    edges: &[ploke_db::LocalBindingEdgeRow],
    binding: Uuid,
    expected: &[EdgeFact],
    label: &str,
) {
    let rows = edges
        .iter()
        .filter(|edge| edge.source_id == binding || edge.target_id == binding)
        .collect::<Vec<_>>();
    let actual = rows
        .iter()
        .map(|row| {
            edge(
                row.source_id,
                row.target_id,
                row.relation,
                &row.source_kind,
                &row.target_kind,
            )
        })
        .collect::<BTreeSet<_>>();
    let expected_len = expected.len();
    let expected = expected.iter().cloned().collect::<BTreeSet<_>>();
    assert_eq!(
        (rows.len(), expected_len),
        (actual.len(), expected.len()),
        "{label} actual/expected edge lists should not contain duplicates: {rows:#?}"
    );
    assert_eq!(
        actual, expected,
        "{label} incident edges should match exactly"
    );
}

fn assert_source_binding(
    db: &ploke_db::Database,
    owner: Uuid,
    expected: BindingExpectation<'_>,
    target: Uuid,
    relation: LocalBindingRelationKind,
    target_kind: &str,
    label: &str,
) -> Result<(), DbError> {
    let bindings = db.local_bindings_for_owner(owner)?;
    let binding = assert_binding(&bindings, owner, expected, label);
    let edges = db.local_binding_edges_for_owner(owner)?;
    assert_edges(
        &edges,
        binding.id,
        &[
            owner_edge(owner, binding.id),
            source_edge(binding.id, target, relation, target_kind),
        ],
        label,
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum LocalCall<'a> {
    Path(&'a [&'a str]),
    Dynamic(&'a [&'a str]),
    ResultCallback,
}

#[derive(Clone, Copy)]
struct PrivateCase<'a> {
    owner: &'a str,
    target: &'a str,
    call: LocalCall<'a>,
}

type PrivateFixture = (ploke_db::Database, Uuid, Uuid, Uuid);

fn private_fixture(case: PrivateCase<'_>) -> Result<PrivateFixture, DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, case.owner)?;
    let caller_name = format!("{}_with_local_target", case.owner);
    let caller = function_id_by_name(&db, &caller_name)?;
    let target = function_id_by_name(&db, case.target)?;
    let context = db.call_context_for_owner(owner)?;
    let (row, relation, kind) = match case.call {
        LocalCall::Path(call_path) => (
            row_by_path(&context, call_path),
            CallRelationKind::Function,
            CallSiteKind::Path,
        ),
        LocalCall::Dynamic(call_path) => (
            row_by_kind_path(&context, CallSiteKind::Dynamic, call_path),
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
        ),
        LocalCall::ResultCallback => (
            row_by_method_receiver(
                &context,
                "and_then",
                &CallReceiver::PathCallResult {
                    path: path(&["Ok"]),
                },
            ),
            CallRelationKind::MethodCallbackFunction,
            CallSiteKind::Method,
        ),
    };
    assert_resolved_target(row, target, relation, kind, CallTargetKind::Function);

    let context = db.call_context_for_owner(caller)?;
    let helper = row_by_path(&context, &[case.owner]);
    assert_resolved_target(
        helper,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let helper = helper.site.id;
    Ok((db, owner, target, helper))
}

enum ParameterProof {
    Argument,
    Callable,
}

fn assert_parameter_case(
    case: PrivateCase<'_>,
    name: &str,
    proof: ParameterProof,
) -> Result<(), DbError> {
    let (db, owner, target, helper) = private_fixture(case)?;
    let bindings = db.local_bindings_for_owner(owner)?;
    let binding = assert_binding(
        &bindings,
        owner,
        BindingExpectation::new("ParameterBinding", name, "Parameter"),
        case.owner,
    );
    let mut expected = vec![
        owner_edge(owner, binding.id),
        argument_edge(helper, binding.id),
    ];
    if matches!(proof, ParameterProof::Callable) {
        expected.push(source_edge(
            binding.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            "Function",
        ));
    }
    let edges = db.local_binding_edges_for_owner(owner)?;
    assert_edges(&edges, binding.id, &expected, case.owner);
    Ok(())
}

enum AliasProof {
    Callable,
    FieldParameter,
}

fn assert_alias_case(
    case: PrivateCase<'_>,
    parameter: &str,
    alias_name: &str,
    proof: AliasProof,
) -> Result<(), DbError> {
    let (db, owner, target, helper) = private_fixture(case)?;
    let bindings = db.local_bindings_for_owner(owner)?;
    let parameter = assert_binding(
        &bindings,
        owner,
        BindingExpectation::new("ParameterBinding", parameter, "Parameter"),
        case.owner,
    );
    let alias = assert_binding(
        &bindings,
        owner,
        BindingExpectation::new("LetBinding", alias_name, "ValueAlias")
            .path(&[parameter.name.as_str()]),
        case.owner,
    );
    let edges = db.local_binding_edges_for_owner(owner)?;
    let mut expected = vec![
        owner_edge(owner, alias.id),
        alias_edge(alias.id, parameter.id),
    ];
    match proof {
        AliasProof::Callable => expected.push(source_edge(
            alias.id,
            target,
            LocalBindingRelationKind::BindingSourceFunction,
            "Function",
        )),
        AliasProof::FieldParameter => {
            assert_edges(
                &edges,
                parameter.id,
                &[
                    owner_edge(owner, parameter.id),
                    alias_edge(alias.id, parameter.id),
                    argument_edge(helper, parameter.id),
                ],
                case.owner,
            );
            assert!(
                edges
                    .iter()
                    .all(|edge| edge.relation != LocalBindingRelationKind::BindingProjectsField),
                "{} must not fabricate a same-owner field projection: {edges:#?}",
                case.owner
            );
        }
    }
    assert_edges(&edges, alias.id, &expected, case.owner);
    Ok(())
}

#[test]
fn fixture_projection_stores_parameter_binding_edges() -> Result<(), DbError> {
    // lib.rs:1523-1528: the private caller supplies `local_target` to parameter `f`.
    assert_parameter_case(
        PrivateCase {
            owner: "call_single_function_pointer_param",
            target: "local_target",
            call: LocalCall::Path(&["f"]),
        },
        "f",
        ParameterProof::Callable,
    )
}

#[test]
fn fixture_projection_stores_targetless_callable_callee_evidence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    #[rustfmt::skip]
    let cases = [
        ("call_function_pointer_param", CallSiteKind::Path, &["f"][..], "ValueBinding"),
        ("call_parenthesized_function_pointer_param", CallSiteKind::Dynamic, &["f"][..], "LocalBinding"),
        ("call_function_pointer_param_cast", CallSiteKind::Dynamic, &["f"][..], "FnPointerCastLocalBinding"),
        ("call_field_function_param", CallSiteKind::Dynamic, &["holder", "callback"][..], "FieldLocalBinding"),
        ("call_if_function_pointer_param_branch", CallSiteKind::Dynamic, &["f"][..], "IfBranchParameter"),
        ("call_match_function_pointer_param_arm", CallSiteKind::Dynamic, &["f"][..], "MatchArmParameter"),
    ];

    for (owner_name, kind, callee_path, callee_kind) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, kind, callee_path);

        let evidence = db.call_callee_evidence_for_owner(owner)?;
        assert_eq!(
            evidence.len(),
            1,
            "{} should expose one callee evidence row for the owner: {evidence:#?}",
            owner_name
        );
        let evidence = &evidence[0];
        assert_eq!(
            (
                row.status.status,
                row.targets.len(),
                evidence.site_id,
                evidence.site_kind,
                evidence.closure_id,
                evidence.callee_kind.as_str(),
                evidence.callee_path.as_slice(),
                relations_for_site(&db, row.site.id)?.rows.len(),
                db.call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?
                    .len(),
            ),
            (
                CallStatusKind::Unsupported,
                0,
                row.site.id,
                kind,
                None,
                callee_kind,
                path(callee_path).as_slice(),
                0,
                2,
            ),
            "{owner_name}: targetless callee evidence projection"
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
        call_path: &'static [&'static str],
        constructed: &'static str,
    }

    #[rustfmt::skip]
    let cases = [
        ProjectionCase { owner: "call_named_field_function_binding", call_path: &["holder", "callback"], constructed: "NamedCallbackHolder" },
        ProjectionCase { owner: "call_aliased_named_field_function_binding", call_path: &["alias", "callback"], constructed: "NamedCallbackHolder" },
        ProjectionCase { owner: "call_indexed_named_field_function_binding", call_path: &["holder", "callbacks", "0"], constructed: "CallbackArrayHolder" },
        ProjectionCase { owner: "call_aliased_indexed_named_field_function_binding", call_path: &["alias", "callbacks", "0"], constructed: "CallbackArrayHolder" },
        ProjectionCase { owner: "call_indexed_tuple_field_function_binding", call_path: &["holder", "0", "0"], constructed: "TupleCallbackArrayHolder" },
        ProjectionCase { owner: "call_aliased_indexed_tuple_field_function_binding", call_path: &["alias", "0", "0"], constructed: "TupleCallbackArrayHolder" },
    ];

    for case in cases {
        let owner = function_id_by_name(&db, case.owner)?;
        let projection_name = case.call_path.join(".");

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
        let holder = assert_binding(
            &bindings,
            owner,
            BindingExpectation::new("LetBinding", case.call_path[0], "Constructed")
                .path(&[case.constructed]),
            case.owner,
        );
        let projection = assert_binding(
            &bindings,
            owner,
            BindingExpectation::new("FieldProjection", &projection_name, "FieldProjection")
                .id(holder.id)
                .path(&case.call_path[1..])
                .callee("Path")
                .callee_path(&["local_target"]),
            case.owner,
        );
        let edges = db.local_binding_edges_for_owner(owner)?;
        assert_edges(
            &edges,
            holder.id,
            &[
                owner_edge(owner, holder.id),
                projection_edge(projection.id, holder.id),
            ],
            case.owner,
        );
        assert_edges(
            &edges,
            projection.id,
            &[
                owner_edge(owner, projection.id),
                projection_edge(projection.id, holder.id),
                source_edge(
                    projection.id,
                    target,
                    LocalBindingRelationKind::BindingSourceFunction,
                    "Function",
                ),
            ],
            case.owner,
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_keeps_unproven_field_parameter_without_projection_edge() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    #[rustfmt::skip]
    let cases = [
        ("call_field_function_param", &["holder", "callback"][..]),
        ("call_indexed_field_function_param", &["holder", "callbacks", "0"][..]),
        ("call_indexed_tuple_field_function_param", &["holder", "0", "0"][..]),
    ];

    for (owner_name, call_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;

        let context = db.call_context_for_owner(owner)?;
        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, call_path);
        let bindings = db.local_bindings_for_owner(owner)?;
        let edges = db.local_binding_edges_for_owner(owner)?;
        assert_eq!(
            (
                row.status.status,
                row.targets.len(),
                bindings
                    .iter()
                    .all(|binding| binding.kind != "FieldProjection"),
                bindings.iter().any(|binding| {
                    binding.kind == "ParameterBinding" && binding.name == "holder"
                }),
                edges.iter().all(|edge| {
                    edge.relation != LocalBindingRelationKind::BindingProjectsField
                }),
            ),
            (CallStatusKind::Unsupported, 0, true, true, true),
            "{owner_name}: unproven projection must stay fail-closed; {row:#?}; {bindings:#?}; {edges:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_constructed_field_argument_parameter_edge() -> Result<(), DbError> {
    // lib.rs:1610-1616: the private caller supplies a constructed callback holder.
    assert_parameter_case(
        PrivateCase {
            owner: "call_single_named_field_function_param",
            target: "local_target",
            call: LocalCall::Dynamic(&["holder", "callback"]),
        },
        "holder",
        ParameterProof::Argument,
    )
}

#[test]
fn fixture_projection_stores_initialized_path_let_binding() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_local_function_item_binding")?;
    let target = function_id_by_name(&db, "local_target")?;

    // lib.rs:185-187: `let f = local_target; f()` sources `f` from a local path.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["f"]);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    assert_source_binding(
        &db,
        owner,
        BindingExpectation::new("LetBinding", "f", "InitializedPath").path(&["local_target"]),
        target,
        LocalBindingRelationKind::BindingSourceFunction,
        "Function",
        "initialized callable binding",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_value_alias_binding_edge() -> Result<(), DbError> {
    // lib.rs:1685-1687: `let g = f; g()` aliases the callee-owned parameter.
    assert_alias_case(
        PrivateCase {
            owner: "call_single_aliased_function_pointer_param",
            target: "local_target",
            call: LocalCall::Path(&["g"]),
        },
        "f",
        "g",
        AliasProof::Callable,
    )
}

#[test]
fn fixture_projection_stores_aliased_field_parameter_edges() -> Result<(), DbError> {
    // lib.rs:2410-2419: a private field-callback parameter is aliased before use.
    assert_alias_case(
        PrivateCase {
            owner: "call_single_aliased_named_field_function_param",
            target: "local_target",
            call: LocalCall::Dynamic(&["alias", "callback"]),
        },
        "holder",
        "alias",
        AliasProof::FieldParameter,
    )
}

#[test]
fn fixture_projection_stores_result_callback_parameter_source_edge() -> Result<(), DbError> {
    // lib.rs:2301-2310: the private `and_then(f)` caller supplies `local_result_target`.
    assert_parameter_case(
        PrivateCase {
            owner: "call_single_result_callback",
            target: "local_result_target",
            call: LocalCall::ResultCallback,
        },
        "f",
        ParameterProof::Callable,
    )
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

    assert_source_binding(
        &db,
        owner,
        BindingExpectation::new("LetBinding", "local_target", "Closure").id(closure_id),
        closure_id,
        LocalBindingRelationKind::BindingSourceClosure,
        "Closure",
        "shadowed closure binding",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_local_function_binding_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "local_fn_body_call_is_not_outer_call_site")?;
    let local_item = local_item_owner_for_parent_with_label(&db, owner, "local_fn:inner")?;

    // lib.rs:1447-1453: block-local `fn inner()` is called from its outer body.
    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["inner"]);
    assert_resolved_target(
        row,
        local_item,
        CallRelationKind::LocalFunction,
        CallSiteKind::Path,
        CallTargetKind::LocalItem,
    );

    assert_source_binding(
        &db,
        owner,
        BindingExpectation::new("LocalFunctionBinding", "inner", "LocalFunction").id(local_item),
        local_item,
        LocalBindingRelationKind::BindingSourceLocalItem,
        "LocalItem",
        "local function binding",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_awaited_future_let_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    // lib.rs:2375-2377: an awaited local stores the earlier dynamic call result.
    assert_returned_future_storage_binding(&db, "call_stored_returned_async_closure", "future")
}

#[test]
fn fixture_projection_stores_aggregate_returned_future_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    #[rustfmt::skip]
    let cases = [
        ("call_stored_returned_async_closure_tuple_field", "futures.0"),
        ("call_stored_returned_async_closure_named_field", "holder.future"),
        ("call_stored_returned_async_closure_indexed_array", "futures.0"),
    ];

    for (owner_name, binding_name) in cases {
        assert_returned_future_storage_binding(&db, owner_name, binding_name)?;
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_aggregate_forwarded_future_call_result_edges() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        "call_stored_forwarded_returned_async_future_tuple_field",
        "call_aliased_stored_forwarded_returned_async_future_tuple_field",
    ];

    for owner_name in cases {
        assert_forwarded_future_storage_binding(&db, owner_name)?;
    }

    Ok(())
}

fn assert_forwarded_future_storage_binding(
    db: &ploke_db::Database,
    owner_name: &str,
) -> Result<(), DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let producer = function_id_by_name(db, "make_forwarded_returned_async_future")?;

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
        "{owner_name} should record exactly one awaited producer call: {awaited_sites:#?}"
    );
    assert_eq!(awaited_sites[0].id, producer_row.site.id);
    assert_eq!(awaited_sites[0].kind, CallSiteKind::Path);

    assert_source_binding(
        db,
        owner,
        BindingExpectation::new("LetBinding", "futures.0", "PathCallResult")
            .id(producer_row.site.id)
            .call("Path")
            .path(&["make_forwarded_returned_async_future"]),
        producer_row.site.id,
        LocalBindingRelationKind::BindingSourceCallResult,
        "Path",
        owner_name,
    )?;

    Ok(())
}

fn assert_returned_future_storage_binding(
    db: &ploke_db::Database,
    owner_name: &str,
    binding_name: &str,
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
        "{owner_name} should resolve the awaited returned async-closure future: {dynamic:#?}"
    );
    assert_eq!(dynamic.site.kind, CallSiteKind::Dynamic, "{owner_name}");

    assert_source_binding(
        db,
        owner,
        BindingExpectation::new("LetBinding", binding_name, "DynamicCallResult")
            .id(dynamic.site.id)
            .call("Dynamic")
            .callee("AwaitedReturnedPathCall")
            .callee_path(&["make_returned_async_closure"]),
        dynamic.site.id,
        LocalBindingRelationKind::BindingSourceCallResult,
        "Dynamic",
        owner_name,
    )?;

    Ok(())
}
