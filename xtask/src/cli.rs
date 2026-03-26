//! CLI definition for xtask commands
//!
//! This module defines the command-line interface using clap derive macros.
//! It provides the root `Cli` struct with global flags and subcommand dispatch.
//!
//! ## Usage
//!
//! ```bash
//! # Show help
//! cargo xtask --help
//!
//! # Run a command
//! cargo xtask parse discovery ./my-crate
//! cargo xtask db count-nodes
//!
//! # With output format
//! cargo xtask --format json parse stats ./my-crate
//! ```

use crate::commands::{
    db::Db,
    parse::Parse,
    CommandContext, OutputFormat, XtaskError,
};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// ploke xtask - Agent-focused command-line utilities
///
/// This tool provides commands for parsing, transforming, and analyzing
/// Rust code in the ploke workspace. It's designed for agent automation
/// with machine-readable output formats.
#[derive(Parser, Debug)]
#[command(
    name = "xtask",
    about = "Agent-focused utilities for ploke workspace",
    version = env!("CARGO_PKG_VERSION"),
    propagate_version = true,
)]
pub struct Cli {
    /// Output format for command results
    #[arg(global = true, short, long, value_enum, default_value = "human")]
    pub format: OutputFormat,

    /// Enable verbose output (use multiple times for more detail)
    #[arg(global = true, short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Suppress non-essential output
    #[arg(global = true, short, long)]
    pub quiet: bool,

    /// Workspace root path (default: auto-detect)
    #[arg(global = true, short, long, value_name = "PATH")]
    pub workspace: Option<PathBuf>,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    /// Execute the CLI command
    ///
    /// This is the main entry point for running commands. It:
    /// 1. Creates the command context with settings from CLI flags
    /// 2. Dispatches to the appropriate subcommand
    /// 3. Formats and prints the output
    ///
    /// # Errors
    ///
    /// Returns an error if command execution fails
    pub fn execute(self) -> Result<(), XtaskError> {
        // Build command context
        let ctx = CommandContext::new()?;

        // Execute the subcommand
        let output = match self.command {
            Commands::Parse(cmd) => {
                let result = cmd.execute(&ctx)?;
                serde_json::to_value(result)?
            }
            Commands::Db(cmd) => {
                let result = cmd.execute(&ctx)?;
                serde_json::to_value(result)?
            }
            Commands::HelpTopic(cmd) => {
                cmd.print_help();
                return Ok(());
            }
        };

        // Format and print output
        let formatted = self.format.format(&output)?;
        println!("{}", formatted);

        Ok(())
    }

    /// Run the CLI from environment arguments
    pub fn run() -> Result<(), XtaskError> {
        let cli = Self::parse();
        cli.execute()
    }
}

/// Available commands
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Parse source code and analyze structure
    ///
    /// Commands for running the syn_parser pipeline:
    /// - discovery: Find crates in a workspace
    /// - phases-resolve: Parse and resolve without merging
    /// - phases-merge: Full parse with graph merging
    /// - workspace: Parse entire workspace
    #[command(subcommand)]
    Parse(Parse),

    /// Database operations and queries
    ///
    /// Commands for ploke_db operations:
    /// - save/load: Backup and restore
    /// - count-nodes: Node statistics
    /// - query: Execute CozoDB queries
    /// - hnsw-build: Build vector index
    /// - bm25-rebuild: Build text index
    #[command(subcommand)]
    Db(Db),

    /// Display detailed help for topics
    ///
    /// Shows detailed help for commands and topics beyond standard --help.
    #[command(name = "help-topic")]
    HelpTopic(HelpTopicCommand),
}

/// Help topic command for detailed documentation
#[derive(Parser, Debug)]
pub struct HelpTopicCommand {
    /// Topic to show help for
    #[arg(value_name = "TOPIC")]
    pub topic: Option<String>,
}

impl HelpTopicCommand {
    /// Print help information
    pub fn print_help(&self) {
        match self.topic.as_deref() {
            Some("parse") => print_parse_help(),
            Some("db") => print_db_help(),
            Some("examples") => print_examples(),
            _ => print_general_help(),
        }
    }
}

/// Find the workspace root directory
fn find_workspace_root() -> Result<PathBuf, crate::error::XtaskError> {
    let mut current = std::env::current_dir()?;

    loop {
        let cargo_toml = current.join("Cargo.toml");
        if cargo_toml.exists() {
            let contents = std::fs::read_to_string(&cargo_toml).ok();
            if contents
                .as_ref()
                .map(|c| c.contains("[workspace]"))
                .unwrap_or(false)
            {
                return Ok(current);
            }
        }

        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }

    Err(XtaskError::new("Could not find workspace root"))
}

