//! Tracks changes to code spans over time
#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Tracks source code spans and their changes
pub struct SpanTracker {
    versions: HashMap<PathBuf, Vec<CodeVersion>>,
}

#[derive(Debug)]
struct CodeVersion {
    hash: String,
    timestamp: u64,
    spans: Vec<(usize, usize)>,
}

impl SpanTracker {
    /// Create a new span tracker
    pub fn new() -> Self {
        Self {
            versions: HashMap::new(),
        }
    }

    /// Record a new version of a file's spans
    pub fn record_version(
        &mut self,
        file: PathBuf,
        hash: String,
        timestamp: u64,
        spans: Vec<(usize, usize)>,
    ) {
        self.versions.entry(file).or_default().push(CodeVersion {
            hash,
            timestamp,
            spans,
        });
    }

    /// Get changed spans between versions
    pub fn get_changed_spans(
        &self,
        _file: &Path,
        _old_hash: &str,
        _new_hash: &str,
    ) -> Option<Vec<(usize, usize)>> {
        // TODO: Implement actual diff logic
        None
    }
}

impl Default for SpanTracker {
    fn default() -> Self {
        Self::new()
    }
}
