//! The discovery phase of the parsing process.
//!
//! This module is responsible for finding all `.rs` files in a given crate,
//! parsing the `Cargo.toml` file, and generating a `CrateContext` for each
//! crate. The `CrateContext` contains information about the crate, such as
//! its name, version, and a list of all its source files.
//!
//! ## Workflow Overview
//! 1. [`run_discovery_phase`] orchestrates manifest parsing, namespace derivation, and file crawling.
//! 2. [`locate_workspace_manifest`] / [`resolve_workspace_version`] resolve workspace-inherited metadata.
//! 3. [`CrateContext`] and [`DiscoveryOutput`] keep the resulting data immutable for Phase 2.
//!
//! These entry points are intentionally narrow so downstream phases can depend on strongly typed
//! structs instead of re-reading `Cargo.toml` or the filesystem.

use ploke_core::PROJECT_NAMESPACE_UUID;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::discovery::DiscoveryError;
use crate::discovery::workspace::WorkspaceVersionLink;
use crate::discovery::workspace::resolve_workspace_version;

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

// Helper structs for deserializing Cargo.toml

/// Represents the `[package]` section of Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: PackageVersion,
    #[serde(default)]
    pub autobins: Option<bool>,
    #[serde(default)]
    pub autotests: Option<bool>,
    #[serde(default)]
    pub autoexamples: Option<bool>,
    #[serde(default)]
    pub autobenches: Option<bool>,
    // edition: Option<String>, // Could be useful later
}

impl fmt::Display for PackageInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.name, self.version)
    }
}

/// Describes where a crate's version string originates.
#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum PackageVersion {
    /// Version is specified directly in the crate's `Cargo.toml`.
    Explicit(String),
    /// Version is inherited from the workspace via `version.workspace = true`.
    Workspace(WorkspaceVersionLink),
}

impl fmt::Display for PackageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Explicit(version) => write!(f, "{version}"),
            Self::Workspace(_) => write!(f, "<workspace>"),
        }
    }
}

