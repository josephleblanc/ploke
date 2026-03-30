//! Focused parser diagnostics persistence for debug-corpus and invariant failures.
//!
//! This module is intentionally small and file-oriented. It is not a general logging framework;
//! it exists so parser code can persist a compact, high-value JSON payload immediately before an
//! invariant trip or panic boundary.
//!
//! Typical usage:
//! - `xtask parse debug corpus` activates a stage-local artifact directory with
//!   [`with_debug_artifact_dir`].
//! - parser code calls [`emit_json_diagnostic`] when it encounters duplicate nodes, duplicate
//!   relations, prune-count mismatches, or other internal-state problems worth preserving.
//! - the resulting JSON files sit beside the stage artifacts for the failing target.
//!
//! The tracing target constants here are also the stable subscription points for parser debugging:
//! - `syn_parser::merge`
//! - `syn_parser::prune`
//! - `syn_parser::invariants`
//! - `syn_parser::panic_context`
//!
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub const TRACE_TARGET_MERGE: &str = "syn_parser::merge";
pub const TRACE_TARGET_PRUNE: &str = "syn_parser::prune";
pub const TRACE_TARGET_INVARIANTS: &str = "syn_parser::invariants";
pub const TRACE_TARGET_PANIC_CONTEXT: &str = "syn_parser::panic_context";

static ACTIVE_ARTIFACT_DIR: OnceLock<Mutex<Vec<PathBuf>>> = OnceLock::new();
static NEXT_ARTIFACT_ID: AtomicU64 = AtomicU64::new(1);

fn artifact_dir_stack() -> &'static Mutex<Vec<PathBuf>> {
    ACTIVE_ARTIFACT_DIR.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn with_debug_artifact_dir<T>(dir: &Path, f: impl FnOnce() -> T) -> T {
    {
        let mut stack = artifact_dir_stack()
            .lock()
            .expect("debug artifact dir mutex poisoned");
        stack.push(dir.to_path_buf());
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

    {
        let mut stack = artifact_dir_stack()
            .lock()
            .expect("debug artifact dir mutex poisoned");
        stack.pop();
    }

    match result {
        Ok(value) => value,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

pub fn current_debug_artifact_dir() -> Option<PathBuf> {
    artifact_dir_stack()
        .lock()
        .ok()
        .and_then(|stack| stack.last().cloned())
}

pub fn emit_json_diagnostic<T: Serialize>(name: &str, payload: &T) -> Option<PathBuf> {
    let dir = current_debug_artifact_dir()?;
    std::fs::create_dir_all(&dir).ok()?;

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let seq = NEXT_ARTIFACT_ID.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{millis:013}_{seq:06}_{}.json", sanitize_filename(name));
    let path = dir.join(filename);

    let file = std::fs::File::create(&path).ok()?;
    serde_json::to_writer_pretty(file, payload).ok()?;
    Some(path)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_json_diagnostic_writes_file_when_dir_is_active() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = with_debug_artifact_dir(temp.path(), || {
            emit_json_diagnostic("duplicate relation", &serde_json::json!({ "ok": true }))
        })
        .expect("artifact path");

        assert!(path.is_file(), "expected persisted diagnostic file");
        let content = std::fs::read_to_string(path).expect("read diagnostic");
        assert!(content.contains("\"ok\": true"), "{content}");
    }
}
