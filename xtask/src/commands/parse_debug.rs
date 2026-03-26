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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Args, Subcommand};
use serde::Serialize;

use syn_parser::discovery::{run_discovery_phase, try_parse_manifest};
use syn_parser::parser::nodes::ModuleNode;
use syn_parser::{
    logical_module_path_for_file, try_run_phases_and_merge, try_run_phases_and_resolve,
    ParsedCodeGraph,
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

    fn name(&self) -> &'static str {
        self.cmd.name()
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

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
}

impl ParseDebugCmd {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ParseDebugCmd::Manifest(_) => "parse debug manifest",
            ParseDebugCmd::Discovery(_) => "parse debug discovery",
            ParseDebugCmd::Workspace(_) => "parse debug workspace",
            ParseDebugCmd::Pipeline(_) => "parse debug pipeline",
            ParseDebugCmd::LogicalPaths(_) => "parse debug logical-paths",
            ParseDebugCmd::ModulesPremerge(_) => "parse debug modules-premerge",
            ParseDebugCmd::PathCollisions(_) => "parse debug path-collisions",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<super::parse::ParseOutput, XtaskError> {
        let out = match self {
            ParseDebugCmd::Manifest(c) => c.execute(ctx)?,
            ParseDebugCmd::Discovery(c) => c.execute(ctx)?,
            ParseDebugCmd::Workspace(c) => c.execute(ctx)?,
            ParseDebugCmd::Pipeline(c) => c.execute(ctx)?,
            ParseDebugCmd::LogicalPaths(c) => c.execute(ctx)?,
            ParseDebugCmd::ModulesPremerge(c) => c.execute(ctx)?,
            ParseDebugCmd::PathCollisions(c) => c.execute(ctx)?,
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

    fn name(&self) -> &'static str {
        "parse debug manifest"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let meta = try_parse_manifest(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let manifest_path = canon.join("Cargo.toml");
        let (has_workspace_section, members, exclude, resolver, workspace_root, workspace_package_version) =
            match &meta.workspace {
                Some(ws) => (
                    true,
                    ws.members
                        .iter()
                        .map(|p| p.display().to_string())
                        .collect(),
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

    fn name(&self) -> &'static str {
        "parse debug discovery"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

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
                    (None, Vec::new(), out.warnings.iter().map(|w| w.to_string()).collect())
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

    fn name(&self) -> &'static str {
        "parse debug workspace"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

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
                        let nodes: usize = graphs.iter().map(|pg| count_code_graph_nodes(&pg.graph)).sum();
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

    fn name(&self) -> &'static str {
        "parse debug pipeline"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;

        let r_start = Instant::now();
        let resolve_out = try_run_phases_and_resolve(&canon);
        let r_ms = r_start.elapsed().as_millis() as u64;
        let resolve = match resolve_out {
            Ok(graphs) => {
                let nodes: usize = graphs.iter().map(|pg| count_code_graph_nodes(&pg.graph)).sum();
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

    fn name(&self) -> &'static str {
        "parse debug logical-paths"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let discovery = run_discovery_phase(None, &[canon.clone()]).map_err(|e| {
            XtaskError::Parse(format!("Discovery failed (needed for file list): {e}"))
        })?;
        let ctx_c = discovery
            .get_crate_context(&canon)
            .ok_or_else(|| XtaskError::Parse("Discovery returned no context for crate root".into()))?;
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

    fn name(&self) -> &'static str {
        "parse debug modules-premerge"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

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

    fn name(&self) -> &'static str {
        "parse debug path-collisions"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

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
            let target = std::fs::read_link(path).ok().map(|p| p.display().to_string());
            (true, target)
        }
        _ => (false, None),
    }
}
