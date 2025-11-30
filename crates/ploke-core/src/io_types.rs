use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::TrackingHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    pub id: Uuid,
    pub name: String,
    pub file_path: PathBuf,
    pub file_tracking_hash: TrackingHash,
    pub start_byte: usize,
    pub end_byte: usize,
    pub node_tracking_hash: TrackingHash,
    pub namespace: Uuid,
}

// TODO: Make these Typed Ids, and put the typed id definitions into ploke-core
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileData {
    /// Uuid is of the owner file-level module
    pub id: Uuid,
    pub namespace: Uuid,
    pub file_tracking_hash: TrackingHash,
    pub file_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFileData {
    /// Uuid is of the owner file-level module
    pub id: Uuid,
    pub namespace: Uuid,
    pub old_tracking_hash: TrackingHash,
    pub new_tracking_hash: TrackingHash,
    pub file_path: PathBuf,
}

impl ChangedFileData {
    pub fn from_file_data(value: FileData, new_tracking_hash: TrackingHash) -> Self {
        let FileData {
            id,
            namespace,
            file_tracking_hash,
            file_path,
        } = value;
        Self {
            id,
            namespace,
            old_tracking_hash: file_tracking_hash,
            new_tracking_hash,
            file_path,
        }
    }
}

/// Placeholder structures to be replaced by shared types in ploke-core.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteSnippetData {
    pub id: uuid::Uuid,
    pub name: String,
    pub file_path: PathBuf,
    pub expected_file_hash: TrackingHash,
    pub start_byte: usize,
    pub end_byte: usize,
    pub replacement: String,
    pub namespace: uuid::Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriteResult {
    pub new_file_hash: TrackingHash,
}

impl WriteResult {
    pub fn new(new_file_hash: TrackingHash) -> Self {
        Self { new_file_hash }
    }
}

// Create-file support (IO-level)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnExists {
    Error,
    Overwrite,
}

impl Default for OnExists {
    fn default() -> Self {
        Self::Error
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileData {
    pub id: uuid::Uuid,
    pub name: String,
    pub file_path: PathBuf,
    pub content: String,
    pub namespace: uuid::Uuid,
    #[serde(default)]
    pub on_exists: OnExists,
    #[serde(default)]
    pub create_parents: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileResult {
    pub new_file_hash: TrackingHash,
}
