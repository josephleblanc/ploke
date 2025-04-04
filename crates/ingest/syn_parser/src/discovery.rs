#![cfg(feature = "uuid_ids")] // Gate the entire module

use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;
use uuid::Uuid;
use walkdir; // Keep walkdir import separate for clarity if needed elsewhere

// Define a stable PROJECT_NAMESPACE UUID.
// This UUID acts as a root namespace for deriving crate-specific namespaces.
// Its purpose is to ensure that the *same crate name + version* combination
// consistently produces the same CRATE_NAMESPACE UUID across different runs
// or environments, *assuming the tool's core namespacing logic remains the same*.
// If the tool's parsing or namespacing logic changes fundamentally in a way
// that should invalidate old IDs, this constant might need to be updated,
// effectively versioning the tool's namespace generation.
// For now, it provides stability relative to the analyzed crate's identity.
// Generated via `uuidgen`: f7f4a9a0-1b1a-4b0e-9c1a-1a1a1a1a1a1a
pub const PROJECT_NAMESPACE_UUID: Uuid = Uuid::from_bytes([
    0xf7, 0xf4, 0xa9, 0xa0, 0x1b, 0x1a, 0x4b, 0x0e, 0x9c, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a, 0x1a,
]);

/// Errors that can occur during the discovery phase.
#[derive(Error, Debug)]
pub enum DiscoveryError {
    #[error("I/O error accessing path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse Cargo.toml at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("Missing 'package.name' in Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf },
    #[error("Missing 'package.version' in Cargo.toml at {path}")]
    MissingPackageVersion { path: PathBuf },
    #[error("Target crate path not found: {path}")]
    CratePathNotFound { path: PathBuf },
    #[error("Walkdir error in {path}: {source}")]
    Walkdir {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
}

/// Context information gathered for a single crate during discovery.
///
/// This struct automatically implements `Send + Sync` because all its members
/// (`String`, `Uuid`, `PathBuf`, `Vec<PathBuf>`) are `Send + Sync`.
#[derive(Debug, Clone)]
pub struct CrateContext {
    /// The simple name of the crate (e.g., "syn_parser").
    pub name: String,
    /// The version string from Cargo.toml (e.g., "0.1.0").
    pub version: String,
    /// The UUID namespace derived for this specific crate version using
    /// `Uuid::new_v5(&PROJECT_NAMESPACE_UUID, ...)`.
    pub namespace: Uuid,
    /// The absolute path to the crate's root directory (containing Cargo.toml).
    pub root_path: PathBuf,
    /// List of all `.rs` files found within the crate's source directories.
    pub files: Vec<PathBuf>,
}

/// Output of the entire discovery phase, containing context for all target crates.
///
/// This struct automatically implements `Send + Sync` because its members
/// (`HashMap<String, CrateContext>` and `HashMap<PathBuf, Vec<String>>`)
/// are composed of types that are `Send + Sync`.
///
/// `HashMap` is used here because this structure is generated once during the
/// single-threaded discovery phase and is expected to be used as read-only
/// context by the parallel parsing phase (Phase 2). If Phase 2 required
/// concurrent *writes* to this shared structure, `dashmap::DashMap` would be
/// necessary.
#[derive(Debug, Clone)]
pub struct DiscoveryOutput {
    /// Context information for each successfully discovered crate, keyed by crate name.
    pub crate_contexts: HashMap<String, CrateContext>,
    /// An initial, potentially incomplete, mapping from file paths to their
    /// anticipated module path (e.g., `src/parser/visitor.rs` -> `["crate", "parser", "visitor"]`).
    /// This is built from `lib.rs`, `main.rs`, and `mod.rs` scans during Phase 1.
    /// It serves as a starting point for more accurate resolution in later phases.
    pub initial_module_map: HashMap<PathBuf, Vec<String>>,
}

/// Runs the single-threaded discovery phase to gather context about target crates.
///
/// This function executes before any parallel parsing begins. It identifies
/// target crates, parses their `Cargo.toml` files, generates namespaces,
/// finds all `.rs` source files, and performs an initial scan for module
/// declarations.
///
/// # Arguments
/// * `_project_root` - The root path of the project being analyzed (may be used later).
/// * `_target_crates` - A slice of paths pointing to the root directories of the crates to analyze.
///
/// # Returns
/// A `Result` containing the `DiscoveryOutput` on success, or a `DiscoveryError` on failure.
pub fn run_discovery_phase(
    _project_root: &PathBuf,
    _target_crates: &[PathBuf], // Assuming we pass paths to crate roots
) -> Result<DiscoveryOutput, DiscoveryError> {
    // Implementation will go here in subsequent steps (3.2.1 onwards)
    todo!("Implement Phase 1 discovery logic")
}

/// Derives a deterministic UUID v5 namespace for a specific crate version.
///
/// This function is intended to run single-threaded as part of the discovery setup.
///
/// # Arguments
/// * `name` - The name of the crate.
/// * `version` - The version of the crate.
///
/// # Returns
/// A `Uuid` representing the namespace for this crate version, derived from
/// the `PROJECT_NAMESPACE_UUID`.
pub fn derive_crate_namespace(_name: &str, _version: &str) -> Uuid {
    // Implementation will go here in step 3.2.3
    todo!("Implement crate namespace derivation using Uuid::new_v5 and PROJECT_NAMESPACE_UUID")
}
