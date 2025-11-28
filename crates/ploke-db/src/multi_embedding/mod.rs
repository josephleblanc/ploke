pub mod db_ext;
pub mod schema;
pub mod hnsw_ext;

#[cfg(test)]
mod test_utils {
    use std::collections::BTreeMap;

    use cozo::{MemStorage, ScriptMutability};

    use crate::DbError;
    use ploke_error::Error;

    pub(crate) fn setup_db() -> Result<cozo::Db<MemStorage>, Error> {
        ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
    }

    pub(crate) fn setup_empty_db() -> Result<cozo::Db<MemStorage>, Error> {
        let empty_db = cozo::Db::new(MemStorage::default()).map_err(DbError::from)?;
        empty_db.initialize().map_err(DbError::from)?;
        Ok( empty_db )
    }

    pub(crate) fn eprint_relations(fixture_db: &cozo::Db<MemStorage>) -> Result<(), Error> {
        let script = "::relations";
        let list_relations = fixture_db
            .run_script(script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        for rel in list_relations {
            eprintln!("{rel:?}");
        }
        Ok(())
    }
}
