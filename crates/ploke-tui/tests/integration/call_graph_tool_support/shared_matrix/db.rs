use ploke_db::CallSiteKind as DbCallSiteKind;
use ploke_test_utils::{
    CallExpected, CallOwnerSelector, CallReceiverSelector, CallShapeCase, CallSiteSelector,
    CallTargetSelector,
};
use std::collections::BTreeSet;

use super::*;

pub(super) fn resolve_owner(db: &Database, case: &CallShapeCase) -> TargetInfo {
    match case.owner {
        CallOwnerSelector::FunctionInModule { module_path, name } => {
            function_by_name_in_module(db, module_path, name, case.name)
        }
        CallOwnerSelector::MethodByBody { name, body, .. } => {
            method_by_name_and_body(db, name, body, None, case.name)
        }
        CallOwnerSelector::MethodByBodyFile {
            name,
            body,
            file_suffix,
        } => method_by_name_and_body(db, name, body, Some(file_suffix), case.name),
    }
}

pub(super) fn resolve_target(db: &Database, target: CallTargetSelector) -> TargetInfo {
    match target {
        CallTargetSelector::FunctionByName { name } => function_by_name(db, name),
        CallTargetSelector::FunctionInModule { module_path, name } => {
            function_by_name_in_module(db, module_path, name, name)
        }
        CallTargetSelector::MethodByBody { name, body, .. } => {
            method_by_name_and_body(db, name, body, None, name)
        }
        CallTargetSelector::Struct { name } => struct_by_name(db, name),
        CallTargetSelector::Variant {
            enum_name,
            variant_name,
        } => variant_by_enum_and_name(db, enum_name, variant_name),
    }
}

pub(super) fn select_site<'a>(
    context: &'a [ploke_db::CallContextRow],
    case: &CallShapeCase,
) -> &'a ploke_db::CallContextRow {
    let matches = context
        .iter()
        .filter(|row| db_site_matches(row, case.site))
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{} should expose exactly one selected shared-matrix call-site row; source: {}; context: {context:#?}",
        case.name,
        case.source
    );
    matches[0]
}

pub(super) fn assert_db_expectation(
    row: &ploke_db::CallContextRow,
    case: &CallShapeCase,
    target: Option<Uuid>,
) {
    match case.expected {
        CallExpected::Resolved {
            relation,
            target_kind,
            edge_count,
            ..
        } => {
            let target = target.unwrap_or_else(|| panic!("{} expected target", case.name));
            assert_eq!(
                row.status.status,
                DbCallStatusKind::Resolved,
                "{} should be resolved in DB context",
                case.name
            );
            assert_eq!(
                row.targets.len(),
                edge_count,
                "{} should preserve expected DB edge count",
                case.name
            );
            assert!(
                row.targets.iter().any(|candidate| {
                    candidate.target_id == target
                        && candidate.relation == relation
                        && candidate.target_kind == target_kind
                }),
                "{} should preserve expected DB target edge: {row:#?}",
                case.name
            );
        }
        CallExpected::AmbiguousCandidates {
            relation,
            target_kind,
            ..
        } => {
            assert_eq!(
                row.status.status,
                DbCallStatusKind::Ambiguous,
                "{} should preserve candidate-only ambiguity in DB context",
                case.name
            );
            assert_eq!(
                row.status.resolution, None,
                "{} should not promote ambiguous candidates to exact resolution",
                case.name
            );
            assert_eq!(
                row.targets.len(),
                case_candidate_count(case),
                "{} should preserve expected DB candidate count",
                case.name
            );
            assert!(
                row.targets.iter().all(|candidate| {
                    candidate.relation == relation && candidate.target_kind == target_kind
                }),
                "{} should preserve relation/kind on all candidate rows: {row:#?}",
                case.name
            );
        }
        CallExpected::Targetless { status } => {
            assert_eq!(row.status.status, status);
            assert!(
                row.targets.is_empty(),
                "{} should remain targetless in DB context: {row:#?}",
                case.name
            );
        }
    }
}

