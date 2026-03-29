//! `parse debug` — structured diagnostics for `syn_parser` discovery, resolve, merge, and corpus runs.
//!
//! This module is the operator-facing debugging surface for parser work. Use it when:
//! - a crate fails somewhere between discovery and merge,
//! - you need to understand how Phase 2 derived logical module paths,
//! - you want to compare pre-merge module nodes with post-merge collisions,
//! - you want to sweep many real-world crates and keep reproducible artifacts.
//!
//! At a glance:
//! - `parse debug discovery` explains what discovery included and why.
//! - `parse debug workspace` finds which workspace member fails and at which stage.
//! - `parse debug logical-paths` shows the file -> logical module path mapping used before merge.
//! - `parse debug modules-premerge` shows per-file `ModuleNode`s before graphs are merged.
//! - `parse debug path-collisions` highlights duplicate logical paths after merge.
//! - `parse debug corpus` clones or reuses many targets and persists stage artifacts and failures.
//!
//! The commands here use [`syn_parser::discovery::try_parse_manifest`],
//! [`syn_parser::discovery::run_discovery_phase`], [`syn_parser::logical_module_path_for_file`],
//! [`syn_parser::try_run_phases_and_resolve`], [`syn_parser::try_run_phases_and_merge`], and
//! [`syn_parser::ParsedCodeGraph::merge_new`].
//!
//! `parse debug corpus` is especially useful when hunting invariant failures in real code. Each run
//! writes a dedicated artifact tree under `target/debug_corpus_runs/<run-id>/` by default. Per
//! target, you get:
//! - `summary.json`
//! - `discovery/discovery.json`
//! - `resolve/resolve.json`
//! - `merge/merged_graph.json`
//! - `failure.json` and `stage_summary.json` for any stage that errors or panics
//!
//! Example commands:
//! ```text
//! cargo xtask parse debug discovery tests/fixture_crates/fixture_nodes
//! cargo xtask parse debug logical-paths tests/fixture_crates/fixture_nodes
//! cargo xtask parse debug modules-premerge tests/fixture_crates/fixture_nodes
//! cargo xtask parse debug path-collisions tests/fixture_crates/fixture_nodes
//! cargo xtask parse debug workspace tests/fixture_workspace/fixture_mock_serde
//! cargo xtask --format json parse debug corpus --limit 5
//! cargo xtask --format json parse debug corpus --list-file /tmp/ploke-targets.txt --checkout-dir /tmp/ploke-corpus
//! cargo xtask --format json parse debug corpus --list-file /tmp/ploke-targets.txt --skip-merge
//! cargo xtask parse debug corpus-show run-1774750473411 --target Amanieu/parking_lot --backtrace
//! cargo xtask parse debug corpus-triage run-1774750473411
//! ```
//!
//! Suggested workflow:
//! 1. Reproduce a failure on one crate with `parse debug workspace` or `parse debug corpus`.
//! 2. Use `logical-paths`, `modules-premerge`, and `path-collisions` to localize where the graph
//!    first diverges from expectations.
//! 3. Inspect the per-stage artifact directory from the corpus run when you need the raw persisted
//!    payloads rather than terminal output.
#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs::File;
use std::io::Write;
use std::panic::PanicHookInfo;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use cargo_metadata::Metadata;
use clap::{Args, Subcommand, ValueEnum};
use ploke_error::DiagnosticInfo;
use serde::{Deserialize, Serialize};

use syn_parser::discovery::CargoManifest;
use syn_parser::discovery::{
    DiscoveryError, ManifestKind, run_discovery_phase, try_parse_manifest,
};
use syn_parser::parser::diagnostics::with_debug_artifact_dir;
use syn_parser::parser::nodes::ModuleNode;
use syn_parser::{
    ParsedCodeGraph, logical_module_path_for_file, try_run_phases_and_merge,
    try_run_phases_and_resolve,
};

use super::parse::{count_code_graph_nodes, resolve_parse_path};
use super::{CommandContext, XtaskError};
use crate::executor::Command;

/// `parse debug …` — nested subcommands (manifest, discovery, workspace, pipeline, path diagnostics).
#[derive(Debug, Clone, Args)]
pub struct ParseDebugCli {
    #[command(subcommand)]
    pub cmd: ParseDebugCmd,
}

impl Command for ParseDebugCli {
    type Output = super::parse::ParseOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        self.cmd.run(ctx)
    }
}

/// Top-level `parse debug` subcommands.
#[derive(Debug, Clone, Subcommand)]
pub enum ParseDebugCmd {
    /// Summarize `Cargo.toml` at a directory (workspace members, resolver, excludes).
    ///
    /// Uses syn_parser `try_parse_manifest` (workspace section optional). For virtual workspace
    /// roots, this succeeds where `parse discovery` fails (no `[package]` required).
    Manifest(DebugManifest),

    /// Run discovery on a **crate root** and return package name, file count, optional file list,
    /// and symlink hints (e.g. `src` → other crate).
    Discovery(DebugDiscovery),

    /// For a **workspace root**, list members and run discovery + resolve + merge per member.
    ///
    /// Pinpoints which crate fails and at which stage (`discovery` vs `resolve` vs `merge`).
    Workspace(DebugWorkspace),

    /// On a single **crate root**, report resolve vs merge success separately (same crate, two stages).
    Pipeline(DebugPipeline),

    /// For a **crate root**, list each discovered `.rs` file and the logical module path Phase 2 assigns
    /// ([`syn_parser::logical_module_path_for_file`]), same heuristic as parallel parse.
    LogicalPaths(DebugLogicalPaths),

    /// For a **crate root**, run resolve only and dump every `ModuleNode` per parsed file (pre-merge).
    ModulesPremerge(DebugModulesPremerge),

    /// For a **crate root**, merge graphs and list logical paths held by more than one module node.
    PathCollisions(DebugPathCollisions),

    /// Inspect Cargo package targets for a crate/workspace path.
    CargoTargets(DebugCargoTargets),

    /// Classify workspace members by target/layout shape from Cargo metadata.
    WorkspaceMembers(DebugWorkspaceMembers),

    /// Explain source discovery rules used by syn_parser for a crate.
    DiscoveryRules(DebugDiscoveryRules),

    /// Clone and parse a corpus of GitHub targets listed in one or more text files.
    Corpus(DebugCorpus),

    /// Re-open a persisted corpus run and inspect saved summaries/backtraces.
    CorpusShow(DebugCorpusShow),

    /// Build a triage index and pending report stubs from a persisted corpus run.
    CorpusTriage(DebugCorpusTriage),
}

impl ParseDebugCmd {
    fn run(&self, ctx: &CommandContext) -> Result<super::parse::ParseOutput, XtaskError> {
        let out = match self {
            ParseDebugCmd::Manifest(c) => c.execute(ctx)?,
            ParseDebugCmd::Discovery(c) => c.execute(ctx)?,
            ParseDebugCmd::Workspace(c) => c.execute(ctx)?,
            ParseDebugCmd::Pipeline(c) => c.execute(ctx)?,
            ParseDebugCmd::LogicalPaths(c) => c.execute(ctx)?,
            ParseDebugCmd::ModulesPremerge(c) => c.execute(ctx)?,
            ParseDebugCmd::PathCollisions(c) => c.execute(ctx)?,
            ParseDebugCmd::CargoTargets(c) => c.execute(ctx)?,
            ParseDebugCmd::WorkspaceMembers(c) => c.execute(ctx)?,
            ParseDebugCmd::DiscoveryRules(c) => c.execute(ctx)?,
            ParseDebugCmd::Corpus(c) => c.execute(ctx)?,
            ParseDebugCmd::CorpusShow(c) => c.execute(ctx)?,
            ParseDebugCmd::CorpusTriage(c) => c.execute(ctx)?,
        };
        Ok(super::parse::ParseOutput::Debug(out))
    }
}

// --- manifest ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugManifest {
    /// Directory whose `Cargo.toml` should be read (workspace root or crate root)
    #[arg(value_name = "PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugManifestOut {
    pub manifest_path: String,
    pub has_workspace_section: bool,
    pub workspace_root: Option<String>,
    pub members: Vec<String>,
    pub exclude: Option<Vec<String>>,
    pub resolver: Option<String>,
    pub workspace_package_version: Option<String>,
}

impl Command for DebugManifest {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let meta = try_parse_manifest(&canon, ManifestKind::WorkspaceRoot)
            .map_err(|e| XtaskError::Parse(e.to_string()))?;
        let manifest_path = canon.join("Cargo.toml");
        let (
            has_workspace_section,
            members,
            exclude,
            resolver,
            workspace_root,
            workspace_package_version,
        ) = match &meta.workspace {
            Some(ws) => (
                true,
                ws.members.iter().map(|p| p.display().to_string()).collect(),
                ws.exclude
                    .as_ref()
                    .map(|v| v.iter().map(|p| p.display().to_string()).collect()),
                ws.resolver.clone(),
                Some(ws.path.display().to_string()),
                ws.package_version().map(str::to_string),
            ),
            None => (false, Vec::new(), None, None, None, None),
        };
        Ok(DebugOutput::Manifest(DebugManifestOut {
            manifest_path: manifest_path.display().to_string(),
            has_workspace_section,
            workspace_root,
            members,
            exclude,
            resolver,
            workspace_package_version,
        }))
    }
}

// --- discovery (crate) ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugDiscovery {
    /// Crate root directory (contains `Cargo.toml` with `[package]`)
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,

    /// Only print counts and errors (omit per-file listing)
    #[arg(long)]
    pub brief: bool,

    /// Cap per-file listing length (0 = unlimited)
    #[arg(long, default_value_t = 500)]
    pub max_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugDiscoveryOut {
    pub crate_root: String,
    pub package_name: Option<String>,
    pub file_count: usize,
    pub unique_canonical_paths: usize,
    pub duplicate_canonical_paths: Vec<String>,
    pub files: Option<Vec<FileEntry>>,
    pub discovery_error: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileEntry {
    pub relative_path: String,
    pub is_symlink: bool,
    pub symlink_target: Option<String>,
}

impl Command for DebugDiscovery {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let target = vec![canon.clone()];
        let discovery = run_discovery_phase(None, &target);
        match discovery {
            Ok(out) => {
                let crate_ctx = out.crate_contexts.get(&canon);
                let (package_name, rs_files, warnings) = if let Some(c) = crate_ctx {
                    (
                        Some(c.name.clone()),
                        c.files.clone(),
                        out.warnings.iter().map(|w| w.to_string()).collect(),
                    )
                } else {
                    (
                        None,
                        Vec::new(),
                        out.warnings.iter().map(|w| w.to_string()).collect(),
                    )
                };

                let mut canon_seen: HashMap<String, Vec<String>> = HashMap::new();
                for f in &rs_files {
                    let rel = path_relative_to(&canon, f);
                    let key = std::fs::canonicalize(f)
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|_| rel.clone());
                    canon_seen.entry(key).or_default().push(rel);
                }
                let duplicate_canonical_paths: Vec<String> = canon_seen
                    .into_iter()
                    .filter(|(_, rels)| rels.len() > 1)
                    .map(|(canon, rels)| format!("{canon} ← {:?}", rels))
                    .collect();

                let unique_canonical_paths: HashSet<String> = rs_files
                    .iter()
                    .filter_map(|f| std::fs::canonicalize(f).ok())
                    .map(|p| p.display().to_string())
                    .collect();
                let unique_canonical_paths = unique_canonical_paths.len();

                let file_list = if !self.brief {
                    let mut rels: Vec<String> = rs_files
                        .iter()
                        .filter_map(|f| Some(path_relative_to(&canon, f)))
                        .collect();
                    rels.sort();
                    if self.max_files > 0 && rels.len() > self.max_files {
                        rels.truncate(self.max_files);
                    }
                    let mut entries = Vec::new();
                    for rel in rels {
                        let abs = canon.join(&rel);
                        let (is_symlink, symlink_target) = symlink_info(&abs);
                        entries.push(FileEntry {
                            relative_path: rel,
                            is_symlink,
                            symlink_target,
                        });
                    }
                    Some(entries)
                } else {
                    None
                };

                Ok(DebugOutput::Discovery(DebugDiscoveryOut {
                    crate_root: canon.display().to_string(),
                    package_name,
                    file_count: rs_files.len(),
                    unique_canonical_paths,
                    duplicate_canonical_paths,
                    files: file_list,
                    discovery_error: None,
                    warnings,
                }))
            }
            Err(e) => Ok(DebugOutput::Discovery(DebugDiscoveryOut {
                crate_root: canon.display().to_string(),
                package_name: None,
                file_count: 0,
                unique_canonical_paths: 0,
                duplicate_canonical_paths: Vec::new(),
                files: None,
                discovery_error: Some(e.to_string()),
                warnings: Vec::new(),
            })),
        }
    }
}

