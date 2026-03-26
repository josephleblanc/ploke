//! Parse commands for syn_parser integration (A.1)
//!
//! This module provides commands for the parsing pipeline:
//! - Discovery phase
//! - Resolution phase
//! - Merging phase
//! - Full workspace parsing
//!
//! ## Commands
//!
//! - `parse discovery` - Run discovery phase on target crate(s)
//! - `parse phases-resolve` - Parse and resolve without merging
//! - `parse phases-merge` - Parse, resolve, and merge graphs
//! - `parse workspace` - Parse entire workspace

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use syn_parser::{
    discovery::run_discovery_phase, parse_workspace, try_run_phases_and_merge,
    try_run_phases_and_resolve, CodeGraph,
};

use super::{CommandContext, XtaskError};
use crate::executor::Command;

/// Parse command enum with all subcommands
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Parse {
    /// Run discovery phase on target crate(s)
    Discovery(Discovery),

    /// Parse and resolve without merging
    PhasesResolve(PhasesResolve),

    /// Parse, resolve, and merge graphs
    PhasesMerge(PhasesMerge),

    /// Parse entire workspace
    Workspace(Workspace),

    /// Show parsing statistics
    Stats(Stats),

    /// List all modules in parsed code
    ListModules(ListModules),

    /// Debug discovery and pipeline stages (manifest, file lists, per-member status)
    Debug(crate::commands::parse_debug::ParseDebugCli),
}

impl Parse {
    /// Execute the parse command
    pub fn execute(&self, ctx: &CommandContext) -> Result<ParseOutput, XtaskError> {
        match self {
            Parse::Discovery(cmd) => cmd.execute(ctx),
            Parse::PhasesResolve(cmd) => cmd.execute(ctx),
            Parse::PhasesMerge(cmd) => cmd.execute(ctx),
            Parse::Workspace(cmd) => cmd.execute(ctx),
            Parse::Stats(cmd) => cmd.execute(ctx),
            Parse::ListModules(cmd) => cmd.execute(ctx),
            Parse::Debug(cmd) => cmd.execute(ctx),
        }
    }
}

/// Resolve a user-supplied path relative to the ploke workspace root and validate `Cargo.toml`.
///
/// Exposed for [`parse_debug`](crate::commands::parse_debug) helpers and other crate-local tooling.
pub fn resolve_parse_path(ctx: &CommandContext, path: &Path) -> Result<PathBuf, XtaskError> {
    let p = if path.is_absolute() {
        path.to_path_buf()
    } else {
        ctx.workspace_root()?.join(path)
    };
    if !p.exists() {
        return Err(
            XtaskError::validation(format!(
                "Path `{}` does not exist (resolved to `{}`)",
                path.display(),
                p.display()
            ))
            .with_recovery(
                "Use a path relative to the ploke workspace root (see `cargo xtask help-topic parse`).",
            ),
        );
    }
    let canon = p.canonicalize().map_err(|e| {
        XtaskError::Resource(format!("Could not canonicalize {}: {e}", p.display()))
    })?;
    let manifest = canon.join("Cargo.toml");
    if !manifest.is_file() {
        return Err(
            XtaskError::validation(format!(
                "No Cargo.toml found at `{}` for this parse command",
                canon.display()
            ))
            .with_recovery(
                "Pass the crate root (the directory that contains Cargo.toml).",
            ),
        );
    }
    Ok(canon)
}

/// Count nodes in a resolved [`CodeGraph`] (used by phase summaries and debug helpers).
pub fn count_code_graph_nodes(g: &CodeGraph) -> usize {
    g.functions.len()
        + g.defined_types.len()
        + g.type_graph.len()
        + g.impls.len()
        + g.traits.len()
        + g.modules.len()
        + g.consts.len()
        + g.statics.len()
        + g.macros.len()
        + g.use_statements.len()
        + g.unresolved_nodes.len()
}

/// Discovery phase command
///
/// Discovers crates in a workspace or single crate.
#[derive(Debug, Clone, clap::Args)]
pub struct Discovery {
    /// Path to crate or workspace
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,

    /// Show warnings from discovery
    #[arg(long)]
    pub warnings: bool,

    /// Include test files in discovery
    #[arg(long)]
    pub include_tests: bool,
}

impl Command for Discovery {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse discovery"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let _ = (self.warnings, self.include_tests);
        let canon = resolve_parse_path(ctx, &self.path)?;
        let target = vec![canon.clone()];
        let out = run_discovery_phase(None, &target).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let warnings: Vec<String> = out.warnings.iter().map(|w| w.to_string()).collect();
        Ok(ParseOutput::Discovery {
            crates_found: out.crate_contexts.len(),
            workspace_root: canon,
            warnings,
        })
    }
}

/// Phases resolve command
///
/// Runs the parsing pipeline through resolution without merging.
#[derive(Debug, Clone, clap::Args)]
pub struct PhasesResolve {
    /// Path to crate directory
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,

