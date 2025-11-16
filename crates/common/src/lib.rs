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

/// UUID suffix shared by all `fixture_nodes` backup filenames.
pub const FIXTURE_NODES_ID: &str = "bfc25988-15c1-5e58-9aa8-3d33b5e58b92";

/// Suffix used for backups that include multi-embedding relations.
pub const MULTI_EMBED_SCHEMA_TAG: &str = "multi_embedding_schema_v1";

pub const LEGACY_FIXTURE_BACKUP_REL_PATH: &str =
    "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92";
pub const LEGACY_FIXTURE_METADATA_REL_PATH: &str =
    "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92.meta.json";

pub const MULTI_EMBED_FIXTURE_BACKUP_REL_PATH: &str =
    "tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92";
pub const MULTI_EMBED_FIXTURE_METADATA_REL_PATH: &str =
    "tests/backup_dbs/fixture_nodes_multi_embedding_schema_v1_bfc25988-15c1-5e58-9aa8-3d33b5e58b92.meta.json";

/// WARNING: Only use this for testing error handling!!!
/// Get the absolute path to the malformed fixtures directory
pub fn malformed_fixtures_dir() -> PathBuf {
    workspace_root().join("tests/malformed_fixtures")
}
