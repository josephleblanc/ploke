//! Eval-owned emission adapters for shared passive records.
//!
//! `ploke-records` defines inert schemas. This module owns the filesystem write
//! capability used by Prototype 1 producers.

use std::fs;
use std::path::{Path, PathBuf};

use ploke_records::record::{Record, RecordFamily, RecordFormat};

use crate::spec::PrepareError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EmittedRecord {
    pub(crate) path: PathBuf,
    pub(crate) family: RecordFamily,
    pub(crate) schema: &'static str,
    pub(crate) format: RecordFormat,
}

pub(crate) trait EmitRecord<R: Record> {
    type Error;
    type Receipt;

    fn emit(&mut self, record: &R) -> Result<Self::Receipt, Self::Error>;
}

pub(crate) struct JsonRecordFile<'a> {
    path: &'a Path,
}

impl<'a> JsonRecordFile<'a> {
    pub(crate) fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl<R> EmitRecord<R> for JsonRecordFile<'_>
where
    R: Record,
{
    type Error = PrepareError;
    type Receipt = EmittedRecord;

    fn emit(&mut self, record: &R) -> Result<Self::Receipt, Self::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
        fs::write(self.path, bytes).map_err(|source| PrepareError::WriteManifest {
            path: self.path.to_path_buf(),
            source,
        })?;
        Ok(EmittedRecord {
            path: self.path.to_path_buf(),
            family: R::FAMILY,
            schema: R::SCHEMA,
            format: R::FORMAT,
        })
    }
}