// --- workspace probe ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugWorkspace {
    /// Workspace root (contains `[workspace]` in `Cargo.toml`)
    #[arg(value_name = "WORKSPACE_PATH")]
    pub path: PathBuf,

    /// Do not run merge / module tree (faster; only discovery + resolve per member)
    #[arg(long)]
    pub skip_merge: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugWorkspaceProbeOut {
    pub workspace_root: String,
    pub member_count: usize,
    pub members: Vec<MemberProbe>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MemberProbe {
    pub path: String,
    pub label: String,
    pub discovery: StageResult,
    pub resolve: Option<StageResult>,
    pub merge: Option<StageResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageResult {
    pub ok: bool,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
    pub nodes_parsed: Option<usize>,
    pub relations_found: Option<usize>,
    pub file_count: Option<usize>,
}

impl Command for DebugWorkspace {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let ws = resolve_parse_path(ctx, &self.path)?;
        let meta = try_parse_manifest(&ws, ManifestKind::WorkspaceRoot)
            .map_err(|e| XtaskError::Parse(e.to_string()))?;
        let section = meta
            .workspace
            .ok_or_else(|| XtaskError::validation("No `[workspace]` section in Cargo.toml; use `parse debug manifest` on this path, or pass a workspace root.").with_recovery("For single-crate diagnostics use `parse debug discovery` or `parse debug pipeline`."))?;

        let mut members = Vec::new();
        for member in &section.members {
            let label = member
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let discovery_start = Instant::now();
            let disc = run_discovery_phase(None, &[member.clone()]);
            let disc_ms = discovery_start.elapsed().as_millis() as u64;

            let (discovery, file_count) = match disc {
                Ok(out) => {
                    let fc = out
                        .crate_contexts
                        .get(member)
                        .map(|c| c.files.len())
                        .unwrap_or(0);
                    (
                        StageResult {
                            ok: true,
                            error: None,
                            duration_ms: Some(disc_ms),
                            nodes_parsed: None,
                            relations_found: None,
                            file_count: Some(fc),
                        },
                        Some(fc),
                    )
                }
                Err(e) => (
                    StageResult {
                        ok: false,
                        error: Some(e.to_string()),
                        duration_ms: Some(disc_ms),
                        nodes_parsed: None,
                        relations_found: None,
                        file_count: None,
                    },
                    None,
                ),
            };

            let mut resolve = None;
            let mut merge = None;

            if discovery.ok {
                let r_start = Instant::now();
                let res = try_run_phases_and_resolve(member);
                let r_ms = r_start.elapsed().as_millis() as u64;
                resolve = Some(match res {
                    Ok(graphs) => {
                        let nodes: usize = graphs
                            .iter()
                            .map(|pg| count_code_graph_nodes(&pg.graph))
                            .sum();
                        let rels: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
                        StageResult {
                            ok: true,
                            error: None,
                            duration_ms: Some(r_ms),
                            nodes_parsed: Some(nodes),
                            relations_found: Some(rels),
                            file_count,
                        }
                    }
                    Err(e) => StageResult {
                        ok: false,
                        error: Some(e.to_string()),
                        duration_ms: Some(r_ms),
                        nodes_parsed: None,
                        relations_found: None,
                        file_count,
                    },
                });

                if !self.skip_merge && resolve.as_ref().is_some_and(|r| r.ok) {
                    let m_start = Instant::now();
                    let mres = try_run_phases_and_merge(member);
                    let m_ms = m_start.elapsed().as_millis() as u64;
                    merge = Some(match mres {
                        Ok(out) => {
                            let (nodes, rels) = if let Some(ref mg) = out.merged_graph {
                                (
                                    Some(count_code_graph_nodes(&mg.graph)),
                                    Some(mg.graph.relations.len()),
                                )
                            } else {
                                (None, None)
                            };
                            StageResult {
                                ok: true,
                                error: None,
                                duration_ms: Some(m_ms),
                                nodes_parsed: nodes,
                                relations_found: rels,
                                file_count,
                            }
                        }
                        Err(e) => StageResult {
                            ok: false,
                            error: Some(e.to_string()),
                            duration_ms: Some(m_ms),
                            nodes_parsed: None,
                            relations_found: None,
                            file_count,
                        },
                    });
                }
            }

            members.push(MemberProbe {
                path: member.display().to_string(),
                label,
                discovery,
                resolve,
                merge,
            });
        }

        Ok(DebugOutput::WorkspaceProbe(DebugWorkspaceProbeOut {
            workspace_root: ws.display().to_string(),
            member_count: members.len(),
            members,
        }))
    }
}

// --- pipeline (single crate) ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugPipeline {
    /// Crate root directory
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugPipelineOut {
    pub crate_root: String,
    pub resolve: StageResult,
    pub merge: StageResult,
}

impl Command for DebugPipeline {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;

        let r_start = Instant::now();
        let resolve_out = try_run_phases_and_resolve(&canon);
        let r_ms = r_start.elapsed().as_millis() as u64;
        let resolve = match resolve_out {
            Ok(graphs) => {
                let nodes: usize = graphs
                    .iter()
                    .map(|pg| count_code_graph_nodes(&pg.graph))
                    .sum();
                let rels: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
                StageResult {
                    ok: true,
                    error: None,
                    duration_ms: Some(r_ms),
                    nodes_parsed: Some(nodes),
                    relations_found: Some(rels),
                    file_count: None,
                }
            }
            Err(e) => StageResult {
                ok: false,
                error: Some(e.to_string()),
                duration_ms: Some(r_ms),
                nodes_parsed: None,
                relations_found: None,
                file_count: None,
            },
        };

        let m_start = Instant::now();
        let merge_out = try_run_phases_and_merge(&canon);
        let m_ms = m_start.elapsed().as_millis() as u64;
        let merge = match merge_out {
            Ok(out) => {
                let (nodes, rels) = if let Some(ref mg) = out.merged_graph {
                    (
                        Some(count_code_graph_nodes(&mg.graph)),
                        Some(mg.graph.relations.len()),
                    )
                } else {
                    (None, None)
                };
                StageResult {
                    ok: true,
                    error: None,
                    duration_ms: Some(m_ms),
                    nodes_parsed: nodes,
                    relations_found: rels,
                    file_count: None,
                }
            }
            Err(e) => StageResult {
                ok: false,
                error: Some(e.to_string()),
                duration_ms: Some(m_ms),
                nodes_parsed: None,
                relations_found: None,
                file_count: None,
            },
        };

        Ok(DebugOutput::Pipeline(DebugPipelineOut {
            crate_root: canon.display().to_string(),
            resolve,
            merge,
        }))
    }
}

// --- logical paths (per discovered file, Phase 2 heuristic) ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugLogicalPaths {
    /// Crate root directory (contains `Cargo.toml`)
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugLogicalPathsOut {
    pub crate_root: String,
    pub src_dir: String,
    pub package_name: Option<String>,
    pub file_count: usize,
    /// Logical path strings (`crate::...`) that appear for more than one source file.
    pub duplicate_derived_path_displays: Vec<String>,
    pub files: Vec<LogicalPathEntry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogicalPathEntry {
    pub path: String,
    pub canonical_path: Option<String>,
    pub derived_logical_path: Vec<String>,
    pub derived_logical_path_display: String,
}

impl Command for DebugLogicalPaths {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let discovery = run_discovery_phase(None, &[canon.clone()]).map_err(|e| {
            XtaskError::Parse(format!("Discovery failed (needed for file list): {e}"))
        })?;
        let ctx_c = discovery.get_crate_context(&canon).ok_or_else(|| {
            XtaskError::Parse("Discovery returned no context for crate root".into())
        })?;
        let src_dir = canon.join("src");
        let src_dir_s = src_dir.display().to_string();

        let mut paths_sorted: Vec<PathBuf> = ctx_c.files.clone();
        paths_sorted.sort();

        let mut path_counts: HashMap<String, usize> = HashMap::new();
        let mut files = Vec::with_capacity(paths_sorted.len());
        for file_path in &paths_sorted {
            let derived = logical_module_path_for_file(&src_dir, file_path);
            let display = derived.join("::");
            *path_counts.entry(display.clone()).or_insert(0) += 1;
            let canonical_path = std::fs::canonicalize(file_path)
                .ok()
                .map(|p| p.display().to_string());
            files.push(LogicalPathEntry {
                path: file_path.display().to_string(),
                canonical_path,
                derived_logical_path: derived,
                derived_logical_path_display: display,
            });
        }

        let mut duplicate_derived_path_displays: Vec<String> = path_counts
            .into_iter()
            .filter(|(_, c)| *c > 1)
            .map(|(k, _)| k)
            .collect();
        duplicate_derived_path_displays.sort();

        Ok(DebugOutput::LogicalPaths(DebugLogicalPathsOut {
            crate_root: canon.display().to_string(),
            src_dir: src_dir_s,
            package_name: Some(ctx_c.name.clone()),
            file_count: files.len(),
            duplicate_derived_path_displays,
            files,
        }))
    }
}

// --- modules pre-merge (per ParsedCodeGraph) ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugModulesPremerge {
    /// Crate root directory
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugModulesPremergeOut {
    pub crate_root: String,
    pub graph_count: usize,
    pub total_module_nodes: usize,
    pub graphs: Vec<PremergeGraphSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PremergeGraphSummary {
    pub source_file: String,
    pub module_count: usize,
    pub modules: Vec<ModuleNodeSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModuleNodeSummary {
    pub id: String,
    pub name: String,
    pub path: Vec<String>,
    pub path_display: String,
    pub is_declaration: bool,
    pub is_file_based: bool,
    pub module_file_path: Option<String>,
}

impl Command for DebugModulesPremerge {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let graphs = try_run_phases_and_resolve(&canon)
            .map_err(|e| XtaskError::Parse(format!("Resolve phase failed: {e}")))?;

        let mut total_module_nodes = 0usize;
        let mut summaries = Vec::with_capacity(graphs.len());
        for pg in &graphs {
            let mut modules: Vec<ModuleNodeSummary> = pg
                .graph
                .modules
                .iter()
                .map(|m| {
                    let path_display = m.path.join("::");
                    ModuleNodeSummary {
                        id: m.id.to_string(),
                        name: m.name.clone(),
                        path: m.path.clone(),
                        path_display,
                        is_declaration: m.is_decl(),
                        is_file_based: m.is_file_based(),
                        module_file_path: m.file_path().map(|p| p.display().to_string()),
                    }
                })
                .collect();
            modules.sort_by(|a, b| a.path_display.cmp(&b.path_display));
            total_module_nodes += modules.len();
            summaries.push(PremergeGraphSummary {
                source_file: pg.file_path.display().to_string(),
                module_count: modules.len(),
                modules,
            });
        }
        summaries.sort_by(|a, b| a.source_file.cmp(&b.source_file));

        Ok(DebugOutput::ModulesPremerge(DebugModulesPremergeOut {
            crate_root: canon.display().to_string(),
            graph_count: graphs.len(),
            total_module_nodes,
            graphs: summaries,
        }))
    }
}

// --- path collisions after merge ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugPathCollisions {
    /// Crate root directory
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugPathCollisionsOut {
    pub crate_root: String,
    pub merged_module_count: usize,
    pub collision_group_count: usize,
    pub collisions: Vec<PathCollisionGroup>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PathCollisionGroup {
    pub path: Vec<String>,
    pub path_display: String,
    pub modules: Vec<ModuleNodeSummary>,
}

impl Command for DebugPathCollisions {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let graphs = try_run_phases_and_resolve(&canon)
            .map_err(|e| XtaskError::Parse(format!("Resolve phase failed: {e}")))?;
        let merged = ParsedCodeGraph::merge_new(graphs)
            .map_err(|e| XtaskError::Parse(format!("Merge failed: {e}")))?;

        let modules = &merged.graph.modules;
        let mut by_path: HashMap<String, Vec<&ModuleNode>> = HashMap::new();
        for m in modules {
            let key = m.path.join("::");
            by_path.entry(key).or_default().push(m);
        }

        let mut collisions: Vec<PathCollisionGroup> = by_path
            .into_iter()
            .filter(|(_, v)| v.len() > 1)
            .map(|(path_display, group)| {
                let path = group[0].path.clone();
                let mut mods: Vec<ModuleNodeSummary> = group
                    .iter()
                    .map(|m| ModuleNodeSummary {
                        id: m.id.to_string(),
                        name: m.name.clone(),
                        path: m.path.clone(),
                        path_display: m.path.join("::"),
                        is_declaration: m.is_decl(),
                        is_file_based: m.is_file_based(),
                        module_file_path: m.file_path().map(|p| p.display().to_string()),
                    })
                    .collect();
                mods.sort_by(|a, b| a.id.cmp(&b.id));
                PathCollisionGroup {
                    path,
                    path_display,
                    modules: mods,
                }
            })
            .collect();
        collisions.sort_by(|a, b| a.path_display.cmp(&b.path_display));

        Ok(DebugOutput::PathCollisions(DebugPathCollisionsOut {
            crate_root: canon.display().to_string(),
            merged_module_count: modules.len(),
            collision_group_count: collisions.len(),
            collisions,
        }))
    }
}

/// Unified JSON-friendly payload for `parse debug` (see [`ParseOutput::Debug`](super::parse::ParseOutput::Debug)).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DebugOutput {
    Manifest(DebugManifestOut),
    Discovery(DebugDiscoveryOut),
    WorkspaceProbe(DebugWorkspaceProbeOut),
    Pipeline(DebugPipelineOut),
    LogicalPaths(DebugLogicalPathsOut),
    ModulesPremerge(DebugModulesPremergeOut),
    PathCollisions(DebugPathCollisionsOut),
    CargoTargets(DebugCargoTargetsOut),
    WorkspaceMembers(DebugWorkspaceMembersOut),
    DiscoveryRules(DebugDiscoveryRulesOut),
    Corpus(DebugCorpusOut),
    CorpusShow(DebugCorpusShowOut),
    CorpusTriage(DebugCorpusTriageOut),
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn symlink_info(path: &Path) -> (bool, Option<String>) {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            let target = std::fs::read_link(path)
                .ok()
                .map(|p| p.display().to_string());
            (true, target)
        }
        _ => (false, None),
    }
}

