#![cfg(test)]
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use lazy_static::lazy_static;
use ploke_error::Error;

use crate::{create_index_primary, Database, DbError};
use ploke_test_utils::{
    workspace_root, LEGACY_FIXTURE_BACKUP_REL_PATH, MULTI_EMBED_FIXTURE_BACKUP_REL_PATH,
};

#[cfg(feature = "multi_embedding")]
const FIXTURE_DB_RELPATH: &str = MULTI_EMBED_FIXTURE_BACKUP_REL_PATH;
#[cfg(not(feature = "multi_embedding_schema"))]
const FIXTURE_DB_RELPATH: &str = LEGACY_FIXTURE_BACKUP_REL_PATH;

pub(crate) fn fixture_db_backup_rel_path() -> &'static str {
    FIXTURE_DB_RELPATH
}

pub(crate) fn fixture_db_backup_path() -> PathBuf {
    let mut path = workspace_root();
    path.push(fixture_db_backup_rel_path());
    path
}

lazy_static! {
    pub static ref TEST_DB_NODES: Result<Arc<Mutex<Database>>, Error> = {
        let db = Database::init_with_schema()?;

        let target_file = fixture_db_backup_path();
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;
        create_index_primary(&db)?;
        Ok(Arc::new(Mutex::new(db)))
    };
}
