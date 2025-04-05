#![cfg(feature = "uuid_ids")] // Gate the entire module

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader};
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
// NOTE: Known limitations:
// * No currently designed methods to track changes in `ploke-db`. Would be a big feature to
// implement, this is fine for the foreseeable future.
//  * Explore ideas on this at leisure, in case there is easy groundwork to lay.
// * Currently uses same namespace for all crates with no project.
//  * Fine for now. Evaluate potential for pros/cons of this approach another time.
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
/// A tuple containing:
///  * `DiscoveryOutput`: Contains context for all *successfully* processed crates.
///  * `Vec<DiscoveryError>`: A list of errors encountered for crates that failed processing.
/// The function always returns, even if some crates fail, providing partial results.
// NOTE: Known limitations:
// * Does not handle case of crate with no `src` directory in project (currently returns SrcNotFound error)
// * Assuming target_crates provides absolute paths for simplicity
//  * No UI design yet, but contract with `run_discovery_phase` should be that `run_discover_phase`
//  should only ever receive full paths. (Seperation of Concerns: UI vs Traversal)
pub fn run_discovery_phase(
    _project_root: &PathBuf,   // Keep for potential future use
    target_crates: &[PathBuf], // Expecting absolute paths to crate root directories
) -> (DiscoveryOutput, Vec<DiscoveryError>) { // Changed return type
    let mut crate_contexts = HashMap::new();
    let mut initial_module_map = HashMap::new(); // Still mutable
    let mut errors = Vec::new(); // Collect errors here

    for crate_root_path in target_crates {
        // Use a closure to handle errors for this specific crate iteration
        let crate_result: Result<(), DiscoveryError> = (|| {
            if !crate_root_path.exists() || !crate_root_path.is_dir() {
                return Err(DiscoveryError::CratePathNotFound {
                    path: crate_root_path.clone(),
                });
            }

            // --- 3.2.2 Implement Cargo.toml Parsing ---
            let cargo_toml_path = crate_root_path.join("Cargo.toml");
            let cargo_content =
                fs::read_to_string(&cargo_toml_path).map_err(|e| DiscoveryError::Io {
                    path: cargo_toml_path.clone(),
                    source: e,
                })?;
            let manifest: CargoManifest =
                toml::from_str(&cargo_content).map_err(|e| DiscoveryError::TomlParse {
                    path: cargo_toml_path.clone(),
                    source: e,
                })?;

            let crate_name = manifest.package.name.clone();
            // .ok_or_else(|| DiscoveryError::MissingPackageName { path: cargo_toml_path.clone() })?;
            let crate_version = manifest.package.version.clone();
            // .ok_or_else(|| DiscoveryError::MissingPackageVersion { path: cargo_toml_path.clone() })?;

            // --- 3.2.3 Implement Namespace Generation (Called below) ---
            let namespace = derive_crate_namespace(&crate_name, &crate_version);

            // --- 3.2.1 Implement File Discovery Logic ---
            let src_path = crate_root_path.join("src");
            if !src_path.exists() || !src_path.is_dir() {
                // Allow crates without a src dir? Maybe just return empty file list.
                // For now, let's error if src isn't found, common case.
                // USER: Agreed, and good call on the clear enum error. We can expand this later once
                // core functionality is built out.
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
            // Handle walkdir errors more robustly
            let walker = WalkDir::new(&src_path).into_iter();
            for entry_result in walker {
                match entry_result {
                    Ok(entry) => {
                        if entry.file_type().is_file()
                            && entry.path().extension().map_or(false, |ext| ext == "rs")
                        {
                            files.push(entry.path().to_path_buf());
                        }
                    }
                    Err(e) => {
                        // Collect WalkDir errors but continue walking
                        let path = e.path().unwrap_or(&src_path).to_path_buf();
                        eprintln!("Warning: Error walking directory {:?}: {}", path, e); // Log to stderr for now
                        errors.push(DiscoveryError::Walkdir { path, source: e });
                        // Decide if we should skip the rest of the processing for this crate?
                        // For now, let's continue and try to gather what we can.
                    }
                }
            }

            // --- Combine into CrateContext ---
            let context = CrateContext {
                name: crate_name.clone(),
                version: crate_version,
                namespace,
                root_path: crate_root_path.clone(),
                files: files.clone(), // Clone needed for module mapping below
            };

            // --- 3.2.4 Implement Initial Module Mapping ---
            // Scan lib.rs and main.rs for `mod xyz;`
            for entry_point_name in ["lib.rs", "main.rs"] {
                let entry_point_path = src_path.join(entry_point_name);
                if files.contains(&entry_point_path) {
                    match scan_for_mods(&entry_point_path, &src_path, &files) {
                        Ok(mods) => {
                            // Merge results into the main map
                            initial_module_map.extend(mods);
                        }
                        Err(e) => {
                            // Log scan error and add to list, but continue
                            eprintln!(
                                "Warning: Error scanning modules in {:?}: {}",
                                entry_point_path, e
                            );
                            errors.push(e) // Add scan error to list
                        }
                    }
                }
            }

            crate_contexts.insert(crate_name, context);
            Ok(()) // Indicate success for this iteration's error handling
        })(); // End of closure for crate processing

        // If the closure returned an error for this crate, add it to the list
        if let Err(e) = crate_result {
            errors.push(e);
        }
    }

    // Always return collected contexts and errors
    (
        DiscoveryOutput {
            crate_contexts,
            initial_module_map,
        },
        errors,
    )
}

/// Scans a single Rust file (typically lib.rs or main.rs) for module declarations (`mod name;`)
/// and attempts to map them to existing files found during discovery.
///
/// # Arguments
/// * `file_to_scan` - Path to the file to scan (e.g., `.../src/lib.rs`).
/// * `src_path` - Path to the crate's `src` directory.
/// * `existing_files` - A slice containing all `.rs` files found in the crate.
///
/// # Returns
/// A `Result` containing a map from the resolved module file path to its module segments
/// (e.g., `.../src/parser.rs` -> `["crate", "parser"]`), or a `DiscoveryError::Io` if
/// the file cannot be read.
fn scan_for_mods(
    file_to_scan: &Path,
    src_path: &Path,
    existing_files: &[PathBuf],
) -> Result<HashMap<PathBuf, Vec<String>>, DiscoveryError> {
    let mut mod_map = HashMap::new();
    let file = fs::File::open(file_to_scan).map_err(|e| DiscoveryError::Io {
        path: file_to_scan.to_path_buf(),
        source: e,
    })?;
    let reader = BufReader::new(file);

    // Basic line-by-line scan for `mod name;` or `pub mod name;`
    // This is a simplification and won't handle complex cases like `#[cfg] mod name;`
    // or mods defined inside functions/blocks.
    for line_result in reader.lines() {
        let line = line_result.map_err(|e| DiscoveryError::Io {
            path: file_to_scan.to_path_buf(),
            source: e,
        })?;
        let trimmed = line.trim();

        if (trimmed.starts_with("mod ") || trimmed.starts_with("pub mod "))
            && trimmed.ends_with(';')
        {
            if let Some(name_part) = trimmed.split_whitespace().nth(1) {
                let mod_name = name_part.trim_end_matches(';');

                // Check for potential file paths: src/mod_name.rs or src/mod_name/mod.rs
                let path1 = src_path.join(format!("{}.rs", mod_name));
                let path2 = src_path.join(mod_name).join("mod.rs");

                let found_path = if existing_files.contains(&path1) {
                    Some(path1)
                } else if existing_files.contains(&path2) {
                    Some(path2)
                } else {
                    None
                };

                if let Some(p) = found_path {
                    // Basic mapping: assumes it's directly under "crate"
                    mod_map.insert(p, vec!["crate".to_string(), mod_name.to_string()]);
                }
            }
        }
    }

    Ok(mod_map)
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
// NOTE: Known Design Limitation
// * Currently uses full version, e.g. "0.5.142", requiring full re-map for smaller changes.
//  * Consider using semver as default, and adding a config (not implemented) for user-specific
//  choices on frequency of remap (breaking, major, minor, never, etc) due to versioning changes.
//  * Fine for now.
pub fn derive_crate_namespace(name: &str, version: &str) -> Uuid {
    // Combine name and version to form the unique identifier string within the project namespace.
    // Using "@" is a common convention.
    let name_version = format!("{}@{}", name, version);
    Uuid::new_v5(&PROJECT_NAMESPACE_UUID, name_version.as_bytes())
}