    /// Output detailed node information
    #[arg(long)]
    pub detailed: bool,

    /// Save intermediate output to file
    #[arg(short, long, value_name = "OUTPUT")]
    pub output: Option<PathBuf>,
}

impl Command for PhasesResolve {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse phases-resolve"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let _ = (&self.detailed, &self.output);
        let canon = resolve_parse_path(ctx, &self.path)?;
        let start = Instant::now();
        let graphs = try_run_phases_and_resolve(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let nodes_parsed: usize = graphs.iter().map(|pg| count_code_graph_nodes(&pg.graph)).sum();
        let relations_found: usize = graphs.iter().map(|pg| pg.graph.relations.len()).sum();
        Ok(ParseOutput::PhaseResult {
            success: true,
            nodes_parsed,
            relations_found,
            duration_ms,
        })
    }
}

/// Phases merge command
///
/// Runs the full parsing pipeline including graph merging.
#[derive(Debug, Clone, clap::Args)]
pub struct PhasesMerge {
    /// Path to crate directory
    #[arg(value_name = "CRATE_PATH")]
    pub path: PathBuf,

    /// Show module tree structure
    #[arg(long)]
    pub tree: bool,

    /// Validate relations after merge
    #[arg(long)]
    pub validate: bool,
}

impl Command for PhasesMerge {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse phases-merge"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let _ = (self.tree, self.validate);
        let canon = resolve_parse_path(ctx, &self.path)?;
        let start = Instant::now();
        let parsed = try_run_phases_and_merge(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let (nodes_parsed, relations_found) =
            if let Some(ref mg) = parsed.merged_graph {
                (
                    count_code_graph_nodes(&mg.graph),
                    mg.graph.relations.len(),
                )
            } else {
                (0, 0)
            };
        Ok(ParseOutput::PhaseResult {
            success: true,
            nodes_parsed,
            relations_found,
            duration_ms,
        })
    }
}

/// Workspace parse command
///
/// Parses an entire workspace with multiple crates.
#[derive(Debug, Clone, clap::Args)]
pub struct Workspace {
    /// Path to workspace root
    #[arg(value_name = "WORKSPACE_PATH", default_value = ".")]
    pub path: PathBuf,

    /// Specific crate(s) to parse (default: all)
    #[arg(short, long, value_name = "CRATE")]
    pub crate_name: Vec<String>,

    /// Skip crates that fail to parse
    #[arg(long)]
    pub continue_on_error: bool,
}

impl Command for Workspace {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse workspace"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let _ = self.continue_on_error;
        let ws_root = resolve_parse_path(ctx, &self.path)?;
        let selected_paths: Vec<PathBuf> = self.crate_name.iter().map(PathBuf::from).collect();
        let selected_refs: Vec<&Path> = selected_paths.iter().map(|p| p.as_path()).collect();
        let sel = if selected_refs.is_empty() {
            None
        } else {
            Some(selected_refs.as_slice())
        };
        let start = Instant::now();
        let parsed = parse_workspace(&ws_root, sel).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let mut nodes_parsed = 0usize;
        let mut relations_found = 0usize;
        for c in &parsed.crates {
            if let Some(ref mg) = c.parser_output.merged_graph {
                nodes_parsed += count_code_graph_nodes(&mg.graph);
                relations_found += mg.graph.relations.len();
            }
        }
        Ok(ParseOutput::PhaseResult {
            success: true,
            nodes_parsed,
            relations_found,
            duration_ms,
        })
    }
}

/// Stats command
///
/// Shows statistics about parsed code.
#[derive(Debug, Clone, clap::Args)]
pub struct Stats {
    /// Path to parsed crate or workspace
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Filter by node type
    #[arg(short, long, value_enum)]
    pub node_type: Option<NodeTypeFilter>,
}

