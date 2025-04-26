use ploke_core::PROJECT_NAMESPACE_UUID;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt; // Import fmt for Display trait
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc; // Import Arc
use thiserror::Error;
use toml;
use uuid::Uuid;
use walkdir::WalkDir;

// PROJECT_NAMESPACE_UUID is now defined in ploke_core
// The old comment block explaining it remains relevant but the constant itself is moved.
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

/// Errors that can occur during the discovery phase.
#[derive(Error, Debug, Clone)] // Add Clone derive
pub enum DiscoveryError {
    #[error("I/O error accessing path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>, // Wrap in Arc
    },
    #[error("Failed to parse Cargo.toml at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: Arc<toml::de::Error>, // Wrap in Arc
    },
    #[error("Missing 'package.name' in Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf }, // This variant is already Clone
    #[error("Missing 'package.version' in Cargo.toml at {path}")]
    MissingPackageVersion { path: PathBuf },
    #[error("Target crate path not found: {path}")]
    CratePathNotFound { path: PathBuf },
    #[error("Walkdir error in {path}: {source}")]
    Walkdir {
        path: PathBuf,
        #[source]
        source: Arc<walkdir::Error>, // Wrap in Arc
    },
    #[error("Source directory not found for crate at: {path}")]
    SrcNotFound { path: PathBuf }, // Critical error: Cannot proceed without source files.
    #[error("Multiple non-fatal errors occurred during discovery")]
    NonFatalErrors(Box<Vec<DiscoveryError>>), // Box to avoid large enum variant
}

// Helper structs for deserializing Cargo.toml

/// Represents the `[package]` section of Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
struct PackageInfo {
    name: String,
    version: String,
    // edition: Option<String>, // Could be useful later
}

impl PackageInfo {
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    fn new(name: String, version: String) -> Self {
        Self { name, version }
    }

    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    fn name(&self) -> &str {
        &self.name
    }

    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    fn version(&self) -> &str {
        &self.version
    }
}

impl fmt::Display for PackageInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.version)
    }
}

/// Represents the `[features]` section. Keys are feature names, values are lists of enabled features/dependencies.
#[derive(Deserialize, Debug, Clone, Default, Serialize)]
pub struct Features(HashMap<String, Vec<String>>);

impl Features {
    /// Returns a reference to the list of features/dependencies enabled by the given feature name.
    pub fn get(&self, feature_name: &str) -> Option<&Vec<String>> {
        self.0.get(feature_name)
    }

    /// Returns `true` if the map contains the specified feature name.
    pub fn contains_key(&self, feature_name: &str) -> bool {
        self.0.contains_key(feature_name)
    }

    /// Returns an iterator over the feature names and their corresponding enabled items.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Vec<String>)> {
        self.0.iter()
    }

    /// Returns an iterator over the feature names.
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.0.keys()
    }

    /// Returns an iterator over the lists of enabled items for each feature.
    pub fn values(&self) -> impl Iterator<Item = &Vec<String>> {
        self.0.values()
    }
}

/// Represents a dependency specification, which can be a simple version string
/// or a more detailed table.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)] // Allows parsing either a string or a table
pub enum DependencySpec {
    Version(String),
    Detailed {
        version: Option<String>,
        path: Option<String>,
        git: Option<String>,
        branch: Option<String>,
        tag: Option<String>,
        rev: Option<String>,
        features: Option<Vec<String>>,
        optional: Option<bool>,
        #[serde(rename = "default-features")]
        default_features: Option<bool>,
        // Add other fields like 'package' if needed
    },
}

impl DependencySpec {
    /// Returns `true` if the dependency spec is [`Detailed`].
    ///
    /// [`Detailed`]: DependencySpec::Detailed
    #[must_use]
    pub fn is_detailed(&self) -> bool {
        matches!(self, Self::Detailed { .. })
    }

    /// Returns `true` if the dependency spec is [`Version`].
    ///
    /// [`Version`]: DependencySpec::Version
    #[must_use]
    pub fn is_version(&self) -> bool {
        matches!(self, Self::Version(..))
    }

