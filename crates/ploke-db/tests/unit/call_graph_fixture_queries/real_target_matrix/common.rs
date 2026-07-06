use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use cozo::DataValue;
use ploke_db::multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE};
use ploke_test_utils::{CORPUS_AXUM_CALL_GRAPH, FixtureDb, fresh_backup_fixture_db};
use uuid::Uuid;

use super::super::*;

#[derive(Clone, Copy)]
pub(super) struct ExpectedCaller {
    pub(super) owner_name: &'static str,
    pub(super) path: &'static [&'static str],
}

#[derive(Clone, Copy)]
pub(super) struct TraversalExpectation {
    pub(super) label: &'static str,
    pub(super) owner: Uuid,
    pub(super) target: Uuid,
    pub(super) site_id: Uuid,
    pub(super) expected_edge_count: usize,
}

pub(super) fn setup_axum_call_graph_db() -> Result<Database, DbError> {
    setup_call_graph_db(&CORPUS_AXUM_CALL_GRAPH)
}

pub(super) fn setup_call_graph_db(fixture: &'static FixtureDb) -> Result<Database, DbError> {
    let db =
        fresh_backup_fixture_db(fixture).map_err(|err| DbError::QueryExecution(err.to_string()))?;
    assert!(
        db.has_call_graph_relations()?,
        "{} must be regenerated with populated call graph relations",
        fixture.id
    );
    Ok(db)
}

