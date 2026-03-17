pub mod error;
pub mod single_crate;
pub mod workspace;

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use error::*;
pub use single_crate::*;
pub use workspace::{locate_workspace_manifest, resolve_workspace_version};

use itertools::Itertools as _;
use walkdir::WalkDir;

use crate::discovery::workspace::WorkspaceManifestMetadata;

/// Runs the single-threaded discovery phase to gather context about target crates.
///
/// This function executes before any parallel parsing begins. It identifies
/// target crates, parses their `Cargo.toml` files, generates namespaces,
/// finds all `.rs` source files, and performs an initial scan for module
/// declarations.
///
/// # Arguments
/// * `workspace_root` - Optional workspace root path for workspace-aware discovery.
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
    workspace_root: Option<&Path>,
    target_crates: &[PathBuf], // Expecting absolute paths to crate root directories
) -> Result<DiscoveryOutput, DiscoveryError> {
    let mut crate_contexts = HashMap::new();
    let mut non_fatal_errors: Vec<DiscoveryError> = Vec::new(); // Collect non-fatal errors

    // cache parsed workspaces
    let mut cached_workspaces: Vec<WorkspaceManifestMetadata> = Vec::new();

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
                    source: Arc::new(e),
                });
            }
        };
        let manifest: CargoManifest = match toml::from_str(&cargo_content) {
            Ok(m) => m,
            Err(e) => {
                // Critical error: Invalid TOML structure prevents further processing.
                return Err(DiscoveryError::TomlParse {
                    path: cargo_toml_path.clone(),
                    source: Arc::new(e),
                });
            }
        };

        let CargoManifest {
            package,
            features,
            dependencies,
            dev_dependencies,
        } = manifest;

        // --- Extract Package Info (Non-Fatal Errors) ---
        let crate_name = package.name.clone();
        let crate_version = package.version.resolve(crate_root_path)?;

        // --- Namespace Generation (Called below) ---
        let namespace = derive_crate_namespace(&crate_name, &crate_version);

        // --- File Discovery Logic ---
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
                    Ok(entry)
                        if entry.file_type().is_file()
                            && entry.path().extension().is_some_and(|ext| ext == "rs") =>
                    {
                        files.push(entry.path().to_path_buf());
                    }
                    Ok(_non_rust_file) => {
                        // non-rust file is fine, doesn't need a warning
                    }
                    Err(e) => {
                        // Non-fatal: Log, collect error, and continue walking.
                        let path = e.path().unwrap_or(&src_path).to_path_buf();
                        non_fatal_errors.push(DiscoveryError::Walkdir {
                            path,
                            source: Arc::new(e),
                        });
                    }
                }
            }
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

        let located_workspace_path: Option<PathBuf> = if let Some(workspace_path) = workspace_root {
            let metadata = cached_workspaces.iter().find(|w| {
                w.workspace.as_ref().is_some_and(|ws| {
                    ws.members.contains(crate_root_path) && ws.path == workspace_path
                })
            });
            if let Some(ws_metadata) = metadata {
                ws_metadata.workspace.as_ref().map(|w| w.path.clone())
            } else {
                let (manifest_path, located_metadata) =
                    match locate_workspace_manifest(crate_root_path) {
                        Ok((workspace_path, metadata)) => (workspace_path, metadata),
                        Err(e)
                            if matches!(e, DiscoveryError::WorkspaceManifestRead { .. })
                                || matches!(e, DiscoveryError::WorkspaceManifestParse { .. })
                                || matches!(
                                    e,
                                    DiscoveryError::WorkspaceManifestNotFound { .. }
                                ) =>
                        {
                            return Err(e);
                        }
                        Err(_) => {
                            panic!("locate_workspace_manifest must not return this error type")
                        }
                    };
                match located_metadata.workspace {
                    Some(ref workspace_section) => {
                        // cache found workspace
                        let path = workspace_section.path.clone();
                        if path == workspace_path {
                            cached_workspaces.push(located_metadata);
                            Some(path)
                        } else {
                            return Err(DiscoveryError::WorkspacePathMismatch {
                                crate_path: crate_root_path.to_path_buf(),
                                expected_workspace_path: workspace_path.to_path_buf(),
                                discovered_workspace_path: path,
                            });
                        }
                    }
                    None => {
                        return Err(DiscoveryError::WorkspaceMissingSection {
                            crate_path: crate_root_path.to_path_buf(),
                            workspace_path: manifest_path,
                            expected: String::from("workspace"),
                        })
                    }
                }
            }
        } else {
            None
        };

        // --- Combine into CrateContext (Always created, might have empty files) ---
        let context = CrateContext {
            name: crate_name.clone(),
            version: crate_version,
            namespace,
            root_path: crate_root_path.clone(),
            workspace_path: located_workspace_path,
            files,            // Clone needed for module mapping below
            features,         // Add the parsed features
            dependencies,     // Add the parsed dependencies
            dev_dependencies, // Add the parsed dev-dependencies
        };

        // Removed: Initial Module Mapping section (scan_for_mods call)

        // Add context regardless of non-fatal errors encountered for this crate.
        crate_contexts.insert(crate_root_path.clone(), context);
    } // End of loop for target_crates

    if let Some(workspace_root) = workspace_root {
        let discovered_workspace_paths = cached_workspaces
            .iter()
            .filter_map(|metadata| {
                metadata
                    .workspace
                    .as_ref()
                    .map(|section| section.path.clone())
            })
            .unique()
            .collect_vec();

        if discovered_workspace_paths.len() > 1 {
            return Err(DiscoveryError::MultipleWorkspacesDetected {
                expected_workspace_path: workspace_root.to_path_buf(),
                discovered_workspace_paths,
                crate_paths: target_crates.to_vec(),
            });
        }
    }

    let workspace = cached_workspaces.pop();
    if workspace_root.is_some() && !cached_workspaces.is_empty() {
        // return new discovery error about multiple workspaces, with directories and crates as
        // context.
        unreachable!("workspace cache must contain at most one unique workspace");
    }

    // --- Final Check and Return ---
    // Always return Ok if critical errors didn't occur.
    // Non-fatal errors are packaged into the DiscoveryOutput.
    Ok(DiscoveryOutput {
        crate_contexts,
        workspace,
        // Removed: initial_module_map,
        warnings: non_fatal_errors, // Include collected warnings
    })
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
    pub workspace: Option<workspace::WorkspaceManifestMetadata>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_common::workspace_root;
    use std::fs;
    use tempfile::tempdir;

    fn write_basic_crate(crate_root: &Path, name: &str, version: &str) {
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            format!(
                r#"[package]
name = "{name}"
version = "{version}"
edition = "2021"
"#
            ),
        )
        .unwrap();
        fs::write(crate_root.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
    }

    #[test]
    // test basic toml parsing of target crate
    fn test_toml_basic() -> Result<(), DiscoveryError> {
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_00"); // Use workspace root for context
        eprintln!(
            "fixture_workspace = {}",
            fixture_workspace_root.display()
        );
        assert!(
            fixture_workspace_root.is_dir(),
            "target fixture workspace expected to be a directory"
        );

        let crate_dir = fixture_workspace_root.join("fixture_toml");
        eprintln!("fixture_crate = {}", crate_dir.display());
        assert!(
            crate_dir.is_dir(),
            "target fixture crate expected to be a directory"
        );

        let discovery_result =
            run_discovery_phase(Some(&fixture_workspace_root), std::slice::from_ref(&crate_dir));
        println!("{discovery_result:#?}");
        let output = discovery_result?;
        let context = output
            .crate_contexts
            .get(&crate_dir)
            .expect("fixture_toml context missing");
        assert_eq!(
            context.version, "0.0.0",
            "version should be inherited from workspace"
        );
        Ok(())
    }

    #[test]
    fn test_workspace_metadata_is_normalized_for_membership_lookup() -> Result<(), DiscoveryError> {
        let fixture_workspace_root = workspace_root().join("tests/fixture_workspace/ws_fixture_00");
        let crate_dir = fixture_workspace_root.join("fixture_toml");
        let output = run_discovery_phase(
            Some(&fixture_workspace_root),
            std::slice::from_ref(&crate_dir),
        )?;

        let workspace = output
            .workspace
            .expect("workspace metadata should be cached");
        let workspace_section = workspace
            .workspace
            .expect("workspace metadata should include a workspace section");

        assert_eq!(workspace_section.path, fixture_workspace_root);
        assert!(workspace_section.members.contains(&crate_dir));
        Ok(())
    }

    #[test]
    fn test_workspace_path_mismatch_returns_explicit_error() {
        let tmp = tempdir().unwrap();

        let expected_workspace = tmp.path().join("expected");
        let discovered_workspace = tmp.path().join("discovered");
        let crate_root = discovered_workspace.join("member");

        fs::create_dir_all(&expected_workspace).unwrap();
        fs::write(
            expected_workspace.join("Cargo.toml"),
            "[workspace]\nmembers = []\n",
        )
        .unwrap();

        write_basic_crate(&crate_root, "member", "0.1.0");
        fs::write(
            discovered_workspace.join("Cargo.toml"),
            "[workspace]\nmembers = [\"member\"]\n",
        )
        .unwrap();

        let err = run_discovery_phase(Some(&expected_workspace), std::slice::from_ref(&crate_root))
            .expect_err("crate should fail when it resolves into a different workspace");

        assert!(matches!(
            err,
            DiscoveryError::WorkspacePathMismatch {
                crate_path,
                expected_workspace_path,
                discovered_workspace_path,
            } if crate_path == crate_root
                && expected_workspace_path == expected_workspace
                && discovered_workspace_path == discovered_workspace
        ));
    }
}
