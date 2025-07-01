use std::path::PathBuf;
use uuid::Uuid;

use crate::{
    database::{to_string, to_usize, to_uuid},
    DbError,
};

/// Represents a node that requires an embedding.
// TODO: Add doc comment linking to `get_nodes_for_embedding`
#[derive(Debug, Clone)]
pub struct EmbeddingNode {
    pub id: Uuid,
    pub path: PathBuf,
    pub content_hash: Uuid,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone)]
pub struct CollectedEmbeddings {
    embeddings: Vec<EmbeddingNode>,
}

fn get_pos(v: &std::vec::Vec<std::string::String>, field: &str) -> Result<usize, DbError> {
    v.iter()
        .position(|s| s == field)
        .ok_or_else(|| DbError::Cozo(format!("Could not locate field {} in NamedRows", field)))
}

impl TryInto<CollectedEmbeddings> for cozo::NamedRows {
    type Error = ploke_error::Error;

    fn try_into(self) -> Result<CollectedEmbeddings, Self::Error> {
        let id_index: usize = get_pos(&self.headers, "id")?;
        let path_index: usize = get_pos(&self.headers, "path")?;
        let content_hash_index: usize = get_pos(&self.headers, "content_hash")?;
        let start_byte_index: usize = get_pos(&self.headers, "start_byte")?;
        let end_byte_index: usize = get_pos(&self.headers, "end_byte")?;

        self.rows.into_iter().map(|r| EmbeddingNode {
            // 
            id: to_uuid(r[id_index]),
            path: PathBuf::from(to_string(r[path_index])),
            content_hash: to_uuid(r[content_hash_index]),
            start_byte: to_usize(r[start_byte_index]),
            end_byte: to_usize(r[end_byte_index]),
        });

        todo!("Implement me!")
    }
}