pub(super) fn file_path_by_suffix(db: &Database, suffix: &str) -> Result<PathBuf, DbError> {
    let suffix_lit = serde_json::to_string(suffix).unwrap_or_else(|_| "\"\"".to_string());
    let rows = db.raw_query(&format!(
        r#"?[file_path] :=
            *file_mod {{ file_path @ 'NOW' }},
            ends_with(file_path, {suffix_lit})"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one file path ending with {suffix}: {:#?}",
        rows.rows
    );
    match &rows.rows[0][0] {
        DataValue::Str(path) => Ok(PathBuf::from(path.as_str())),
        other => Err(DbError::Cozo(format!(
            "expected file_path string for {suffix}, got {other:?}"
        ))),
    }
}

pub(super) fn assert_one_edge_traversal(
    db: &Database,
    expected: TraversalExpectation,
) -> Result<(), DbError> {
    assert_eq!(
        expected.expected_edge_count, 1,
        "{} should declare the direct call-edge count explicitly",
        expected.label
    );

    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(expected.owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_matching_traversal_count(
        &outgoing,
        expected.target,
        expected.target,
        ploke_db::CallContextRelation::OutgoingTarget,
        expected.site_id,
        expected.expected_edge_count,
        expected.label,
    );
    assert_outgoing_candidate(&outgoing, expected.target, expected.site_id, expected.label);

    let incoming = db.expand_call_context(
        CallContextSeed::Target(expected.target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert_matching_traversal_count(
        &incoming,
        expected.owner,
        expected.target,
        ploke_db::CallContextRelation::IncomingCaller,
        expected.site_id,
        expected.expected_edge_count,
        expected.label,
    );
    assert_incoming_candidate(
        &incoming,
        expected.owner,
        expected.site_id,
        expected.target,
        expected.label,
    );

    let target_sites = db.call_sites_for_target(expected.target)?;
    let matching_sites = target_sites
        .iter()
        .filter(|row| row.owner_id == expected.owner && row.id == expected.site_id)
        .collect::<Vec<_>>();
    assert_eq!(
        matching_sites.len(),
        expected.expected_edge_count,
        "{} should expose the same call site through call_sites_for_target: {target_sites:#?}",
        expected.label
    );

    Ok(())
}

pub(super) fn assert_sites_match_callers(
    db: &Database,
    target: Uuid,
    callers: &[ploke_db::CallCallerRow],
    label: &str,
) -> Result<(), DbError> {
    let site_rows = db.call_sites_for_target(target)?;
    assert_eq!(
        site_rows.len(),
        callers.len(),
        "{label} should expose the same row count through call_sites_for_target and callers_for_target"
    );

    let caller_ids = callers
        .iter()
        .map(|row| row.site.id)
        .collect::<BTreeSet<_>>();
    let site_ids = site_rows.iter().map(|row| row.id).collect::<BTreeSet<_>>();
    assert_eq!(
        site_ids, caller_ids,
        "{label} should expose the same call-site ids through both target-centered query surfaces"
    );

    Ok(())
}

fn assert_matching_traversal_count(
    candidates: &[ploke_db::CallContextCandidate],
    node_id: Uuid,
    target_id: Uuid,
    relation: ploke_db::CallContextRelation,
    site_id: Uuid,
    expected_edge_count: usize,
    label: &str,
) {
    let matching = candidates
        .iter()
        .filter(|candidate| {
            candidate.node_id == node_id
                && candidate.target_id == target_id
                && candidate.relation == relation
                && candidate.call_site_id == site_id
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matching.len(),
        expected_edge_count,
        "{label} should traverse exactly {expected_edge_count} matching call edge(s): {candidates:#?}"
    );
    for candidate in matching {
        assert_eq!(
            candidate.distance, expected_edge_count as u32,
            "{label} should expose the expected direct traversal distance"
        );
    }
}

pub(super) fn assert_external_targetless(row: &ploke_db::CallContextRow) {
    assert_targetless_status(row, CallStatusKind::External);
}

pub(super) fn assert_owner_path_targetless(
    db: &Database,
    owner: Uuid,
    path_parts: &[&str],
    status: CallStatusKind,
    label: &str,
) -> Result<Uuid, DbError> {
    let sites = assert_owner_path_targetless_count(db, owner, path_parts, status, 1, label)?;
    Ok(sites[0])
}

pub(super) fn assert_owner_path_targetless_count(
    db: &Database,
    owner: Uuid,
    path_parts: &[&str],
    status: CallStatusKind,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Uuid>, DbError> {
    let context = db.call_context_for_owner(owner)?;
    let rows = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Path && row.site.path.as_ref() == Some(&path(path_parts))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        expected_count,
        "{label} should expose exactly {expected_count} targetless path row(s): {context:#?}"
    );

    let mut sites = Vec::new();
    for row in rows {
        assert_targetless_status(row, status);
        assert!(
            relations_for_site(db, row.site.id)?.rows.is_empty(),
            "{label} should not have raw call_relation targets"
        );
        sites.push(row.site.id);
    }
    assert_no_traversal_candidates_for_sites(
        db,
        &sites.iter().map(|site| (owner, *site)).collect::<Vec<_>>(),
        label,
    )?;
    Ok(sites)
}

pub(super) fn assert_owner_method_targetless(
    db: &Database,
    owner: Uuid,
    method: &str,
    receiver: &CallReceiver,
    status: CallStatusKind,
    label: &str,
) -> Result<Uuid, DbError> {
    let sites =
        assert_owner_method_targetless_count(db, owner, method, receiver, status, 1, label)?;
    Ok(sites[0])
}

pub(super) fn assert_owner_method_targetless_count(
    db: &Database,
    owner: Uuid,
    method: &str,
    receiver: &CallReceiver,
    status: CallStatusKind,
    expected_count: usize,
    label: &str,
) -> Result<Vec<Uuid>, DbError> {
    let context = db.call_context_for_owner(owner)?;
    let rows = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some(method)
                && row.site.receiver.as_ref() == Some(receiver)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.len(),
        expected_count,
        "{label} should expose exactly {expected_count} targetless method row(s): {context:#?}"
    );

    let mut sites = Vec::new();
    for row in rows {
        assert_targetless_status(row, status);
        assert!(
            relations_for_site(db, row.site.id)?.rows.is_empty(),
            "{label} should not have raw call_relation targets"
        );
        sites.push(row.site.id);
    }
    assert_no_traversal_candidates_for_sites(
        db,
        &sites.iter().map(|site| (owner, *site)).collect::<Vec<_>>(),
        label,
    )?;
    Ok(sites)
}

pub(super) fn assert_targetless_status(row: &ploke_db::CallContextRow, status: CallStatusKind) {
    assert_eq!(row.status.status, status);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "{status:?} row should not expose local traversal targets: {row:#?}"
    );
}

pub(super) fn assert_no_traversal_candidates_for_site(
    db: &Database,
    owner: Uuid,
    site_id: Uuid,
    label: &str,
) -> Result<(), DbError> {
    assert_no_traversal_candidates_for_sites(db, &[(owner, site_id)], label)
}

pub(super) fn assert_no_traversal_candidates_for_sites(
    db: &Database,
    sites: &[(Uuid, Uuid)],
    label: &str,
) -> Result<(), DbError> {
    let mut by_owner = BTreeMap::<Uuid, BTreeSet<Uuid>>::new();
    for (owner, site) in sites {
        by_owner.entry(*owner).or_default().insert(*site);
    }

    for (owner, blocked) in by_owner {
        assert_no_traversal_candidates_for_owner_sites(db, owner, &blocked, label)?;
    }

    Ok(())
}

fn assert_no_traversal_candidates_for_owner_sites(
    db: &Database,
    owner: Uuid,
    blocked: &BTreeSet<Uuid>,
    label: &str,
) -> Result<(), DbError> {
    let outgoing = db.expand_call_context(
        CallContextSeed::Owner(owner),
        CallContextOptions {
            include_incoming_callers: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        outgoing
            .iter()
            .all(|candidate| !blocked.contains(&candidate.call_site_id)),
        "{label} should have zero traversable call edges for targetless sites {blocked:#?}: {outgoing:#?}"
    );

    Ok(())
}

pub(super) fn assert_no_incoming_traversal_to_target(
    db: &Database,
    target: Uuid,
    label: &str,
) -> Result<(), DbError> {
    let incoming = db.expand_call_context(
        CallContextSeed::Target(target),
        CallContextOptions {
            include_outgoing_targets: false,
            max_candidates: 512,
            ..CallContextOptions::default()
        },
    )?;
    assert!(
        incoming.is_empty(),
        "{label} should have zero incoming traversal candidates: {incoming:#?}"
    );

    Ok(())
}

pub(super) fn method_id_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Uuid, DbError> {
    let matching = method_ids_by_name_and_body_substring(db, name, body_marker)?;
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} whose body contains {body_marker:?}"
    );
    Ok(matching[0])
}

pub(super) fn macro_id_by_name(db: &Database, name: &str) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id] :=
            *macro { id, name: $name @ 'NOW' }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one macro node named {name:?}; rows: {:#?}",
        rows.rows
    );
    to_uuid(&rows.rows[0][0])
}