fn print_general_help() {
    println!(
        r#"ploke xtask - Agent-focused utilities for ploke workspace

USAGE:
    cargo xtask [OPTIONS] <COMMAND>

OPTIONS:
    -f, --format <FORMAT>    Output format: human, json, table, compact [default: human]
    -v, --verbose            Enable verbose output (use multiple times)
    -q, --quiet              Suppress non-essential output
    -w, --workspace <PATH>   Workspace root path
    -h, --help               Print help
    -V, --version            Print version

COMMANDS:
    parse    Parse source code and analyze structure
    db       Database operations and queries
    help     Display help information

EXAMPLES:
    cargo xtask parse discovery ./my-crate
    cargo xtask --format json db count-nodes
    cargo xtask help examples

For more help on a specific command:
    cargo xtask help parse
    cargo xtask help db
"#
    );
}

fn print_parse_help() {
    println!(
        r#"parse - Parse source code and analyze structure

SUBCOMMANDS:
    discovery        Run discovery phase on target crate(s)
    phases-resolve   Parse and resolve without merging
    phases-merge     Parse, resolve, and merge graphs
    workspace        Parse entire workspace
    stats            Show parsing statistics
    list-modules     List all modules in parsed code
    debug            Debug manifest, discovery file lists, workspace members, pipeline stages

EXAMPLES:
    # Discover crates in a workspace
    cargo xtask parse discovery ./workspace

    # Parse a single crate with full merging
    cargo xtask parse phases-merge ./my-crate --tree

    # Get statistics in JSON format
    cargo xtask --format json parse stats ./my-crate

    # Parse entire workspace
    cargo xtask parse workspace ./workspace --crate-name my-crate

    # Debug workspace layout and per-member pipeline (JSON)
    cargo xtask --format json parse debug manifest ./workspace
    cargo xtask parse debug workspace ./workspace --skip-merge
"#
    );
}

fn print_db_help() {
    println!(
        r#"db - Database operations and queries

SUBCOMMANDS:
    save              Save database to backup file
    load              Load database from backup file
    load-fixture      Load a fixture database
    count-nodes       Count nodes in database
    hnsw-build        Build HNSW index
    hnsw-rebuild      Rebuild HNSW index
    bm25-rebuild      Rebuild BM25 index
    query             Execute CozoDB query
    stats             Show database statistics
    list-relations    List relations in database
    embedding-status  Show embedding status

EXAMPLES:
    # Count all nodes
    cargo xtask db count-nodes

    # Execute a query
    cargo xtask db query '*relation[node]'

    # Load a fixture
    cargo xtask db load-fixture fixture_nodes_canonical

    # Show embedding statistics
    cargo xtask db embedding-status --detailed
"#
    );
}

fn print_examples() {
    println!(
        r#"ploke xtask - Common usage examples

WORKFLOW EXAMPLES:

1. Parse and analyze a crate:
    cargo xtask parse discovery ./my-crate
    cargo xtask parse phases-merge ./my-crate --tree
    cargo xtask parse stats ./my-crate

2. Work with the database:
    cargo xtask db count-nodes
    cargo xtask db list-relations --counts
    cargo xtask db query '?[count(x)] := *node[x]'

3. Load a test fixture:
    cargo xtask db load-fixture fixture_nodes_canonical
    cargo xtask db count-nodes --kind function

4. Build indexes:
    cargo xtask db hnsw-build
    cargo xtask db bm25-rebuild

OUTPUT FORMATS:

    # Human-readable (default)
    cargo xtask parse stats ./my-crate

    # JSON for scripting
    cargo xtask --format json parse stats ./my-crate

    # Table for quick viewing
    cargo xtask --format table db count-nodes

    # Compact for piping
    cargo xtask --format compact db list-relations
"#
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_format() {
        // Parse with no format specified should default to Human
        let cli = Cli::parse_from(["xtask", "help"]);
        assert!(matches!(cli.format, OutputFormat::Human));
    }

    #[test]
    fn test_cli_json_format() {
        let cli = Cli::parse_from(["xtask", "--format", "json", "help"]);
        assert!(matches!(cli.format, OutputFormat::Json));
    }

    #[test]
    fn test_cli_verbose_count() {
        let cli = Cli::parse_from(["xtask", "-vvv", "help-topic"]);
        assert_eq!(cli.verbose, 3);
    }

    #[test]
    fn test_cli_quiet() {
        let cli = Cli::parse_from(["xtask", "--quiet", "help-topic"]);
        assert!(cli.quiet);
    }
}