    pub fn as_version(&self) -> Option<&String> {
        if let Self::Version(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn try_into_version(self) -> Result<String, Box<Self>> {
        if let Self::Version(v) = self {
            Ok(v)
        } else {
            // Box the error variant to reduce the size of the Result's error type
            Err(Box::new(self))
        }
    }

    // --- Convenience Getters for Detailed variant ---

    /// Returns the version string if this is a detailed spec.
    pub fn version(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { version, .. } => version.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the path string if this is a detailed spec.
    pub fn path(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { path, .. } => path.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the git URL string if this is a detailed spec.
    pub fn git(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { git, .. } => git.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the branch name string if this is a detailed spec.
    pub fn branch(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { branch, .. } => branch.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the tag string if this is a detailed spec.
    pub fn tag(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { tag, .. } => tag.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the revision string if this is a detailed spec.
    pub fn rev(&self) -> Option<&str> {
        match self {
            DependencySpec::Detailed { rev, .. } => rev.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns the list of features if this is a detailed spec.
    pub fn features(&self) -> Option<&[String]> {
        match self {
            DependencySpec::Detailed { features, .. } => features.as_deref(),
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns whether the dependency is optional if this is a detailed spec.
    pub fn is_optional(&self) -> Option<bool> {
        match self {
            DependencySpec::Detailed { optional, .. } => *optional,
            DependencySpec::Version(_) => None,
        }
    }

    /// Returns whether default features are enabled if this is a detailed spec.
    pub fn has_default_features(&self) -> Option<bool> {
        match self {
            DependencySpec::Detailed {
                default_features, ..
            } => *default_features,
            DependencySpec::Version(_) => None,
        }
    }
}

/// Represents the `[dependencies]` section.
#[derive(Deserialize, Debug, Clone, Default)]
pub struct Dependencies(HashMap<String, DependencySpec>);

/// Represents the `[dev-dependencies]` section.
#[derive(Deserialize, Debug, Clone, Default)]
#[serde(rename = "dev-dependencies")] // Match the TOML key
pub struct DevDependencies(HashMap<String, DependencySpec>);

/// Represents the overall structure of a parsed Cargo.toml manifest.
#[derive(Deserialize, Debug)]
struct CargoManifest {
    package: PackageInfo,
    #[serde(default)] // Use default empty map if section is missing
    features: Features,
    #[serde(default)]
    dependencies: Dependencies,
    #[serde(default)]
    #[serde(rename = "dev-dependencies")]
    dev_dependencies: DevDependencies,
    // Add other fields like [lib], [bin] if needed later for module mapping
}

/// Context information gathered for a single crate during discovery.
///
/// This struct automatically implements `Send + Sync` because all its members
/// (`String`, `Uuid`, `PathBuf`, `Vec<PathBuf>`) are `Send + Sync`.
#[derive(Debug, Clone, Deserialize)]
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
    /// Parsed features from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    features: Features,
    /// Parsed dependencies from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    dependencies: Dependencies,
    /// Parsed dev-dependencies from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    dev_dependencies: DevDependencies,
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
    /// Context information for each successfully discovered crate, keyed by the absolute crate root path.
    pub crate_contexts: HashMap<PathBuf, CrateContext>,
    // Removed initial_module_map: HashMap<PathBuf, Vec<String>>
    /// A list of non-fatal errors (warnings) encountered during discovery.
    /// The caller can inspect this list to decide how to handle issues like
    /// walkdir errors or module scanning problems that didn't prevent the
    /// overall discovery process from completing for the affected crate(s).
    /// Note: Missing `src` directories are treated as critical errors and will
    /// cause `run_discovery_phase` to return `Err`, not just add a warning here.
    pub warnings: Vec<DiscoveryError>,
}

impl DiscoveryOutput {
    /// Returns a reference to the `CrateContext` for the given crate root path, if found.
    pub fn get_crate_context(&self, crate_root_path: &Path) -> Option<&CrateContext> {
        self.crate_contexts.get(crate_root_path)
    }

    /// Returns an iterator over the crate root paths and their corresponding `CrateContext`.
    pub fn iter_crate_contexts(&self) -> impl Iterator<Item = (&PathBuf, &CrateContext)> + '_ {
        self.crate_contexts.iter()
    }

    /// Returns a slice containing all non-fatal warnings collected during discovery.
    pub fn warnings(&self) -> &[DiscoveryError] {
        &self.warnings
    }

    /// Returns `true` if any non-fatal warnings were collected during discovery.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
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
/// * `target_crates` - A slice of paths pointing to the root directories of the crates to analyze.
///
/// # Returns
/// A `Result` containing the `DiscoveryOutput` on success, or the first critical
/// `DiscoveryError` encountered during processing (e.g., `CratePathNotFound`,
/// `Io` error reading `Cargo.toml`, `TomlParse` error, `SrcNotFound`).
/// If successful, it means all target crates were processed without critical errors,
/// though non-fatal warnings might still be present in `DiscoveryOutput.warnings`.
// NOTE: Known limitations:
// * Assuming target_crates provides absolute paths for simplicity
//  * No UI design yet, but contract with `run_discovery_phase` should be that `run_discover_phase`
//  should only ever receive full paths. (Seperation of Concerns: UI vs Traversal)
pub fn run_discovery_phase(
    _project_root: &Path,      // Keep for potential future use
    target_crates: &[PathBuf], // Expecting absolute paths to crate root directories
) -> Result<DiscoveryOutput, DiscoveryError> {
    let mut crate_contexts = HashMap::new();
    // Removed: let mut initial_module_map = HashMap::new();
    let mut non_fatal_errors: Vec<DiscoveryError> = Vec::new(); // Collect non-fatal errors

    for crate_root_path in target_crates {
        // --- Check Crate Path Existence (Critical Error) ---
        if !crate_root_path.exists() || !crate_root_path.is_dir() {
            // This is considered critical, return immediately.
            return Err(DiscoveryError::CratePathNotFound {
                path: crate_root_path.clone(),
            });
            // No need to continue if the path is invalid.
        }

        // --- 3.2.2 Implement Cargo.toml Parsing (Critical Errors) ---
        let cargo_toml_path = crate_root_path.join("Cargo.toml");
        let cargo_content = match fs::read_to_string(&cargo_toml_path) {
            Ok(content) => content,
            Err(e) => {
                // Critical error: Cannot proceed without Cargo.toml content.
                return Err(DiscoveryError::Io {
                    path: cargo_toml_path.clone(),
                    source: Arc::new(e), // Wrap error in Arc
                });
            }
        };
        let manifest: CargoManifest = match toml::from_str(&cargo_content) {
            Ok(m) => m,
            Err(e) => {
                // Critical error: Invalid TOML structure prevents further processing.
                return Err(DiscoveryError::TomlParse {
                    path: cargo_toml_path.clone(),
                    source: Arc::new(e), // Wrap error in Arc
                });
            }
        };

        // --- Extract Package Info (Non-Fatal Errors) ---
        // Although PackageInfo deserialization requires name/version, we handle potential
        // future scenarios or direct struct manipulation by checking here.
        // For now, serde handles this, but let's keep the structure for robustness.
        let crate_name = manifest.package.name.clone(); // Assume present due to serde
        let crate_version = manifest.package.version.clone(); // Assume present due to serde

        // --- Extract Optional Sections ---
        let features = manifest.features; // Cloned implicitly by struct move/copy if needed later
        let dependencies = manifest.dependencies;
        let dev_dependencies = manifest.dev_dependencies;

        // --- 3.2.3 Implement Namespace Generation (Called below) ---
        let namespace = derive_crate_namespace(&crate_name, &crate_version);

        // --- 3.2.1 Implement File Discovery Logic ---
        let src_path = crate_root_path.join("src");
        let mut files = Vec::new();

        if !src_path.exists() || !src_path.is_dir() {
            // Critical error: Cannot proceed without a source directory.
            return Err(DiscoveryError::SrcNotFound {
                path: src_path.clone(),
            });
        } else {
            // --- Perform File Discovery (Non-Fatal Walkdir Errors) ---
            let walker = WalkDir::new(&src_path).into_iter();
            for entry_result in walker {
                match entry_result {
                    Ok(entry) => {
                        if entry.file_type().is_file()
                            && entry.path().extension().is_some_and(|ext| ext == "rs")
                        {
                            files.push(entry.path().to_path_buf());
                        }
                    }
                    Err(e) => {
                        // Non-fatal: Log, collect error, and continue walking.
                        let path = e.path().unwrap_or(&src_path).to_path_buf();
                        eprintln!("Warning: Error walking directory {:?}: {}", path, e);
                        // Note: walkdir::Error might not directly implement Error needed for #[source]
                        // depending on its structure. Wrapping it ensures compatibility.
                        // If walkdir::Error *does* implement std::error::Error, Arc::new(e) is fine.
                        // If not, we might need Arc::new(e.into_io_error().unwrap_or_else(...)) or similar.
                        // Assuming walkdir::Error implements std::error::Error for now.
                        non_fatal_errors.push(DiscoveryError::Walkdir {
                            path,
                            source: Arc::new(e), // Wrap error in Arc
                        });
                    }
                }
            }
        }

        // --- Combine into CrateContext (Always created, might have empty files) ---
        let context = CrateContext {
            name: crate_name.clone(),
            version: crate_version,
            namespace,
            root_path: crate_root_path.clone(),
            files: files.clone(), // Clone needed for module mapping below
            features,             // Add the parsed features
            dependencies,         // Add the parsed dependencies
            dev_dependencies,     // Add the parsed dev-dependencies
        };

        // Removed: Initial Module Mapping section (scan_for_mods call)

        // Add context regardless of non-fatal errors encountered for this crate.
        crate_contexts.insert(crate_root_path.clone(), context);
    } // End of loop for target_crates

    // --- Final Check and Return ---
    // Always return Ok if critical errors didn't occur.
    // Non-fatal errors are packaged into the DiscoveryOutput.
    Ok(DiscoveryOutput {
        crate_contexts,
        // Removed: initial_module_map,
        warnings: non_fatal_errors, // Include collected warnings
    })
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
// Removed: scan_for_mods function

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_derive_crate_namespace_consistency() {
        let ns1 = derive_crate_namespace("my-crate", "1.0.0");
        let ns2 = derive_crate_namespace("my-crate", "1.0.0");
        assert_eq!(
            ns1, ns2,
            "Namespace should be consistent for the same input"
        );
    }

    #[test]
    fn test_derive_crate_namespace_uniqueness() {
        let ns1 = derive_crate_namespace("my-crate", "1.0.0");
        let ns2 = derive_crate_namespace("my-crate", "1.0.1");
        let ns3 = derive_crate_namespace("other-crate", "1.0.0");
        assert_ne!(
            ns1, ns2,
            "Different versions should produce different namespaces"
        );
        assert_ne!(
            ns1, ns3,
            "Different crate names should produce different namespaces"
        );
    }

    // Removed tests for scan_for_mods:
    // - test_scan_for_mods_simple_file
    // - test_scan_for_mods_simple_dir
    // - test_scan_for_mods_no_mods
    // - test_scan_for_mods_target_missing
    // - test_scan_for_mods_file_not_found
}