fn resolve_corpus_summary_path(
    ctx: &CommandContext,
    run: &Path,
    artifact_dir: &Path,
) -> Result<PathBuf, XtaskError> {
    let workspace_root = ctx.workspace_root()?;
    let artifact_root = if artifact_dir.is_absolute() {
        artifact_dir.to_path_buf()
    } else {
        workspace_root.join(artifact_dir)
    };

    let direct_path = if run.is_absolute() {
        run.to_path_buf()
    } else {
        workspace_root.join(run)
    };
    if direct_path.exists() {
        return normalize_corpus_summary_path(&direct_path);
    }

    normalize_corpus_summary_path(&artifact_root.join(run))
}

fn normalize_corpus_summary_path(path: &Path) -> Result<PathBuf, XtaskError> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    let summary_path = path.join("summary.json");
    if summary_path.is_file() {
        return Ok(summary_path);
    }
    Err(XtaskError::validation(format!(
        "Could not find corpus summary at `{}` or `{}`",
        path.display(),
        summary_path.display()
    ))
    .into())
}

fn corpus_target_matches(target: &CorpusTargetResult, needle: &str) -> bool {
    let slug = target.normalized_repo.replace('/', "__");
    target.normalized_repo == needle
        || slug == needle
        || target.target == needle
        || Path::new(&target.artifact_dir)
            .file_name()
            .and_then(|s| s.to_str())
            == Some(needle)
}

fn corpus_workspace_member_matches(member: &CorpusWorkspaceMemberResult, needle: &str) -> bool {
    member.label == needle
        || member.path == needle
        || Path::new(&member.path).file_name().and_then(|s| s.to_str()) == Some(needle)
        || Path::new(&member.artifact_dir)
            .file_name()
            .and_then(|s| s.to_str())
            == Some(needle)
}

fn collect_corpus_triage_failures(run: &DebugCorpusOut) -> Vec<CorpusTriageFailure> {
    let mut failures = Vec::new();
    let mut next_id = 1usize;

    for target in &run.targets {
        collect_target_stage_failure(
            run,
            target,
            None,
            "target",
            "discovery",
            target.discovery.as_ref(),
            target.summary_path.as_deref(),
            None,
            &mut next_id,
            &mut failures,
        );
        collect_target_stage_failure(
            run,
            target,
            None,
            "target",
            "resolve",
            target.resolve.as_ref(),
            target.summary_path.as_deref(),
            None,
            &mut next_id,
            &mut failures,
        );
        collect_target_stage_failure(
            run,
            target,
            None,
            "target",
            "merge",
            target.merge.as_ref(),
            target.summary_path.as_deref(),
            None,
            &mut next_id,
            &mut failures,
        );

        if let Some(probe) = &target.workspace_probe {
            for member in &probe.members {
                collect_target_stage_failure(
                    run,
                    target,
                    Some(member),
                    "workspace_member",
                    "discovery",
                    Some(&member.discovery),
                    target.summary_path.as_deref(),
                    probe.summary_path.as_deref(),
                    &mut next_id,
                    &mut failures,
                );
                collect_target_stage_failure(
                    run,
                    target,
                    Some(member),
                    "workspace_member",
                    "resolve",
                    member.resolve.as_ref(),
                    target.summary_path.as_deref(),
                    probe.summary_path.as_deref(),
                    &mut next_id,
                    &mut failures,
                );
                collect_target_stage_failure(
                    run,
                    target,
                    Some(member),
                    "workspace_member",
                    "merge",
                    member.merge.as_ref(),
                    target.summary_path.as_deref(),
                    probe.summary_path.as_deref(),
                    &mut next_id,
                    &mut failures,
                );
            }
        }
    }

    failures
}

#[allow(clippy::too_many_arguments)]
fn collect_target_stage_failure(
    run: &DebugCorpusOut,
    target: &CorpusTargetResult,
    member: Option<&CorpusWorkspaceMemberResult>,
    source: &str,
    stage: &str,
    result: Option<&CorpusStageResult>,
    target_summary_path: Option<&str>,
    workspace_summary_path: Option<&str>,
    next_id: &mut usize,
    failures: &mut Vec<CorpusTriageFailure>,
) {
    let Some(result) = result else {
        return;
    };
    if result.ok {
        return;
    }

    let failure_kind = result
        .failure_kind
        .clone()
        .unwrap_or_else(|| if result.panic { "panic" } else { "error" }.into());
    let error_excerpt = triage_error_excerpt(result.error.as_deref());
    let error_signature = triage_error_signature(result.error.as_deref(), stage, &failure_kind);
    let cluster_key = format!("{stage}|{failure_kind}|{error_signature}");
    let cluster_slug = sanitize_artifact_component(&cluster_key);
    let id = format!("failure-{next_id:04}");
    *next_id += 1;

    failures.push(CorpusTriageFailure {
        id,
        run_id: run.run_id.clone(),
        target: target.target.clone(),
        normalized_repo: target.normalized_repo.clone(),
        repository_kind: target.repository_kind.clone(),
        commit_sha: target.commit_sha.clone(),
        source: source.into(),
        member_label: member.map(|entry| entry.label.clone()),
        member_path: member.map(|entry| entry.path.clone()),
        member_artifact_dir: member.map(|entry| entry.artifact_dir.clone()),
        stage: stage.into(),
        failure_kind,
        panic: result.panic,
        error_signature,
        error_excerpt,
        failure_artifact_path: result.failure_artifact_path.clone(),
        artifact_path: result.artifact_path.clone(),
        target_summary_path: target_summary_path.map(str::to_string),
        workspace_summary_path: workspace_summary_path.map(str::to_string),
        cluster_key,
        cluster_slug,
    });
}

fn triage_error_excerpt(error: Option<&str>) -> String {
    let Some(error) = error else {
        return "unknown failure".into();
    };
    let single_line = error.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_progress_error(&single_line)
}

fn triage_error_signature(error: Option<&str>, stage: &str, failure_kind: &str) -> String {
    let Some(error) = error else {
        return format!("{stage} {failure_kind}");
    };
    let mut first_line = error
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(error)
        .to_string();
    if let Some((_, rest)) = first_line.split_once("panicked at ") {
        first_line = rest.to_string();
    }
    if let Some((head, _)) = first_line.split_once(" at ") {
        if !head.trim().is_empty() {
            first_line = head.trim().to_string();
        }
    }
    truncate_progress_error(&first_line)
}

fn build_corpus_triage_clusters(
    failures: &[CorpusTriageFailure],
    run_id: &str,
    pending_report_dir: Option<&Path>,
) -> Result<Vec<CorpusTriageCluster>, XtaskError> {
    let mut grouped: BTreeMap<&str, Vec<&CorpusTriageFailure>> = BTreeMap::new();
    for failure in failures {
        grouped
            .entry(&failure.cluster_key)
            .or_default()
            .push(failure);
    }

    let mut clusters = Vec::with_capacity(grouped.len());
    for (key, entries) in grouped {
        let first = entries[0];
        let mut repos: BTreeSet<String> = BTreeSet::new();
        let mut examples = Vec::new();
        for entry in &entries {
            repos.insert(entry.normalized_repo.clone());
            if examples.len() < 5 {
                examples.push(entry.id.clone());
            }
        }

        let pending_report_path = if let Some(dir) = pending_report_dir {
            std::fs::create_dir_all(dir).map_err(|e| {
                XtaskError::Resource(format!(
                    "Failed to create pending report dir {}: {e}",
                    dir.display()
                ))
            })?;
            let path = dir.join(format!("{}.json", first.cluster_slug));
            let stub = CorpusTriageReportTemplate {
                version: 1,
                run_id: Some(run_id.to_string()),
                cluster_key: Some(key.to_string()),
                cluster_slug: Some(first.cluster_slug.clone()),
                status: "pending".into(),
                signature: Some(first.error_signature.clone()),
                stage: Some(first.stage.clone()),
                failure_kind: Some(first.failure_kind.clone()),
                occurrence_count: Some(entries.len()),
                example_failures: examples.clone(),
                suspected_root_cause: None,
                confidence: None,
                scope_assessment: None,
                touches_sensitive_pipeline: None,
                recommended_next_step: None,
                minimal_repro_status: "not_started".into(),
                relevant_artifacts: entries
                    .iter()
                    .filter_map(|entry| entry.failure_artifact_path.clone())
                    .take(5)
                    .collect(),
                relevant_code_paths: Vec::new(),
                notes: Vec::new(),
            };
            write_json_file(&path, &stub)?;
            Some(path.display().to_string())
        } else {
            None
        };

        clusters.push(CorpusTriageCluster {
            key: key.to_string(),
            slug: first.cluster_slug.clone(),
            stage: first.stage.clone(),
            failure_kind: first.failure_kind.clone(),
            error_signature: first.error_signature.clone(),
            count: entries.len(),
            repos: repos.into_iter().collect(),
            examples,
            pending_report_path,
        });
    }

    Ok(clusters)
}

fn recompute_corpus_counts(run: &mut DebugCorpusOut) {
    run.processed_targets = run.targets.len();
    run.single_crate_targets = run
        .targets
        .iter()
        .filter(|t| t.repository_kind == "single_crate")
        .count();
    run.workspace_targets = run
        .targets
        .iter()
        .filter(|t| t.repository_kind == "workspace")
        .count();
    run.skipped_targets = run
        .targets
        .iter()
        .filter(|t| t.clone.action == "skipped_missing")
        .count();
    run.cloned_targets = run
        .targets
        .iter()
        .filter(|t| t.clone.action == "cloned")
        .count();
    run.reused_targets = run
        .targets
        .iter()
        .filter(|t| t.clone.action == "reused")
        .count();
    run.clone_failures = run.targets.iter().filter(|t| !t.clone.ok).count();
    run.discovery_failures = 0;
    run.resolve_failures = 0;
    run.merge_failures = 0;
    run.panic_failures = 0;
    for target in &run.targets {
        if target.discovery.as_ref().is_some_and(|s| !s.ok) {
            run.discovery_failures += 1;
        }
        if target.resolve.as_ref().is_some_and(|s| !s.ok) {
            run.resolve_failures += 1;
        }
        if target.merge.as_ref().is_some_and(|s| !s.ok) {
            run.merge_failures += 1;
        }
        if target.discovery.as_ref().is_some_and(|s| s.panic)
            || target.resolve.as_ref().is_some_and(|s| s.panic)
            || target.merge.as_ref().is_some_and(|s| s.panic)
        {
            run.panic_failures += 1;
        }
        if let Some(probe) = &target.workspace_probe {
            for member in &probe.members {
                if !member.discovery.ok {
                    run.discovery_failures += 1;
                    if member.discovery.panic {
                        run.panic_failures += 1;
                    }
                }
                if let Some(resolve) = &member.resolve {
                    if !resolve.ok {
                        run.resolve_failures += 1;
                        if resolve.panic {
                            run.panic_failures += 1;
                        }
                    }
                }
                if let Some(merge) = &member.merge {
                    if !merge.ok {
                        run.merge_failures += 1;
                        if merge.panic {
                            run.panic_failures += 1;
                        }
                    }
                }
            }
        }
    }
}

fn workspace_member_failed(member: &CorpusWorkspaceMemberResult) -> bool {
    !member.discovery.ok
        || member.resolve.as_ref().is_some_and(|stage| !stage.ok)
        || member.merge.as_ref().is_some_and(|stage| !stage.ok)
}

