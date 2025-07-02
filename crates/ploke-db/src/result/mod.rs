//! Query result handling and formatting

mod formatter;
mod snippet;

use std::path::PathBuf;

pub use formatter::ResultFormatter;
pub use snippet::CodeSnippet;

use crate::{
    database::{to_string, to_usize, to_uuid}, embedding::EmbeddingNode, error::DbError
};
use cozo::NamedRows;

/// Result of a database query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

impl QueryResult {
    /// Convert query results into code snippets
    pub fn into_snippets(self) -> Result<Vec<CodeSnippet>, DbError> {
        self.rows
            .iter()
            .map(|row| CodeSnippet::from_db_row(row))
            .collect()
    }

    pub fn to_embedding_nodes(self) -> Result<Vec<EmbeddingNode>, ploke_error::Error> {
        let id_index: usize = get_pos(&self.headers, "id")?;
        let path_index: usize = get_pos(&self.headers, "path")?;
        let content_hash_index: usize = get_pos(&self.headers, "content_hash")?;
        let start_byte_index: usize = get_pos(&self.headers, "start_byte")?;
        let end_byte_index: usize = get_pos(&self.headers, "end_byte")?;

        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let embeddings = self
            .rows
            .into_iter()
            .map(|row| {
                let id = to_uuid(&row[id_index]).map_err(map_err)?;
                let path_str = to_string(&row[path_index]).map_err(map_err)?;
                let content_hash = to_uuid(&row[content_hash_index]).map_err(map_err)?;
                let start_byte = to_usize(&row[start_byte_index]).map_err(map_err)?;
                let end_byte = to_usize(&row[end_byte_index]).map_err(map_err)?;

                Ok(EmbeddingNode {
                    id,
                    path: PathBuf::from(path_str),
                    content_hash,
                    start_byte,
                    end_byte,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }
}
fn get_pos(v: &[String], field: &str) -> Result<usize, DbError> {
    v.iter()
        .position(|s| s == field)
        .ok_or_else(|| DbError::Cozo(format!("Could not locate field {} in NamedRows", field)))
}

impl From<NamedRows> for QueryResult {
    fn from(named_rows: NamedRows) -> Self {
        Self {
            rows: named_rows.rows,
            headers: named_rows.headers,
        }
    }
}

// TODO: maybe use this to replace the `Into<CollectedEmbeddings> for NamedRows` in `embedding.rs`
// - this approach might be more direct, might be doing an intermediate step
//
// impl TryInto<CollectedEmbeddings> for QueryResult {
//     type Error = ploke_error::Error;
//
//     fn try_into(self) -> Result<CollectedEmbeddings, Self::Error> {
//         let id_index: usize = get_pos(&self.headers, "id")?;
//         let path_index: usize = get_pos(&self.headers, "path")?;
//         let content_hash_index: usize = get_pos(&self.headers, "content_hash")?;
//         let start_byte_index: usize = get_pos(&self.headers, "start_byte")?;
//         let end_byte_index: usize = get_pos(&self.headers, "end_byte")?;
//
//         let map_err = |e: DbError| ploke_error::Error::Internal(
//             ploke_error::InternalError::CompilerError(e.to_string())
//         );
//
//         let embeddings = self.rows
//             .into_iter()
//             .map(|row| {
//                 let id = to_uuid(&row[id_index]).map_err(map_err)?;
//                 let path_str = to_string(&row[path_index]).map_err(map_err)?;
//                 let content_hash = to_uuid(&row[content_hash_index]).map_err(map_err)?;
//                 let start_byte = to_usize(&row[start_byte_index]).map_err(map_err)?;
//                 let end_byte = to_usize(&row[end_byte_index]).map_err(map_err)?;
//
//                 Ok(EmbeddingNode {
//                     id,
//                     path: PathBuf::from(path_str),
//                     content_hash,
//                     start_byte,
//                     end_byte,
//                 })
//             })
//             .collect::<Result<Vec<_>, ploke_error::Error>>()?;
//
//         Ok(CollectedEmbeddings { embeddings })
//     }
// }