impl PackageVersion {
    /// Produce the concrete version string, loading workspace metadata when needed.
    ///
    /// # Success
    /// Returns a `String` borrowed from the crate manifest (`Explicit`) or from the workspace
    /// (`Workspace`) once validation passes.
    ///
    /// # Errors
    /// * [`DiscoveryError::WorkspaceVersionFlagDisabled`] if `workspace = false`.
    /// * Any error emitted by [`resolve_workspace_version`] when escalation to a workspace lookup fails.
    pub fn resolve(&self, crate_root: &Path) -> Result<String, DiscoveryError> {
        match self {
            PackageVersion::Explicit(version) => Ok(version.clone()),
            PackageVersion::Workspace(link) => {
                let workspace_path = resolve_workspace_version(crate_root)?;
                if !link.workspace {
                    return Err(DiscoveryError::WorkspaceVersionFlagDisabled {
                        crate_path: crate_root.to_path_buf(),
                        manifest_path: PathBuf::from(workspace_path),
                    });
                }
                resolve_workspace_version(crate_root)
            }
        }
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
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert("serde".to_string(), DependencySpec::Version("1.0".to_string()));
    /// let deps = Dependencies(map);
    ///
    /// // Walk the spec to pluck a version number similar to how later pipeline stages inspect manifests.
    /// let spec = deps.get("serde").unwrap();
    /// assert_eq!(spec.as_version(), Some(&"1.0".to_string()));
    /// ```
    fn get(&self, crate_name: &str) -> Option<&DependencySpec> {
        self.inner_map().get(crate_name)
    }

    /// Returns `true` if the dependencies map contains the specified crate name.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([(
    ///     "serde".to_string(),
    ///     DependencySpec::Detailed {
    ///         version: Some("1.0".to_string()),
    ///         path: None,
    ///         git: Some("https://github.com/serde-rs/serde".to_string()),
    ///         branch: Some("main".to_string()),
    ///         tag: None,
    ///         rev: None,
    ///         features: None,
    ///         optional: Some(false),
    ///         default_features: Some(true),
    ///     },
    /// )]));
    ///
    /// assert!(deps.contains_crate("serde"));
    /// assert!(!deps.contains_crate("tokio"));
    /// ```
    fn contains_crate(&self, crate_name: &str) -> bool {
        self.inner_map().contains_key(crate_name)
    }

    /// Returns an iterator over the dependency names (crate names).
    /// This is equivalent to iterating over the keys of the underlying map.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([
    ///     ("serde".to_string(), DependencySpec::Version("1.0".to_string())),
    ///     (
    ///         "tokio".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: Some("1.37".to_string()),
    ///             path: None,
    ///             git: None,
    ///             branch: None,
    ///             tag: None,
    ///             rev: None,
    ///             features: Some(vec!["rt".into(), "macros".into()]),
    ///             optional: None,
    ///             default_features: Some(false),
    ///         },
    ///     ),
    /// ]));
    ///
    /// let mut crate_names: Vec<_> = deps.names().map(|name| name.as_str()).collect();
    /// crate_names.sort();
    /// assert_eq!(crate_names, ["serde", "tokio"]);
    /// ```
    fn names(&self) -> impl Iterator<Item = &String> {
        self.inner_map().keys()
    }

    /// Returns an iterator over the dependency specifications.
    /// This is equivalent to iterating over the values of the underlying map.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([
    ///     (
    ///         "serde".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: Some("1.0".to_string()),
    ///             path: None,
    ///             git: Some("https://github.com/serde-rs/serde".to_string()),
    ///             branch: None,
    ///             tag: None,
    ///             rev: None,
    ///             features: Some(vec!["derive".into()]),
    ///             optional: Some(false),
    ///             default_features: Some(true),
    ///         },
    ///     ),
    ///     (
    ///         "tokio".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: Some("1.37".to_string()),
    ///             path: None,
    ///             git: None,
    ///             branch: None,
    ///             tag: None,
    ///             rev: None,
    ///             features: Some(vec!["macros".into()]),
    ///             optional: None,
    ///             default_features: Some(false),
    ///         },
    ///     ),
    /// ]));
    ///
    /// // Filter for dependencies that opt into additional features, similar to how we inspect manifests later.
    /// let crates_with_features = deps
    ///     .specs()
    ///     .filter(|spec| spec.features().map_or(false, |f| !f.is_empty()))
    ///     .count();
    /// assert_eq!(crates_with_features, 2);
    /// ```
    fn specs(&self) -> impl Iterator<Item = &DependencySpec> {
        self.inner_map().values()
    }

    /// Returns an iterator over the (crate name, dependency specification) pairs.
    /// This is equivalent to iterating over the items of the underlying map.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([
    ///     (
    ///         "serde".to_string(),
    ///         DependencySpec::Version("1.0".to_string()),
    ///     ),
    ///     (
    ///         "local-crate".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: None,
    ///             path: Some("../local-crate".to_string()),
    ///             git: None,
    ///             branch: None,
    ///             tag: None,
    ///             rev: None,
    ///             features: None,
    ///             optional: None,
    ///             default_features: None,
    ///         },
    ///     ),
    /// ]));
    ///
    /// // Partition dependencies by type (path vs registry) similar to discovery call sites.
    /// let (path_deps, registry_deps): (Vec<_>, Vec<_>) =
    ///     deps.iter().partition(|(_, spec)| spec.path().is_some());
    ///
    /// assert_eq!(path_deps[0].0.as_str(), "local-crate");
    /// assert_eq!(registry_deps[0].0.as_str(), "serde");
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
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([
    ///     (
    ///         "workspace-helper".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: None,
    ///             path: Some("../helper".to_string()),
    ///             git: None,
    ///             branch: None,
    ///             tag: None,
    ///             rev: None,
    ///             features: None,
    ///             optional: Some(true),
    ///             default_features: None,
    ///         },
    ///     ),
    ///     (
    ///         "serde".to_string(),
    ///         DependencySpec::Version("1.0".to_string()),
    ///     ),
    /// ]));
    ///
    /// let path_deps: Vec<_> = deps.path_dependencies().collect();
    /// assert_eq!(path_deps, [(&"workspace-helper".to_string(), "../helper")]);
    /// ```
    fn path_dependencies(&self) -> impl Iterator<Item = (&String, &str)> {
        self.inner_map()
            .iter()
            .filter_map(|(name, spec)| spec.path().map(|p| (name, p)))
    }

    // /// Returns an iterator over dependencies specified by a git repository.
    /// Returns an iterator over dependencies specified by a git repository.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{Dependencies, DependencyMap, DependencySpec};
    /// use std::collections::HashMap;
    ///
    /// let deps = Dependencies(HashMap::from([
    ///     (
    ///         "serde".to_string(),
    ///         DependencySpec::Detailed {
    ///             version: Some("1.0".to_string()),
    ///             path: None,
    ///             git: Some("https://github.com/serde-rs/serde".to_string()),
    ///             branch: Some("main".to_string()),
    ///             tag: None,
    ///             rev: None,
    ///             features: None,
    ///             optional: None,
    ///             default_features: Some(true),
    ///         },
    ///     ),
    ///     (
    ///         "tokio".to_string(),
    ///         DependencySpec::Version("1.37".to_string()),
    ///     ),
    /// ]));
    ///
    /// let git_deps: Vec<_> = deps.git_dependencies().collect();
    /// assert_eq!(
    ///     git_deps,
    ///     [(&"serde".to_string(), "https://github.com/serde-rs/serde")]
    /// );
    /// ```
    fn git_dependencies(&self) -> impl Iterator<Item = (&String, &str)> {
        self.inner_map()
            .iter()
            .filter_map(|(name, spec)| spec.git().map(|g| (name, g)))
    }
}

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

/// Represents a `[lib]` target section in Cargo.toml with a custom path.
#[derive(Deserialize, Debug, Clone)]
pub struct LibTarget {
    /// The path to the library root file (e.g., "lib.rs" or "src/lib.rs").
    /// Defaults to "src/lib.rs" if not specified, per Cargo convention.
    #[serde(default = "default_lib_path")]
    pub path: PathBuf,
}

fn default_lib_path() -> PathBuf {
    PathBuf::from("src/lib.rs")
}

/// Represents a `[[bin]]` target section in Cargo.toml with a custom path.
#[derive(Deserialize, Debug, Clone)]
pub struct BinTarget {
    /// The name of the binary.
    pub name: String,
    /// The path to the binary root file (e.g., "main.rs" or "src/main.rs").
    pub path: PathBuf,
}

/// Represents an explicit `[[test]]` target section in Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
pub struct TestTarget {
    /// Optional target name. If omitted, discovery derives one from the root file stem.
    pub name: Option<String>,
    /// The path to the test crate root file.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

/// Represents an explicit `[[example]]` target section in Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
pub struct ExampleTarget {
    /// Optional target name. If omitted, discovery derives one from the root file stem.
    pub name: Option<String>,
    /// The path to the example crate root file.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

/// Represents an explicit `[[bench]]` target section in Cargo.toml.
#[derive(Deserialize, Debug, Clone)]
pub struct BenchTarget {
    /// Optional target name. If omitted, discovery derives one from the root file stem.
    pub name: Option<String>,
    /// The path to the benchmark crate root file.
    #[serde(default)]
    pub path: Option<PathBuf>,
}

/// Cargo target kind surfaced by discovery.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum TargetKind {
    Lib,
    Bin,
    Test,
    Example,
    Bench,
}

/// A discovered target root emitted for a crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TargetSpec {
    pub kind: TargetKind,
    pub name: String,
    pub root: PathBuf,
}

/// Optional selector to focus discovery output on one target.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct TargetSelector {
    pub kind: TargetKind,
    pub name: String,
}

/// Represents the overall structure of a parsed Cargo.toml manifest.
#[derive(Deserialize, Debug)]
pub struct CargoManifest {
    pub package: PackageInfo,
    #[serde(default)] // Use default empty map if section is missing
    pub features: Features,
    #[serde(default)]
    pub dependencies: Dependencies,
    #[serde(default)]
    #[serde(rename = "dev-dependencies")]
    pub dev_dependencies: DevDependencies,
    /// The `[lib]` section, if present, containing custom library path configuration.
    pub lib: Option<LibTarget>,
    /// The `[[bin]]` sections, if present, containing binary target configurations.
    pub bin: Option<Vec<BinTarget>>,
    /// The `[[test]]` sections, if present, containing integration test target configurations.
    #[serde(default)]
    pub test: Option<Vec<TestTarget>>,
    /// The `[[example]]` sections, if present, containing example target configurations.
    #[serde(default)]
    pub example: Option<Vec<ExampleTarget>>,
    /// The `[[bench]]` sections, if present, containing benchmark target configurations.
    #[serde(default)]
    pub bench: Option<Vec<BenchTarget>>,
}

/// Context information gathered for a single crate during discovery.
///
/// This struct automatically implements `Send + Sync` because all its members
/// (`String`, `Uuid`, `PathBuf`, `Vec<PathBuf>`) are `Send + Sync`.
#[derive(Debug, Clone, Deserialize)]
pub struct CrateContext {
    /// The simple name of the crate (e.g., "syn_parser").
    pub name: String,
    /// The resolved version string for the crate (e.g., "0.1.0").
    pub version: String,
    /// The UUID namespace derived for this specific crate using
    /// `Uuid::new_v5(&PROJECT_NAMESPACE_UUID, ...)`.
    pub namespace: Uuid,
    /// The absolute path to the crate's root directory (containing Cargo.toml).
    pub root_path: PathBuf,
    /// List of all `.rs` files found within the crate's source directories.
    pub files: Vec<PathBuf>,
    /// Enumerated Cargo target roots discovered for this crate.
    pub targets: Vec<TargetSpec>,
    /// Parsed features from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving features")]
    pub features: Features,
    /// Parsed dependencies from Cargo.toml.
    #[allow(unused_variables, reason = "Useful later for resolving dependencies")]
    pub dependencies: Dependencies,
    /// Parsed dev-dependencies from Cargo.toml.
    #[allow(
        unused_variables,
        reason = "Useful later for resolving dev dependencies"
    )]
    pub dev_dependencies: DevDependencies,
    /// Location of workspace, if present. Defaults to `None` and skips for serde, not
    /// reflective of Cargo.toml structure
    #[serde(skip)]
    pub workspace_path: Option<PathBuf>,
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
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{run_discovery_phase, DiscoveryOutput};
    /// use tempfile::tempdir;
    /// use std::fs;
    ///
    /// let root = tempdir().unwrap();
    /// let crate_root = root.path().join("demo");
    /// fs::create_dir_all(crate_root.join("src")).unwrap();
    /// fs::write(
    ///     root.path().join("Cargo.toml"),
    ///     "[workspace]\nmembers = [\"demo\"]\n",
    /// ).unwrap();
    /// fs::write(
    ///     crate_root.join("Cargo.toml"),
    ///     r#"[package]
    /// name = "demo"
    /// version = "0.1.0"
    /// edition = "2021"
    /// "#,
    /// ).unwrap();
    /// fs::write(crate_root.join("src/lib.rs"), "pub fn demo() {}").unwrap();
    ///
    /// let discovery = run_discovery_phase(Some(root.path()), &[crate_root.clone()]).unwrap();
    /// let context = discovery.get_crate_context(&crate_root).unwrap();
    /// assert_eq!(context.name, "demo");
    /// ```
    pub fn get_crate_context(&self, crate_root_path: &Path) -> Option<&CrateContext> {
        self.crate_contexts.get(crate_root_path)
    }

