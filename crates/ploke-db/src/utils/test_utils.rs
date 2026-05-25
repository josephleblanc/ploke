#![cfg(test)]
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use ploke_error::Error;

use crate::{Database, DbError, create_index_primary};
use ploke_test_utils::FIXTURE_NODES_CANONICAL;

lazy_static! {
    pub static ref TEST_DB_NODES: Result<Arc<Mutex<Database>>, Error> = {
        let db = Database::init_with_schema()?;

        let target_file = FIXTURE_NODES_CANONICAL.checked_path()?.into_path();
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.prior_rels_for_plain_backup_import()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;
        db.ensure_compilation_unit_relations()
            .map_err(ploke_error::Error::from)?;
        create_index_primary(&db)?;
        Ok(Arc::new(Mutex::new(db)))
    };
}
