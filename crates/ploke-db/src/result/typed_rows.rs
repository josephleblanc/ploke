use std::path::PathBuf;

use ploke_core::rag_types::CanonPath;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedEdgeData {
    pub source_id: Uuid,
    pub source_name: String,
    pub target_id: Uuid,
    pub target_name: String,
    pub canon_path: CanonPath,
    pub relation_kind: String,
    pub file_path: PathBuf,
}