    /// Returns an iterator over the crate root paths and their corresponding `CrateContext`.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::run_discovery_phase;
    /// use tempfile::tempdir;
    /// use std::fs;
    ///
    /// let root = tempdir().unwrap();
    /// fs::write(
    ///     root.path().join("Cargo.toml"),
    ///     "[workspace]\nmembers = [\"crate_a\", \"crate_b\"]\n",
    /// ).unwrap();
    /// for (name, version) in [("crate_a", "0.1.0"), ("crate_b", "0.2.0")] {
    ///     let crate_root = root.path().join(name);
    ///     fs::create_dir_all(crate_root.join("src")).unwrap();
    ///     fs::write(
    ///         crate_root.join("Cargo.toml"),
    ///         format!(
    ///             "[package]\nname = \"{}\"\nversion = \"{}\"\nedition = \"2021\"\n",
    ///             name, version
    ///         ),
    ///     )
    ///     .unwrap();
    ///     fs::write(crate_root.join("src/lib.rs"), format!("pub fn {}_fn() {{}}\n", name)).unwrap();
    /// }
    ///
    /// let crate_paths = ["crate_a", "crate_b"]
    ///     .into_iter()
    ///     .map(|name| root.path().join(name))
    ///     .collect::<Vec<_>>();
    /// let discovery = run_discovery_phase(Some(root.path()), &crate_paths).unwrap();
    ///
    /// let mut names: Vec<_> = discovery
    ///     .iter_crate_contexts()
    ///     .map(|(_, ctx)| ctx.name.as_str())
    ///     .collect();
    /// names.sort();
    /// assert_eq!(names, ["crate_a", "crate_b"]);
    /// ```
    pub fn iter_crate_contexts(&self) -> impl Iterator<Item = (&PathBuf, &CrateContext)> + '_ {
        self.crate_contexts.iter()
    }

    /// Returns a slice containing all non-fatal warnings collected during discovery.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{DiscoveryError, DiscoveryOutput};
    /// use std::collections::HashMap;
    /// use std::path::PathBuf;
    ///
    /// let warning = DiscoveryError::MissingPackageName {
    ///     path: PathBuf::from("/tmp/bad/Cargo.toml"),
    /// };
    /// let discovery = DiscoveryOutput {
    ///     crate_contexts: HashMap::new(),
    ///     workspace: None,
    ///     warnings: vec![warning.clone()],
    /// };
    ///
    /// assert!(matches!(
    ///     discovery.warnings(),
    ///     [DiscoveryError::MissingPackageName { .. }]
    /// ));
    /// ```
    pub fn warnings(&self) -> &[DiscoveryError] {
        &self.warnings
    }

    /// Returns `true` if any non-fatal warnings were collected during discovery.
    ///
    /// # Example
    /// ```
    /// use syn_parser::discovery::{DiscoveryError, DiscoveryOutput};
    /// use std::collections::HashMap;
    /// use std::path::PathBuf;
    ///
    /// let discovery = DiscoveryOutput {
    ///     crate_contexts: HashMap::new(),
    ///     workspace: None,
    ///     warnings: vec![DiscoveryError::MissingPackageName {
    ///         path: PathBuf::from("/tmp/bad/Cargo.toml"),
    ///     }],
    /// };
    ///
    /// assert!(discovery.has_warnings());
    ///
    /// let clean = DiscoveryOutput {
    ///     crate_contexts: HashMap::new(),
    ///     workspace: None,
    ///     warnings: vec![],
    /// };
    /// assert!(!clean.has_warnings());
    /// ```
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

