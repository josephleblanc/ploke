use super::super::super::super::*;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};

pub(super) const AXUM_DOMAIN: &str = "bd:corpus-axum-call-graph";
pub(super) const MEMCHR_DOMAIN: &str = "bd:corpus-memchr-call-graph";

pub(super) struct ProofCase {
    pub(super) label: &'static str,
    pub(super) target: Uuid,
    pub(super) edges: usize,
}

pub(super) struct ProjectedCase {
    pub(super) case: ProofCase,
    pub(super) callers: Vec<ploke_db::CallCallerRow>,
}

pub(super) fn axum_db() -> Result<Arc<Database>, Error> {
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_AXUM_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_axum_call_graph must include call graph relations for proof-context tests"
    );
    Ok(db)
}

pub(super) fn memchr_db() -> Result<Arc<Database>, Error> {
    let db = Arc::new(fresh_backup_fixture_db(
        &ploke_test_utils::CORPUS_MEMCHR_CALL_GRAPH,
    )?);
    assert!(
        db.has_call_graph_relations()?,
        "corpus_memchr_call_graph must include call graph relations for proof-context tests"
    );
    Ok(db)
}

pub(super) fn project_case(db: &Database, case: ProofCase) -> Result<ProjectedCase, Error> {
    let callers = db.callers_for_target(case.target)?;
    assert_eq!(
        callers.len(),
        case.edges,
        "{} should expose the expected DB caller count: {callers:#?}",
        case.label
    );

    let projected = db.project_call_proof_facts_for_target(case.target, AXUM_DOMAIN)?;
    assert_eq!(
        projected,
        case.edges * 3,
        "{} should project call_site/call_edge/call_resolution facts for every caller",
        case.label
    );

    Ok(ProjectedCase { case, callers })
}

pub(super) fn assert_case_rows(rag: &RagService, projected: &ProjectedCase) -> Result<(), Error> {
    let case = &projected.case;
    let rows = rag.exact_proof_context(case.target)?;
    for caller in &projected.callers {
        assert_resolved_site(&rows, caller, case.target, case.label);
    }

    Ok(())
}

fn assert_resolved_site(
    rows: &[ProofContextInfo],
    caller: &ploke_db::CallCallerRow,
    target: Uuid,
    label: &str,
) {
    assert_eq!(
        caller.target.target_id, target,
        "{label} DB caller row should point at the expected target"
    );
    let owner = caller.site.owner_id.to_string();
    let site = caller.site.id.to_string();
    let target = target.to_string();

    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.build_domain_id.as_deref() == Some(AXUM_DOMAIN)
        }),
        "{label} should include call_site proof row for site {site}: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.callee_def_id.as_deref() == Some(target.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
        }),
        "{label} should include resolved call_edge proof row for site {site}: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site.as_str())
                && row.resolution_state.as_deref() == Some("resolved")
                && row.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "{label} should include resolved call_resolution proof row for site {site}: {rows:#?}"
    );
}

pub(super) fn function_id(db: &Database, module_path: &[&str], name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));
    params.insert(
        "path".to_string(),
        DataValue::List(
            module_path
                .iter()
                .map(|part| DataValue::from(*part))
                .collect(),
        ),
    );

    let rows = db.raw_query_params(
        r#"?[id] :=
            *function { id, name: $name, module_id @ 'NOW' },
            *module { id: module_id, path: $path @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one function named {name:?} in module {module_path:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

pub(super) fn method_id_by_name_and_body(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body).contains(&marker).then(|| row[0].clone())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0]).map_err(Error::from)
}