// --- cargo targets ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugCargoTargets {
    /// Crate or workspace path (must contain `Cargo.toml`)
    #[arg(value_name = "WORKSPACE_OR_CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugCargoTargetsOut {
    pub input_path: String,
    pub manifest_path: String,
    pub workspace_root: String,
    pub package_count: usize,
    pub packages: Vec<CargoPackageSummary>,
    pub tests_only_packages: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CargoPackageSummary {
    pub name: String,
    pub manifest_path: String,
    pub is_workspace_member: bool,
    pub targets: Vec<CargoTargetSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CargoTargetSummary {
    pub name: String,
    pub kind: Vec<String>,
    pub crate_types: Vec<String>,
    pub src_path: String,
}

impl Command for DebugCargoTargets {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_debug_target_path(ctx, &self.path)?;
        let metadata = load_cargo_metadata(&canon)?;
        let workspace_members: HashSet<_> = metadata.workspace_members.iter().cloned().collect();

        let mut warnings = Vec::new();
        let mut packages = Vec::with_capacity(metadata.packages.len());
        let mut tests_only_packages = Vec::new();
        for pkg in &metadata.packages {
            let is_workspace_member = workspace_members.contains(&pkg.id);
            let targets: Vec<CargoTargetSummary> = pkg
                .targets
                .iter()
                .map(|t| CargoTargetSummary {
                    name: t.name.to_string(),
                    kind: t.kind.iter().map(|k| k.to_string()).collect(),
                    crate_types: t.crate_types.iter().map(|ct| ct.to_string()).collect(),
                    src_path: t.src_path.as_std_path().display().to_string(),
                })
                .collect();

            let has_lib = pkg.targets.iter().any(|t| {
                t.kind.iter().any(|k| {
                    let ks = k.to_string();
                    ks == "lib" || ks == "proc-macro"
                })
            });
            let has_bin = pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k.to_string() == "bin"));
            if !has_lib && !has_bin {
                tests_only_packages.push(pkg.name.to_string());
                warnings.push(format!(
                    "package `{}` has no lib/bin targets (tests/examples/benches only)",
                    pkg.name.to_string()
                ));
            }

            packages.push(CargoPackageSummary {
                name: pkg.name.to_string(),
                manifest_path: pkg.manifest_path.as_std_path().display().to_string(),
                is_workspace_member,
                targets,
            });
        }
        packages.sort_by(|a, b| a.name.cmp(&b.name));
        tests_only_packages.sort();
        warnings.sort();

        Ok(DebugOutput::CargoTargets(DebugCargoTargetsOut {
            input_path: canon.display().to_string(),
            manifest_path: canon.join("Cargo.toml").display().to_string(),
            workspace_root: metadata.workspace_root.as_std_path().display().to_string(),
            package_count: packages.len(),
            packages,
            tests_only_packages,
            warnings,
        }))
    }
}

// --- workspace members classification ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugWorkspaceMembers {
    /// Workspace root path (must contain `Cargo.toml`)
    #[arg(value_name = "WORKSPACE_PATH")]
    pub path: PathBuf,
    /// Include classification fields for each workspace member.
    #[arg(long)]
    pub classify: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugWorkspaceMembersOut {
    pub workspace_root: String,
    pub member_count: usize,
    pub members: Vec<WorkspaceMemberClassified>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceMemberClassified {
    pub name: String,
    pub manifest_path: String,
    pub member_root: String,
    pub has_lib_target: bool,
    pub has_bin_targets: bool,
    pub has_test_targets: bool,
    pub has_example_targets: bool,
    pub has_bench_targets: bool,
    pub lib_src_path: Option<String>,
    pub bin_src_paths: Vec<String>,
    pub has_src_dir: bool,
    pub has_tests_dir: bool,
    pub has_build_rs: bool,
    pub classification: Option<String>,
}

impl Command for DebugWorkspaceMembers {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_debug_target_path(ctx, &self.path)?;
        let metadata = load_cargo_metadata(&canon)?;
        let members: HashSet<_> = metadata.workspace_members.iter().cloned().collect();
        let mut out_members = Vec::new();

        for pkg in metadata.packages.iter().filter(|p| members.contains(&p.id)) {
            let member_root = pkg
                .manifest_path
                .as_std_path()
                .parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| {
                    XtaskError::Internal("Package manifest missing parent directory".into())
                })?;
            let has_lib_target = pkg.targets.iter().any(|t| {
                t.kind.iter().any(|k| {
                    let ks = k.to_string();
                    ks == "lib" || ks == "proc-macro"
                })
            });
            let has_bin_targets = pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k.to_string() == "bin"));
            let has_test_targets = pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k.to_string() == "test"));
            let has_example_targets = pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k.to_string() == "example"));
            let has_bench_targets = pkg
                .targets
                .iter()
                .any(|t| t.kind.iter().any(|k| k.to_string() == "bench"));
            let lib_src_path = pkg
                .targets
                .iter()
                .find(|t| {
                    t.kind.iter().any(|k| {
                        let ks = k.to_string();
                        ks == "lib" || ks == "proc-macro"
                    })
                })
                .map(|t| t.src_path.as_std_path().display().to_string());
            let mut bin_src_paths: Vec<String> = pkg
                .targets
                .iter()
                .filter(|t| t.kind.iter().any(|k| k.to_string() == "bin"))
                .map(|t| t.src_path.as_std_path().display().to_string())
                .collect();
            bin_src_paths.sort();

            let has_src_dir = member_root.join("src").is_dir();
            let has_tests_dir = member_root.join("tests").is_dir();
            let has_build_rs = member_root.join("build.rs").is_file();

            let classification = if self.classify {
                Some(classify_member(
                    has_lib_target,
                    has_bin_targets,
                    has_test_targets,
                    has_example_targets,
                    has_bench_targets,
                    has_tests_dir,
                ))
            } else {
                None
            };

            out_members.push(WorkspaceMemberClassified {
                name: pkg.name.to_string(),
                manifest_path: pkg.manifest_path.as_std_path().display().to_string(),
                member_root: member_root.display().to_string(),
                has_lib_target,
                has_bin_targets,
                has_test_targets,
                has_example_targets,
                has_bench_targets,
                lib_src_path,
                bin_src_paths,
                has_src_dir,
                has_tests_dir,
                has_build_rs,
                classification,
            });
        }
        out_members.sort_by(|a, b| a.name.cmp(&b.name));

        Ok(DebugOutput::WorkspaceMembers(DebugWorkspaceMembersOut {
            workspace_root: metadata.workspace_root.as_std_path().display().to_string(),
            member_count: out_members.len(),
            members: out_members,
        }))
    }
}

// --- discovery rules explainer ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugDiscoveryRules {
    /// Crate root path (must contain `Cargo.toml`)
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugDiscoveryRulesOut {
    pub crate_root: String,
    pub manifest_path: String,
    pub src_scan_root: String,
    pub custom_lib_path: Option<String>,
    pub custom_bin_paths: Vec<String>,
    pub candidate_sources: Vec<DiscoveryRuleCandidate>,
    pub include_rules: Vec<String>,
    pub exclusion_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoveryRuleCandidate {
    pub source: String,
    pub path: String,
    pub exists: bool,
    pub is_file: bool,
    pub is_dir: bool,
}

impl Command for DebugDiscoveryRules {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_debug_target_path(ctx, &self.path)?;
        let manifest_path = canon.join("Cargo.toml");
        let manifest_str = std::fs::read_to_string(&manifest_path).map_err(|e| {
            XtaskError::Resource(format!("Failed to read {}: {e}", manifest_path.display()))
        })?;
        let manifest: CargoManifest = toml::from_str(&manifest_str).map_err(|e| {
            XtaskError::Parse(format!("Failed to parse {}: {e}", manifest_path.display()))
        })?;

        let src_scan_root = canon.join("src");
        let custom_lib_path = manifest
            .lib
            .as_ref()
            .map(|lib| canon.join(&lib.path).display().to_string());
        let mut custom_bin_paths: Vec<String> = manifest
            .bin
            .as_ref()
            .map(|bins| {
                bins.iter()
                    .map(|b| canon.join(&b.path).display().to_string())
                    .collect()
            })
            .unwrap_or_default();
        custom_bin_paths.sort();

        let mut candidates = Vec::new();
        candidates.push(candidate_for("src_walk", &src_scan_root));
        if let Some(ref lib) = manifest.lib {
            candidates.push(candidate_for("lib_target", &canon.join(&lib.path)));
        }
        if let Some(ref bins) = manifest.bin {
            for b in bins {
                candidates.push(candidate_for("bin_target", &canon.join(&b.path)));
            }
        }

        Ok(DebugOutput::DiscoveryRules(DebugDiscoveryRulesOut {
            crate_root: canon.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            src_scan_root: src_scan_root.display().to_string(),
            custom_lib_path,
            custom_bin_paths,
            candidate_sources: candidates,
            include_rules: vec![
                "Include explicit `[lib].path` when it exists and is a file.".into(),
                "Include explicit `[[bin]].path` entries when they exist and are files.".into(),
                "If `src/` exists and is a directory, recursively include `*.rs` files under `src/`.".into(),
                "Discovery errors with `SrcNotFound` when no source files are collected.".into(),
            ],
            exclusion_rules: vec![
                "When discovered files include `lib.rs`, `main.rs` files are filtered out.".into(),
                "Non-Rust files are ignored during `src/` walk.".into(),
            ],
        }))
    }
}

// --- corpus harness ---

#[derive(Debug, Clone, clap::Args)]
pub struct DebugCorpus {
    /// Run the single-crate corpus harness against many Git repositories.
    ///
    /// This command is intended for broad parser validation against real-world code. It classifies
    /// each checkout first:
    /// - single-crate repositories run through discovery, resolve, and optional merge
    /// - workspace repositories are recorded in the corpus summary but skipped here, since they
    ///   are meant to be handled via `parse workspace-config` / `parse_workspace_with_config`
    ///
    /// The command persists artifacts for later inspection, making it useful when a parser panic
    /// or invariant failure is intermittent or expensive to reproduce by hand.
    ///
    /// Example commands:
    /// ```text
    /// cargo xtask --format json parse debug corpus --limit 1
    /// cargo xtask --format json parse debug corpus --list-file /tmp/ploke-targets.txt
    /// cargo xtask --format json parse debug corpus --list-file /tmp/ploke-targets.txt --checkout-dir /tmp/ploke-corpus
    /// cargo xtask --format json parse debug corpus --skip-merge
    /// ```
    ///
    /// Artifact layout:
    /// ```text
    /// target/debug_corpus_runs/<run-id>/
    ///   summary.json
    ///   <target-slug>/
    ///     summary.json
    ///     discovery/
    ///       discovery.json
    ///       stage_summary.json
    ///     resolve/
    ///       resolve.json
    ///       stage_summary.json
    ///     merge/
    ///       merged_graph.json
    ///       stage_summary.json
    /// ```
    ///
    /// On stage failure or panic, the corresponding stage directory also includes `failure.json`.
    ///
    /// Additional list file(s) of targets (`owner/repo(.git)`, URL, or local git path).
    ///
    /// When omitted, defaults to `top_100_stars.txt` and `top_100_downloads.txt`
    /// from the workspace root.
    #[arg(long = "list-file", value_name = "PATH")]
    pub list_files: Vec<PathBuf>,

