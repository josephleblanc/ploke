//! Query result handling and formatting

mod formatter;
mod snippet;

use std::path::PathBuf;

pub use formatter::ResultFormatter;
use itertools::Itertools;
use ploke_core::{
    embeddings::{EmbeddingSet, EmbeddingSetId}, io_types::ResolvedEdgeData, rag_types::CanonPath, EmbeddingData, FileData, TrackingHash
};
pub use snippet::CodeSnippet;
use uuid::Uuid;

use crate::{
    database::{to_string, to_u64, to_usize, to_uuid, to_vector},
    error::DbError,
    get_by_id::CommonFields,
    multi_embedding::schema::EmbeddingVector,
    NodeType,
};
use cozo::{DataValue, NamedRows};

/// Result of a database query
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub rows: Vec<Vec<cozo::DataValue>>,
    pub headers: Vec<String>,
}

// // TODO: Make these Typed Ids, and put the typed id definitions into ploke-core
// #[derive(Debug, Clone)]
// pub struct FileData {
//     pub id: Uuid,
//     pub namespace: Uuid,
//     pub file_tracking_hash: TrackingHash,
//     pub file_path: PathBuf,
// }

impl QueryResult {
    /// Convert query results into code snippets
    pub fn into_snippets(self) -> Result<Vec<CodeSnippet>, DbError> {
        self.rows
            .iter()
            .map(|row| CodeSnippet::from_db_row(row))
            .collect()
    }

