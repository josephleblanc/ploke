use super::*;

pub(super) const UNKNOWN_PATH: &[&str] = &["unknown"];
pub(super) const CRATE_TARGET_PATH: &[&str] = &["crate", "target"];
const VALUE_RECEIVER: (&str, &[&str]) = ("LocalBinding", &["value"]);

#[derive(Clone, Copy)]
pub(super) struct LocalTargetRow<'a> {
    pub(super) owner: Uuid,
    module: Uuid,
    site: Uuid,
    target: Uuid,
    file: &'a str,
    kind: &'a str,
    span: (i64, i64),
    path: Option<&'a [&'a str]>,
    method: Option<&'a str>,
    receiver: Option<(&'a str, &'a [&'a str])>,
    status: &'a str,
    resolution: Option<&'a str>,
}

pub(super) fn path_row(
    base: u128,
    target: u128,
    file: &'static str,
    span: (i64, i64),
    path: &'static [&'static str],
    status: &'static str,
    resolution: Option<&'static str>,
) -> LocalTargetRow<'static> {
    LocalTargetRow {
        owner: Uuid::from_u128(base),
        module: Uuid::from_u128(base + 1),
        site: Uuid::from_u128(base + 2),
        target: Uuid::from_u128(target),
        file,
        kind: "Path",
        span,
        path: Some(path),
        method: None,
        receiver: None,
        status,
        resolution,
    }
}

pub(super) fn method_row(
    base: u128,
    target: u128,
    file: &'static str,
    span: (i64, i64),
) -> LocalTargetRow<'static> {
    LocalTargetRow {
        owner: Uuid::from_u128(base),
        module: Uuid::from_u128(base + 1),
        site: Uuid::from_u128(base + 2),
        target: Uuid::from_u128(target),
        file,
        kind: "Method",
        span,
        path: None,
        method: Some("overlap"),
        receiver: Some(VALUE_RECEIVER),
        status: "Ambiguous",
        resolution: None,
    }
}

pub(super) fn assert_owner_projection_rejects(row: LocalTargetRow<'_>) -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    insert_local_target_row(&db, row)?;

    let error = db
        .project_call_proof_facts_for_owner(row.owner, "bd:test")
        .expect_err("owner projection must reject non-resolved local targets");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(db.proof_graphrag_context("")?.is_empty());

    Ok(())
}

pub(super) fn assert_target_projection_rejects(
    target: Uuid,
    rows: &[LocalTargetRow<'_>],
) -> Result<(), DbError> {
    let db =
        Database::init_with_schema().map_err(|err| DbError::QueryExecution(err.to_string()))?;
    for row in rows {
        insert_local_target_row(&db, *row)?;
    }

    let callers = db.callers_for_target(target)?;
    assert_eq!(callers.len(), rows.len(), "target callers: {callers:#?}");

    let error = db
        .project_call_proof_facts_for_target(target, "bd:test")
        .expect_err("target-centered projection must reject non-resolved local targets");
    assert!(
        error.to_string().contains("non-resolved call site"),
        "unexpected error: {error}"
    );
    assert!(
        db.proof_graphrag_context("")?.is_empty(),
        "target-centered projection must not store partial proof facts after rejection"
    );

    Ok(())
}

fn insert_local_target_row(db: &Database, row: LocalTargetRow<'_>) -> Result<(), DbError> {
    let (relation, target_kind) = match row.kind {
        "Path" => ("Function", "Function"),
        "Method" => ("Method", "Method"),
        other => panic!("unexpected local target row kind {other}"),
    };

    insert_owner_source(db, row.owner, row.module, row.file)?;
    insert_call_site(
        db,
        SiteSeed {
            id: row.site,
            owner: row.owner,
            kind: row.kind,
            span: row.span,
            path: row.path.map(|path| path.to_vec()),
            method: row.method,
            macro_name: None,
            receiver: row.receiver.map(|(kind, path)| (kind, path.to_vec())),
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(db, row.owner, row.site, row.kind)?;
    insert_relation(db, row.site, row.target, relation, row.kind, target_kind)?;
    insert_status(db, row.site, row.kind, row.status, row.resolution)?;
    Ok(())
}
