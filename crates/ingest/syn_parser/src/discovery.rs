//! The discovery phase of the parsing process.
//!
//! This module is responsible for finding all `.rs` files in a given crate,
//! parsing the `Cargo.toml` file, and generating a `CrateContext` for each
//! crate. The `CrateContext` contains information about the crate, such as
//! its name, version, and a list of all its source files.

use itertools::Itertools;
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

use crate::error::SynParserError;

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
    /// An I/O error occurred while accessing a path.
    #[error("I/O error accessing path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: Arc<std::io::Error>, // Wrap in Arc
    },
    /// Failed to parse a `Cargo.toml` file.
    #[error("Failed to parse Cargo.toml at {path}: {source}")]
    TomlParse {
        path: PathBuf,
        #[source]
        source: Arc<toml::de::Error>, // Wrap in Arc
    },
    /// The `package.name` field was missing from a `Cargo.toml` file.
    #[error("Missing 'package.name' in Cargo.toml at {path}")]
    MissingPackageName { path: PathBuf }, // This variant is already Clone
    /// The `package.version` field was missing from a `Cargo.toml` file.
    #[error("Missing 'package.version' in Cargo.toml at {path}")]
    MissingPackageVersion { path: PathBuf },
    /// The target crate path was not found.
    #[error("Target crate path not found: {path}")]
    CratePathNotFound { path: PathBuf },
    /// An error occurred while walking a directory.
    #[error("Walkdir error in {path}: {source}")]
    Walkdir {
        path: PathBuf,
        #[source]
        source: Arc<walkdir::Error>, // Wrap in Arc
    },
    /// The source directory was not found for a crate.
    #[error("Source directory not found for crate at: {path}")]
    SrcNotFound { path: PathBuf }, // Critical error: Cannot proceed without source files.
    /// Multiple non-fatal errors occurred during discovery.
    #[error("Multiple non-fatal errors occurred during discovery")]
    NonFatalErrors(Box<Vec<DiscoveryError>>), // Box to avoid large enum variant
}

impl TryFrom<DiscoveryError> for SynParserError {
    type Error = SynParserError;

