use std::sync::Arc;

use ploke_test_utils::{
    CallExpected, CallOwnerSelector, CallPipelineCoverage, CallReceiverSelector, CallShapeCase,
    CallSiteSelector, CallTargetSelector, call_shape_cases,
};

use super::super::expected::path;
use super::*;

#[tokio::test]
async fn shared_call_shape_matrix_rows_reach_rag_call_context() -> Result<(), Error> {
    init_tracing_once();

    for case in call_shape_cases()
        .iter()
        .filter(|case| covers(case, CallPipelineCoverage::RagApi))
    {
        eprintln!("shared RAG call-shape case: {}", case.name);
        let (db, rag) = setup_matrix_rag(case)?;
        assert_case(&db, &rag, case)?;
    }

    Ok(())
}

fn assert_case(db: &Database, rag: &RagService, case: &CallShapeCase) -> Result<(), Error> {
    let owner = resolve_owner(db, case.owner)?;
    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context.get(&owner).unwrap_or_else(|| {
        panic!(
            "{} should receive outgoing RAG call context; source: {}",
            case.name, case.source
        )
    });
    let call = select_site(context, case);
    assert_eq!(call.owner_id, owner, "{}", case.name);

    match case.expected {
        CallExpected::Resolved {
            target,
            relation,
            edge_count,
            ..
        } => {
            let target = resolve_target(db, target)?;
            assert_eq!(
                call.status,
                CallStatusKind::Resolved,
                "{} should remain resolved in RAG call context; source: {}",
                case.name,
                case.source
            );
            assert_eq!(
                call.resolution,
                Some(CallResolutionKind::LocalExact),
                "{} should preserve exact-local resolution; source: {}",
                case.name,
                case.source
            );
            assert_eq!(
                call.targets.len(),
                edge_count,
                "{} should preserve exactly {edge_count} RAG target edge(s); source: {}",
                case.name,
                case.source
            );
            assert!(
                call.targets.iter().any(|candidate| {
                    candidate.target_id == target
                        && candidate.relation == rag_relation_kind(relation)
                }),
                "{} should preserve the expected resolved target; source: {}; call: {call:#?}",
                case.name,
                case.source
            );

            assert_target_centered_view(db, rag, case, owner, target, call.site_id, edge_count)?;
        }
        CallExpected::Targetless { status } => {
            assert_eq!(
                call.status,
                rag_status_kind(status),
                "{} should preserve targetless status; source: {}",
                case.name,
                case.source
            );
            assert_eq!(
                call.resolution, None,
                "{} should not invent a resolution for a targetless row; source: {}",
                case.name, case.source
            );
            assert!(
                call.targets.is_empty(),
                "{} should remain targetless in RAG call context; source: {}; call: {call:#?}",
                case.name,
                case.source
            );
        }
    }

    Ok(())
}

fn assert_target_centered_view(
    db: &Database,
    rag: &RagService,
    case: &CallShapeCase,
    owner: Uuid,
    target: Uuid,
    site_id: Uuid,
    edge_count: usize,
) -> Result<(), Error> {
    let callers = db.callers_for_target(target)?;
    let matching_callers = callers
        .iter()
        .filter(|caller| caller.site.owner_id == owner && caller.site.id == site_id)
        .count();
    assert_eq!(
        matching_callers, edge_count,
        "{} should expose the same direct caller edge through DB target lookup; source: {}; callers: {callers:#?}",
        case.name, case.source
    );

    let incoming = rag.exact_call_context(target)?;
    let matching_incoming = incoming
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.site_id == site_id
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching_incoming.len(),
        edge_count,
        "{} should preserve the matrix edge through RAG exact target context; source: {}; incoming: {incoming:#?}",
        case.name,
        case.source
    );

    Ok(())
}

fn setup_matrix_rag(case: &CallShapeCase) -> Result<(Arc<Database>, RagService), Error> {
    let fixture = case.fixture.fixture();
    let db = Arc::new(ploke_test_utils::fresh_backup_fixture_db(fixture)?);
    assert!(
        db.has_call_graph_relations()?,
        "{} must include call graph relations for shared RAG call-shape matrix case {}",
        fixture.id,
        case.name
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "{} should enable RAG call context for shared call-shape case {}",
        fixture.id,
        case.name
    );

    Ok((db, rag))
}

fn resolve_owner(db: &Database, owner: CallOwnerSelector) -> Result<Uuid, Error> {
    match owner {
        CallOwnerSelector::FunctionInModule { module_path, name } => {
            function_id_by_name_in_module(db, module_path, name)
        }
        CallOwnerSelector::MethodByBody {
            name,
            body,
            owner_type,
            ..
        } => {
            if let Some(owner_type) = owner_type {
                method_id_by_name_body_and_owner_type(db, name, body, owner_type)
            } else {
                method_id_by_name_and_body_substring(db, name, body)
            }
        }
        CallOwnerSelector::MethodByBodyFile {
            name,
            body,
            file_suffix,
        } => method_id_by_file(db, name, body, file_suffix),
    }
}

fn resolve_target(db: &Database, target: CallTargetSelector) -> Result<Uuid, Error> {
    match target {
        CallTargetSelector::FunctionInModule { module_path, name } => {
            function_id_by_name_in_module(db, module_path, name)
        }
        CallTargetSelector::Struct { name } => struct_id_by_name(db, name),
        CallTargetSelector::Variant {
            enum_name,
            variant_name,
        } => variant_id_by_enum_and_variant_names(db, enum_name, variant_name),
    }
}