pub(super) fn method_ids_by_name_and_body_substring(
    db: &Database,
    name: &str,
    body_marker: &str,
) -> Result<Vec<Uuid>, DbError> {
    let mut params = BTreeMap::new();
    params.insert("name".to_string(), DataValue::from(name));

    let rows = db.raw_query_params(
        r#"?[id, body] :=
            *method { id, name: $name, body @ 'NOW' }"#,
        params,
    )?;
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            body_key(body)
                .contains(&normalized_marker)
                .then(|| row[0].clone())
        })
        .collect::<Vec<_>>();

    matching.iter().map(to_uuid).collect()
}

pub(super) fn method_id_by_name_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Uuid, DbError> {
    let matching = method_ids_by_name_body_and_file_suffix(db, name, body_marker, file_suffix)?;
    assert_eq!(
        matching.len(),
        1,
        "expected exactly one method named {name:?} in {file_suffix:?} whose body contains {body_marker:?}"
    );

    Ok(matching[0])
}

pub(super) fn assert_no_method_owner_by_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
    label: &str,
) -> Result<(), DbError> {
    let matching = method_ids_by_name_body_and_file_suffix(db, name, body_marker, file_suffix)?;
    assert!(
        matching.is_empty(),
        "{label} should remain absent as a method owner in the current fixture: {matching:#?}"
    );

    Ok(())
}

pub(super) fn method_ids_by_name_body_and_file_suffix(
    db: &Database,
    name: &str,
    body_marker: &str,
    file_suffix: &str,
) -> Result<Vec<Uuid>, DbError> {
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
    let normalized_marker = body_key(body_marker);
    let matching = rows
        .rows
        .iter()
        .filter_map(|row| {
            let body = match &row[1] {
                DataValue::Str(body) => body.as_str(),
                _ => return None,
            };
            let file_path = data_str(&row[2], "file_path");
            (body_key(body).contains(&normalized_marker) && file_path.ends_with(file_suffix))
                .then(|| row[0].clone())
        })
        .collect::<std::collections::BTreeSet<_>>();

    matching.iter().map(to_uuid).collect()
}

fn body_key(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

pub(super) fn assert_targetless_path_rows(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "expected {expected_count} {status:?} targetless path rows for {path_parts:?}: {:#?}",
        rows.rows
    );
    let mut sites = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[2], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{path_parts:?} row should not have call_relation targets"
        );
        sites.push((owner, site_id));
    }
    assert_no_traversal_candidates_for_sites(
        db,
        &sites,
        &format!("{status:?} path rows for {path_parts:?}"),
    )?;

    Ok(())
}

pub(super) fn assert_targetless_path_owner_kind_rows(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    owner_kind: &str,
    expected_count: usize,
    label: &str,
) -> Result<Vec<(Uuid, Uuid)>, DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    params.insert("owner_kind".to_string(), DataValue::from(owner_kind));

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: $status,
                resolution_kind @ 'NOW'
            },
            *call_body_owner {
                id: owner_id,
                owner_kind: $owner_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "{label} should expose {expected_count} {status:?} {owner_kind} path row(s) for {path_parts:?}: {:#?}",
        rows.rows
    );

    let mut sites = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[2], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{label} should not have raw call_relation targets"
        );
        sites.push((owner, site_id));
    }
    assert_no_traversal_candidates_for_sites(db, &sites, label)?;

    Ok(sites)
}

