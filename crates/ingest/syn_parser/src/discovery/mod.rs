pub mod error;
pub mod single_crate;
pub mod workspace;

use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

pub use error::*;
pub use single_crate::*;
pub use workspace::{
    WorkspaceManifestMetadata, locate_workspace_manifest, resolve_workspace_version,
    try_parse_manifest,
};

use itertools::Itertools as _;
use tracing::instrument;
use walkdir::WalkDir;

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
#[instrument(err)]
pub fn run_discovery_phase_with_target(
    workspace_root: Option<&Path>,
    target_crates: &[PathBuf], // Expecting absolute paths to crate root directories
    selected_target: Option<&TargetSelector>,
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

        tracing::info!(?manifest);
        let CargoManifest {
            package,
            features,
            dependencies,
            dev_dependencies,
            lib,
            bin,
            test,
            example,
            bench,
        } = manifest;

        // --- Extract Package Info (Non-Fatal Errors) ---
        let crate_name = package.name.clone();
        let autobins_enabled = package.autobins != Some(false);
        let autotests_enabled = package.autotests != Some(false);
        let autoexamples_enabled = package.autoexamples != Some(false);
        let autobenches_enabled = package.autobenches != Some(false);
        let crate_version = package.version.resolve(crate_root_path)?;

        // --- Namespace Generation (Called below) ---
        let namespace = derive_crate_namespace(&crate_name, &crate_version);

        // --- File Discovery Logic ---
        let src_path = crate_root_path.join("src");
        let tests_path = crate_root_path.join("tests");
        let examples_path = crate_root_path.join("examples");
        let benches_path = crate_root_path.join("benches");
        let mut files_set: HashSet<PathBuf> = HashSet::new();
        let mut target_specs: Vec<TargetSpec> = Vec::new();

        // --- Add custom lib path if specified (for non-standard layouts) ---
        if let Some(lib_target) = lib {
            let lib_path = crate_root_path.join(lib_target.path);
            if lib_path.exists() && lib_path.is_file() {
                files_set.insert(lib_path.clone());
                target_specs.push(TargetSpec {
                    kind: TargetKind::Lib,
                    name: crate_name.clone(),
                    root: lib_path,
                });
            }
        } else {
            let lib_path = crate_root_path.join("src/lib.rs");
            if lib_path.exists() && lib_path.is_file() {
                files_set.insert(lib_path.clone());
                target_specs.push(TargetSpec {
                    kind: TargetKind::Lib,
                    name: crate_name.clone(),
                    root: lib_path,
                });
            }
        }

        // --- Add custom bin paths if specified ---
        if let Some(bin_targets) = bin {
            for bin_target in bin_targets {
                let bin_path = crate_root_path.join(bin_target.path);
                if bin_path.exists() && bin_path.is_file() {
                    files_set.insert(bin_path.clone());
                    target_specs.push(TargetSpec {
                        kind: TargetKind::Bin,
                        name: bin_target.name,
                        root: bin_path,
                    });
                }
            }
        }

        if autobins_enabled {
            let main_path = src_path.join("main.rs");
            if main_path.exists() && main_path.is_file() {
                files_set.insert(main_path.clone());
                target_specs.push(TargetSpec {
                    kind: TargetKind::Bin,
                    name: crate_name.clone(),
                    root: main_path,
                });
            }

            let src_bin_path = src_path.join("bin");
            if src_bin_path.exists() && src_bin_path.is_dir() {
                let walker = WalkDir::new(&src_bin_path).max_depth(1).into_iter();
                for entry_result in walker {
                    match entry_result {
                        Ok(entry)
                            if entry.file_type().is_file()
                                && entry.path().extension().is_some_and(|ext| ext == "rs") =>
                        {
                            let file_path = entry.path().to_path_buf();
                            files_set.insert(file_path.clone());
                            let name = file_path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or("bin")
                                .to_string();
                            target_specs.push(TargetSpec {
                                kind: TargetKind::Bin,
                                name,
                                root: file_path,
                            });
                        }
                        Ok(_non_rust_file) => {}
                        Err(e) => {
                            let path = e.path().unwrap_or(&src_bin_path).to_path_buf();
                            non_fatal_errors.push(DiscoveryError::Walkdir {
                                path,
                                source: Arc::new(e),
                            });
                        }
                    }
                }
            }
        }

        if let Some(test_targets) = test {
            for test_target in test_targets {
                let Some(test_path) = resolve_explicit_target_path(
                    crate_root_path,
                    "tests",
                    test_target.name.as_deref(),
                    test_target.path.as_deref(),
                ) else {
                    continue;
                };
                if test_path.exists() && test_path.is_file() {
                    files_set.insert(test_path.clone());
                    let name = test_target
                        .name
                        .unwrap_or_else(|| file_stem_name(&test_path, "test"));
                    target_specs.push(TargetSpec {
                        kind: TargetKind::Test,
                        name,
                        root: test_path,
                    });
                }
            }
        }

        if let Some(example_targets) = example {
            for example_target in example_targets {
                let Some(example_path) = resolve_explicit_target_path(
                    crate_root_path,
                    "examples",
                    example_target.name.as_deref(),
                    example_target.path.as_deref(),
                ) else {
                    continue;
                };
                if example_path.exists() && example_path.is_file() {
                    files_set.insert(example_path.clone());
                    let name = example_target
                        .name
                        .unwrap_or_else(|| file_stem_name(&example_path, "example"));
                    target_specs.push(TargetSpec {
                        kind: TargetKind::Example,
                        name,
                        root: example_path,
                    });
                }
            }
        }

        if let Some(bench_targets) = bench {
            for bench_target in bench_targets {
                let Some(bench_path) = resolve_explicit_target_path(
                    crate_root_path,
                    "benches",
                    bench_target.name.as_deref(),
                    bench_target.path.as_deref(),
                ) else {
                    continue;
                };
                if bench_path.exists() && bench_path.is_file() {
                    files_set.insert(bench_path.clone());
                    let name = bench_target
                        .name
                        .unwrap_or_else(|| file_stem_name(&bench_path, "bench"));
                    target_specs.push(TargetSpec {
                        kind: TargetKind::Bench,
                        name,
                        root: bench_path,
                    });
                }
            }
        }

        // --- Walk src/ directory if it exists ---
        if src_path.exists() && src_path.is_dir() {
            // --- Perform File Discovery (Non-Fatal Walkdir Errors) ---
            let walker = WalkDir::new(&src_path).into_iter();
            for entry_result in walker {
                match entry_result {
                    Ok(entry)
                        if entry.file_type().is_file()
                            && entry.path().extension().is_some_and(|ext| ext == "rs") =>
                    {
                        files_set.insert(entry.path().to_path_buf());
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

        if autotests_enabled {
            collect_rs_files_under(&tests_path, &mut files_set, &mut non_fatal_errors);
            if tests_path.exists() && tests_path.is_dir() {
                collect_root_targets_from_dir(
                    &tests_path,
                    TargetKind::Test,
                    &mut target_specs,
                    &mut non_fatal_errors,
                );
            }
        }
        if autoexamples_enabled {
            collect_rs_files_under(&examples_path, &mut files_set, &mut non_fatal_errors);
            if examples_path.exists() && examples_path.is_dir() {
                collect_root_targets_from_dir(
                    &examples_path,
                    TargetKind::Example,
                    &mut target_specs,
                    &mut non_fatal_errors,
                );
            }
        }
        if autobenches_enabled {
            collect_rs_files_under(&benches_path, &mut files_set, &mut non_fatal_errors);
            if benches_path.exists() && benches_path.is_dir() {
                collect_root_targets_from_dir(
                    &benches_path,
                    TargetKind::Bench,
                    &mut target_specs,
                    &mut non_fatal_errors,
                );
            }
        }

        target_specs = dedup_targets_by_root(target_specs);

        if let Some(selector) = selected_target {
            target_specs
                .retain(|target| target.kind == selector.kind && target.name == selector.name);
        }

        // Ensure we found enough context to proceed. Tests/examples/benches-only crates are valid.
        if files_set.is_empty() && target_specs.is_empty() {
            return Err(DiscoveryError::SrcNotFound {
                path: src_path.clone(),
            });
        }

        // Convert HashSet to Vec for further processing
        let files: Vec<PathBuf> = files_set.into_iter().collect();

        let mut files = files;
        files.sort();

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
                        });
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
            files, // Clone needed for module mapping below
            targets: target_specs,
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

pub fn discovery_phase(
    workspace_root: Option<&Path>,
    target_crates: &[PathBuf],
) -> Result<DiscoveryOutput, DiscoveryError> {
    run_discovery_phase_with_target(workspace_root, target_crates, None)
}

pub fn run_discovery_phase(
    workspace_root: Option<&Path>,
    target_crates: &[PathBuf],
) -> Result<DiscoveryOutput, DiscoveryError> {
    discovery_phase(workspace_root, target_crates)
}

fn collect_rs_files_under(
    root: &Path,
    files_set: &mut HashSet<PathBuf>,
    non_fatal_errors: &mut Vec<DiscoveryError>,
) {
    if !root.exists() || !root.is_dir() {
        return;
    }
    let walker = WalkDir::new(root).into_iter();
    for entry_result in walker {
        match entry_result {
            Ok(entry)
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "rs") =>
            {
                files_set.insert(entry.path().to_path_buf());
            }
            Ok(_non_rust_file) => {}
            Err(e) => {
                let path = e.path().unwrap_or(root).to_path_buf();
                non_fatal_errors.push(DiscoveryError::Walkdir {
                    path,
                    source: Arc::new(e),
                });
            }
        }
    }
}

fn collect_root_targets_from_dir(
    root: &Path,
    kind: TargetKind,
    targets: &mut Vec<TargetSpec>,
    non_fatal_errors: &mut Vec<DiscoveryError>,
) {
    let walker = WalkDir::new(root).max_depth(1).into_iter();
    for entry_result in walker {
        match entry_result {
            Ok(entry)
                if entry.file_type().is_file()
                    && entry.path().extension().is_some_and(|ext| ext == "rs") =>
            {
                let file_path = entry.path().to_path_buf();
                targets.push(TargetSpec {
                    kind: kind.clone(),
                    name: file_stem_name(&file_path, "target"),
                    root: file_path,
                });
            }
            Ok(_non_rust_file) => {}
            Err(e) => {
                let path = e.path().unwrap_or(root).to_path_buf();
                non_fatal_errors.push(DiscoveryError::Walkdir {
                    path,
                    source: Arc::new(e),
                });
            }
        }
    }
}

fn file_stem_name(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(fallback)
        .to_string()
}

fn resolve_explicit_target_path(
    crate_root_path: &Path,
    default_dir: &str,
    name: Option<&str>,
    explicit_path: Option<&Path>,
) -> Option<PathBuf> {
    if let Some(path) = explicit_path {
        return Some(crate_root_path.join(path));
    }
    name.map(|target_name| {
        crate_root_path
            .join(default_dir)
            .join(format!("{target_name}.rs"))
    })
}

fn dedup_targets_by_root(targets: Vec<TargetSpec>) -> Vec<TargetSpec> {
    let mut best_by_root: HashMap<PathBuf, TargetSpec> = HashMap::new();
    for target in targets {
        match best_by_root.entry(target.root.clone()) {
            Entry::Vacant(v) => {
                v.insert(target);
            }
            Entry::Occupied(mut o) => {
                if target_precedence_key(&target) < target_precedence_key(o.get()) {
                    o.insert(target);
                }
            }
        }
    }

    let mut deduped: Vec<TargetSpec> = best_by_root.into_values().collect();
    deduped.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.name.cmp(&b.name))
            .then_with(|| a.root.cmp(&b.root))
    });
    deduped
}

