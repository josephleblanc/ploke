use std::collections::BTreeMap;

use crate::embedding::EmbeddingNode;
use crate::error::DbError;
use crate::NodeType;
use crate::QueryResult;
use cozo::DataValue;
use cozo::Db;
use cozo::MemStorage;
use cozo::UuidWrapper;

/// Main database connection and query interface
#[derive(Debug)]
pub struct Database {
    db: Db<MemStorage>,
}

impl std::ops::Deref for Database {
    type Target = Db<MemStorage>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

impl Database {
    /// Create new database connection
    pub fn new(db: Db<MemStorage>) -> Self {
        Self { db }
    }

    /// Execute a raw CozoScript query
    pub fn raw_query(&self, script: &str) -> Result<QueryResult, DbError> {
        let result = self
            .db
            .run_script(
                script,
                std::collections::BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }

    // /// Create a new query builder
    // pub fn create_query_builder(&self) -> QueryBuilder {
    //     QueryBuilder::new(&self.db)
    // }

    pub async fn mock_get_nodes_for_embedding(&self) -> Result<Vec<EmbeddingNode>, DbError> {
        // TODO: The CozoScript query needs to be validated and might require adjustments
        // based on the final schema. For now, we'll return mock data.
        let mock_nodes = vec![
            // Example node. In a real scenario, this would come from the database.
            // EmbeddingNode {
            //     id: Uuid::new_v4(),
            //     path: PathBuf::from("/path/to/your/file.rs"),
            //     content_hash: 123456789,
            //     start_byte: 100,
            //     end_byte: 500,
            // },
        ];
        Ok(mock_nodes)
    }

    pub async fn index_embeddings(
        &mut self,
        node_type: NodeType,
        dim: usize,
    ) -> Result<(), DbError> {
        // TODO: This is so dirty.
        // I know there is a better way to do this with cozo using the BTreeMap approach.
        // Figure it out.
        let dim_string = dim.to_string();
        let embedding_query: [&str; 8] = [
            r#"::hnsw create"#,
            node_type.relation_str(),
            r#":embedding_idx { "#,
            dim_string.as_str(),
            r#": "#,
            r#"fields: [embedding"#,
            // embedding
            r#"],
        distance: Cosine,
        filter: !is_null(embedding
        "#,
            // embedding
            r#"
        )
}
"#,
        ];
        let query_string = embedding_query.concat();
        self.run_script(
            query_string.as_str(),
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )
        .map_err(|e| DbError::Cozo(e.to_string()))?;

        Ok(())
    }
    pub async fn update_embeddings_batch(
        &self,
        // TODO: Update with a better type maybe? Might want to parse the nodes into typed IDs
        // earlier in the process, actually.
        updates: Vec<(uuid::Uuid, Vec<f32>)>,
    ) -> Result<(), DbError> {
        // Placeholder implementation
        // TODO: Add actual CozoDB update logic, use the batch inserts if possible.
        Ok(())
    }

    /// Fetches all primary nodes that do not yet have an embedding.
    ///
    /// This query retrieves the necessary information to fetch the node's content
    /// and later associate the generated embedding with the correct node.
    // In your impl Database block
    pub fn get_nodes_for_embedding(
        &self,
        limit: usize,
        cursor: Option<uuid::Uuid>,
    ) -> Result<Vec<EmbeddingNode>, ploke_error::Error> {
        let script = r#"
        parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

        ancestor[desc, asc] := parent_of[desc, asc]
        ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

        needs_embedding[id, hash, span] := *function{id, tracking_hash: hash, span, embedding}, is_null(embedding)

        is_root_module[id] := *module{id}, NOT parent_of[id, _]

        batch[func_id, path, hash, span] := 
            needs_embedding[func_id, hash, span],
            ancestor[func_id, mod_id],
            is_root_module[mod_id],
            *module{id: mod_id, path}

        ?[func_id, path, hash, span] := batch[func_id, path, hash, span]
        :sort func_id
        :limit $limit
        :start-from $cursor
    "#;

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert(
            "cursor".into(),
            cursor.map(DataValue::from).unwrap_or(DataValue::List(vec![])),
        );

        let query_result = self.db.run_script(
            script,
            params,
            cozo::ScriptMutability::Immutable,
        ).map_err(|e| DbError::Cozo(e.to_string()))?;
        QueryResult::from(query_result).to_embedding_nodes()
    }
}

/// Safely converts a Cozo DataValue to a Uuid.
pub fn to_uuid(val: &DataValue) -> Result<uuid::Uuid, DbError> {
    if let DataValue::Uuid(UuidWrapper(uuid)) = val {
        Ok(*uuid)
    } else {
        Err(DbError::Cozo(format!("Expected Uuid, found {:?}", val)))
    }
}

/// Safely converts a Cozo DataValue to a String.
pub fn to_string(val: &DataValue) -> Result<String, DbError> {
    if let DataValue::Str(s) = val {
        Ok(s.to_string())
    } else {
        Err(DbError::Cozo(format!("Expected String, found {:?}", val)))
    }
}

/// Safely converts a Cozo DataValue to a usize.
pub fn to_usize(val: &DataValue) -> Result<usize, DbError> {
    if let DataValue::Num(cozo::Num::Int(n)) = val {
        // Cozo stores numbers that can be i64, u64, or f64. Safest to try as i64 for span.
        usize::try_from(*n).map_err(|e| {
            DbError::Cozo(format!(
                "Could not convert Num::Int to i64 for usize: {:?}, original error {}",
                n, e
            ))
        })
    } else {
        Err(DbError::Cozo(format!("Expected Number, found {:?}", val)))
    }
}