pub(super) fn assert_resolved_path_target_count(
    db: &Database,
    path_parts: &[&str],
    expected_count: usize,
    label: &str,
) -> Result<Uuid, DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));

    let rows = db.raw_query_params(
        r#"?[target_id, count(site_id)] :=
            *call_site {
                id: site_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Path",
                status_kind: "Resolved",
                resolution_kind: "LocalExact" @ 'NOW'
            },
            *call_relation {
                source_id: site_id,
                source_kind: "Path",
                relation_kind: "AssociatedFunction",
                target_id,
                target_kind: "Method" @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "{label} should resolve to exactly one associated-function target: {:#?}",
        rows.rows
    );

    let DataValue::Num(cozo::Num::Int(count)) = &rows.rows[0][1] else {
        panic!(
            "{label} resolved row count should be an integer: {:#?}",
            rows.rows
        );
    };
    assert_eq!(
        *count as usize, expected_count,
        "{label} should expose exactly {expected_count} resolved path rows"
    );

    to_uuid(&rows.rows[0][0])
}

pub(super) fn assert_path_file_fanout(
    db: &Database,
    path_parts: &[&str],
    status: CallStatusKind,
    expected: &[(&str, usize)],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

    let script = format!(
        r#"
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path, site_id, owner_id, resolution_kind] :=
    *call_site {{
        id: site_id,
        owner_id,
        call_kind: "Path",
        path: $path @ 'NOW'
    }},
    *call_resolution_status {{
        source_id: site_id,
        source_kind: "Path",
        status_kind: $status,
        resolution_kind @ 'NOW'
    }},
    ancestor[owner_id, module_id],
    *module {{ id: module_id @ 'NOW' }},
    file_owner_for_module[module_id, file_id],
    *file_mod {{ owner_id: file_id, file_path @ 'NOW' }}
:sort file_path, site_id
"#
    );

    let rows = db.raw_query_params(&script, params)?;
    let expected_suffixes = expected
        .iter()
        .map(|(suffix, _)| *suffix)
        .collect::<BTreeSet<_>>();
    let mut actual = BTreeMap::<String, usize>::new();
    let mut sites = Vec::new();

    for row in &rows.rows {
        let file_path = data_str(&row[0], "file_path");
        let Some(suffix) = expected_suffixes
            .iter()
            .find(|suffix| file_path.ends_with(**suffix))
        else {
            panic!(
                "unexpected file path for {status:?} {path_parts:?} row: {file_path}; rows: {:#?}",
                rows.rows
            );
        };
        assert_eq!(row[3], DataValue::Null);

        let site_id = to_uuid(&row[1])?;
        let owner_id = to_uuid(&row[2])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{status:?} {path_parts:?} row in {suffix} should not have call_relation targets"
        );

        *actual.entry((*suffix).to_string()).or_default() += 1;
        sites.push((owner_id, site_id));
    }

    let expected = expected
        .iter()
        .map(|(suffix, count)| ((*suffix).to_string(), *count))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        actual, expected,
        "unexpected {status:?} file fanout for {path_parts:?}"
    );
    assert_no_traversal_candidates_for_sites(
        db,
        &sites,
        &format!("{status:?} file fanout rows for {path_parts:?}"),
    )?;

    Ok(())
}

pub(super) fn assert_no_path_rows(db: &Database, path_parts: &[&str]) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("path".to_string(), path_value(path_parts));

    let rows = db.raw_query_params(
        r#"?[site_id] :=
            *call_site {
                id: site_id,
                call_kind: "Path",
                path: $path @ 'NOW'
            }"#,
        params,
    )?;
    assert!(
        rows.rows.is_empty(),
        "expected no path rows for {path_parts:?}: {:#?}",
        rows.rows
    );

    Ok(())
}

pub(super) fn assert_targetless_method_rows(
    db: &Database,
    method: &str,
    receiver_kind: &str,
    receiver_path: Option<&[&str]>,
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));
    params.insert("receiver_kind".to_string(), DataValue::from(receiver_kind));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));
    params.insert(
        "receiver_path".to_string(),
        receiver_path.map_or(DataValue::Null, path_value),
    );

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Method",
                method_name: $method,
                receiver_kind: $receiver_kind,
                receiver_path: $receiver_path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "expected {expected_count} {status:?} targetless method rows for {method}.{receiver_path:?}: {:#?}",
        rows.rows
    );
    let mut sites = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[2], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{method}.{receiver_path:?} row should not have call_relation targets"
        );
        sites.push((owner, site_id));
    }
    assert_no_traversal_candidates_for_sites(
        db,
        &sites,
        &format!("{status:?} method rows for {method}.{receiver_path:?}"),
    )?;

    Ok(())
}

