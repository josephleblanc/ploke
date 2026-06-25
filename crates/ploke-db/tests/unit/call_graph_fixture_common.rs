use std::collections::BTreeMap;

use cozo::{DataValue, Db, MemStorage, UuidWrapper};
use ploke_db::{
    CallCallerRow, CallContextCandidate, CallContextRelation, CallContextRow, CallReceiver,
    CallRelationKind, CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database,
    DbError, ProofGraphContextRow, ProofGraphStore, QueryResult, to_uuid,
};
use ploke_transform::{schema::create_schema_all, transform::transform_parsed_graph};
use uuid::Uuid;

mod constructor;
mod dynamic;
mod lookup;
mod proof;
mod rows;
mod selectors;

pub(super) use constructor::*;
pub(super) use dynamic::*;
pub(super) use lookup::*;
pub(super) use proof::*;
pub(super) use rows::*;
pub(super) use selectors::*;

pub(super) fn setup_call_graph_fixture_db(fixture: &'static str) -> Result<Database, DbError> {
    let db = Db::new(MemStorage::default()).expect("in-memory cozo db");
    db.initialize().expect("initialize cozo db");
    create_schema_all(&db).map_err(|err| DbError::QueryExecution(err.to_string()))?;

    let mut merged = syn_parser::parser::ParsedCodeGraph::merge_new(
        ploke_test_utils::test_run_phases_and_collect(fixture),
    )
    .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    let tree = merged
        .build_tree_and_prune()
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;
    transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| DbError::QueryExecution(err.to_string()))?;

    Ok(Database::new(db))
}
