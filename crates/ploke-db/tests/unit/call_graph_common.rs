use std::collections::BTreeMap;

use cozo::DataValue;
use ploke_db::{Database, DbError};
use uuid::Uuid;

mod facts;
mod resolved;
mod site;
mod source;
mod targetless;
mod targets;
mod values;

pub(super) use facts::{fact_count, fact_for_call_site};
pub(super) use resolved::{ResolvedGraphSeed, insert_resolved_graph};
pub(super) use site::{SiteSeed, insert_call_site, insert_call_site_raw_receiver};
pub(super) use source::insert_owner_source;
pub(super) use targetless::{TargetlessStatusSeed, insert_targetless_status};
pub(super) use values::list;

use targets::{ensure_call_target, ensure_function_owner};
use values::{option_str, uuid};

pub(super) fn insert_edge(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    kind: &str,
) -> Result<(), DbError> {
    insert_edge_with_source_kind(db, owner, site, "Function", kind)
}

pub(super) fn insert_edge_with_source_kind(
    db: &Database,
    owner: Uuid,
    site: Uuid,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    ensure_function_owner(db, owner)?;

    let mut params = BTreeMap::new();
    params.insert("owner_id".to_string(), uuid(owner));
    params.insert("site_id".to_string(), uuid(site));
    params.insert("source_kind".to_string(), DataValue::from(source_kind));
    params.insert("target_kind".to_string(), DataValue::from(target_kind));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $owner_id,
            target_id = $site_id,
            relation_kind = "BodyContainsCall",
            source_kind = $source_kind,
            target_kind = $target_kind,
            at = 'ASSERT'
        :put call_site_edge { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

pub(super) fn insert_relation(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    ensure_call_target(db, target, target_kind)?;
    insert_relation_raw(db, site, target, relation, source_kind, target_kind)
}

pub(super) fn insert_relation_raw(
    db: &Database,
    site: Uuid,
    target: Uuid,
    relation: &str,
    source_kind: &str,
    target_kind: &str,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("target_id".to_string(), uuid(target));
    params.insert("relation_kind".to_string(), DataValue::from(relation));
    params.insert("source_kind".to_string(), DataValue::from(source_kind));
    params.insert("target_kind".to_string(), DataValue::from(target_kind));

    db.raw_query_mut_params(
        r#"?[source_id, target_id, at, relation_kind, source_kind, target_kind] :=
            source_id = $site_id,
            target_id = $target_id,
            relation_kind = $relation_kind,
            source_kind = $source_kind,
            target_kind = $target_kind,
            at = 'ASSERT'
        :put call_relation { source_id, target_id, at => relation_kind, source_kind, target_kind }"#,
        params,
    )?;
    Ok(())
}

pub(super) fn insert_status(
    db: &Database,
    site: Uuid,
    kind: &str,
    status: &str,
    resolution: Option<&str>,
) -> Result<(), DbError> {
    let mut params = BTreeMap::new();
    params.insert("site_id".to_string(), uuid(site));
    params.insert("source_kind".to_string(), DataValue::from(kind));
    params.insert("status_kind".to_string(), DataValue::from(status));
    params.insert("resolution_kind".to_string(), option_str(resolution));

    db.raw_query_mut_params(
        r#"?[source_id, at, source_kind, status_kind, resolution_kind] :=
            source_id = $site_id,
            source_kind = $source_kind,
            status_kind = $status_kind,
            resolution_kind = $resolution_kind,
            at = 'ASSERT'
        :put call_resolution_status { source_id, at => source_kind, status_kind, resolution_kind }"#,
        params,
    )?;
    Ok(())
}