    /// Directory used to store cloned repositories.
    ///
    /// Keeping this outside the main `ploke` git worktree is usually the cleanest option for
    /// ad-hoc corpus runs.
    #[arg(
        long,
        value_name = "DIR",
        default_value = "tests/fixture_github_clones/corpus"
    )]
    pub checkout_dir: PathBuf,

    /// Directory used to store persisted corpus artifacts and diagnostics.
    ///
    /// The command creates a timestamp-like run directory under this root and writes both
    /// run-level and per-target summaries there.
    #[arg(long, value_name = "DIR", default_value = "target/debug_corpus_runs")]
    pub artifact_dir: PathBuf,

    /// Max number of unique targets to process after deduplication (0 = unlimited).
    #[arg(long, default_value_t = 0)]
    pub limit: usize,

    /// Reuse only already-cloned repos; do not run `git clone`.
    #[arg(long)]
    pub skip_clone: bool,

    /// Stop after discovery + resolve (skip merge / module-tree build).
    ///
    /// Useful when you are narrowing a failure to pre-merge behavior or want a faster first pass.
    #[arg(long)]
    pub skip_merge: bool,

    /// How to handle workspace repositories in the corpus.
    ///
    /// `skip` preserves the existing behavior and only records workspace classification.
    /// `probe` runs discovery/resolve/(optional) merge per workspace member and persists artifacts.
    #[arg(long, value_enum, default_value_t = CorpusWorkspaceMode::Skip)]
    pub workspace_mode: CorpusWorkspaceMode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugCorpusOut {
    pub run_id: String,
    pub checkout_root: String,
    pub artifact_root: String,
    pub list_files: Vec<String>,
    pub requested_entries: usize,
    pub unique_targets: usize,
    pub processed_targets: usize,
    pub single_crate_targets: usize,
    pub workspace_targets: usize,
    pub skipped_targets: usize,
    pub cloned_targets: usize,
    pub reused_targets: usize,
    pub clone_failures: usize,
    pub discovery_failures: usize,
    pub resolve_failures: usize,
    pub merge_failures: usize,
    pub panic_failures: usize,
    pub workspace_mode: String,
    pub targets: Vec<CorpusTargetResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusTargetResult {
    pub target: String,
    pub normalized_repo: String,
    pub clone_url: String,
    pub datasets: Vec<String>,
    pub checkout_path: String,
    pub artifact_dir: String,
    pub repository_kind: String,
    pub recommended_parser: String,
    pub workspace_member_count: Option<usize>,
    pub classification_error: Option<String>,
    pub classification_diagnostic: Option<CorpusDiagnostic>,
    pub clone: CorpusCloneStatus,
    pub commit_sha: Option<String>,
    pub discovery: Option<CorpusStageResult>,
    pub resolve: Option<CorpusStageResult>,
    pub merge: Option<CorpusStageResult>,
    pub workspace_probe: Option<CorpusWorkspaceProbeResult>,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusCloneStatus {
    pub ok: bool,
    pub action: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusStageResult {
    pub ok: bool,
    pub panic: bool,
    pub failure_kind: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub nodes_parsed: Option<usize>,
    pub relations_found: Option<usize>,
    pub file_count: Option<usize>,
    pub artifact_path: Option<String>,
    pub failure_artifact_path: Option<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum CorpusWorkspaceMode {
    Skip,
    Probe,
}

impl CorpusWorkspaceMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Skip => "skip",
            Self::Probe => "probe",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusWorkspaceProbeResult {
    pub workspace_root: String,
    pub member_count: usize,
    pub failed_members: usize,
    pub members: Vec<CorpusWorkspaceMemberResult>,
    pub summary_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusWorkspaceMemberResult {
    pub path: String,
    pub label: String,
    pub artifact_dir: String,
    pub discovery: CorpusStageResult,
    pub resolve: Option<CorpusStageResult>,
    pub merge: Option<CorpusStageResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusDiagnostic {
    pub kind: String,
    pub summary: String,
    pub detail: Option<String>,
    pub source_path: Option<String>,
    pub source_span: Option<CorpusSourceSpan>,
    pub emission_site: Option<CorpusEmissionSite>,
    pub backtrace: Option<String>,
    pub context: Vec<CorpusDiagnosticField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusSourceSpan {
    pub start: Option<usize>,
    pub end: Option<usize>,
    pub line: Option<u32>,
    pub col: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusDiagnosticField {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusEmissionSite {
    pub file: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone)]
struct CorpusTargetSpec {
    original: String,
    normalized_repo: String,
    clone_url: String,
    datasets: BTreeSet<String>,
    checkout_slug: String,
}

#[derive(Debug, Clone)]
struct CorpusCheckoutClassification {
    repository_kind: String,
    recommended_parser: String,
    workspace_member_count: Option<usize>,
    classification_error: Option<String>,
    classification_diagnostic: Option<CorpusDiagnostic>,
    should_run_single_crate_pipeline: bool,
}

impl Command for DebugCorpus {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let workspace_root = ctx.workspace_root()?;
        let checkout_root = if self.checkout_dir.is_absolute() {
            self.checkout_dir.clone()
        } else {
            workspace_root.join(&self.checkout_dir)
        };
        let artifact_base = if self.artifact_dir.is_absolute() {
            self.artifact_dir.clone()
        } else {
            workspace_root.join(&self.artifact_dir)
        };
        let run_id = corpus_run_id();
        let artifact_root = artifact_base.join(&run_id);
        std::fs::create_dir_all(&checkout_root).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create corpus checkout dir {}: {e}",
                checkout_root.display()
            ))
        })?;
        std::fs::create_dir_all(&artifact_root).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create corpus artifact dir {}: {e}",
                artifact_root.display()
            ))
        })?;

        let list_files = if self.list_files.is_empty() {
            vec![
                workspace_root.join("tests/fixture_github_clones/corpus/top_100_stars.txt"),
                workspace_root.join("tests/fixture_github_clones/corpus/top_100_downloads.txt"),
            ]
        } else {
            self.list_files
                .iter()
                .map(|p| {
                    if p.is_absolute() {
                        p.clone()
                    } else {
                        workspace_root.join(p)
                    }
                })
                .collect()
        };

        let mut requested_entries = 0usize;
        let mut target_map: HashMap<String, CorpusTargetSpec> = HashMap::new();
        for list_file in &list_files {
            let dataset_name = list_file
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();
            let content = std::fs::read_to_string(list_file).map_err(|e| {
                XtaskError::Resource(format!(
                    "Failed to read corpus list {}: {e}",
                    list_file.display()
                ))
            })?;
            for raw in content.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                requested_entries += 1;
                let parsed = parse_corpus_target(line)?;
                let key = parsed.normalized_repo.clone();
                target_map
                    .entry(key)
                    .and_modify(|existing| {
                        existing.datasets.insert(dataset_name.clone());
                    })
                    .or_insert_with(|| {
                        let mut datasets = BTreeSet::new();
                        datasets.insert(dataset_name.clone());
                        CorpusTargetSpec {
                            original: parsed.original,
                            normalized_repo: parsed.normalized_repo,
                            clone_url: parsed.clone_url,
                            datasets,
                            checkout_slug: parsed.checkout_slug,
                        }
                    });
            }
        }

        let unique_targets = target_map.len();
        let mut specs: Vec<CorpusTargetSpec> = target_map.into_values().collect();
        specs.sort_by(|a, b| a.normalized_repo.cmp(&b.normalized_repo));
        if self.limit > 0 && specs.len() > self.limit {
            specs.truncate(self.limit);
        }

        let mut targets = Vec::with_capacity(specs.len());
        let mut single_crate_targets = 0usize;
        let mut workspace_targets = 0usize;
        let mut skipped_targets = 0usize;
        let mut cloned_targets = 0usize;
        let mut reused_targets = 0usize;
        let mut clone_failures = 0usize;
        let mut discovery_failures = 0usize;
        let mut resolve_failures = 0usize;
        let mut merge_failures = 0usize;
        let mut panic_failures = 0usize;

        let total_targets = specs.len();
        for (index, spec) in specs.into_iter().enumerate() {
            let checkout_path = checkout_root.join(&spec.checkout_slug);
            let target_artifact_dir = artifact_root.join(&spec.checkout_slug);
            let target_label = spec.normalized_repo.clone();
            emit_corpus_progress_line(format_args!(
                "[{}/{}] target {}",
                index + 1,
                total_targets,
                target_label
            ));
            std::fs::create_dir_all(&target_artifact_dir).map_err(|e| {
                XtaskError::Resource(format!(
                    "Failed to create target artifact dir {}: {e}",
                    target_artifact_dir.display()
                ))
            })?;
            if !checkout_path.join(".git").is_dir() {
                emit_corpus_progress_line(format_args!(
                    "[{}/{}] {} checkout {}",
                    index + 1,
                    total_targets,
                    target_label,
                    if self.skip_clone {
                        "missing (skip-clone)"
                    } else {
                        "clone start"
                    }
                ));
            }
            let clone = ensure_corpus_checkout(&spec, &checkout_path, self.skip_clone)?;
            emit_corpus_progress_line(format_args!(
                "[{}/{}] {} checkout {}{}",
                index + 1,
                total_targets,
                target_label,
                clone.action,
                clone
                    .error
                    .as_deref()
                    .map(|err| format!(": {}", truncate_progress_error(err)))
                    .unwrap_or_default()
            ));
            match clone.action.as_str() {
                "cloned" => cloned_targets += 1,
                "reused" => reused_targets += 1,
                "skipped_missing" => skipped_targets += 1,
                _ => {}
            }
            if !clone.ok {
                clone_failures += 1;
                let target_result = CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    artifact_dir: target_artifact_dir.display().to_string(),
                    repository_kind: "unknown".into(),
                    recommended_parser: "unknown".into(),
                    workspace_member_count: None,
                    classification_error: Some(
                        "clone failed before manifest classification".into(),
                    ),
                    classification_diagnostic: None,
                    clone,
                    commit_sha: None,
                    discovery: Some(CorpusStageResult {
                        ok: false,
                        panic: false,
                        failure_kind: Some("clone".into()),
                        error: Some("clone step did not produce a usable checkout".into()),
                        duration_ms: 0,
                        nodes_parsed: None,
                        relations_found: None,
                        file_count: None,
                        artifact_path: None,
                        failure_artifact_path: None,
                    }),
                    resolve: None,
                    merge: None,
                    workspace_probe: None,
                    summary_path: None,
                };
                let summary_path = persist_target_summary(&target_artifact_dir, &target_result)?;
                let mut target_result = target_result;
                target_result.summary_path = Some(summary_path.display().to_string());
                targets.push(target_result);
                emit_corpus_progress_line(format_args!(
                    "[{}/{}] {} complete (clone failure)",
                    index + 1,
                    total_targets,
                    target_label
                ));
                continue;
            }

            let commit_sha = git_stdout(&checkout_path, &["rev-parse", "HEAD"]).ok();
            let classification = classify_corpus_checkout(&checkout_path);
            emit_corpus_progress_line(format_args!(
                "[{}/{}] {} classified as {}",
                index + 1,
                total_targets,
                target_label,
                classification.repository_kind
            ));
            match classification.repository_kind.as_str() {
                "workspace" => workspace_targets += 1,
                "single_crate" => single_crate_targets += 1,
                _ => {}
            }
            if !classification.should_run_single_crate_pipeline {
                let workspace_probe = if classification.repository_kind == "workspace"
                    && matches!(self.workspace_mode, CorpusWorkspaceMode::Probe)
                {
                    Some(run_corpus_workspace_probe(
                        &target_label,
                        &checkout_path,
                        &target_artifact_dir,
                        self.skip_merge,
                    )?)
                } else {
                    None
                };
                if let Some(probe) = &workspace_probe {
                    for member in &probe.members {
                        if !member.discovery.ok {
                            discovery_failures += 1;
                            if member.discovery.panic {
                                panic_failures += 1;
                            }
                        }
                        if let Some(resolve) = &member.resolve {
                            if !resolve.ok {
                                resolve_failures += 1;
                                if resolve.panic {
                                    panic_failures += 1;
                                }
                            }
                        }
                        if let Some(merge) = &member.merge {
                            if !merge.ok {
                                merge_failures += 1;
                                if merge.panic {
                                    panic_failures += 1;
                                }
                            }
                        }
                    }
                }
                let target_result = CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    artifact_dir: target_artifact_dir.display().to_string(),
                    repository_kind: classification.repository_kind,
                    recommended_parser: classification.recommended_parser,
                    workspace_member_count: classification.workspace_member_count,
                    classification_error: classification.classification_error,
                    classification_diagnostic: classification.classification_diagnostic,
                    clone,
                    commit_sha,
                    discovery: None,
                    resolve: None,
                    merge: None,
                    workspace_probe,
                    summary_path: None,
                };
                let summary_path = persist_target_summary(&target_artifact_dir, &target_result)?;
                let mut target_result = target_result;
                target_result.summary_path = Some(summary_path.display().to_string());
                targets.push(target_result);
                emit_corpus_progress_line(format_args!(
                    "[{}/{}] {} complete",
                    index + 1,
                    total_targets,
                    target_label
                ));
                continue;
            }

            let discovery =
                run_corpus_discovery_stage(&checkout_path, &target_artifact_dir, &target_label)?;
            if !discovery.ok {
                discovery_failures += 1;
                if discovery.panic {
                    panic_failures += 1;
                }
                let target_result = CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    artifact_dir: target_artifact_dir.display().to_string(),
                    repository_kind: classification.repository_kind.clone(),
                    recommended_parser: classification.recommended_parser.clone(),
                    workspace_member_count: classification.workspace_member_count,
                    classification_error: classification.classification_error.clone(),
                    classification_diagnostic: classification.classification_diagnostic.clone(),
                    clone,
                    commit_sha,
                    discovery: Some(discovery),
                    resolve: None,
                    merge: None,
                    workspace_probe: None,
                    summary_path: None,
                };
                let summary_path = persist_target_summary(&target_artifact_dir, &target_result)?;
                let mut target_result = target_result;
                target_result.summary_path = Some(summary_path.display().to_string());
                targets.push(target_result);
                emit_corpus_progress_line(format_args!(
                    "[{}/{}] {} complete (discovery failure)",
                    index + 1,
                    total_targets,
                    target_label
                ));
                continue;
            }

            let resolve = run_corpus_stage("resolve", &target_artifact_dir, &target_label, || {
                let graphs =
                    try_run_phases_and_resolve(&checkout_path).map_err(|e| e.to_string())?;
                let nodes: usize = graphs
                    .iter()
                    .map(|pg| count_code_graph_nodes(&pg.graph))
                    .sum();
                let rels: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
                let artifact_path =
                    persist_stage_payload(&target_artifact_dir, "resolve", "resolve.json", &graphs)
                        .map_err(|e| e.to_string())?;
                Ok(CorpusStageMetrics {
                    nodes_parsed: Some(nodes),
                    relations_found: Some(rels),
                    file_count: None,
                    artifact_path: Some(artifact_path.display().to_string()),
                })
            })?;
            if !resolve.ok {
                resolve_failures += 1;
                if resolve.panic {
                    panic_failures += 1;
                }
            }

            let merge = if !self.skip_merge && resolve.ok {
                let stage = run_corpus_stage("merge", &target_artifact_dir, &target_label, || {
                    let out =
                        try_run_phases_and_merge(&checkout_path).map_err(|e| e.to_string())?;
                    let (nodes, rels) = if let Some(ref mg) = out.merged_graph {
                        (
                            Some(count_code_graph_nodes(&mg.graph)),
                            Some(mg.graph.relations.len()),
                        )
                    } else {
                        (None, None)
                    };
                    let artifact_path = if let Some(ref merged_graph) = out.merged_graph {
                        Some(
                            persist_stage_payload(
                                &target_artifact_dir,
                                "merge",
                                "merged_graph.json",
                                merged_graph,
                            )
                            .map_err(|e| e.to_string())?
                            .display()
                            .to_string(),
                        )
                    } else {
                        None
                    };
                    Ok(CorpusStageMetrics {
                        nodes_parsed: nodes,
                        relations_found: rels,
                        file_count: None,
                        artifact_path,
                    })
                })?;
                if !stage.ok {
                    merge_failures += 1;
                    if stage.panic {
                        panic_failures += 1;
                    }
                }
                Some(stage)
            } else {
                None
            };

            let target_result = CorpusTargetResult {
                target: spec.original,
                normalized_repo: spec.normalized_repo,
                clone_url: spec.clone_url,
                datasets: spec.datasets.into_iter().collect(),
                checkout_path: checkout_path.display().to_string(),
                artifact_dir: target_artifact_dir.display().to_string(),
                repository_kind: classification.repository_kind,
                recommended_parser: classification.recommended_parser,
                workspace_member_count: classification.workspace_member_count,
                classification_error: classification.classification_error,
                classification_diagnostic: classification.classification_diagnostic,
                clone,
                commit_sha,
                discovery: Some(discovery),
                resolve: Some(resolve),
                merge,
                workspace_probe: None,
                summary_path: None,
            };
            let summary_path = persist_target_summary(&target_artifact_dir, &target_result)?;
            let mut target_result = target_result;
            target_result.summary_path = Some(summary_path.display().to_string());
            targets.push(target_result);
            emit_corpus_progress_line(format_args!(
                "[{}/{}] {} complete",
                index + 1,
                total_targets,
                target_label
            ));
        }

        let out = DebugCorpusOut {
            run_id,
            checkout_root: checkout_root.display().to_string(),
            artifact_root: artifact_root.display().to_string(),
            list_files: list_files.iter().map(|p| p.display().to_string()).collect(),
            requested_entries,
            unique_targets,
            processed_targets: targets.len(),
            single_crate_targets,
            workspace_targets,
            skipped_targets,
            cloned_targets,
            reused_targets,
            clone_failures,
            discovery_failures,
            resolve_failures,
            merge_failures,
            panic_failures,
            workspace_mode: self.workspace_mode.as_str().to_string(),
            targets,
        };
        persist_run_summary(&artifact_root, &out)?;
        Ok(DebugOutput::Corpus(out))
    }
}