    pub fn try_into_file_data(self) -> Result<Vec<FileData>, ploke_error::Error> {
        let id_index: usize = get_pos(&self.headers, "id")?;
        let file_path_index: usize = get_pos(&self.headers, "file_path")?;
        let file_th_index: usize = get_pos(&self.headers, "tracking_hash")?;
        let namespace_index: usize = get_pos(&self.headers, "namespace")?;

        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let file_data = self
            .rows
            .into_iter()
            .map(|row| {
                let id = to_uuid(&row[id_index]).map_err(map_err)?;
                let file_path_str = to_string(&row[file_path_index]).map_err(map_err)?;
                let file_tracking_hash =
                    TrackingHash(to_uuid(&row[file_th_index]).map_err(map_err)?);
                let namespace = to_uuid(&row[namespace_index]).map_err(map_err)?;

                Ok(FileData {
                    id,
                    file_path: PathBuf::from(file_path_str),
                    file_tracking_hash,
                    namespace,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(file_data)
    }

    // TODO: Delete namespace and file_path, maybe also file_th
    pub fn to_embedding_nodes(self) -> Result<Vec<EmbeddingData>, ploke_error::Error> {
        let id_index: usize = get_pos(&self.headers, "id")?;
        let name_index: usize = get_pos(&self.headers, "name")?;
        let file_path_index: usize = get_pos(&self.headers, "file_path")?;
        let file_th_index: usize = get_pos(&self.headers, "file_hash")?;
        let node_th_index: usize = get_pos(&self.headers, "hash")?;
        let namespace_index: usize = get_pos(&self.headers, "namespace")?;
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
                let node_tracking_hash =
                    TrackingHash(to_uuid(&row[node_th_index]).map_err(map_err)?);
                let file_tracking_hash =
                    TrackingHash(to_uuid(&row[file_th_index]).map_err(map_err)?);
                let namespace = to_uuid(&row[namespace_index]).map_err(map_err)?;
                let span = &row[span_index].get_slice().unwrap();

                let (start_byte, end_byte) = get_byte_offsets(span);

                Ok(EmbeddingData {
                    id,
                    name,
                    file_path: PathBuf::from(file_path_str),
                    start_byte,
                    end_byte,
                    node_tracking_hash,
                    file_tracking_hash,
                    namespace,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }

    pub fn to_resolved_edges(self) -> Result<Vec<ResolvedEdgeData>, ploke_error::Error> {
        let target_name_index: usize = get_pos(&self.headers, "target_name")?;
        let source_name_index: usize = get_pos(&self.headers, "source_name")?;
        let source_id_index: usize = get_pos(&self.headers, "source_id")?;
        let target_id_index: usize = get_pos(&self.headers, "target_id")?;
        let canon_path_index: usize = get_pos(&self.headers, "canon_path")?;
        let file_path_index: usize = get_pos(&self.headers, "file_path")?;
        let relation_kind_index: usize = get_pos(&self.headers, "relation_kind")?;

        let map_err = |e: DbError| {
            ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
        };

        let embeddings = self
            .rows
            .into_iter()
            .map(|row| {
                let source_id = to_uuid(&row[source_id_index]).map_err(map_err)?;
                let target_id = to_uuid(&row[target_id_index]).map_err(map_err)?;
                let target_name = to_string(&row[target_name_index]).map_err(map_err)?;
                let source_name = to_string(&row[source_name_index]).map_err(map_err)?;
                let canon_path = to_string(&row[canon_path_index]).map_err(map_err)?;
                let relation_kind = to_string(&row[relation_kind_index]).map_err(map_err)?;
                let file_path_str = to_string(&row[file_path_index]).map_err(map_err)?;

                Ok(ResolvedEdgeData {
                    file_path: PathBuf::from(file_path_str),
                    source_id,
                    source_name,
                    target_id,
                    target_name,
                    canon_path: CanonPath::new(canon_path),
                    relation_kind,
                })
            })
            .collect::<Result<Vec<_>, ploke_error::Error>>()?;

        Ok(embeddings)
    }

    // pub fn to_embedding_vector(self, embedding_set: &EmbeddingSet) -> Result<Vec<EmbeddingVector>, ploke_error::Error> {
    //     let map_err = |e: DbError| {
    //         ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(e.to_string()))
    //     };
    //
    //     let node_id_index: usize = get_pos(&self.headers, "node_id")?;
    //     let embedding_set_id_index: usize = get_pos(&self.headers, "embedding_set_id")?;
    //     let vector_index: usize = get_pos(&self.headers, "vector")?;
    //
    //     let embeddings = self
    //         .rows
    //         .into_iter()
    //         .map(
    //             |row| {
    //             let node_id = to_uuid(&row[node_id_index]).map_err(map_err)?;
    //             let embedding_set_id = to_u64(&row[embedding_set_id_index]).map_err(map_err)?;
    //             let vector = to_vector(&row[vector_index], embedding_set).map_err(map_err)?;
    //
    //             Ok(EmbeddingVector {
    //                 node_id,
    //                 vector,
    //                 embedding_set_id: EmbeddingSetId::from_db_raw(embedding_set_id),
    //             })
    //         })
    //         .collect::<Result<Vec<_>, ploke_error::Error>>()?;
    //
    //     Ok(embeddings)
    // }

    pub fn iter_col<'a>(&'a self, col_title: &str) -> Option<impl Iterator<Item = &'a DataValue>> {
        use std::ops::Index;
        let col_idx = self
            .headers
            .iter()
            .enumerate()
            .find(|(idx, col)| col.as_str() == col_title)
            .map(|(idx, col)| idx)?;
        Some(self.rows.iter().map(move |r| r.index(col_idx)))
    }

    /// Converts the headers and row to debug string format.
    ///
    /// All the headers are in debug format, then each row on a new line following the debug
    /// header.
    pub fn debug_string_all(&self) -> String {
        let header = &self.headers;
        let mut s = format!("{:?}", header);

        for row in &self.rows {
            s.push('\n');
            let row_debug_str = format!("{row:?}");
            s.push_str(&row_debug_str);
        }
        s
    }
}

pub(crate) fn get_byte_offsets(span: &&[cozo::DataValue]) -> (usize, usize) {
    let error_msg = "Invariant Violated: All Nodes must have a start/end byte";
    let start_byte = span.first().expect(error_msg).get_int().expect(error_msg) as usize;
    let end_byte = span.last().expect(error_msg).get_int().expect(error_msg) as usize;
    (start_byte, end_byte)
}
pub(crate) fn get_pos(v: &[String], field: &str) -> Result<usize, DbError> {
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
