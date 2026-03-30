use std::env;
use std::path::{Path, PathBuf};

/// Gets the absolute path to the workspace root directory            
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Error parsing workspace directory from crate `common`") // crates/
        .parent() // workspace root
        .expect("Failed to get workspace root")
        .to_path_buf()
}

/// Gets the absolute path to the fixtures directory                  
pub fn fixtures_dir() -> PathBuf {
    workspace_root().join("tests/fixtures")
}

pub fn fixtures_crates_dir() -> PathBuf {
    workspace_root().join("tests/fixture_crates")
}

/// WARNING: Only use this for testing error handling!!!
/// Get the absolute path to the malformed fixtures directory
pub fn malformed_fixtures_dir() -> PathBuf {
    workspace_root().join("tests/malformed_fixtures")
}

/// Gets the absolute path to the fixture_github_clones directory.
///
/// This directory holds real-world crates cloned from GitHub, used for
/// robustness and integration testing against production-grade Rust code.
// NOTE:2026-03-28 updated from tests/fixture_github_clones to tests/fixture_github_clones/corpus
// where corpus is a symlink. See xtask for more details on corpus contents.
pub fn fixture_github_clones_dir() -> PathBuf {
    workspace_root().join("tests/fixture_github_clones/corpus")
}