impl Command for Stats {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse stats"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let out = try_run_phases_and_merge(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let mg = out
            .merged_graph
            .as_ref()
            .ok_or_else(|| XtaskError::Parse("merge produced no graph".into()))?;
        let g = &mg.graph;
        let mut by_type: HashMap<String, usize> = HashMap::new();
        by_type.insert("function".into(), g.functions.len());
        by_type.insert("module".into(), g.modules.len());
        by_type.insert("defined_types".into(), g.defined_types.len());
        by_type.insert("impl".into(), g.impls.len());
        by_type.insert("trait".into(), g.traits.len());
        by_type.insert("const".into(), g.consts.len());
        by_type.insert("static".into(), g.statics.len());
        by_type.insert("macro".into(), g.macros.len());
        by_type.insert("import".into(), g.use_statements.len());
        let total_nodes: usize = by_type.values().sum();
        let by_type = match self.node_type {
            None | Some(NodeTypeFilter::All) => by_type,
            Some(NodeTypeFilter::Function) => {
                let mut m = HashMap::new();
                m.insert("function".into(), g.functions.len());
                m
            }
            Some(NodeTypeFilter::Type) => {
                let mut m = HashMap::new();
                m.insert("defined_types".into(), g.defined_types.len());
                m
            }
            Some(NodeTypeFilter::Module) => {
                let mut m = HashMap::new();
                m.insert("module".into(), g.modules.len());
                m
            }
            Some(NodeTypeFilter::Trait) => {
                let mut m = HashMap::new();
                m.insert("trait".into(), g.traits.len());
                m
            }
            Some(NodeTypeFilter::Impl) => {
                let mut m = HashMap::new();
                m.insert("impl".into(), g.impls.len());
                m
            }
        };
        let total_nodes = match self.node_type {
            None | Some(NodeTypeFilter::All) => total_nodes,
            Some(NodeTypeFilter::Function) => g.functions.len(),
            Some(NodeTypeFilter::Type) => g.defined_types.len(),
            Some(NodeTypeFilter::Module) => g.modules.len(),
            Some(NodeTypeFilter::Trait) => g.traits.len(),
            Some(NodeTypeFilter::Impl) => g.impls.len(),
        };
        Ok(ParseOutput::Stats {
            total_nodes,
            by_type,
        })
    }
}

/// List modules command
///
/// Lists all modules in parsed code.
#[derive(Debug, Clone, clap::Args)]
pub struct ListModules {
    /// Path to parsed crate
    #[arg(value_name = "PATH")]
    pub path: PathBuf,

    /// Show module paths
    #[arg(long)]
    pub full_path: bool,
}

impl Command for ListModules {
    type Output = ParseOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "parse list-modules"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Parse
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let canon = resolve_parse_path(ctx, &self.path)?;
        let out = try_run_phases_and_merge(&canon).map_err(|e| XtaskError::Parse(e.to_string()))?;
        let mg = out
            .merged_graph
            .as_ref()
            .ok_or_else(|| XtaskError::Parse("merge produced no graph".into()))?;
        let modules: Vec<ModuleInfo> = mg
            .graph
            .modules
            .iter()
            .map(|m| {
                let logical = m.path.join("::");
                let path_str = if self.full_path {
                    m.file_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or(logical.clone())
                } else {
                    logical
                };
                let is_root = m.path.len() == 1;
                ModuleInfo {
                    name: m.name.clone(),
                    path: path_str,
                    is_root,
                }
            })
            .collect();
        Ok(ParseOutput::ModuleList { modules })
    }
}

/// Node type filter for stats and queries
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum, serde::Serialize)]
pub enum NodeTypeFilter {
    /// Filter for functions
    Function,
    /// Filter for types (structs, enums, etc.)
    Type,
    /// Filter for modules
    Module,
    /// Filter for traits
    Trait,
    /// Filter for impl blocks
    Impl,
    /// All node types (default)
    #[default]
    All,
}

/// Output type for parse commands
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum ParseOutput {
    /// Discovery phase output
    Discovery {
        crates_found: usize,
        workspace_root: PathBuf,
        warnings: Vec<String>,
    },
    /// Phase execution output
    PhaseResult {
        success: bool,
        nodes_parsed: usize,
        relations_found: usize,
        duration_ms: u64,
    },
    /// Statistics output
    Stats {
        total_nodes: usize,
        by_type: std::collections::HashMap<String, usize>,
    },
    /// Module list output
    ModuleList {
        modules: Vec<ModuleInfo>,
    },
    /// Structured debug output from `parse debug` (manifest, discovery dump, workspace probe, pipeline)
    Debug(crate::commands::parse_debug::DebugOutput),
    /// Error output
    Error {
        message: String,
    },
}

/// Module information for list output
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModuleInfo {
    pub name: String,
    pub path: String,
    pub is_root: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discovery_default_path() {
        let cmd = Discovery {
            path: PathBuf::from("."),
            warnings: false,
            include_tests: false,
        };
        assert_eq!(cmd.name(), "parse discovery");
        assert!(!cmd.requires_async());
    }

    #[test]
    fn test_phases_merge_fields() {
        let cmd = PhasesMerge {
            path: PathBuf::from("/test"),
            tree: true,
            validate: false,
        };
        assert!(cmd.tree);
        assert!(!cmd.validate);
    }

    #[test]
    fn test_parse_output_serialization() {
        let output = ParseOutput::Discovery {
            crates_found: 3,
            workspace_root: PathBuf::from("/workspace"),
            warnings: vec!["warning 1".to_string()],
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("crates_found"));
        assert!(json.contains("3"));
    }

    #[test]
    fn test_module_info() {
        let info = ModuleInfo {
            name: "test".to_string(),
            path: "src/test.rs".to_string(),
            is_root: false,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test"));
        assert!(json.contains("src/test.rs"));
    }
}