fn method_id_by_name_body_and_owner_type(
    db: &Database,
    name: &str,
    body_marker: &str,
    owner_type: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert("owner_type".to_string(), DataValue::from(owner_type));
    params.insert(
        "owner_path".to_string(),
        DataValue::List(vec![DataValue::from(owner_type)]),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

impl_self_target[self_target_id] := *struct {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *enum {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_target[self_target_id] := *union {{ id: self_target_id, name: $owner_type @ 'NOW' }}
impl_self_type[self_type_id] :=
    *type_relation {{
        source_id: self_type_id,
        target_id: self_target_id,
        relation_kind: "Ordinary" @ 'NOW'
    }},
    impl_self_target[self_target_id]
impl_self_type[self_type_id] :=
    *named_type {{ type_id: self_type_id, path @ 'NOW' }},
    path == $owner_path

?[id, body] :=
    *method {{ id, name: $name, body, owner_id: impl_id @ 'NOW' }},
    *impl {{ id: impl_id, self_type: self_type_id @ 'NOW' }},
    impl_self_type[self_type_id]
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let DataValue::Str(body) = &row[1] else {
                return None;
            };
            body_key(body)
                .contains(&marker)
                .then(|| to_uuid(&row[0]).map_err(Error::from))
        })
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} on owner_type {owner_type:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    Ok(matching[0])
}

fn select_site<'a>(context: &'a [CallContextInfo], case: &CallShapeCase) -> &'a CallContextInfo {
    let matches = context
        .iter()
        .filter(|call| site_matches(call, case.site))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{} should expose exactly one selected RAG call-site row; source: {}; context: {context:#?}",
        case.name,
        case.source
    );

    matches[0]
}

fn site_matches(call: &CallContextInfo, site: CallSiteSelector) -> bool {
    match site {
        CallSiteSelector::Path {
            segments,
            arg_count,
        } => {
            call.kind == CallSiteKind::Path
                && call.arg_count == arg_count
                && call.callee
                    == CallCalleeInfo::Path {
                        path: path(segments),
                    }
        }
        CallSiteSelector::Dynamic { arg_count } => {
            call.kind == CallSiteKind::Dynamic
                && call.arg_count == arg_count
                && call.callee == CallCalleeInfo::Dynamic
        }
        CallSiteSelector::Method {
            name,
            arg_count,
            receiver,
        } => {
            call.kind == CallSiteKind::Method
                && call.arg_count == arg_count
                && rag_method_matches(&call.callee, name, receiver)
        }
    }
}

fn rag_method_matches(
    callee: &CallCalleeInfo,
    name: &str,
    receiver: Option<CallReceiverSelector>,
) -> bool {
    let CallCalleeInfo::Method {
        name: method,
        receiver: actual,
    } = callee
    else {
        return false;
    };

    method == name && rag_receiver_matches(actual, receiver)
}

fn rag_receiver_matches(
    actual: &Option<CallReceiverInfo>,
    expected: Option<CallReceiverSelector>,
) -> bool {
    match expected {
        None => true,
        Some(CallReceiverSelector::SelfField { path }) => matches!(
            actual,
            Some(CallReceiverInfo::SelfField { path: actual })
                if actual.iter().map(String::as_str).eq(path.iter().copied())
        ),
        Some(CallReceiverSelector::MethodResultLocalBinding { method_name }) => matches!(
            actual,
            Some(CallReceiverInfo::MethodResultLocalBinding { method_name: actual, .. })
                if actual == method_name
        ),
        Some(CallReceiverSelector::Unsupported) => {
            matches!(actual, Some(CallReceiverInfo::Unsupported))
        }
    }
}

fn covers(case: &CallShapeCase, coverage: CallPipelineCoverage) -> bool {
    case.coverage.contains(&coverage)
}

fn rag_relation_kind(relation: ploke_db::CallRelationKind) -> CallTargetKind {
    match relation {
        ploke_db::CallRelationKind::Function => CallTargetKind::Function,
        ploke_db::CallRelationKind::DynamicFunction => CallTargetKind::DynamicFunction,
        ploke_db::CallRelationKind::Closure => CallTargetKind::Closure,
        ploke_db::CallRelationKind::LocalFunction => CallTargetKind::LocalFunction,
        ploke_db::CallRelationKind::DynamicClosure => CallTargetKind::DynamicClosure,
        ploke_db::CallRelationKind::MethodCallbackFunction => {
            CallTargetKind::MethodCallbackFunction
        }
        ploke_db::CallRelationKind::MethodCallbackClosure => CallTargetKind::MethodCallbackClosure,
        ploke_db::CallRelationKind::Method => CallTargetKind::Method,
        ploke_db::CallRelationKind::AssociatedFunction => CallTargetKind::AssociatedFunction,
        ploke_db::CallRelationKind::TupleStructConstructor => {
            CallTargetKind::TupleStructConstructor
        }
        ploke_db::CallRelationKind::EnumVariantConstructor => {
            CallTargetKind::EnumVariantConstructor
        }
    }
}

fn rag_status_kind(status: ploke_db::CallStatusKind) -> CallStatusKind {
    match status {
        ploke_db::CallStatusKind::Resolved => CallStatusKind::Resolved,
        ploke_db::CallStatusKind::Unresolved => CallStatusKind::Unresolved,
        ploke_db::CallStatusKind::Ambiguous => CallStatusKind::Ambiguous,
        ploke_db::CallStatusKind::External => CallStatusKind::External,
        ploke_db::CallStatusKind::Unsupported => CallStatusKind::Unsupported,
    }
}