    fn try_from(value: DiscoveryError) -> Result<Self, Self::Error> {
        use DiscoveryError::*;
        Ok(match value {
            MissingPackageName { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            MissingPackageVersion { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            CratePathNotFound { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            SrcNotFound { path } => SynParserError::SimpleDiscovery {
                path: path.display().to_string(),
            },
            Io { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "Io".to_string(),
            },
            TomlParse { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "Toml".to_string(),
            },
            Walkdir { path, .. } => SynParserError::ComplexDiscovery {
                name: "".to_string(),
                path: path.display().to_string(),
                source_string: "walkdir".to_string(),
            },
            NonFatalErrors(..) => todo!("Decide what to do with this one later."),
        })
    }
}

// Helper structs for deserializing Cargo.toml

/// Represents the `[package]` section of Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
struct PackageInfo {
    name: String,
    version: String,
    // edition: Option<String>, // Could be useful later
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
    /// A simple version string, e.g. `"1.0"`.
    Version(String),
    /// A more detailed dependency specification.
    Detailed {
        /// The version of the dependency.
        version: Option<String>,
        /// The path to the dependency.
        path: Option<String>,
        /// The git repository of the dependency.
        git: Option<String>,
        /// The git branch of the dependency.
        branch: Option<String>,
        /// The git tag of the dependency.
        tag: Option<String>,
        /// The git revision of the dependency.
        rev: Option<String>,
        /// The features to enable for the dependency.
        features: Option<Vec<String>>,
        /// Whether the dependency is optional.
        optional: Option<bool>,
        /// Whether to use the default features of the dependency.
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

    /// Returns the version string if this is a version spec.
    pub fn as_version(&self) -> Option<&String> {
        if let Self::Version(v) = self {
            Some(v)
        } else {
            None
        }
    }

    /// Tries to convert the dependency spec into a version string.
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
pub struct Dependencies(pub HashMap<String, DependencySpec>); // Made inner map public for direct access if needed

/// A trait for accessing dependency information.
pub trait DependencyMap {
    // Associated type for the inner map (optional, but can be useful)
    // type InnerMap = HashMap<String, DependencySpec>;

    // TODO: Turn these tests back on once the migration to typed ids is complete.
    /// Returns a reference to the inner map of dependencies.
    fn inner_map(&self) -> &HashMap<String, DependencySpec>;
    /// Returns a reference to the dependency specification for the given crate name, if it exists.
    ///
    /// # Example
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use syn_parser::discovery::{Dependencies, DependencySpec}; // Adjust path as needed
    /// # let mut map = HashMap::new();
    /// # map.insert("serde".to_string(), DependencySpec::Version("1.0".to_string()));
    /// # let deps = Dependencies(map);
    /// if let Some(spec) = deps.get("serde") {
    ///     // ... use spec ...
    /// }
    /// ```
    fn get(&self, crate_name: &str) -> Option<&DependencySpec> {
        self.inner_map().get(crate_name)
    }

    /// Returns `true` if the dependencies map contains the specified crate name.
    ///
    /// # Example
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use syn_parser::discovery::{Dependencies, DependencySpec}; // Adjust path as needed
    /// # let deps = Dependencies(HashMap::new());
    /// if deps.contains_crate("serde") {
    ///     // ...
    /// }
    /// ```
    fn contains_crate(&self, crate_name: &str) -> bool {
        self.inner_map().contains_key(crate_name)
    }

    /// Returns an iterator over the dependency names (crate names).
    /// This is equivalent to iterating over the keys of the underlying map.
    ///
    /// # Example
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use syn_parser::discovery::{Dependencies, DependencySpec}; // Adjust path as needed
    /// # let deps = Dependencies(HashMap::new());
    /// for crate_name in deps.names() {
    ///     println!("Dependency: {}", crate_name);
    /// }
    /// ```
    fn names(&self) -> impl Iterator<Item = &String> {
        self.inner_map().keys()
    }

    /// Returns an iterator over the dependency specifications.
    /// This is equivalent to iterating over the values of the underlying map.
    ///
    /// # Example
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use syn_parser::discovery::{Dependencies, DependencySpec}; // Adjust path as needed
    /// # let deps = Dependencies(HashMap::new());
    /// for spec in deps.specs() {
    ///     // ... inspect spec ...
    /// }
    /// ```
    fn specs(&self) -> impl Iterator<Item = &DependencySpec> {
        self.inner_map().values()
    }

    /// Returns an iterator over the (crate name, dependency specification) pairs.
    /// This is equivalent to iterating over the items of the underlying map.
    ///
    /// # Example
    /// ```ignore
    /// # use std::collections::HashMap;
    /// # use syn_parser::discovery::{Dependencies, DependencySpec}; // Adjust path as needed
    /// # let deps = Dependencies(HashMap::new());
    /// for (name, spec) in deps.iter() {
    ///     println!("Dep: {}, Spec: {:?}", name, spec);
    /// }
    /// ```
    fn iter(&self) -> impl Iterator<Item = (&String, &DependencySpec)> {
        self.inner_map().iter()
    }

    /// Returns the number of dependencies listed.
    fn len(&self) -> usize {
        self.inner_map().len()
    }

    /// Returns `true` if there are no dependencies listed.
    fn is_empty(&self) -> bool {
        self.inner_map().is_empty()
    }

    // --- Potential Future Additions (Consider if needed) ---

    // /// Returns an iterator over dependencies specified by a local path.
    /// Returns an iterator over dependencies specified by a local path.
    fn path_dependencies(&self) -> impl Iterator<Item = (&String, &str)> {
        self.inner_map()
            .iter()
            .filter_map(|(name, spec)| spec.path().map(|p| (name, p)))
    }

    // /// Returns an iterator over dependencies specified by a git repository.
    /// Returns an iterator over dependencies specified by a git repository.
    fn git_dependencies(&self) -> impl Iterator<Item = (&String, &str)> {
        self.inner_map()
            .iter()
            .filter_map(|(name, spec)| spec.git().map(|g| (name, g)))
    }
}

// --- Optional Trait Implementations ---

// Allow iterating directly over the Dependencies struct
impl<'a> IntoIterator for &'a Dependencies {
    type Item = (&'a String, &'a DependencySpec);
    type IntoIter = std::collections::hash_map::Iter<'a, String, DependencySpec>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

// Allow getting the inner map if direct HashMap methods are needed
impl AsRef<HashMap<String, DependencySpec>> for Dependencies {
    fn as_ref(&self) -> &HashMap<String, DependencySpec> {
        &self.0
    }
}
// Allow getting the inner map if direct HashMap methods are needed
impl AsRef<HashMap<String, DependencySpec>> for DevDependencies {
    fn as_ref(&self) -> &HashMap<String, DependencySpec> {
        &self.0
    }
}

// 2. Implement the Trait for Dependencies
impl DependencyMap for Dependencies {
    fn inner_map(&self) -> &HashMap<String, DependencySpec> {
        &self.0 // Access the inner HashMap
    }
    // Default implementations from the trait are automatically inherited
}

// 3. Implement the Trait for DevDependencies
impl DependencyMap for DevDependencies {
    fn inner_map(&self) -> &HashMap<String, DependencySpec> {
        &self.0 // Access the inner HashMap
    }
    // Default implementations from the trait are automatically inherited
}

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
    #[allow(unused_variables, reason = "Useful later for resolving features")]
    features: Features,
    /// Parsed dependencies from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    dependencies: Dependencies,
    /// Parsed dev-dependencies from Cargo.toml.
    #[allow(
        unused_variables,
        reason = "Useful later for resolving dev dependencies"
    )]
    dev_dependencies: DevDependencies,
}

impl CrateContext {
    /// Returns the features of the crate.
    pub fn features(&self) -> &Features {
        &self.features
    }

    /// Returns the dependencies of the crate.
    pub fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    /// Returns the dev-dependencies of the crate.
    pub fn dev_dependencies(&self) -> &DevDependencies {
        &self.dev_dependencies
    }
    /// Returns `true` if the crate is a binary crate.
    pub fn is_bin(&self) -> bool {
        self.files.iter().any(|fp| fp.ends_with("main.rs"))
    }
    /// Returns `true` if the crate is a library crate.
    pub fn is_lib(&self) -> bool {
        self.files.iter().any(|fp| fp.ends_with("lib.rs"))
    }
    /// Returns the root file of the crate.
    pub fn root_file(&self) -> Option<&Path> {
        self.files
            .iter()
            .filter(|fp| fp.ends_with("main.rs") || fp.ends_with("lib.rs"))
            .map(|fp| fp.as_path())
            .next()
    }
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
    /// Context information for each successfully discovered crate, keyed by the absolute crate
    /// root path.
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
                        // eprintln!("Warning: Error walking directory {:?}: {}", path, e);
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
            // Debugging:
            // let env_vars = std::env::vars()
            //     .filter(|(k, _)| k.starts_with("CARGO_"))
            //     .collect::<Vec<_>>();
            // println!("discovery: will parse files with std::env::vars() {:?}", env_vars);
            // println!("discovery: will parse files (stripping prefix)");
            // for file in &files {
            //     let different_vars = std::env::vars()
            //         .filter(|(k, _)| k.starts_with("CARGO_"))
            //         .filter(|item| !env_vars.contains(item))
            //         .collect::<Vec<_>>();
            //     println!(
            //         "\t{} with changed cfgs = {:?}",
            //         file.strip_prefix(current_dir().expect("error getting current dir"))
            //             .expect("error stripping prefix")
            //             .display(),
            //         different_vars
            //     );
            // }
        }

        // WARN: We are not including the main.rs file (and hopefully not its imports either) in
        // the case of a project having both a main.rs and a lib.rs
        // - This is a stopgap for now. We would like to provide the user with the ability to parse
        // both of these code graphs into the database at the same time as separate packages in the
        // same crate, but it is beyond our scope for now.
        // - See [known limitation](ploke/docs/plans/uuid_refactor/01b_phase1_known_limitations.md)
        let files = if files
            .iter()
            .any(|p| p.file_name().is_some_and(|f| f == "lib.rs"))
        {
            files
                .into_iter()
                .filter(|p| p.file_name().is_some_and(|f| f != "main.rs"))
                .collect_vec()
        } else {
            files
        };

        // --- Combine into CrateContext (Always created, might have empty files) ---
        let context = CrateContext {
            name: crate_name.clone(),
            version: crate_version,
            namespace,
            root_path: crate_root_path.clone(),
            files,            // Clone needed for module mapping below
            features,         // Add the parsed features
            dependencies,     // Add the parsed dependencies
            dev_dependencies, // Add the parsed dev-dependencies
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

/// Conversion from DiscoveryError to ploke_error::Error
///
/// This conversion maps discovery-phase errors to appropriate ploke_error variants
/// based on their severity and impact on the overall parsing process:
///
/// - **FatalError::FileOperation**: Used for I/O errors and walkdir errors during
///   file discovery, as these indicate filesystem access issues.
/// - **FatalError::PathResolution**: Used for TOML parse errors and missing
///   critical files (Cargo.toml, src directory). These prevent any meaningful parsing.
/// - **WarningError**: Used for non-fatal discovery issues that don't prevent
///   parsing but may affect completeness.
///
/// This mapping maintains the dependency direction (syn_parser depends on ploke_error)
/// while providing clear error categorization for upstream error handling.
impl From<DiscoveryError> for ploke_error::Error {
    fn from(err: DiscoveryError) -> Self {
        match err {
            DiscoveryError::Io { path, source } => ploke_error::FatalError::FileOperation {
                operation: "read",
                path,
                source,
            }
            .into(),
            DiscoveryError::TomlParse { path, source } => ploke_error::FatalError::PathResolution {
                path: format!("Failed to parse Cargo.toml at {}", path.display()),
                source: Some(source),
            }
            .into(),
            DiscoveryError::MissingPackageName { path } => {
                ploke_error::FatalError::PathResolution {
                    path: format!("Missing package.name in Cargo.toml at {}", path.display()),
                    source: None,
                }
                .into()
            }
            DiscoveryError::MissingPackageVersion { path } => {
                ploke_error::FatalError::PathResolution {
                    path: format!(
                        "Missing package.version in Cargo.toml at {}",
                        path.display()
                    ),
                    source: None,
                }
                .into()
            }
            DiscoveryError::CratePathNotFound { path } => ploke_error::FatalError::PathResolution {
                path: format!("Crate path not found: {}", path.display()),
                source: None,
            }
            .into(),
            DiscoveryError::Walkdir { path, source } => {
                // Convert walkdir::Error to std::io::Error using string representation
                let io_error = std::io::Error::new(std::io::ErrorKind::Other, source.to_string());
                ploke_error::FatalError::FileOperation {
                    operation: "walk",
                    path,
                    source: Arc::new(io_error),
                }
                .into()
            }
            DiscoveryError::SrcNotFound { path } => ploke_error::FatalError::PathResolution {
                path: format!(
                    "Source directory not found for crate at: {}",
                    path.display()
                ),
                source: None,
            }
            .into(),
            DiscoveryError::NonFatalErrors(errors) => {
                // Convert the boxed vector to a warning about multiple issues
                ploke_error::WarningError::UnresolvedRef {
                    name: "Discovery phase".to_string(),
                    location: Some(format!("{} non-fatal errors occurred", errors.len())),
                }
                .into()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_discovery_error_conversion() {
        use std::io;
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let discovery_err = DiscoveryError::Io {
            path: PathBuf::from("/test/path"),
            source: Arc::new(io_error),
        };

        let ploke_err: ploke_error::Error = discovery_err.into();
        assert!(matches!(ploke_err, ploke_error::Error::Fatal(_)));
    }
}
