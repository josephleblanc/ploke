#![cfg(test)]
use std::sync::{Arc, Mutex};

use lazy_static::lazy_static;
use ploke_error::Error;

use crate::{create_index_primary, Database, DbError};
use ploke_test_utils::workspace_root;

lazy_static! {
    pub static ref TEST_DB_NODES: Result<Arc<Mutex<Database>>, Error> = {
        let db = Database::init_with_schema()?;

        let mut target_file = workspace_root();
        target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;
        create_index_primary(&db)?;
        Ok(Arc::new(Mutex::new(db)))
    };
}