#[derive(Debug, Clone, clap::Args)]
pub struct DebugCorpusShow {
    /// Corpus run ID like `run-1774750473411`, or a path to a run directory / `summary.json`.
    #[arg(value_name = "RUN_OR_PATH")]
    pub run: PathBuf,

    /// Corpus artifact base dir used when resolving a run ID.
    #[arg(long, value_name = "DIR", default_value = "target/debug_corpus_runs")]
    pub artifact_dir: PathBuf,

    /// Narrow output to one target (`owner/repo` or `owner__repo`).
    #[arg(long, value_name = "TARGET")]
    pub target: Option<String>,

    /// Narrow workspace probe output to one member label or path component.
    #[arg(long, value_name = "MEMBER")]
    pub member: Option<String>,

    /// Print a concise parser/xtask backtrace summary for the displayed failed target(s).
    #[arg(long, conflicts_with = "backtrace_full")]
    pub backtrace: bool,

    /// Print the full persisted backtrace for the displayed failed target(s).
    #[arg(long)]
    pub backtrace_full: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugCorpusShowOut {
    pub run: DebugCorpusOut,
    pub selected_target: Option<String>,
    pub selected_member: Option<String>,
    pub show_backtrace: bool,
    pub show_backtrace_full: bool,
    pub summary_path: String,
}

#[derive(Debug, Clone, clap::Args)]
pub struct DebugCorpusTriage {
    /// Corpus run ID like `run-1774750473411`, or a path to a run directory / `summary.json`.
    #[arg(value_name = "RUN_OR_PATH")]
    pub run: PathBuf,

    /// Corpus artifact base dir used when resolving a run ID.
    #[arg(long, value_name = "DIR", default_value = "target/debug_corpus_runs")]
    pub artifact_dir: PathBuf,

    /// Directory used to store triage indexes and pending report stubs.
    ///
    /// Defaults to `<run-dir>/triage`.
    #[arg(long, value_name = "DIR")]
    pub out_dir: Option<PathBuf>,

    /// Skip creating one pending JSON report stub per failure cluster.
    #[arg(long)]
    pub no_report_stubs: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugCorpusTriageOut {
    pub run_id: String,
    pub summary_path: String,
    pub triage_dir: String,
    pub failures_path: String,
    pub clusters_path: String,
    pub report_template_path: String,
    pub pending_report_dir: String,
    pub failure_count: usize,
    pub cluster_count: usize,
    pub pending_report_count: usize,
    pub failures: Vec<CorpusTriageFailure>,
    pub clusters: Vec<CorpusTriageCluster>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusTriageFailure {
    pub id: String,
    pub run_id: String,
    pub target: String,
    pub normalized_repo: String,
    pub repository_kind: String,
    pub commit_sha: Option<String>,
    pub source: String,
    pub member_label: Option<String>,
    pub member_path: Option<String>,
    pub member_artifact_dir: Option<String>,
    pub stage: String,
    pub failure_kind: String,
    pub panic: bool,
    pub error_signature: String,
    pub error_excerpt: String,
    pub failure_artifact_path: Option<String>,
    pub artifact_path: Option<String>,
    pub target_summary_path: Option<String>,
    pub workspace_summary_path: Option<String>,
    pub cluster_key: String,
    pub cluster_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorpusTriageCluster {
    pub key: String,
    pub slug: String,
    pub stage: String,
    pub failure_kind: String,
    pub error_signature: String,
    pub count: usize,
    pub repos: Vec<String>,
    pub examples: Vec<String>,
    pub pending_report_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CorpusTriageReportTemplate {
    version: u32,
    run_id: Option<String>,
    cluster_key: Option<String>,
    cluster_slug: Option<String>,
    status: String,
    signature: Option<String>,
    stage: Option<String>,
    failure_kind: Option<String>,
    occurrence_count: Option<usize>,
    example_failures: Vec<String>,
    suspected_root_cause: Option<String>,
    confidence: Option<String>,
    scope_assessment: Option<String>,
    touches_sensitive_pipeline: Option<bool>,
    recommended_next_step: Option<String>,
    minimal_repro_status: String,
    relevant_artifacts: Vec<String>,
    relevant_code_paths: Vec<String>,
    notes: Vec<String>,
}

impl Command for DebugCorpusShow {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let summary_path = resolve_corpus_summary_path(ctx, &self.run, &self.artifact_dir)?;
        let content = std::fs::read_to_string(&summary_path).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to read corpus summary {}: {e}",
                summary_path.display()
            ))
        })?;
        let mut run: DebugCorpusOut = serde_json::from_str(&content).map_err(|e| {
            XtaskError::Parse(format!(
                "Failed to parse corpus summary {}: {e}",
                summary_path.display()
            ))
        })?;

        if let Some(target) = &self.target {
            let matches: Vec<_> = run
                .targets
                .iter()
                .filter(|entry| corpus_target_matches(entry, target))
                .cloned()
                .collect();
            if matches.is_empty() {
                return Err(XtaskError::validation(format!(
                    "Target `{target}` not found in corpus run `{}`",
                    run.run_id
                ))
                .into());
            }
            run.targets = matches;
            recompute_corpus_counts(&mut run);
        }

        if let Some(member) = &self.member {
            let mut matched_targets = Vec::new();
            for mut target in run.targets.drain(..) {
                let Some(probe) = target.workspace_probe.as_mut() else {
                    continue;
                };
                probe
                    .members
                    .retain(|entry| corpus_workspace_member_matches(entry, member));
                if probe.members.is_empty() {
                    continue;
                }
                probe.member_count = probe.members.len();
                probe.failed_members = probe
                    .members
                    .iter()
                    .filter(|entry| workspace_member_failed(entry))
                    .count();
                target.workspace_member_count = Some(probe.member_count);
                matched_targets.push(target);
            }

            if matched_targets.is_empty() {
                return Err(XtaskError::validation(format!(
                    "Workspace member `{member}` not found in corpus run `{}`",
                    run.run_id
                ))
                .into());
            }

            run.targets = matched_targets;
            recompute_corpus_counts(&mut run);
        }

        Ok(DebugOutput::CorpusShow(DebugCorpusShowOut {
            run,
            selected_target: self.target.clone(),
            selected_member: self.member.clone(),
            show_backtrace: self.backtrace,
            show_backtrace_full: self.backtrace_full,
            summary_path: summary_path.display().to_string(),
        }))
    }
}

impl Command for DebugCorpusTriage {
    type Output = DebugOutput;
    type Error = XtaskError;

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let summary_path = resolve_corpus_summary_path(ctx, &self.run, &self.artifact_dir)?;
        let content = std::fs::read_to_string(&summary_path).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to read corpus summary {}: {e}",
                summary_path.display()
            ))
        })?;
        let run: DebugCorpusOut = serde_json::from_str(&content).map_err(|e| {
            XtaskError::Parse(format!(
                "Failed to parse corpus summary {}: {e}",
                summary_path.display()
            ))
        })?;

        let triage_dir = if let Some(out_dir) = &self.out_dir {
            let workspace_root = ctx.workspace_root()?;
            if out_dir.is_absolute() {
                out_dir.clone()
            } else {
                workspace_root.join(out_dir)
            }
        } else {
            summary_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("triage")
        };
        std::fs::create_dir_all(&triage_dir).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create corpus triage dir {}: {e}",
                triage_dir.display()
            ))
        })?;

        let failures = collect_corpus_triage_failures(&run);
        let pending_report_dir = triage_dir.join("reports").join("pending");
        let report_template_path = triage_dir.join("reports").join("_report_template.json");
        let clusters = build_corpus_triage_clusters(
            &failures,
            &run.run_id,
            if self.no_report_stubs {
                None
            } else {
                Some(&pending_report_dir)
            },
        )?;

        let template = CorpusTriageReportTemplate {
            version: 1,
            run_id: None,
            cluster_key: None,
            cluster_slug: None,
            status: "pending".into(),
            signature: None,
            stage: None,
            failure_kind: None,
            occurrence_count: None,
            example_failures: Vec::new(),
            suspected_root_cause: None,
            confidence: None,
            scope_assessment: None,
            touches_sensitive_pipeline: None,
            recommended_next_step: None,
            minimal_repro_status: "not_started".into(),
            relevant_artifacts: Vec::new(),
            relevant_code_paths: Vec::new(),
            notes: Vec::new(),
        };
        write_json_file(&report_template_path, &template)?;

        let failures_path = triage_dir.join("failures.jsonl");
        write_jsonl_file(&failures_path, &failures)?;
        let clusters_path = triage_dir.join("clusters.json");
        write_json_file(&clusters_path, &clusters)?;

        let out = DebugCorpusTriageOut {
            run_id: run.run_id.clone(),
            summary_path: summary_path.display().to_string(),
            triage_dir: triage_dir.display().to_string(),
            failures_path: failures_path.display().to_string(),
            clusters_path: clusters_path.display().to_string(),
            report_template_path: report_template_path.display().to_string(),
            pending_report_dir: pending_report_dir.display().to_string(),
            failure_count: failures.len(),
            cluster_count: clusters.len(),
            pending_report_count: clusters
                .iter()
                .filter(|cluster| cluster.pending_report_path.is_some())
                .count(),
            failures,
            clusters,
        };
        write_json_file(&triage_dir.join("index.json"), &out)?;

        Ok(DebugOutput::CorpusTriage(out))
    }
}

#[derive(Debug, Clone)]
struct ParsedCorpusTarget {
    original: String,
    normalized_repo: String,
    clone_url: String,
    checkout_slug: String,
}

#[derive(Debug, Clone, Default)]
struct CorpusStageMetrics {
    nodes_parsed: Option<usize>,
    relations_found: Option<usize>,
    file_count: Option<usize>,
    artifact_path: Option<String>,
}