pub(super) fn assert_candidate_ids(
    row: &ploke_db::CallContextRow,
    candidates: &[Uuid],
    case: &CallShapeCase,
    tool: &str,
) {
    let actual = row
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<BTreeSet<_>>();
    let expected = candidates.iter().copied().collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "{tool} should preserve the expected ambiguous candidate set for {}",
        case.name
    );
}

fn case_candidate_count(case: &CallShapeCase) -> usize {
    match case.expected {
        CallExpected::AmbiguousCandidates { candidates, .. } => candidates.len(),
        _ => 0,
    }
}

fn function_by_name(db: &Database, name: &str) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *function {{ id, name: $name, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query function {name}: {err}")),
        name,
    )
}

fn db_site_matches(row: &ploke_db::CallContextRow, site: CallSiteSelector) -> bool {
    match site {
        CallSiteSelector::Path {
            segments,
            arg_count,
        } => {
            row.site.kind == DbCallSiteKind::Path
                && row.site.arg_count == arg_count
                && row.site.path.as_ref().is_some_and(|path| {
                    path.iter().map(String::as_str).eq(segments.iter().copied())
                })
        }
        CallSiteSelector::Dynamic { arg_count } => {
            row.site.kind == DbCallSiteKind::Dynamic && row.site.arg_count == arg_count
        }
        CallSiteSelector::Method {
            name,
            arg_count,
            receiver,
        } => {
            row.site.kind == DbCallSiteKind::Method
                && row.site.arg_count == arg_count
                && row.site.method.as_deref() == Some(name)
                && db_receiver_matches(&row.site.receiver, receiver)
        }
    }
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

fn function_by_name_in_module(
    db: &Database,
    module_path: &[&str],
    name: &str,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert(
        "module_path".to_string(),
        DataValue::List(
            module_path
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *function {{ id, name: $name, module_id @ 'NOW' }},
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    mod_path == $module_path,
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query function {label}: {err}")),
        label,
    )
}

fn method_by_name_and_body(
    db: &Database,
    name: &str,
    body: &str,
    file_suffix: Option<&str>,
    label: &str,
) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, body, file_path, mod_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db
        .raw_query_params(&script, params)
        .unwrap_or_else(|err| panic!("query method {label}: {err}"));
    let marker = body_key(body);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            body_key(body).contains(&marker)
                && file_suffix.is_none_or(|suffix| data_str(&row[2], "file_path").ends_with(suffix))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method owner for {label}; rows: {:#?}",
        rows.rows
    );
    let row = matching[0];
    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[2], "file_path")),
        module_path: data_path(&row[3], "module path"),
    }
}

fn variant_by_enum_and_name(db: &Database, enum_name: &str, variant_name: &str) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant_name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *enum {{ id: enum_id, name: $enum_name @ 'NOW' }},
    *variant {{ id, name: $variant_name, owner_id: enum_id @ 'NOW' }},
    ancestor[enum_id, module_id],
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query variant {enum_name}::{variant_name}: {err}")),
        variant_name,
    )
}

fn struct_by_name(db: &Database, name: &str) -> TargetInfo {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[id, file_path, mod_path] :=
    *struct {{ id, name: $name @ 'NOW' }},
    ancestor[id, module_id],
    *module{{ id: module_id, path: mod_path @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    one_target_info(
        db.raw_query_params(&script, params)
            .unwrap_or_else(|err| panic!("query struct {name}: {err}")),
        name,
    )
}

fn one_target_info(rows: ploke_db::QueryResult, label: &str) -> TargetInfo {
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one target for {label}; rows: {:#?}",
        rows.rows
    );
    let row = &rows.rows[0];
    TargetInfo {
        id: to_uuid(&row[0]).unwrap_or_else(|err| panic!("{label} uuid: {err}")),
        file_path: PathBuf::from(data_str(&row[1], "file_path")),
        module_path: data_path(&row[2], "module path"),
    }
}