pub(super) fn method_id_by_file(
    db: &Database,
    name: &str,
    body_marker: &str,
    file: &str,
) -> Result<Uuid, Error> {
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

?[id, body, file_path] :=
    *method {{ id, name: $name, body @ 'NOW' }},
    ancestor[id, mod_id],
    *module{{ id: mod_id @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );
    let rows = db.raw_query_params(&script, params)?;
    let marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter(|row| {
            let DataValue::Str(body) = &row[1] else {
                return false;
            };
            let DataValue::Str(file_path) = &row[2] else {
                return false;
            };
            body_key(body).contains(&marker) && file_path.ends_with(file)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} in {file:?} whose body contains {body_marker:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&matching[0][0]).map_err(Error::from)
}

pub(super) fn trait_method_id(
    db: &Database,
    trait_name: &str,
    method: &str,
) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("trait_name".to_string(), DataValue::from(trait_name));
    params.insert("method_name".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *trait { id: trait_id, name: $trait_name @ 'NOW' },
            *method { id, name: $method_name, owner_id: trait_id @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one trait method {trait_name}::{method}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

pub(super) fn struct_id(db: &Database, name: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *struct { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one struct named {name:?}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

pub(super) fn variant_id(db: &Database, enum_name: &str, variant: &str) -> Result<Uuid, Error> {
    let mut params = BTreeMap::new();
    params.insert("enum_name".to_string(), DataValue::from(enum_name));
    params.insert("variant_name".to_string(), DataValue::from(variant));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *enum { id: enum_id, name: $enum_name @ 'NOW' },
            *variant { id, name: $variant_name, owner_id: enum_id @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one enum variant {enum_name}::{variant}; rows: {:#?}",
        rows.rows
    );

    to_uuid(&rows.rows[0][0]).map_err(Error::from)
}

pub(super) fn conn_limiter_accept_owner(db: &Database) -> Result<Uuid, Error> {
    method_id_by_name_and_body(
        db,
        "accept",
        "self.sem.clone().acquire_owned().await.unwrap()",
    )
}

pub(super) fn await_result_unwrap_site(calls: &[CallContextInfo], owner: Uuid) -> Uuid {
    let callee = CallCalleeInfo::Method {
        name: "unwrap".to_string(),
        receiver: Some(CallReceiverInfo::AwaitMethodCallResult {
            method_name: "acquire_owned".to_string(),
        }),
    };
    targetless_method_site(calls, owner, &callee, "AwaitMethodCallResult unwrap")
}

pub(super) fn targetless_method_site(
    calls: &[CallContextInfo],
    owner: Uuid,
    callee: &CallCalleeInfo,
    label: &str,
) -> Uuid {
    targetless_method_site_with_status(calls, owner, callee, CallStatusKind::Unsupported, label)
}

pub(super) fn targetless_method_site_with_status(
    calls: &[CallContextInfo],
    owner: Uuid,
    callee: &CallCalleeInfo,
    status: CallStatusKind,
    label: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Method
                && &call.callee == callee
                && call.status == status
                && call.resolution.is_none()
                && call.targets.is_empty()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "expected one targetless {label} call row: {calls:#?}"
    );
    matching[0].site_id
}

pub(super) fn targetless_path_site(
    calls: &[CallContextInfo],
    owner: Uuid,
    path: &[&str],
    status: CallStatusKind,
    label: &str,
) -> Uuid {
    let callee = CallCalleeInfo::Path {
        path: path.iter().map(|segment| (*segment).to_string()).collect(),
    };
    let matching = calls
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Path
                && call.callee == callee
                && call.status == status
                && call.resolution.is_none()
                && call.targets.is_empty()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{label} should expose one targetless path call row: {calls:#?}"
    );
    matching[0].site_id
}

pub(super) fn dynamic_site(
    calls: &[CallContextInfo],
    owner: Uuid,
    expected_path: &[&str],
    label: &str,
) -> Uuid {
    let matching = calls
        .iter()
        .filter(|call| {
            call.owner_id == owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call.status == CallStatusKind::Unsupported
                && call.resolution.is_none()
                && call.targets.is_empty()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        1,
        "{label} should expose one targetless dynamic call row: {calls:#?}"
    );
    let call = matching[0];
    assert!(
        call.path.as_ref().is_some_and(|path| path
            .iter()
            .map(String::as_str)
            .eq(expected_path.iter().copied())),
        "{label} should preserve the dynamic callee path: {call:#?}"
    );
    call.site_id
}

pub(super) fn assert_site_blocker(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    reason: &str,
    label: &str,
) {
    assert_site_blocker_in_domain(rows, owner, site_id, AXUM_DOMAIN, reason, label);
}

pub(super) fn assert_site_blocker_in_domain(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    domain: &str,
    reason: &str,
    label: &str,
) {
    assert_site_resolution_blocker_in_domain(
        rows, owner, site_id, domain, "blocked", reason, label,
    );
}

pub(super) fn assert_site_resolution_blocker(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    state: &str,
    reason: &str,
    label: &str,
) {
    assert_site_resolution_blocker_in_domain(
        rows,
        owner,
        site_id,
        AXUM_DOMAIN,
        state,
        reason,
        label,
    );
}

pub(super) fn assert_site_resolution_blocker_in_domain(
    rows: &[ProofContextInfo],
    owner: Uuid,
    site_id: Uuid,
    domain: &str,
    state: &str,
    reason: &str,
    label: &str,
) {
    let owner = owner.to_string();
    let site_id = site_id.to_string();
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.caller_def_id.as_deref() == Some(owner.as_str())
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.build_domain_id.as_deref() == Some(domain)
        }),
        "{label} proof context should include the blocked call_site fact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(site_id.as_str())
                && row.resolution_state.as_deref() == Some(state)
                && row.blocker_reason.as_deref() == Some(reason)
        }),
        "{label} proof context should include the {state} call_resolution fact: {rows:#?}"
    );
    assert!(
        rows.iter().all(|row| {
            row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site_id.as_str())
        }),
        "{label} proof context should not fabricate a call_edge for targetless site {site_id}: {rows:#?}"
    );
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}