fn parse_corpus_target(input: &str) -> Result<ParsedCorpusTarget, XtaskError> {
    let original = input.trim().to_string();
    if original.is_empty() {
        return Err(XtaskError::validation("empty corpus target line").into());
    }

    let source = original.trim_end_matches('/');
    let (normalized_repo, clone_url, checkout_slug) =
        if source.starts_with("http://") || source.starts_with("https://") {
            let slug = url_slug(source).ok_or_else(|| {
                XtaskError::validation(format!("Unsupported corpus URL format `{source}`"))
            })?;
            (
                slug.clone(),
                source.to_string(),
                slug.replace('/', "__").replace(".git", ""),
            )
        } else if source.starts_with("./") || source.starts_with("../") || source.starts_with('/') {
            let path = Path::new(source);
            let repo = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or_else(|| {
                    XtaskError::validation(format!(
                        "Local corpus target `{source}` has no final path component"
                    ))
                })?
                .trim_end_matches(".git")
                .to_string();
            (
                repo.clone(),
                path.to_string_lossy().to_string(),
                repo.replace('/', "__"),
            )
        } else {
            let slug = source.trim_end_matches(".git");
            let mut parts = slug.split('/');
            let owner = parts.next().ok_or_else(|| {
                XtaskError::validation(format!("Corpus target `{source}` is missing an owner"))
            })?;
            let repo = parts.next().ok_or_else(|| {
                XtaskError::validation(format!("Corpus target `{source}` is missing a repo"))
            })?;
            if parts.next().is_some() {
                return Err(XtaskError::validation(format!(
                    "Corpus target `{source}` must be `owner/repo(.git)` or a URL"
                ))
                .into());
            }
            let normalized = format!("{owner}/{repo}");
            (
                normalized.clone(),
                format!("https://github.com/{normalized}.git"),
                normalized.replace('/', "__"),
            )
        };

    Ok(ParsedCorpusTarget {
        original,
        normalized_repo,
        clone_url,
        checkout_slug,
    })
}

fn url_slug(source: &str) -> Option<String> {
    let without_scheme = source.split_once("://")?.1;
    let path = without_scheme.split_once('/')?.1;
    let trimmed = path.trim_end_matches('/');
    let trimmed = trimmed.trim_end_matches(".git");
    let mut parts = trimmed.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    Some(format!("{owner}/{repo}"))
}

fn ensure_corpus_checkout(
    spec: &CorpusTargetSpec,
    checkout_path: &Path,
    skip_clone: bool,
) -> Result<CorpusCloneStatus, XtaskError> {
    if checkout_path.join(".git").is_dir() {
        return Ok(CorpusCloneStatus {
            ok: true,
            action: "reused".into(),
            error: None,
        });
    }
    if checkout_path.exists() && !checkout_path.join(".git").is_dir() {
        return Ok(CorpusCloneStatus {
            ok: false,
            action: "invalid_existing_path".into(),
            error: Some(format!(
                "Checkout path {} exists but is not a git repository",
                checkout_path.display()
            )),
        });
    }
    if skip_clone {
        return Ok(CorpusCloneStatus {
            ok: false,
            action: "skipped_missing".into(),
            error: Some(format!(
                "Checkout {} does not exist and --skip-clone was set",
                checkout_path.display()
            )),
        });
    }
    if let Some(parent) = checkout_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create checkout parent {}: {e}",
                parent.display()
            ))
        })?;
    }
    let output = ProcessCommand::new("git")
        .arg("clone")
        .arg("--depth")
        .arg("1")
        .arg(&spec.clone_url)
        .arg(checkout_path)
        .output()
        .map_err(|e| XtaskError::Resource(format!("Failed to spawn git clone: {e}")))?;
    if output.status.success() {
        Ok(CorpusCloneStatus {
            ok: true,
            action: "cloned".into(),
            error: None,
        })
    } else {
        Ok(CorpusCloneStatus {
            ok: false,
            action: "clone_failed".into(),
            error: Some(stderr_or_status(&output)),
        })
    }
}

fn git_stdout(repo_dir: &Path, args: &[&str]) -> Result<String, XtaskError> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(args)
        .output()
        .map_err(|e| XtaskError::Resource(format!("Failed to spawn git {:?}: {e}", args)))?;
    if !output.status.success() {
        return Err(XtaskError::Parse(format!(
            "git {:?} failed for {}: {}",
            args,
            repo_dir.display(),
            stderr_or_status(&output)
        )));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn stderr_or_status(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("process exited with status {}", output.status)
    } else {
        stderr
    }
}

fn classify_corpus_checkout(checkout_path: &Path) -> CorpusCheckoutClassification {
    match try_parse_manifest(checkout_path, ManifestKind::WorkspaceRoot) {
        Ok(metadata) => {
            if let Some(workspace) = metadata.workspace {
                CorpusCheckoutClassification {
                    repository_kind: "workspace".into(),
                    recommended_parser: "parse_workspace_with_config".into(),
                    workspace_member_count: Some(workspace.members.len()),
                    classification_error: None,
                    classification_diagnostic: None,
                    should_run_single_crate_pipeline: false,
                }
            } else {
                CorpusCheckoutClassification {
                    repository_kind: "single_crate".into(),
                    recommended_parser: "try_run_phases_and_merge".into(),
                    workspace_member_count: None,
                    classification_error: None,
                    classification_diagnostic: None,
                    should_run_single_crate_pipeline: true,
                }
            }
        }
        Err(err) => classify_corpus_checkout_via_metadata(checkout_path).unwrap_or_else(|| {
            CorpusCheckoutClassification {
                repository_kind: "unknown".into(),
                recommended_parser: "try_run_phases_and_merge".into(),
                workspace_member_count: None,
                classification_error: Some(err.diagnostic_summary()),
                classification_diagnostic: Some(corpus_diagnostic_from_discovery_error(&err)),
                should_run_single_crate_pipeline: true,
            }
        }),
    }
}

fn classify_corpus_checkout_via_metadata(
    checkout_path: &Path,
) -> Option<CorpusCheckoutClassification> {
    let metadata = load_cargo_metadata(checkout_path).ok()?;
    let workspace_root = metadata.workspace_root.as_std_path();
    let member_count = metadata.workspace_members.len();
    let package_count = metadata.packages.len();

    if workspace_root == checkout_path && (member_count > 1 || package_count > 1) {
        return Some(CorpusCheckoutClassification {
            repository_kind: "workspace".into(),
            recommended_parser: "parse_workspace_with_config".into(),
            workspace_member_count: Some(member_count),
            classification_error: None,
            classification_diagnostic: None,
            should_run_single_crate_pipeline: false,
        });
    }

    if package_count > 0 {
        return Some(CorpusCheckoutClassification {
            repository_kind: "single_crate".into(),
            recommended_parser: "try_run_phases_and_merge".into(),
            workspace_member_count: None,
            classification_error: None,
            classification_diagnostic: None,
            should_run_single_crate_pipeline: true,
        });
    }

    None
}

fn corpus_diagnostic_from_discovery_error(err: &DiscoveryError) -> CorpusDiagnostic {
    CorpusDiagnostic {
        kind: err.diagnostic_kind().to_string(),
        summary: err.diagnostic_summary(),
        detail: err.diagnostic_detail(),
        source_path: err
            .diagnostic_source_path()
            .map(|path| path.display().to_string()),
        source_span: err.diagnostic_span().map(|span| CorpusSourceSpan {
            start: span.start(),
            end: span.end(),
            line: span.line().map(|line| line as u32),
            col: span.column().map(|col| col as u32),
        }),
        emission_site: err
            .diagnostic_emission_site()
            .map(|site| CorpusEmissionSite {
                file: site.file.to_string(),
                line: site.line,
                column: site.column,
            }),
        backtrace: err.diagnostic_backtrace().map(ToString::to_string),
        context: err
            .diagnostic_context()
            .into_iter()
            .map(|field| CorpusDiagnosticField {
                key: field.key.to_string(),
                value: field.value,
            })
            .collect(),
    }
}

fn run_corpus_discovery_stage(
    crate_root: &Path,
    target_artifact_dir: &Path,
    progress_label: &str,
) -> Result<CorpusStageResult, XtaskError> {
    run_corpus_stage("discovery", target_artifact_dir, progress_label, || {
        let out =
            run_discovery_phase(None, &[crate_root.to_path_buf()]).map_err(|e| e.to_string())?;
        let artifact_path =
            persist_stage_payload(target_artifact_dir, "discovery", "discovery.json", &out)
                .map_err(|e| e.to_string())?;
        let file_count = out
            .crate_contexts
            .get(crate_root)
            .map(|c| c.files.len())
            .or_else(|| out.crate_contexts.values().next().map(|c| c.files.len()));
        Ok(CorpusStageMetrics {
            file_count,
            artifact_path: Some(artifact_path.display().to_string()),
            ..CorpusStageMetrics::default()
        })
    })
}

fn run_corpus_workspace_probe(
    workspace_label: &str,
    workspace_root: &Path,
    target_artifact_dir: &Path,
    skip_merge: bool,
) -> Result<CorpusWorkspaceProbeResult, XtaskError> {
    let members = resolve_workspace_probe_members(workspace_root)?;

    let workspace_artifact_dir = target_artifact_dir.join("workspace_probe");
    std::fs::create_dir_all(&workspace_artifact_dir).map_err(|e| {
        XtaskError::Resource(format!(
            "Failed to create workspace probe artifact dir {}: {e}",
            workspace_artifact_dir.display()
        ))
    })?;

    let mut results = Vec::with_capacity(members.len());
    let mut failed_members = 0usize;

    for (index, member) in members.iter().enumerate() {
        let label = member
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let rel_path = member.strip_prefix(workspace_root).unwrap_or(member);
        let member_artifact_dir = workspace_artifact_dir.join("members").join(format!(
            "{index:03}_{}",
            sanitize_artifact_component(&rel_path.display().to_string())
        ));
        let member_progress_label = format!("{workspace_label}::{}", rel_path.display());
        emit_corpus_progress_line(format_args!(
            "[workspace] member {}/{} {}",
            index + 1,
            members.len(),
            member_progress_label
        ));

        let discovery =
            run_corpus_discovery_stage(member, &member_artifact_dir, &member_progress_label)?;
        let mut member_failed = !discovery.ok;
        let mut resolve = None;
        let mut merge = None;

        if discovery.ok {
            let resolve_stage = run_corpus_stage(
                "resolve",
                &member_artifact_dir,
                &member_progress_label,
                || {
                    let graphs = try_run_phases_and_resolve(member).map_err(|e| e.to_string())?;
                    let nodes: usize = graphs
                        .iter()
                        .map(|pg| count_code_graph_nodes(&pg.graph))
                        .sum();
                    let rels: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
                    let artifact_path = persist_stage_payload(
                        &member_artifact_dir,
                        "resolve",
                        "resolve.json",
                        &graphs,
                    )
                    .map_err(|e| e.to_string())?;
                    Ok(CorpusStageMetrics {
                        nodes_parsed: Some(nodes),
                        relations_found: Some(rels),
                        file_count: None,
                        artifact_path: Some(artifact_path.display().to_string()),
                    })
                },
            )?;
            member_failed |= !resolve_stage.ok;
            resolve = Some(resolve_stage);

            if !skip_merge && resolve.as_ref().is_some_and(|stage| stage.ok) {
                let merge_stage = run_corpus_stage(
                    "merge",
                    &member_artifact_dir,
                    &member_progress_label,
                    || {
                        let out = try_run_phases_and_merge(member).map_err(|e| e.to_string())?;
                        let (nodes, rels) = if let Some(ref mg) = out.merged_graph {
                            (
                                Some(count_code_graph_nodes(&mg.graph)),
                                Some(mg.graph.relations.len()),
                            )
                        } else {
                            (None, None)
                        };
                        let artifact_path = if let Some(ref merged_graph) = out.merged_graph {
                            Some(
                                persist_stage_payload(
                                    &member_artifact_dir,
                                    "merge",
                                    "merged_graph.json",
                                    merged_graph,
                                )
                                .map_err(|e| e.to_string())?
                                .display()
                                .to_string(),
                            )
                        } else {
                            None
                        };
                        Ok(CorpusStageMetrics {
                            nodes_parsed: nodes,
                            relations_found: rels,
                            file_count: None,
                            artifact_path,
                        })
                    },
                )?;
                member_failed |= !merge_stage.ok;
                merge = Some(merge_stage);
            }
        }

        if member_failed {
            failed_members += 1;
        }

        results.push(CorpusWorkspaceMemberResult {
            path: member.display().to_string(),
            label,
            artifact_dir: member_artifact_dir.display().to_string(),
            discovery,
            resolve,
            merge,
        });
    }

    let out = CorpusWorkspaceProbeResult {
        workspace_root: workspace_root.display().to_string(),
        member_count: results.len(),
        failed_members,
        members: results,
        summary_path: None,
    };
    let summary_path =
        persist_stage_payload(&workspace_artifact_dir, "", "workspace_summary.json", &out)?;
    Ok(CorpusWorkspaceProbeResult {
        summary_path: Some(summary_path.display().to_string()),
        ..out
    })
}

fn resolve_workspace_probe_members(workspace_root: &Path) -> Result<Vec<PathBuf>, XtaskError> {
    if let Ok(metadata) = try_parse_manifest(workspace_root, ManifestKind::WorkspaceRoot) {
        if let Some(workspace) = metadata.workspace {
            return Ok(workspace.members);
        }
    }

    let metadata = load_cargo_metadata(workspace_root)?;
    if metadata.workspace_root.as_std_path() != workspace_root {
        return Err(XtaskError::validation(format!(
            "Workspace probe expected workspace root `{}`, but cargo metadata reported `{}`",
            workspace_root.display(),
            metadata.workspace_root
        ))
        .into());
    }

    let workspace_members: HashSet<_> = metadata.workspace_members.iter().cloned().collect();
    let mut members: Vec<PathBuf> = metadata
        .packages
        .iter()
        .filter(|pkg| workspace_members.contains(&pkg.id))
        .filter_map(|pkg| {
            pkg.manifest_path
                .as_std_path()
                .parent()
                .map(Path::to_path_buf)
        })
        .collect();
    members.sort();
    members.dedup();

    if members.is_empty() {
        return Err(XtaskError::validation(format!(
            "No workspace members found for `{}` via cargo metadata",
            workspace_root.display()
        ))
        .into());
    }

    Ok(members)
}

