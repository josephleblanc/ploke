//! `parse debug` — structured diagnostics for discovery and pipeline failures.
//!
//! Uses [`syn_parser::discovery::try_parse_manifest`], [`syn_parser::discovery::run_discovery_phase`],
//! [`syn_parser::logical_module_path_for_file`], [`syn_parser::try_run_phases_and_resolve`],
//! [`syn_parser::try_run_phases_and_merge`], and [`syn_parser::ParsedCodeGraph::merge_new`].
//!
//! **Agent-oriented workflow:** When `parse workspace` / `parse phases-*` fails, run
//! `parse debug workspace` on the same path, then `parse debug logical-paths`,
//! `parse debug modules-premerge`, and `parse debug path-collisions` on the failing crate root
//! to see derived paths, per-file module nodes, and post-merge path duplicates.
#![allow(missing_docs)]

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::time::Instant;

use cargo_metadata::Metadata;
use clap::{Args, Subcommand};
use serde::Serialize;

use syn_parser::discovery::CargoManifest;
use syn_parser::discovery::{run_discovery_phase, try_parse_manifest};
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
        let meta = try_parse_manifest(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
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
        let meta = try_parse_manifest(&ws).map_err(|e| XtaskError::Parse(e.to_string()))?;
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
    /// Additional list file(s) of targets (`owner/repo(.git)`, URL, or local git path).
    ///
    /// When omitted, defaults to `top_100_stars.txt` and `top_100_downloads.txt`
    /// from the workspace root.
    #[arg(long = "list-file", value_name = "PATH")]
    pub list_files: Vec<PathBuf>,

    /// Directory used to store cloned repositories.
    #[arg(
        long,
        value_name = "DIR",
        default_value = "tests/fixture_github_clones/corpus"
    )]
    pub checkout_dir: PathBuf,

    /// Max number of unique targets to process after deduplication (0 = unlimited).
    #[arg(long, default_value_t = 0)]
    pub limit: usize,

    /// Reuse only already-cloned repos; do not run `git clone`.
    #[arg(long)]
    pub skip_clone: bool,

    /// Stop after discovery + resolve (skip merge / module-tree build).
    #[arg(long)]
    pub skip_merge: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DebugCorpusOut {
    pub checkout_root: String,
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
    pub targets: Vec<CorpusTargetResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusTargetResult {
    pub target: String,
    pub normalized_repo: String,
    pub clone_url: String,
    pub datasets: Vec<String>,
    pub checkout_path: String,
    pub repository_kind: String,
    pub recommended_parser: String,
    pub workspace_member_count: Option<usize>,
    pub classification_error: Option<String>,
    pub clone: CorpusCloneStatus,
    pub commit_sha: Option<String>,
    pub discovery: Option<CorpusStageResult>,
    pub resolve: Option<CorpusStageResult>,
    pub merge: Option<CorpusStageResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusCloneStatus {
    pub ok: bool,
    pub action: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CorpusStageResult {
    pub ok: bool,
    pub panic: bool,
    pub failure_kind: Option<String>,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub nodes_parsed: Option<usize>,
    pub relations_found: Option<usize>,
    pub file_count: Option<usize>,
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
        std::fs::create_dir_all(&checkout_root).map_err(|e| {
            XtaskError::Resource(format!(
                "Failed to create corpus checkout dir {}: {e}",
                checkout_root.display()
            ))
        })?;

        let list_files = if self.list_files.is_empty() {
            vec![
                workspace_root.join("top_100_stars.txt"),
                workspace_root.join("top_100_downloads.txt"),
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

        for spec in specs {
            let checkout_path = checkout_root.join(&spec.checkout_slug);
            let clone = ensure_corpus_checkout(&spec, &checkout_path, self.skip_clone)?;
            match clone.action.as_str() {
                "cloned" => cloned_targets += 1,
                "reused" => reused_targets += 1,
                "skipped_missing" => skipped_targets += 1,
                _ => {}
            }
            if !clone.ok {
                clone_failures += 1;
                targets.push(CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    repository_kind: "unknown".into(),
                    recommended_parser: "unknown".into(),
                    workspace_member_count: None,
                    classification_error: Some(
                        "clone failed before manifest classification".into(),
                    ),
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
                    }),
                    resolve: None,
                    merge: None,
                });
                continue;
            }

            let commit_sha = git_stdout(&checkout_path, &["rev-parse", "HEAD"]).ok();
            let classification = classify_corpus_checkout(&checkout_path);
            match classification.repository_kind.as_str() {
                "workspace" => workspace_targets += 1,
                "single_crate" => single_crate_targets += 1,
                _ => {}
            }
            if !classification.should_run_single_crate_pipeline {
                targets.push(CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    repository_kind: classification.repository_kind,
                    recommended_parser: classification.recommended_parser,
                    workspace_member_count: classification.workspace_member_count,
                    classification_error: classification.classification_error,
                    clone,
                    commit_sha,
                    discovery: None,
                    resolve: None,
                    merge: None,
                });
                continue;
            }

            let discovery = run_corpus_discovery_stage(&checkout_path);
            if !discovery.ok {
                discovery_failures += 1;
                if discovery.panic {
                    panic_failures += 1;
                }
                targets.push(CorpusTargetResult {
                    target: spec.original,
                    normalized_repo: spec.normalized_repo,
                    clone_url: spec.clone_url,
                    datasets: spec.datasets.into_iter().collect(),
                    checkout_path: checkout_path.display().to_string(),
                    repository_kind: classification.repository_kind.clone(),
                    recommended_parser: classification.recommended_parser.clone(),
                    workspace_member_count: classification.workspace_member_count,
                    classification_error: classification.classification_error.clone(),
                    clone,
                    commit_sha,
                    discovery: Some(discovery),
                    resolve: None,
                    merge: None,
                });
                continue;
            }

            let resolve = run_corpus_stage(|| {
                let graphs =
                    try_run_phases_and_resolve(&checkout_path).map_err(|e| e.to_string())?;
                let nodes: usize = graphs
                    .iter()
                    .map(|pg| count_code_graph_nodes(&pg.graph))
                    .sum();
                let rels: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
                Ok(CorpusStageMetrics {
                    nodes_parsed: Some(nodes),
                    relations_found: Some(rels),
                    file_count: None,
                })
            });
            if !resolve.ok {
                resolve_failures += 1;
                if resolve.panic {
                    panic_failures += 1;
                }
            }

            let merge = if !self.skip_merge && resolve.ok {
                let stage = run_corpus_stage(|| {
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
                    Ok(CorpusStageMetrics {
                        nodes_parsed: nodes,
                        relations_found: rels,
                        file_count: None,
                    })
                });
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

            targets.push(CorpusTargetResult {
                target: spec.original,
                normalized_repo: spec.normalized_repo,
                clone_url: spec.clone_url,
                datasets: spec.datasets.into_iter().collect(),
                checkout_path: checkout_path.display().to_string(),
                repository_kind: classification.repository_kind,
                recommended_parser: classification.recommended_parser,
                workspace_member_count: classification.workspace_member_count,
                classification_error: classification.classification_error,
                clone,
                commit_sha,
                discovery: Some(discovery),
                resolve: Some(resolve),
                merge,
            });
        }

        Ok(DebugOutput::Corpus(DebugCorpusOut {
            checkout_root: checkout_root.display().to_string(),
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
            targets,
        }))
    }
}

#[derive(Debug, Clone)]
struct ParsedCorpusTarget {
    original: String,
    normalized_repo: String,
    clone_url: String,
    checkout_slug: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct CorpusStageMetrics {
    nodes_parsed: Option<usize>,
    relations_found: Option<usize>,
    file_count: Option<usize>,
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
    match try_parse_manifest(checkout_path) {
        Ok(metadata) => {
            if let Some(workspace) = metadata.workspace {
                CorpusCheckoutClassification {
                    repository_kind: "workspace".into(),
                    recommended_parser: "parse_workspace_with_config".into(),
                    workspace_member_count: Some(workspace.members.len()),
                    classification_error: None,
                    should_run_single_crate_pipeline: false,
                }
            } else {
                CorpusCheckoutClassification {
                    repository_kind: "single_crate".into(),
                    recommended_parser: "try_run_phases_and_merge".into(),
                    workspace_member_count: None,
                    classification_error: None,
                    should_run_single_crate_pipeline: true,
                }
            }
        }
        Err(err) => CorpusCheckoutClassification {
            repository_kind: "unknown".into(),
            recommended_parser: "try_run_phases_and_merge".into(),
            workspace_member_count: None,
            classification_error: Some(err.to_string()),
            should_run_single_crate_pipeline: true,
        },
    }
}

fn run_corpus_discovery_stage(crate_root: &Path) -> CorpusStageResult {
    run_corpus_stage(|| {
        let out =
            run_discovery_phase(None, &[crate_root.to_path_buf()]).map_err(|e| e.to_string())?;
        let file_count = out
            .crate_contexts
            .get(crate_root)
            .map(|c| c.files.len())
            .or_else(|| out.crate_contexts.values().next().map(|c| c.files.len()));
        Ok(CorpusStageMetrics {
            file_count,
            ..CorpusStageMetrics::default()
        })
    })
}

fn run_corpus_stage<F>(op: F) -> CorpusStageResult
where
    F: FnOnce() -> Result<CorpusStageMetrics, String>,
{
    let start = Instant::now();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(op));
    let duration_ms = start.elapsed().as_millis() as u64;
    match outcome {
        Ok(Ok(metrics)) => CorpusStageResult {
            ok: true,
            panic: false,
            failure_kind: None,
            error: None,
            duration_ms,
            nodes_parsed: metrics.nodes_parsed,
            relations_found: metrics.relations_found,
            file_count: metrics.file_count,
        },
        Ok(Err(err)) => CorpusStageResult {
            ok: false,
            panic: false,
            failure_kind: Some("error".into()),
            error: Some(err),
            duration_ms,
            nodes_parsed: None,
            relations_found: None,
            file_count: None,
        },
        Err(panic) => CorpusStageResult {
            ok: false,
            panic: true,
            failure_kind: Some("panic".into()),
            error: Some(panic_payload_string(panic)),
            duration_ms,
            nodes_parsed: None,
            relations_found: None,
            file_count: None,
        },
    }
}

fn panic_payload_string(payload: Box<dyn std::any::Any + Send>) -> String {
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
