use std::path::PathBuf;
use uuid::Uuid;

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
