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

use super::{CommandContext, XtaskError};
use crate::executor::Command;
use std::path::PathBuf;

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
        }
    }
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Discovery command implementation")
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("PhasesResolve command implementation")
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("PhasesMerge command implementation")
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Workspace command implementation")
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Stats command implementation")
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

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("ListModules command implementation")
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
