
use std::sync::Arc;
use ploke_db::Database;
use cozo::{Db, MemStorage, ScriptMutability};
use cozo::DataValue;
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource};
use ploke_embed::error::EmbedError;
use std::future::Future;
use std::pin::Pin;

pub fn create_mock_db(num_unindexed: usize) -> Arc<Database> {
    let storage = MemStorage::default();
    let db = Arc::new(Database::new(Db::new(storage).unwrap()));
    
    let script = r#"
    ?[id, path, tracking_hash, start_byte, end_byte] <- [
        $unindexed,
    ]

    :create embedding_nodes {
        id => Uuid
    }
    "#;
    
    todo!("define and insert params, ensure db.run_script works correctly");
    
    // db.run_script(script, params, ScriptMutability::Mutable).unwrap();
    db
}



#[derive(Debug, PartialEq, Eq)]
pub enum MockBehavior {
    Normal,
    RateLimited,
    DimensionMismatch,
    NetworkError,
}

pub struct MockEmbedder {
    pub dimensions: usize,
    pub behavior: MockBehavior,
}
