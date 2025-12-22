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

impl EmbeddingData {
    pub fn builder() -> EmbeddingDataBuilder {
        EmbeddingDataBuilder::default()
    }
}

#[derive(Debug, Default)]
pub struct EmbeddingDataBuilder {
    id: Option<Uuid>,
    name: Option<String>,
    file_path: Option<PathBuf>,
    file_tracking_hash: Option<TrackingHash>,
    start_byte: Option<usize>,
    end_byte: Option<usize>,
    node_tracking_hash: Option<TrackingHash>,
    namespace: Option<Uuid>,
}

impl EmbeddingDataBuilder {
    pub fn id(mut self, id: Uuid) -> Self {
        self.id = Some(id);
        self
    }

    pub fn name<S: Into<String>>(mut self, name: S) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn file_path<P: Into<PathBuf>>(mut self, file_path: P) -> Self {
        self.file_path = Some(file_path.into());
        self
    }

    pub fn file_tracking_hash(mut self, file_tracking_hash: TrackingHash) -> Self {
        self.file_tracking_hash = Some(file_tracking_hash);
        self
    }

    pub fn start_byte(mut self, start_byte: usize) -> Self {
        self.start_byte = Some(start_byte);
        self
    }

    pub fn end_byte(mut self, end_byte: usize) -> Self {
        self.end_byte = Some(end_byte);
        self
    }

    pub fn node_tracking_hash(mut self, node_tracking_hash: TrackingHash) -> Self {
        self.node_tracking_hash = Some(node_tracking_hash);
        self
    }

    pub fn namespace(mut self, namespace: Uuid) -> Self {
        self.namespace = Some(namespace);
        self
    }

    pub fn build(self) -> Result<EmbeddingData, EmbeddingDataBuilderError> {
        let id = self.id.unwrap_or_else(Uuid::new_v4);
        let name = self
            .name
            .ok_or(EmbeddingDataBuilderError::MissingField("name"))?;
        let file_path = self
            .file_path
            .ok_or(EmbeddingDataBuilderError::MissingField("file_path"))?;
        let file_tracking_hash =
            self.file_tracking_hash
                .ok_or(EmbeddingDataBuilderError::MissingField(
                    "file_tracking_hash",
                ))?;
        let start_byte = self
            .start_byte
            .ok_or(EmbeddingDataBuilderError::MissingField("start_byte"))?;
        let end_byte = self
            .end_byte
            .ok_or(EmbeddingDataBuilderError::MissingField("end_byte"))?;
        let node_tracking_hash =
            self.node_tracking_hash
                .ok_or(EmbeddingDataBuilderError::MissingField(
                    "node_tracking_hash",
                ))?;
        let namespace = self
            .namespace
            .ok_or(EmbeddingDataBuilderError::MissingField("namespace"))?;

        if start_byte >= end_byte {
            return Err(EmbeddingDataBuilderError::InvalidRange);
        }

        if name.is_empty() {
            return Err(EmbeddingDataBuilderError::EmptyName);
        }

        Ok(EmbeddingData {
            id,
            name,
            file_path,
            file_tracking_hash,
            start_byte,
            end_byte,
            node_tracking_hash,
            namespace,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum EmbeddingDataBuilderError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("start byte must be less than end byte")]
    InvalidRange,
    #[error("name cannot be empty")]
    EmptyName,
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
#[derive(Default, Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnExists {
    #[default]
    Error,
    Overwrite,
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
