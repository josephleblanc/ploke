//! `parse debug` — structured diagnostics for discovery and pipeline failures.
//!
//! Uses [`syn_parser::discovery::try_parse_manifest`], [`syn_parser::discovery::run_discovery_phase`],
//! [`syn_parser::try_run_phases_and_resolve`], and [`syn_parser::try_run_phases_and_merge`].
#![allow(missing_docs)]

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

use clap::{Args, Subcommand};
use serde::Serialize;

use syn_parser::discovery::{run_discovery_phase, try_parse_manifest};
use syn_parser::{try_run_phases_and_merge, try_run_phases_and_resolve};

use super::parse::{count_code_graph_nodes, resolve_parse_path};
use super::{CommandContext, XtaskError};
use crate::executor::Command;

/// `parse debug …` — nested subcommands (manifest, discovery, workspace, pipeline).
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
}

impl ParseDebugCmd {
    pub(crate) fn name(&self) -> &'static str {
        match self {
            ParseDebugCmd::Manifest(_) => "parse debug manifest",
            ParseDebugCmd::Discovery(_) => "parse debug discovery",
            ParseDebugCmd::Workspace(_) => "parse debug workspace",
            ParseDebugCmd::Pipeline(_) => "parse debug pipeline",
        }
    }

    fn run(&self, ctx: &CommandContext) -> Result<super::parse::ParseOutput, XtaskError> {
        let out = match self {
            ParseDebugCmd::Manifest(c) => c.execute(ctx)?,
            ParseDebugCmd::Discovery(c) => c.execute(ctx)?,
            ParseDebugCmd::Workspace(c) => c.execute(ctx)?,
            ParseDebugCmd::Pipeline(c) => c.execute(ctx)?,
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

/// Unified JSON-friendly payload for `parse debug` (see [`ParseOutput::Debug`](super::parse::ParseOutput::Debug)).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DebugOutput {
    Manifest(DebugManifestOut),
    Discovery(DebugDiscoveryOut),
    WorkspaceProbe(DebugWorkspaceProbeOut),
    Pipeline(DebugPipelineOut),
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