fn target_precedence_key(target: &TargetSpec) -> (u8, &str) {
    let kind_rank = match target.kind {
        TargetKind::Lib => 0,
        TargetKind::Bin => 1,
        TargetKind::Test => 2,
        TargetKind::Example => 3,
        TargetKind::Bench => 4,
    };
    (kind_rank, target.name.as_str())
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
        eprintln!("fixture_workspace = {}", fixture_workspace_root.display());
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

        let discovery_result = run_discovery_phase(
            Some(&fixture_workspace_root),
            std::slice::from_ref(&crate_dir),
        );
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

    #[test]
    fn discovery_includes_tests_only_package_targets() -> Result<(), DiscoveryError> {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("tests_only_pkg");
        fs::create_dir_all(crate_root.join("tests")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "tests_only_pkg"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(
            crate_root.join("tests").join("integration.rs"),
            "fn smoke() {}\n",
        )
        .unwrap();

        let out = discovery_phase(None, std::slice::from_ref(&crate_root))?;
        let ctx = out
            .crate_contexts
            .get(&crate_root)
            .expect("crate context missing");

        assert!(
            ctx.targets
                .iter()
                .any(|t| t.kind == TargetKind::Test && t.name == "integration"),
            "expected implicit integration test target"
        );
        assert!(
            ctx.files
                .iter()
                .any(|f| f.ends_with("tests/integration.rs")),
            "expected test source file in discovered file set"
        );
        Ok(())
    }

    #[test]
    fn discovery_selector_limits_targets_only() -> Result<(), DiscoveryError> {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("multi_target_pkg");
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::create_dir_all(crate_root.join("tests")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "multi_target_pkg"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(
            crate_root.join("src").join("lib.rs"),
            "pub fn lib_fn() {}\n",
        )
        .unwrap();
        fs::write(
            crate_root.join("tests").join("integration.rs"),
            "fn integration() {}\n",
        )
        .unwrap();

        let selector = TargetSelector {
            kind: TargetKind::Test,
            name: "integration".to_string(),
        };

        let out = run_discovery_phase_with_target(
            None,
            std::slice::from_ref(&crate_root),
            Some(&selector),
        )?;
        let ctx = out
            .crate_contexts
            .get(&crate_root)
            .expect("crate context missing");

        assert_eq!(ctx.targets.len(), 1, "selector should limit targets");
        assert_eq!(ctx.targets[0].kind, TargetKind::Test);
        assert_eq!(ctx.targets[0].name, "integration");
        assert!(
            ctx.files.iter().any(|f| f.ends_with("src/lib.rs")),
            "files set should remain superset and include src/lib.rs"
        );
        Ok(())
    }

    #[test]
    fn discovery_uses_explicit_test_targets_when_autotests_disabled() -> Result<(), DiscoveryError>
    {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("explicit_test_pkg");
        fs::create_dir_all(crate_root.join("custom-tests")).unwrap();
        fs::create_dir_all(crate_root.join("tests")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "explicit_test_pkg"
version = "0.1.0"
edition = "2021"
autotests = false

[[test]]
name = "explicit_case"
path = "custom-tests/explicit_case.rs"
"#,
        )
        .unwrap();
        fs::write(
            crate_root.join("custom-tests").join("explicit_case.rs"),
            "fn explicit_case() {}\n",
        )
        .unwrap();
        fs::write(
            crate_root.join("tests").join("implicit.rs"),
            "fn implicit() {}\n",
        )
        .unwrap();

        let out = discovery_phase(None, std::slice::from_ref(&crate_root))?;
        let ctx = out
            .crate_contexts
            .get(&crate_root)
            .expect("crate context missing");

        assert!(
            ctx.targets
                .iter()
                .any(|t| t.kind == TargetKind::Test && t.name == "explicit_case"),
            "expected explicit [[test]] target"
        );
        assert!(
            !ctx.targets
                .iter()
                .any(|t| t.kind == TargetKind::Test && t.name == "implicit"),
            "autotests=false should disable implicit tests/*.rs targets"
        );
        Ok(())
    }

    #[test]
    fn discovery_dedups_targets_by_root_path_with_stable_precedence() -> Result<(), DiscoveryError>
    {
        let tmp = tempdir().unwrap();
        let crate_root = tmp.path().join("dedup_targets_pkg");
        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "dedup_targets_pkg"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "named_bin"
path = "src/main.rs"
"#,
        )
        .unwrap();
        fs::write(crate_root.join("src/main.rs"), "fn main() {}\n").unwrap();

        let out = discovery_phase(None, std::slice::from_ref(&crate_root))?;
        let ctx = out
            .crate_contexts
            .get(&crate_root)
            .expect("crate context missing");

        let main_targets: Vec<_> = ctx
            .targets
            .iter()
            .filter(|t| t.root.ends_with("src/main.rs"))
            .collect();
        assert_eq!(
            main_targets.len(),
            1,
            "src/main.rs should dedup to one target"
        );
        assert_eq!(
            main_targets[0].name, "dedup_targets_pkg",
            "implicit bin name should win by deterministic precedence"
        );
        Ok(())
    }

    #[test]
    fn discovery_with_glob_workspace_member_expands_simple_wildcard() {
        let tmp = tempdir().unwrap();
        let workspace_root = tmp.path().join("glob_ws");
        let crate_root = workspace_root.join("crates/member_one");

        fs::create_dir_all(crate_root.join("src")).unwrap();
        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        fs::write(
            crate_root.join("Cargo.toml"),
            r#"[package]
name = "member_one"
version = "0.1.0"
edition = "2021"
"#,
        )
        .unwrap();
        fs::write(crate_root.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();

        let workspace_metadata = try_parse_manifest(&workspace_root)
            .expect("workspace fixture should parse")
            .workspace
            .expect("workspace section should be present");
        let expanded_member_path = workspace_metadata
            .members
            .first()
            .expect("workspace should include one member")
            .clone();

        assert_eq!(workspace_metadata.members.len(), 1);
        assert_eq!(expanded_member_path, crate_root);

        let discovery = run_discovery_phase_with_target(
            Some(&workspace_root),
            std::slice::from_ref(&expanded_member_path),
            None,
        )
        .expect("expanded member path should be discoverable");

        assert!(
            discovery.crate_contexts.contains_key(&expanded_member_path),
            "discovery output should include expanded workspace member path"
        );
    }

    #[test]
    fn discovery_with_glob_workspace_member_expands_prefix_star_suffix_dirs() {
        let tmp = tempdir().unwrap();
        let workspace_root = tmp.path().join("glob_ws_prefix_suffix");

        let axum_core = workspace_root.join("axum-core");
        let axum_extra = workspace_root.join("axum-extra");

        for member in [&axum_core, &axum_extra] {
            fs::create_dir_all(member.join("src")).unwrap();
            fs::write(
                member.join("Cargo.toml"),
                format!(
                    r#"[package]
name = "{}"
version = "0.1.0"
edition = "2021"
"#,
                    member.file_name().unwrap().to_string_lossy()
                ),
            )
            .unwrap();
            fs::write(member.join("src/lib.rs"), "pub fn demo() {}\n").unwrap();
        }

        fs::write(
            workspace_root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"axum-*\"]\n",
        )
        .unwrap();

        let workspace_metadata = try_parse_manifest(&workspace_root)
            .expect("workspace fixture should parse")
            .workspace
            .expect("workspace section should be present");

        assert_eq!(workspace_metadata.members.len(), 2, "expected two glob matches");
        assert_eq!(workspace_metadata.members[0], axum_core);
        assert_eq!(workspace_metadata.members[1], axum_extra);

        let discovery = run_discovery_phase_with_target(
            Some(&workspace_root),
            &workspace_metadata.members,
            None,
        )
        .expect("expanded member paths should be discoverable");

        for member in [&axum_core, &axum_extra] {
            assert!(
                discovery.crate_contexts.contains_key(member),
                "discovery output should include expanded workspace member path {}",
                member.display()
            );
        }
    }
}
