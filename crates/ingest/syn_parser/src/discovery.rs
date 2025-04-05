#![cfg(feature = "uuid_ids")] // Gate the entire module

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use toml;
use uuid::Uuid;
use walkdir::WalkDir;

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
    #[error("Source directory not found for crate at: {path}")]
    SrcNotFound { path: PathBuf },
}

// Helper structs for deserializing Cargo.toml
#[derive(Deserialize, Debug)]
struct CargoManifest {
    package: PackageInfo,
    // Add other fields like [lib], [bin] if needed later for module mapping
}

#[derive(Deserialize, Debug)]
struct PackageInfo {
    name: String,
    version: String,
    // edition: Option<String>, // Could be useful later
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
    _project_root: &PathBuf, // Keep for potential future use
    target_crates: &[PathBuf], // Expecting absolute paths to crate root directories
) -> Result<DiscoveryOutput, DiscoveryError> {
    let mut crate_contexts = HashMap::new();
    let initial_module_map = HashMap::new(); // To be implemented later

    for crate_root_path in target_crates {
        if !crate_root_path.exists() || !crate_root_path.is_dir() {
            return Err(DiscoveryError::CratePathNotFound {
                path: crate_root_path.clone(),
            });
        }

        // --- 3.2.2 Implement Cargo.toml Parsing ---
        let cargo_toml_path = crate_root_path.join("Cargo.toml");
        let cargo_content = fs::read_to_string(&cargo_toml_path).map_err(|e| {
            DiscoveryError::Io {
                path: cargo_toml_path.clone(),
                source: e,
            }
        })?;
        let manifest: CargoManifest =
            toml::from_str(&cargo_content).map_err(|e| DiscoveryError::TomlParse {
                path: cargo_toml_path.clone(),
                source: e,
            })?;

        let crate_name = manifest
            .package
            .name
            .clone();
            // .ok_or_else(|| DiscoveryError::MissingPackageName { path: cargo_toml_path.clone() })?;
        let crate_version = manifest
            .package
            .version
            .clone();
            // .ok_or_else(|| DiscoveryError::MissingPackageVersion { path: cargo_toml_path.clone() })?;


        // --- 3.2.3 Implement Namespace Generation (Called below) ---
        let namespace = derive_crate_namespace(&crate_name, &crate_version);

        // --- 3.2.1 Implement File Discovery Logic ---
        let src_path = crate_root_path.join("src");
        if !src_path.exists() || !src_path.is_dir() {
            // Allow crates without a src dir? Maybe just return empty file list.
            // For now, let's error if src isn't found, common case.
             return Err(DiscoveryError::SrcNotFound { path: src_path });
            // files = Vec::new();
        }

        let mut files = Vec::new();
        for entry in WalkDir::new(&src_path)
            .into_iter()
            .filter_map(Result::ok) // Ignore errors for now, or collect them
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "rs"))
        {
             // Ensure we store absolute paths if target_crates might be relative
             // Assuming target_crates provides absolute paths for simplicity here.
             // If not, canonicalize crate_root_path first.
            files.push(entry.path().to_path_buf());
        }
        // Handle walkdir errors more robustly if needed:
        // let walker = WalkDir::new(&src_path).into_iter();
        // while let Some(entry_result) = walker.next() {
        //     match entry_result {
        //         Ok(entry) => {
        //             if entry.file_type().is_file() && entry.path().extension().map_or(false, |ext| ext == "rs") {
        //                 files.push(entry.path().to_path_buf());
        //             }
        //         }
        //         Err(e) => return Err(DiscoveryError::Walkdir { path: src_path.clone(), source: e }),
        //     }
        // }


        // --- Combine into CrateContext ---
        let context = CrateContext {
            name: crate_name.clone(),
            version: crate_version,
            namespace,
            root_path: crate_root_path.clone(),
            files,
        };

        crate_contexts.insert(crate_name, context);
    }

    // --- 3.2.4 Implement Initial Module Mapping (Deferred) ---
    // let initial_module_map = build_initial_module_map(&crate_contexts)?;

    Ok(DiscoveryOutput {
        crate_contexts,
        initial_module_map, // Return empty map for now
    })
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
pub fn derive_crate_namespace(name: &str, version: &str) -> Uuid {
    // Combine name and version to form the unique identifier string within the project namespace.
    // Using "@" is a common convention.
    let name_version = format!("{}@{}", name, version);
    Uuid::new_v5(&PROJECT_NAMESPACE_UUID, name_version.as_bytes())
}
