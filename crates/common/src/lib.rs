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