/// Derives a deterministic UUID v5 namespace for a specific crate.
///
/// This function is intended to run single-threaded as part of the discovery setup.
///
/// # Arguments
/// * `name` - The name of the crate.
///
/// # Returns
/// A `Uuid` representing the namespace for this crate, derived from
/// the `PROJECT_NAMESPACE_UUID`.
pub fn derive_crate_namespace(name: &str, _version: &str) -> Uuid {
    // Combine crate name within the project namespace for stable UUIDs across version changes.
    Uuid::new_v5(&PROJECT_NAMESPACE_UUID, name.as_bytes())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

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
        assert_eq!(
            ns1, ns2,
            "Different versions should produce the same namespace"
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

    #[test]
    fn test_lib_target_default_path() {
        // Test that [lib] section without explicit path defaults to "src/lib.rs"
        let cargo_toml = r#"
[package]
name = "test-proc-macro"
version = "0.1.0"
edition = "2021"

[lib]
proc-macro = true
"#;

        let manifest: CargoManifest =
            toml::from_str(cargo_toml).expect("Should parse manifest with [lib] but no path");
        let lib = manifest.lib.expect("Should have lib section");
        assert_eq!(
            lib.path,
            PathBuf::from("src/lib.rs"),
            "Default path should be src/lib.rs"
        );
    }

    #[test]
    fn test_lib_target_explicit_path() {
        // Test that explicit path in [lib] is respected
        let cargo_toml = r#"
[package]
name = "test-crate"
version = "0.1.0"
edition = "2021"

[lib]
path = "src/my_lib.rs"
"#;

        let manifest: CargoManifest =
            toml::from_str(cargo_toml).expect("Should parse manifest with explicit lib path");
        let lib = manifest.lib.expect("Should have lib section");
        assert_eq!(
            lib.path,
            PathBuf::from("src/my_lib.rs"),
            "Explicit path should be preserved"
        );
    }
}