pub(super) fn assert_targetless_method_rows_by_name(
    db: &Database,
    method: &str,
    status: CallStatusKind,
    expected_count: usize,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));
    params.insert("status".to_string(), DataValue::from(format!("{status:?}")));

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, receiver_kind, receiver_path, resolution_kind] :=
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Method",
                method_name: $method,
                receiver_kind,
                receiver_path @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Method",
                status_kind: $status,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_count,
        "expected {expected_count} {status:?} targetless method rows for {method:?}: {:#?}",
        rows.rows
    );
    let mut sites = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[4], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "{method:?} row should not have call_relation targets"
        );
        sites.push((owner, site_id));
    }
    assert_no_traversal_candidates_for_sites(
        db,
        &sites,
        &format!("{status:?} method rows for {method:?}"),
    )?;

    Ok(())
}

pub(super) fn assert_targetless_dynamic_rows_by_method_name(
    db: &Database,
    method: &str,
    expected_arg_counts: &[u32],
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[site_id, owner_id, arg_count, status_kind, resolution_kind] :=
            *method { id: owner_id, name: $method @ 'NOW' },
            *call_site {
                id: site_id,
                owner_id,
                call_kind: "Dynamic",
                arg_count @ 'NOW'
            },
            *call_resolution_status {
                source_id: site_id,
                source_kind: "Dynamic",
                status_kind,
                resolution_kind @ 'NOW'
            }"#,
        params,
    )?;
    assert_eq!(
        rows.rows.len(),
        expected_arg_counts.len(),
        "expected {} targetless dynamic rows owned by methods named {method:?}: {:#?}",
        expected_arg_counts.len(),
        rows.rows
    );

    let mut actual_arg_counts = Vec::new();
    for row in &rows.rows {
        assert_eq!(row[3], DataValue::from("Unsupported"));
        assert_eq!(row[4], DataValue::Null);
        let site_id = to_uuid(&row[0])?;
        let owner_id = to_uuid(&row[1])?;
        assert!(
            relations_for_site(db, site_id)?.rows.is_empty(),
            "dynamic row owned by {method:?} should not have call_relation targets"
        );
        assert_no_traversal_candidates_for_sites(db, &[(owner_id, site_id)], method)?;
        let DataValue::Num(cozo::Num::Int(arg_count)) = &row[2] else {
            panic!("dynamic row arg_count should be numeric: {row:#?}");
        };
        actual_arg_counts.push(*arg_count as u32);
    }
    actual_arg_counts.sort_unstable();

    let mut expected = expected_arg_counts.to_vec();
    expected.sort_unstable();
    assert_eq!(
        actual_arg_counts, expected,
        "unexpected dynamic arg counts for methods named {method:?}"
    );

    Ok(())
}

pub(super) fn assert_no_dynamic_rows_by_method_name(
    db: &Database,
    method: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("method".to_string(), DataValue::from(method));

    let rows = db.raw_query_params(
        r#"?[site_id] :=
            *method { id: owner_id, name: $method @ 'NOW' },
            *call_site { id: site_id, owner_id, call_kind: "Dynamic" }"#,
        params,
    )?;
    assert!(
        rows.rows.is_empty(),
        "expected no dynamic rows owned by methods named {method:?}: {:#?}",
        rows.rows
    );

    Ok(())
}

pub(super) fn assert_no_dynamic_rows_by_function_names(
    db: &Database,
    function_names: &[&str],
) -> Result<(), DbError> {
    for function_name in function_names {
        let mut params = BTreeMap::new();
        params.insert("function".to_string(), DataValue::from(*function_name));

        let rows = db.raw_query_params(
            r#"?[site_id] :=
                *function { id: owner_id, name: $function @ 'NOW' },
                *call_site { id: site_id, owner_id, call_kind: "Dynamic" }"#,
            params,
        )?;
        assert!(
            rows.rows.is_empty(),
            "expected no dynamic rows owned by functions named {function_name:?}: {:#?}",
            rows.rows
        );
    }

    Ok(())
}

fn path_value(path_parts: &[&str]) -> DataValue {
    DataValue::List(
        path_parts
            .iter()
            .map(|part| DataValue::from(*part))
            .collect(),
    )
}