fn run_corpus_stage<F>(
    stage_name: &str,
    target_artifact_dir: &Path,
    progress_label: &str,
    op: F,
) -> Result<CorpusStageResult, XtaskError>
where
    F: FnOnce() -> Result<CorpusStageMetrics, String>,
{
    let stage_dir = target_artifact_dir.join(stage_name);
    std::fs::create_dir_all(&stage_dir).map_err(|e| {
        XtaskError::Resource(format!(
            "Failed to create stage artifact dir {}: {e}",
            stage_dir.display()
        ))
    })?;
    emit_corpus_progress_line(format_args!("{progress_label} {stage_name} start"));
    let start = Instant::now();
    let heartbeat = CorpusStageHeartbeat::start(progress_label.to_string(), stage_name.to_string());
    let outcome = catch_unwind_silencing_hook(|| with_debug_artifact_dir(&stage_dir, op));
    let duration_ms = start.elapsed().as_millis() as u64;
    heartbeat.finish();
    let result = match outcome {
        Ok(Ok(metrics)) => CorpusStageResult {
            ok: true,
            panic: false,
            failure_kind: None,
            error: None,
            duration_ms,
            nodes_parsed: metrics.nodes_parsed,
            relations_found: metrics.relations_found,
            file_count: metrics.file_count,
            artifact_path: metrics.artifact_path,
            failure_artifact_path: None,
        },
        Ok(Err(err)) => CorpusStageResult {
            ok: false,
            panic: false,
            failure_kind: Some("error".into()),
            error: Some(err.clone()),
            duration_ms,
            nodes_parsed: None,
            relations_found: None,
            file_count: None,
            artifact_path: None,
            failure_artifact_path: Some(
                persist_stage_failure(&stage_dir, stage_name, false, &err)?
                    .display()
                    .to_string(),
            ),
        },
        Err(panic) => CorpusStageResult {
            ok: false,
            panic: true,
            failure_kind: Some("panic".into()),
            error: Some(panic.message),
            duration_ms,
            nodes_parsed: None,
            relations_found: None,
            file_count: None,
            artifact_path: None,
            failure_artifact_path: None,
        },
    };
    let result = if result.panic {
        let panic_msg = result.error.clone().unwrap_or_else(|| "panic".into());
        CorpusStageResult {
            failure_artifact_path: Some(
                persist_stage_failure(&stage_dir, stage_name, true, &panic_msg)?
                    .display()
                    .to_string(),
            ),
            ..result
        }
    } else {
        result
    };
    persist_stage_payload(&stage_dir, "", "stage_summary.json", &result)?;
    emit_corpus_progress_line(format_args!(
        "{progress_label} {stage_name} {} {}ms",
        if result.ok {
            "ok"
        } else if result.panic {
            "panic"
        } else {
            "err"
        },
        result.duration_ms
    ));
    Ok(result)
}

const CORPUS_STAGE_HEARTBEAT_SECS: u64 = 15;

struct CorpusStageHeartbeat {
    done: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl CorpusStageHeartbeat {
    fn start(progress_label: String, stage_name: String) -> Self {
        let done = Arc::new(AtomicBool::new(false));
        let done_for_thread = Arc::clone(&done);
        let handle = thread::spawn(move || {
            let mut elapsed_secs = 0u64;
            while !done_for_thread.load(Ordering::Relaxed) {
                thread::sleep(std::time::Duration::from_secs(CORPUS_STAGE_HEARTBEAT_SECS));
                elapsed_secs += CORPUS_STAGE_HEARTBEAT_SECS;
                if done_for_thread.load(Ordering::Relaxed) {
                    break;
                }
                emit_corpus_progress_line(format_args!(
                    "{progress_label} {stage_name} running {}s",
                    elapsed_secs
                ));
            }
        });
        Self {
            done,
            handle: Some(handle),
        }
    }

    fn finish(mut self) {
        self.done.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn emit_corpus_progress_line(args: std::fmt::Arguments<'_>) {
    eprintln!("[corpus] {args}");
}

fn truncate_progress_error(err: &str) -> String {
    let trimmed = err.replace('\n', " ");
    let mut chars = trimmed.chars();
    let shortened: String = chars.by_ref().take(120).collect();
    if chars.next().is_some() {
        format!("{shortened}...")
    } else {
        shortened
    }
}

fn corpus_run_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("run-{millis}")
}

fn persist_run_summary(artifact_root: &Path, out: &DebugCorpusOut) -> Result<PathBuf, XtaskError> {
    write_json_file(&artifact_root.join("summary.json"), out)
}

fn persist_target_summary(
    target_artifact_dir: &Path,
    out: &CorpusTargetResult,
) -> Result<PathBuf, XtaskError> {
    write_json_file(&target_artifact_dir.join("summary.json"), out)
}

fn persist_stage_payload<T: Serialize>(
    target_artifact_dir: &Path,
    stage_name: &str,
    filename: &str,
    payload: &T,
) -> Result<PathBuf, XtaskError> {
    let dir = if stage_name.is_empty() {
        target_artifact_dir.to_path_buf()
    } else {
        target_artifact_dir.join(stage_name)
    };
    std::fs::create_dir_all(&dir).map_err(|e| {
        XtaskError::Resource(format!(
            "Failed to create stage artifact dir {}: {e}",
            dir.display()
        ))
    })?;
    write_json_file(&dir.join(filename), payload)
}

fn persist_stage_failure(
    stage_dir: &Path,
    stage_name: &str,
    panic: bool,
    message: &str,
) -> Result<PathBuf, XtaskError> {
    write_json_file(
        &stage_dir.join("failure.json"),
        &serde_json::json!({
            "stage": stage_name,
            "panic": panic,
            "message": message,
        }),
    )
}

fn write_json_file<T: Serialize>(path: &Path, payload: &T) -> Result<PathBuf, XtaskError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create artifact parent {}: {e}",
                parent.display()
            ))
        })?;
    }
    let file = File::create(path).map_err(|e| {
        XtaskError::Resource(format!("Failed to create artifact {}: {e}", path.display()))
    })?;
    serde_json::to_writer_pretty(file, payload).map_err(|e| {
        XtaskError::Resource(format!(
            "Failed to serialize artifact {}: {e}",
            path.display()
        ))
    })?;
    Ok(path.to_path_buf())
}

fn write_jsonl_file<T: Serialize>(path: &Path, payloads: &[T]) -> Result<PathBuf, XtaskError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create artifact parent {}: {e}",
                parent.display()
            ))
        })?;
    }

    let mut file = File::create(path).map_err(|e| {
        XtaskError::Resource(format!("Failed to create artifact {}: {e}", path.display()))
    })?;
    for payload in payloads {
        serde_json::to_writer(&mut file, payload).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to serialize artifact {}: {e}",
                path.display()
            ))
        })?;
        file.write_all(b"\n").map_err(|e| {
            XtaskError::Resource(format!("Failed to write artifact {}: {e}", path.display()))
        })?;
    }

    Ok(path.to_path_buf())
}

fn sanitize_artifact_component(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out.trim_matches('_').to_string()
}

fn catch_unwind_silencing_hook<T>(f: impl FnOnce() -> T) -> Result<T, CapturedPanic> {
    let _guard = panic_hook_guard()
        .lock()
        .expect("panic hook mutex poisoned");
    let captured = Arc::new(Mutex::new(None));
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new({
        let captured = Arc::clone(&captured);
        move |info| {
            let mut slot = captured.lock().expect("panic capture mutex poisoned");
            *slot = Some(CapturedPanic::from_hook(info));
        }
    }));

    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(previous_hook);

    match outcome {
        Ok(value) => Ok(value),
        Err(payload) => {
            let captured_panic = captured
                .lock()
                .expect("panic capture mutex poisoned")
                .clone();
            Err(captured_panic.unwrap_or_else(|| CapturedPanic {
                message: panic_payload_string(&payload),
            }))
        }
    }
}

fn panic_hook_guard() -> &'static Mutex<()> {
    static PANIC_HOOK_GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    PANIC_HOOK_GUARD.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Clone)]
struct CapturedPanic {
    message: String,
}

impl CapturedPanic {
    fn from_hook(info: &PanicHookInfo<'_>) -> Self {
        let mut message = panic_payload_string(info.payload());
        if let Some(location) = info.location() {
            message = format!(
                "{message} (at {}:{}:{})",
                location.file(),
                location.line(),
                location.column()
            );
        }
        Self { message }
    }
}

fn panic_payload_string(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(msg) = payload.downcast_ref::<&str>() {
        (*msg).to_string()
    } else if let Some(msg) = payload.downcast_ref::<String>() {
        msg.clone()
    } else {
        "panic payload was not a string".into()
    }
}

fn resolve_debug_target_path(ctx: &CommandContext, path: &Path) -> Result<PathBuf, XtaskError> {
    let p = if path.is_absolute() {
        path.to_path_buf()
    } else {
        ctx.workspace_root()?.join(path)
    };
    if !p.exists() {
        return Err(XtaskError::validation(format!(
            "Path `{}` does not exist (resolved to `{}`)",
            path.display(),
            p.display()
        ))
        .with_recovery("Provide a valid crate/workspace directory path."));
    }
    if !p.is_dir() {
        return Err(
            XtaskError::validation(format!("Path `{}` must be a directory", p.display()))
                .with_recovery("Pass the crate or workspace root directory."),
        );
    }
    let canon = p.canonicalize().map_err(|e| {
        XtaskError::Resource(format!("Could not canonicalize {}: {e}", p.display()))
    })?;
    let manifest = canon.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(XtaskError::validation(format!(
            "No Cargo.toml found at `{}` for this debug command",
            canon.display()
        ))
        .with_recovery("Pass the crate/workspace root that contains Cargo.toml."));
    }
    Ok(canon)
}

fn load_cargo_metadata(target_dir: &Path) -> Result<Metadata, XtaskError> {
    let manifest_path = target_dir.join("Cargo.toml");
    cargo_metadata::MetadataCommand::new()
        .manifest_path(&manifest_path)
        .current_dir(target_dir)
        .no_deps()
        .exec()
        .map_err(|e| {
            XtaskError::Parse(format!(
                "cargo metadata failed for {}: {e}",
                manifest_path.display()
            ))
        })
}

fn classify_member(
    has_lib_target: bool,
    has_bin_targets: bool,
    has_test_targets: bool,
    has_example_targets: bool,
    has_bench_targets: bool,
    has_tests_dir: bool,
) -> String {
    if has_lib_target || has_bin_targets {
        return "normal".into();
    }
    if has_test_targets || has_tests_dir {
        return "tests_only".into();
    }
    if has_example_targets {
        return "examples_only".into();
    }
    if has_bench_targets {
        return "benches_only".into();
    }
    "missing_sources".into()
}

fn candidate_for(source: &str, path: &Path) -> DiscoveryRuleCandidate {
    let md = std::fs::metadata(path).ok();
    DiscoveryRuleCandidate {
        source: source.to_string(),
        path: path.display().to_string(),
        exists: md.is_some(),
        is_file: md.as_ref().is_some_and(std::fs::Metadata::is_file),
        is_dir: md.as_ref().is_some_and(std::fs::Metadata::is_dir),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn catch_unwind_silencing_hook_returns_panic_message_without_invoking_outer_hook() {
        let panic_count = Arc::new(AtomicUsize::new(0));
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new({
            let panic_count = Arc::clone(&panic_count);
            move |_| {
                panic_count.fetch_add(1, Ordering::SeqCst);
            }
        }));

        let result = catch_unwind_silencing_hook(|| panic!("hook should be silenced"));
        std::panic::set_hook(previous_hook);

        let panic = result.expect_err("expected panic capture");
        assert!(
            panic.message.contains("hook should be silenced"),
            "unexpected panic message: {}",
            panic.message
        );
        assert_eq!(
            panic_count.load(Ordering::SeqCst),
            0,
            "outer panic hook should not run while panic is being captured"
        );
    }

    #[test]
    fn catch_unwind_silencing_hook_restores_previous_hook_after_panic() {
        let panic_count = Arc::new(AtomicUsize::new(0));
        let previous_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new({
            let panic_count = Arc::clone(&panic_count);
            move |_| {
                panic_count.fetch_add(1, Ordering::SeqCst);
            }
        }));

        let _ = catch_unwind_silencing_hook(|| panic!("captured panic"));

        let uncaught = std::panic::catch_unwind(|| panic!("outer hook should run"));
        assert!(uncaught.is_err(), "expected outer panic");
        std::panic::set_hook(previous_hook);

        assert_eq!(
            panic_count.load(Ordering::SeqCst),
            1,
            "previous panic hook should be restored after captured panic"
        );
    }
}
