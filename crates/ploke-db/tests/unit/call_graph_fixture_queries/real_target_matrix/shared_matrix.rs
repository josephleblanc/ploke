//! DB assertions for the shared real-target call-shape matrix.

use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_test_utils::{
    CallExpected, CallOwnerSelector, CallReceiverSelector, CallShapeCase, CallSiteSelector,
    CallTargetSelector, call_shape_cases,
};
use uuid::Uuid;

use super::super::*;
use super::common::*;

#[test]
fn shared_call_shape_matrix_cases_match_registered_backups() -> Result<(), DbError> {
    for case in call_shape_cases() {
        eprintln!("shared call-shape case: {}", case.name);
        let db = setup_call_graph_db(case.fixture.fixture())?;
        assert_case(&db, case)?;
    }

    Ok(())
}

fn assert_case(db: &Database, case: &CallShapeCase) -> Result<(), DbError> {
    let owner = resolve_owner(db, case.owner)?;
    let context = db.call_context_for_owner(owner)?;
    let row = select_site(&context, case)?;

    match case.expected {
        CallExpected::Resolved {
            target,
            relation,
            target_kind,
            edge_count,
        } => {
            let target = resolve_target(db, target)?;
            assert_resolved_target(row, target, relation, row.site.kind, target_kind);
            assert_eq!(
                relations_for_site(db, row.site.id)?.rows.len(),
                edge_count,
                "{} should preserve exactly {edge_count} raw edge(s); source: {}",
                case.name,
                case.source
            );
            assert_one_edge_traversal(
                db,
                TraversalExpectation {
                    label: case.name,
                    owner,
                    target,
                    site_id: row.site.id,
                    expected_edge_count: edge_count,
                },
            )?;
        }
        CallExpected::Targetless { status } => {
            assert_targetless_status(row, status);
            assert!(
                relations_for_site(db, row.site.id)?.rows.is_empty(),
                "{} should remain targetless with zero raw edges; source: {}",
                case.name,
                case.source
            );
            assert_no_traversal_candidates_for_site(db, owner, row.site.id, case.name)?;
        }
    }

    Ok(())
}

fn resolve_owner(db: &Database, owner: CallOwnerSelector) -> Result<Uuid, DbError> {
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
        } => method_id_by_name_body_and_file_suffix(db, name, body, file_suffix),
    }
}

fn resolve_target(db: &Database, target: CallTargetSelector) -> Result<Uuid, DbError> {
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
) -> Result<Uuid, DbError> {
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
            body_key(body).contains(&marker).then(|| to_uuid(&row[0]))
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

fn body_key(value: &str) -> String {
    value.split_whitespace().collect::<String>()
}

fn select_site<'a>(
    context: &'a [ploke_db::CallContextRow],
    case: &CallShapeCase,
) -> Result<&'a ploke_db::CallContextRow, DbError> {
    let row = match case.site {
        CallSiteSelector::Path {
            segments,
            arg_count,
        } => {
            let row = row_by_path(context, segments);
            assert_eq!(
                row.site.arg_count, arg_count,
                "{} should preserve path argument count; source: {}",
                case.name, case.source
            );
            row
        }
        CallSiteSelector::Dynamic { arg_count } => {
            let matches = context
                .iter()
                .filter(|row| row.site.kind == CallSiteKind::Dynamic)
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "{} should expose exactly one dynamic row; source: {}; context: {context:#?}",
                case.name,
                case.source
            );
            let row = matches[0];
            assert_eq!(
                row.site.arg_count, arg_count,
                "{} should preserve dynamic argument count; source: {}",
                case.name, case.source
            );
            row
        }
        CallSiteSelector::Method {
            name,
            arg_count,
            receiver,
        } => {
            let matches = context
                .iter()
                .filter(|row| {
                    row.site.kind == CallSiteKind::Method
                        && row.site.method.as_deref() == Some(name)
                        && db_receiver_matches(&row.site.receiver, receiver)
                })
                .collect::<Vec<_>>();
            assert_eq!(
                matches.len(),
                1,
                "{} should expose exactly one method row named {name}; source: {}; context: {context:#?}",
                case.name,
                case.source
            );
            let row = matches[0];
            assert_eq!(
                row.site.arg_count, arg_count,
                "{} should preserve method argument count; source: {}",
                case.name, case.source
            );
            row
        }
    };

    Ok(row)
}

fn db_receiver_matches(
    actual: &Option<ploke_db::CallReceiver>,
    expected: Option<CallReceiverSelector>,
) -> bool {
    match expected {
        None => true,
        Some(CallReceiverSelector::SelfField { path }) => matches!(
            actual,
            Some(ploke_db::CallReceiver::SelfField { path: actual })
                if actual.iter().map(String::as_str).eq(path.iter().copied())
        ),
        Some(CallReceiverSelector::MethodResultLocalBinding { method_name }) => matches!(
            actual,
            Some(ploke_db::CallReceiver::MethodResultLocalBinding { method_name: actual, .. })
                if actual == method_name
        ),
        Some(CallReceiverSelector::Unsupported) => {
            matches!(actual, Some(ploke_db::CallReceiver::Unsupported))
        }
    }
}
