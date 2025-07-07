//! Query result handling and formatting

mod formatter;
mod snippet;

use std::path::PathBuf;

pub use formatter::ResultFormatter;
use ploke_core::{EmbeddingData, TrackingHash};
pub use snippet::CodeSnippet;

use crate::{
    database::{to_string, to_usize, to_uuid},
    error::DbError,
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

    pub fn to_embedding_nodes(self) -> Result<Vec<EmbeddingData>, ploke_error::Error> {
        let id_index: usize = get_pos(&self.headers, "id")?;
        let name_index: usize = get_pos(&self.headers, "name")?;
        let file_path_index: usize = get_pos(&self.headers, "file_path")?;
        let tracking_hash_index: usize = get_pos(&self.headers, "hash")?;
        let span_index = get_pos(&self.headers, "span")?;

        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let embeddings = self
            .rows
            .into_iter()
            .map(|row| {
                let id = to_uuid(&row[id_index]).map_err(map_err)?;
                let name = to_string(&row[name_index]).map_err(map_err)?;
                let file_path_str = to_string(&row[file_path_index]).map_err(map_err)?;
                let file_tracking_hash =
                    TrackingHash(to_uuid(&row[tracking_hash_index]).map_err(map_err)?);
                let span = &row[span_index].get_slice().unwrap();

                let (start_byte, end_byte) = get_byte_offsets(span);

                Ok(EmbeddingData {
                    id,
                    name,
                    file_path: PathBuf::from(file_path_str),
                    start_byte,
                    end_byte,
                    file_tracking_hash,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }
}

fn get_byte_offsets(span: &&[cozo::DataValue]) -> (usize, usize) {
    let error_msg = "Invariant Violated: All Nodes must have a start/end byte";
    let start_byte = span.first().expect(error_msg).get_int().expect(error_msg) as usize;
    let end_byte = span.last().expect(error_msg).get_int().expect(error_msg) as usize;
    (start_byte, end_byte)
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
//         let tracking_hash_index: usize = get_pos(&self.headers, "tracking_hash")?;
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
//                 let tracking_hash = to_uuid(&row[tracking_hash_index]).map_err(map_err)?;
//                 let start_byte = to_usize(&row[start_byte_index]).map_err(map_err)?;
//                 let end_byte = to_usize(&row[end_byte_index]).map_err(map_err)?;
//
//                 Ok(EmbeddingData {
//                     id,
//                     path: PathBuf::from(path_str),
//                     tracking_hash,
//                     start_byte,
//                     end_byte,
//                 })
//             })
//             .collect::<Result<Vec<_>, ploke_error::Error>>()?;
//
//         Ok(CollectedEmbeddings { embeddings })
//     }
// }
