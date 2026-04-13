# Architecture Proposal 2: xtask Commands Feature

**Agent:** Architecture Agent 2  
**Date:** 2026-03-25  
**Task:** Milestone M.2.1 - Design Architecture for xtask Commands  

---

## Executive Summary

This proposal presents a **layered, composable architecture** for the xtask commands feature that emphasizes:

1. **Strong type safety** through clap derive macros
2. **Pluggable output formatting** via a trait-based system
3. **Modular subcommand organization** with shared components
4. **Unified error handling** with recovery paths
5. **Tracing integration** for observability

The design follows Rust best practices and builds upon the existing `profile_ingest.rs` pattern while addressing the broader requirements from the command matrix.

---

## 1. Command Enum Hierarchy (Using clap)

### 1.1 Root Command Structure

```rust
// xtask/src/cli/mod.rs
use clap::{Parser, Subcommand, Args, ValueEnum};
use std::path::PathBuf;

/// ploke xtask - Agent-focused command-line utilities
#[derive(Parser, Debug)]
#[command(
    name = "xtask",
    about = "Agent-focused utilities for ploke workspace",
    version = "0.1.0",
    propagate_version = true,
)]
pub struct Cli {
    /// Output format for command results
    #[arg(global = true, short, long, value_enum, default_value = "human")]
    pub format: OutputFormat,

    /// Suppress non-essential output
    #[arg(global = true, short, long)]
    pub quiet: bool,

    /// Enable verbose tracing output
    #[arg(global = true, short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Subcommand to execute
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Parse source code and analyze structure
    #[command(subcommand)]
    Parse(ParseCommand),

    /// Transform parsed code to database
    #[command(subcommand)]
    Transform(TransformCommand),

    /// Database operations and queries
    #[command(subcommand)]
    Db(DbCommand),

    /// Ingest code and generate embeddings
    #[command(subcommand)]
    Ingest(IngestCommand),

    /// Run TUI in headless mode for testing
    #[command(subcommand)]
    Tui(TuiCommand),

    /// Execute tools directly (bypass LLM loop)
    #[command(subcommand)]
    Tool(ToolCommand),

    /// Pipeline commands that chain operations
    #[command(subcommand)]
    Pipeline(PipelineCommand),

    /// Validation and diagnostic commands
    #[command(subcommand)]
    Validate(ValidateCommand),

    /// Setup and fixture management
    #[command(subcommand)]
    Setup(SetupCommand),

    /// Display help information
    #[command(name = "help")]
    Help(HelpArgs),
}

#[derive(ValueEnum, Clone, Copy, Debug, Default)]
pub enum OutputFormat {
    /// Human-readable formatted output
    #[default]
    Human,
    /// JSON output for programmatic consumption
    Json,
    /// Tab-separated table output
    Table,
    /// Compact single-line output
    Compact,
}
```

### 1.2 Parse Commands (A.1)

```rust
// xtask/src/cli/parse.rs
#[derive(Subcommand, Debug)]
pub enum ParseCommand {
    /// Run discovery phase on target crate(s)
    Discovery {
        /// Path to crate or workspace
        #[arg(value_name = "PATH", default_value = ".")]
        path: PathBuf,
        /// Show warnings from discovery
        #[arg(long)]
        warnings: bool,
    },

    /// Parse and resolve without merging
    PhasesResolve {
        /// Path to crate directory
        #[arg(value_name = "CRATE_PATH")]
        path: PathBuf,
        /// Output detailed node information
        #[arg(long)]
        detailed: bool,
    },

    /// Parse, resolve, and merge graphs
    PhasesMerge {
        /// Path to crate directory
        #[arg(value_name = "CRATE_PATH")]
        path: PathBuf,
        /// Show module tree structure
        #[arg(long)]
        tree: bool,
    },

    /// Parse entire workspace
    Workspace {
        /// Path to workspace root
        #[arg(value_name = "WORKSPACE_PATH", default_value = ".")]
        path: PathBuf,
        /// Specific crate(s) to parse (default: all)
        #[arg(short, long, value_name = "CRATE")]
        crate_name: Vec<String>,
    },

    /// Show parsing statistics
    Stats {
        /// Path to parsed crate or workspace
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Filter by node type
        #[arg(short, long, value_enum)]
        node_type: Option<NodeTypeFilter>,
    },

    /// List all modules in parsed code
    ListModules {
        /// Path to parsed crate
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },

    /// Validate graph relations
    ValidateRelations {
        /// Path to parsed crate
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum NodeTypeFilter {
    Function,
    Type,
    Module,
    Trait,
    Impl,
}
```

### 1.3 Transform Commands (A.2)

```rust
// xtask/src/cli/transform.rs
#[derive(Subcommand, Debug)]
pub enum TransformCommand {
    /// Transform parsed graph to database
    Graph {
        /// Path to parsed crate
        #[arg(value_name = "CRATE_PATH")]
        path: PathBuf,
        /// Database output path (default: in-memory)
        #[arg(short, long, value_name = "DB_PATH")]
        db: Option<PathBuf>,
        /// Skip schema creation (use existing)
        #[arg(long)]
        no_schema: bool,
    },

    /// Transform workspace to database
    Workspace {
        /// Path to workspace
        #[arg(value_name = "WORKSPACE_PATH")]
        path: PathBuf,
        /// Database output path
        #[arg(short, long, value_name = "DB_PATH")]
        db: Option<PathBuf>,
        /// Specific crates to transform
        #[arg(short, long)]
        crates: Vec<String>,
    },
}
```

### 1.4 Database Commands (A.4)

```rust
// xtask/src/cli/db.rs
#[derive(Subcommand, Debug)]
pub enum DbCommand {
    /// Save database to backup file
    Save {
        /// Database path (default: active)
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Output backup file path
        #[arg(value_name = "OUTPUT_PATH")]
        output: PathBuf,
    },

    /// Load database from backup file
    Load {
        /// Backup file path
        #[arg(value_name = "BACKUP_PATH")]
        path: PathBuf,
        /// Target database path (default: new in-memory)
        #[arg(short, long)]
        target: Option<PathBuf>,
    },

    /// Load a fixture database
    LoadFixture {
        /// Fixture identifier
        #[arg(value_name = "FIXTURE_ID")]
        fixture: String,
        /// Create HNSW index after loading
        #[arg(long)]
        index: bool,
    },

    /// Count nodes in database
    CountNodes {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Count specific node types
        #[arg(long, value_enum)]
        kind: Option<NodeKind>,
    },

    /// Build HNSW index
    HnswBuild {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Embedding set to index
        #[arg(long)]
        embedding_set: Option<String>,
    },

    /// Rebuild HNSW index
    HnswRebuild {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
    },

    /// Rebuild BM25 index
    Bm25Rebuild {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
    },

    /// Execute arbitrary CozoDB query
    Query {
        /// CozoScript query string
        #[arg(value_name = "QUERY")]
        query: String,
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Query parameters (key=value)
        #[arg(short, long, value_parser = parse_key_val::<String, String>)]
        param: Vec<(String, String)>,
        /// Allow mutating query
        #[arg(long)]
        mutable: bool,
    },

    /// Show database statistics
    Stats {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Stats category
        #[arg(value_enum, default_value = "all")]
        category: StatsCategory,
    },

    /// List relations in database
    ListRelations {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Exclude HNSW indices
        #[arg(long)]
        no_hnsw: bool,
    },

    /// Show embedding status
    EmbeddingStatus {
        /// Database path
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Specific embedding set
        #[arg(long)]
        set: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum NodeKind {
    Function,
    Type,
    Module,
    All,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum StatsCategory {
    All,
    Embeddings,
    Nodes,
    Relations,
    Indexes,
}
```

### 1.5 Ingest Commands (A.3)

```rust
// xtask/src/cli/ingest.rs
#[derive(Subcommand, Debug)]
pub enum IngestCommand {
    /// Run embedding generation
    Embed {
        /// Path to crate/workspace
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Embedding backend
        #[arg(short, long, value_enum, default_value = "mock")]
        backend: EmbeddingBackend,
        /// Batch size override
        #[arg(short, long)]
        batch_size: Option<usize>,
        /// Timeout in seconds
        #[arg(long, default_value = "300")]
        timeout: u64,
        /// Skip HNSW index creation
        #[arg(long)]
        no_index: bool,
    },

    /// Run indexing pipeline
    Index {
        /// Path to crate/workspace
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Database path (creates new if not specified)
        #[arg(short, long)]
        db: Option<PathBuf>,
        /// Embedding backend
        #[arg(short, long, value_enum, default_value = "mock")]
        backend: EmbeddingBackend,
    },
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum EmbeddingBackend {
    /// Mock embedder (fast, for testing)
    Mock,
    /// Local model (CPU/GPU)
    Local,
    /// OpenRouter API (requires TEST_OPENROUTER_API_KEY)
    OpenRouter,
}
```

### 1.6 Pipeline Commands (Cross-crate)

```rust
// xtask/src/cli/pipeline.rs
#[derive(Subcommand, Debug)]
pub enum PipelineCommand {
    /// Parse and transform in one step
    ParseTransform {
        /// Path to crate
        #[arg(value_name = "CRATE_PATH")]
        path: PathBuf,
        /// Output database path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Keep intermediate files
        #[arg(long)]
        keep_intermediate: bool,
    },

    /// Full pipeline: parse, transform, embed
    FullIngest {
        /// Path to crate/workspace
        #[arg(value_name = "PATH")]
        path: PathBuf,
        /// Output database path
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Embedding backend
        #[arg(short, long, value_enum, default_value = "mock")]
        backend: EmbeddingBackend,
        /// Preserve existing database
        #[arg(long)]
        preserve_existing: bool,
    },

    /// Parse and transform entire workspace
    Workspace {
        /// Path to workspace
        #[arg(value_name = "WORKSPACE_PATH")]
        path: PathBuf,
        /// Specific crates to process
        #[arg(short, long)]
        crates: Vec<String>,
        /// Output database path
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
}
```

---

## 2. Shared Configuration Types

### 2.1 Core Config Types

```rust
// xtask/src/config/mod.rs
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};

/// Shared database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Database file path (None = in-memory)
    pub path: Option<PathBuf>,
    /// Auto-create schema on init
    #[serde(default = "default_true")]
    pub auto_schema: bool,
    /// Setup multi-embedding support
    #[serde(default = "default_true")]
    pub multi_embedding: bool,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: None,
            auto_schema: true,
            multi_embedding: true,
        }
    }
}

/// Parsing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseConfig {
    /// Target crate/workspace path
    pub target_path: PathBuf,
    /// Workspace root (if applicable)
    pub workspace_root: Option<PathBuf>,
    /// Selected crates (empty = all)
    #[serde(default)]
    pub selected_crates: Vec<String>,
    /// Include test files
    #[serde(default)]
    pub include_tests: bool,
}

/// Embedding configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingConfig {
    /// Backend type
    pub backend: BackendType,
    /// Batch size for embedding generation
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    /// Request timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    /// Device preference (for local backend)
    pub device: DevicePreference,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendType {
    Mock,
    Local,
    OpenRouter,
    OpenAi,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevicePreference {
    Auto,
    Cpu,
    Gpu,
}

/// Tracing/Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing output
    #[serde(default)]
    pub enabled: bool,
    /// Log file path (None = stdout only)
    pub log_file: Option<PathBuf>,
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Include span timings
    #[serde(default = "default_true")]
    pub span_timings: bool,
}

// Helper functions for defaults
fn default_true() -> bool { true }
fn default_batch_size() -> usize { 32 }
fn default_timeout() -> u64 { 300 }
fn default_log_level() -> String { "info".to_string() }
```

### 2.2 Command Context

```rust
// xtask/src/context.rs
use std::path::PathBuf;
use std::sync::Arc;
use ploke_db::Database;

/// Shared context passed to command handlers
pub struct CommandContext {
    /// Output format
    pub format: OutputFormat,
    /// Verbosity level (0-3)
    pub verbose: u8,
    /// Quiet mode
    pub quiet: bool,
    /// Workspace root path
    pub workspace_root: PathBuf,
    /// Active database (if any)
    pub active_db: Option<Arc<Database>>,
    /// Tracing configuration
    pub tracing: TracingConfig,
    /// Output writer
    pub output: Box<dyn OutputWriter>,
}

impl CommandContext {
    /// Create a new context with defaults
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            format: OutputFormat::Human,
            verbose: 0,
            quiet: false,
            workspace_root,
            active_db: None,
            tracing: TracingConfig::default(),
            output: Box::new(StdOutputWriter),
        }
    }

    /// Check if we should output at given verbosity
    pub fn should_output(&self, min_verbosity: u8) -> bool {
        !self.quiet && self.verbose >= min_verbosity
    }
}
```

---

## 3. Output Formatting System

### 3.1 Output Formatter Trait

```rust
// xtask/src/output/mod.rs
use serde::Serialize;
use std::io::Write;

/// Trait for output formatters
pub trait OutputFormatter {
    /// Format a serializable value
    fn format<T: Serialize>(&self, value: &T) -> Result<String, OutputError>;
    
    /// Format an error with recovery hint
    fn format_error(&self, error: &CommandError) -> String;
    
    /// Format a table of records
    fn format_table<T: TableRow>(&self, headers: &[&str], rows: &[T]) -> String;
    
    /// Get the format type
    fn format_type(&self) -> OutputFormat;
}

/// Trait for types that can be formatted as table rows
pub trait TableRow {
    fn to_cells(&self) -> Vec<String>;
}

/// Output error types
#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("Serialization failed: {0}")]
    Serialization(String),
    #[error("Formatting failed: {0}")]
    Formatting(String),
}
```

### 3.2 Formatter Implementations

```rust
// xtask/src/output/human.rs
pub struct HumanFormatter;

impl OutputFormatter for HumanFormatter {
    fn format<T: Serialize>(&self, value: &T) -> Result<String, OutputError> {
        // Use serde to convert to a Value, then pretty-print
        let json = serde_json::to_value(value)
            .map_err(|e| OutputError::Serialization(e.to_string()))?;
        Ok(format_value_human(&json, 0))
    }

    fn format_error(&self, error: &CommandError) -> String {
        let mut output = format!("Error: {}", error.message);
        
        if let Some(recovery) = &error.recovery_hint {
            output.push_str(&format!("\n\n{}", recovery));
        }
        
        if !error.suggestions.is_empty() {
            output.push_str("\n\nSuggestions:");
            for suggestion in &error.suggestions {
                output.push_str(&format!("\n  • {}", suggestion));
            }
        }
        
        output
    }

    fn format_table<T: TableRow>(&self, headers: &[&str], rows: &[T]) -> String {
        if rows.is_empty() {
            return "(no data)".to_string();
        }

        let mut col_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        
        // Calculate column widths
        for row in rows {
            for (i, cell) in row.to_cells().iter().enumerate() {
                if i < col_widths.len() {
                    col_widths[i] = col_widths[i].max(cell.len());
                }
            }
        }

        // Build table
        let mut lines = Vec::new();
        
        // Header row
        let header_row: String = headers
            .iter()
            .enumerate()
            .map(|(i, h)| format!("{:width$}", h, width = col_widths[i]))
            .collect::<Vec<_>>()
            .join(" | ");
        lines.push(header_row);
        lines.push("-".repeat(lines[0].len()));
        
        // Data rows
        for row in rows {
            let cells = row.to_cells();
            let row_str: String = cells
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{:width$}", c, width = col_widths.get(i).copied().unwrap_or(0)))
                .collect::<Vec<_>>()
                .join(" | ");
            lines.push(row_str);
        }
        
        lines.join("\n")
    }

    fn format_type(&self) -> OutputFormat {
        OutputFormat::Human
    }
}

// xtask/src/output/json.rs
pub struct JsonFormatter {
    pretty: bool,
}

impl OutputFormatter for JsonFormatter {
    fn format<T: Serialize>(&self, value: &T) -> Result<String, OutputError> {
        if self.pretty {
            serde_json::to_string_pretty(value)
        } else {
            serde_json::to_string(value)
        }
        .map_err(|e| OutputError::Serialization(e.to_string()))
    }

    fn format_error(&self, error: &CommandError) -> String {
        // JSON errors follow a structured format
        let error_json = serde_json::json!({
            "error": {
                "message": error.message,
                "code": error.code,
                "recovery_hint": error.recovery_hint,
                "suggestions": error.suggestions,
            }
        });
        serde_json::to_string_pretty(&error_json).unwrap_or_default()
    }

    fn format_table<T: TableRow>(&self, _headers: &[&str], rows: &[T]) -> String {
        // Tables become JSON arrays
        serde_json::to_string_pretty(rows)
            .unwrap_or_else(|_| "[]".to_string())
    }

    fn format_type(&self) -> OutputFormat {
        OutputFormat::Json
    }
}

// xtask/src/output/table.rs
pub struct TableFormatter;

impl OutputFormatter for TableFormatter {
    fn format<T: Serialize>(&self, value: &T) -> Result<String, OutputError> {
        // Try to flatten to table if possible, fall back to JSON
        match try_flatten_to_table(value) {
            Some((headers, rows)) => Ok(self.format_table(&headers, &rows)),
            None => {
                // Fall back to compact JSON
                serde_json::to_string(value)
                    .map_err(|e| OutputError::Serialization(e.to_string()))
            }
        }
    }

    fn format_error(&self, error: &CommandError) -> String {
        format!("ERROR\t{}\t{}", 
            error.code,
            error.message.replace('\t', " ")
        )
    }

    fn format_table<T: TableRow>(&self, headers: &[&str], rows: &[T]) -> String {
        // TSV format for easy parsing
        let mut lines = vec![headers.join("\t")];
        for row in rows {
            lines.push(row.to_cells().join("\t"));
        }
        lines.join("\n")
    }

    fn format_type(&self) -> OutputFormat {
        OutputFormat::Table
    }
}

// xtask/src/output/compact.rs
pub struct CompactFormatter;

impl OutputFormatter for CompactFormatter {
    fn format<T: Serialize>(&self, value: &T) -> Result<String, OutputError> {
        // Single-line compact output
        serde_json::to_string(value)
            .map_err(|e| OutputError::Serialization(e.to_string()))
    }

    fn format_error(&self, error: &CommandError) -> String {
        format!("ERROR: {}", error.message)
    }

    fn format_table<T: TableRow>(&self, _headers: &[&str], rows: &[T]) -> String {
        rows.iter()
            .map(|r| r.to_cells().join(","))
            .collect::<Vec<_>>()
            .join(";")
    }

    fn format_type(&self) -> OutputFormat {
        OutputFormat::Compact
    }
}
```

### 3.3 Formatter Factory

```rust
// xtask/src/output/factory.rs
pub fn create_formatter(format: OutputFormat) -> Box<dyn OutputFormatter> {
    match format {
        OutputFormat::Human => Box::new(HumanFormatter),
        OutputFormat::Json => Box::new(JsonFormatter { pretty: true }),
        OutputFormat::Table => Box::new(TableFormatter),
        OutputFormat::Compact => Box::new(CompactFormatter),
    }
}
```

---

## 4. Subcommand Module Organization

### 4.1 Module Structure

```
xtask/src/
├── main.rs              # Entry point, command dispatch
├── cli/
│   ├── mod.rs           # Root CLI types and parser
│   ├── parse.rs         # Parse subcommands
│   ├── transform.rs     # Transform subcommands
│   ├── db.rs            # Database subcommands
│   ├── ingest.rs        # Ingest/embedding subcommands
│   ├── tui.rs           # TUI headless subcommands
│   ├── tool.rs          # Tool execution subcommands
│   ├── pipeline.rs      # Pipeline commands
│   ├── validate.rs      # Validation commands
│   ├── setup.rs         # Setup commands
│   └── help.rs          # Help system
├── commands/            # Command implementations
│   ├── mod.rs           # Command trait and dispatch
│   ├── parse.rs         # Parse command handlers
│   ├── transform.rs     # Transform command handlers
│   ├── db.rs            # Database command handlers
│   ├── ingest.rs        # Ingest command handlers
│   ├── pipeline.rs      # Pipeline command handlers
│   └── ...
├── output/              # Output formatting
│   ├── mod.rs           # Formatter trait
│   ├── human.rs         # Human formatter
│   ├── json.rs          # JSON formatter
│   ├── table.rs         # Table formatter
│   └── compact.rs       # Compact formatter
├── config/              # Configuration types
│   ├── mod.rs           # Config structs
│   └── loader.rs        # Config file loading
├── context.rs           # Command context
├── error.rs             # Error types and recovery
├── tracing_ext.rs       # Tracing extensions
├── persistence.rs       # Output persistence for grepping
└── stats.rs             # Usage statistics
```

### 4.2 Command Handler Trait

```rust
// xtask/src/commands/mod.rs
use async_trait::async_trait;

/// Trait for command handlers
#[async_trait]
pub trait CommandHandler {
    type Args;
    
    /// Execute the command
    async fn execute(&self, args: &Self::Args, ctx: &mut CommandContext) -> CommandResult;
    
    /// Get command metadata for help
    fn metadata(&self) -> CommandMetadata;
}

/// Command execution result
pub type CommandResult = Result<CommandOutput, CommandError>;

/// Command output (serializable)
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum CommandOutput {
    /// Simple success message
    Success { message: String },
    /// Structured data output
    Data(serde_json::Value),
    /// Table output
    Table { headers: Vec<String>, rows: Vec<Vec<String>> },
    /// File path output
    File { path: PathBuf, description: String },
    /// Multiple outputs
    Multi(Vec<CommandOutput>),
}

/// Command metadata for help generation
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub examples: Vec<CommandExample>,
    pub related_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandExample {
    pub command: String,
    pub description: String,
}
```

---

## 5. Common Argument Types

### 5.1 Shared Arguments

```rust
// xtask/src/args/mod.rs

/// Common database arguments (can be flattened)
#[derive(Args, Debug, Clone)]
pub struct DatabaseArgs {
    /// Database file path
    #[arg(short, long, global = true, value_name = "PATH")]
    pub db: Option<PathBuf>,
    
    /// Use in-memory database
    #[arg(long, conflicts_with = "db")]
    pub memory: bool,
}

/// Common path arguments
#[derive(Args, Debug, Clone)]
pub struct PathArgs {
    /// Target path (crate, workspace, or file)
    #[arg(value_name = "PATH", default_value = ".")]
    pub path: PathBuf,
    
    /// Verify path exists
    #[arg(skip)]
    verify_exists: bool,
}

impl PathArgs {
    /// Resolve path relative to workspace root
    pub fn resolve(&self, workspace_root: &Path) -> PathBuf {
        if self.path.is_absolute() {
            self.path.clone()
        } else {
            workspace_root.join(&self.path)
        }
    }
}

/// Common output arguments
#[derive(Args, Debug, Clone)]
pub struct OutputArgs {
    /// Output file (default: stdout)
    #[arg(short, long, value_name = "FILE")]
    pub output: Option<PathBuf>,
    
    /// Append to file instead of overwrite
    #[arg(long, requires = "output")]
    pub append: bool,
    
    /// Persist output for later grepping
    #[arg(long, env = "XTASK_PERSIST_OUTPUT")]
    pub persist: bool,
}

/// Common filtering arguments
#[derive(Args, Debug, Clone)]
pub struct FilterArgs {
    /// Filter by name pattern
    #[arg(short, long, value_name = "PATTERN")]
    pub filter: Option<String>,
    
    /// Include/exclude specific items
    #[arg(short, long, value_name = "ITEM")]
    pub include: Vec<String>,
    
    #[arg(long, value_name = "ITEM")]
    pub exclude: Vec<String>,
}

/// Timing and performance arguments
#[derive(Args, Debug, Clone)]
pub struct TimingArgs {
    /// Enable detailed timing
    #[arg(long)]
    pub timings: bool,
    
    /// Maximum execution time (seconds)
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<u64>,
}

/// Tracing arguments
#[derive(Args, Debug, Clone)]
pub struct TracingArgs {
    /// Enable tracing to log file
    #[arg(long, value_name = "FILE")]
    pub trace_log: Option<PathBuf>,
    
    /// Tracing level
    #[arg(long, value_enum, default_value = "info")]
    pub trace_level: TraceLevel,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum TraceLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}
```

### 5.2 Argument Validation

```rust
// xtask/src/args/validate.rs

/// Validate that a path points to a valid crate
pub fn validate_crate_path(path: &Path) -> Result<PathBuf, String> {
    let canonical = path.canonicalize()
        .map_err(|e| format!("Cannot access path: {e}"))?;
    
    let cargo_toml = canonical.join("Cargo.toml");
    if !cargo_toml.exists() {
        return Err(format!(
            "No Cargo.toml found at {}. \
             Please provide a path to a valid Rust crate directory.",
            canonical.display()
        ));
    }
    
    Ok(canonical)
}

/// Validate that a path points to a workspace
pub fn validate_workspace_path(path: &Path) -> Result<PathBuf, String> {
    let canonical = validate_crate_path(path)?;
    
    // Check if it's actually a workspace
    let content = std::fs::read_to_string(canonical.join("Cargo.toml"))
        .map_err(|e| format!("Cannot read Cargo.toml: {e}"))?;
    
    if !content.contains("[workspace]") {
        return Err(format!(
            "{} is a crate but not a workspace. \
             Use 'parse phases-merge' for single crates.",
            canonical.display()
        ));
    }
    
    Ok(canonical)
}

/// Parse key=value pairs from CLI
pub fn parse_key_val<T, U>(s: &str) -> Result<(T, U), String>
where
    T: std::str::FromStr,
    U: std::str::FromStr,
    T::Err: std::fmt::Display,
    U::Err: std::fmt::Display,
{
    let pos = s.find('=')
        .ok_or_else(|| format!("Invalid KEY=value: no '=' found in '{}'", s))?;
    
    let key = s[..pos].parse()
        .map_err(|e| format!("Invalid key: {e}"))?;
    let val = s[pos + 1..].parse()
        .map_err(|e| format!("Invalid value: {e}"))?;
    
    Ok((key, val))
}
```

---

## 6. Example Command Implementations

### 6.1 Example 1: `parse stats` Command

```rust
// xtask/src/commands/parse.rs
use crate::cli::parse::{ParseCommand, NodeTypeFilter};
use crate::commands::{CommandHandler, CommandResult, CommandOutput};
use crate::context::CommandContext;
use crate::error::{CommandError, RecoveryHint};
use syn_parser::{try_run_phases_and_merge, parser::graph::GraphAccess};
use async_trait::async_trait;

pub struct ParseStatsHandler;

#[async_trait]
impl CommandHandler for ParseStatsHandler {
    type Args = ParseCommand;
    
    async fn execute(&self, args: &Self::Args, ctx: &mut CommandContext) -> CommandResult {
        let ParseCommand::Stats { path, node_type } = args else {
            unreachable!()
        };
        
        // Resolve and validate path
        let resolved = ctx.resolve_path(path);
        let canonical = validate_crate_path(&resolved)
            .map_err(|e| CommandError::invalid_input(e)
                .with_recovery(RecoveryHint::check_path()))?;
        
        // Run parsing with tracing
        let _span = tracing::info_span!("parse_stats", path = %canonical.display()).entered();
        
        let output = try_run_phases_and_merge(&canonical)
            .map_err(|e| CommandError::from_syn_parser(e)
                .with_recovery(RecoveryHint::check_crate_structure()))?;
        
        let graph = output.extract_merged_graph()
            .ok_or_else(|| CommandError::internal("Missing merged graph")
                .with_recovery(RecoveryHint::report_bug()))?;
        
        // Collect statistics
        let stats = GraphStats {
            crate_name: graph.crate_context.as_ref().map(|c| c.name.clone()),
            total_files: 1, // Simplified
            functions: graph.functions().len(),
            types: graph.defined_types().len(),
            modules: graph.modules().len(),
            traits: graph.traits().len(),
            impls: graph.impls().len(),
            relations: graph.relations().len(),
        };
        
        // Apply filter if specified
        let output = match node_type {
            Some(NodeTypeFilter::Function) => CommandOutput::Data(
                serde_json::json!({ "functions": stats.functions })
            ),
            Some(NodeTypeFilter::Type) => CommandOutput::Data(
                serde_json::json!({ "types": stats.types })
            ),
            Some(NodeTypeFilter::Module) => CommandOutput::Data(
                serde_json::json!({ "modules": stats.modules })
            ),
            _ => CommandOutput::Data(serde_json::to_value(stats).unwrap()),
        };
        
        // Persist output if requested
        if ctx.should_persist_output() {
            ctx.persist_output(&output, "parse_stats")?;
        }
        
        Ok(output)
    }
    
    fn metadata(&self) -> CommandMetadata {
        CommandMetadata {
            name: "parse stats".to_string(),
            description: "Show statistics for parsed code".to_string(),
            examples: vec![
                CommandExample {
                    command: "cargo xtask parse stats ./my-crate".to_string(),
                    description: "Show all statistics for a crate".to_string(),
                },
                CommandExample {
                    command: "cargo xtask parse stats ./my-crate --node-type function".to_string(),
                    description: "Show only function count".to_string(),
                },
            ],
            related_commands: vec![
                "parse phases-merge".to_string(),
                "parse list-modules".to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize)]
struct GraphStats {
    crate_name: Option<String>,
    total_files: usize,
    functions: usize,
    types: usize,
    modules: usize,
    traits: usize,
    impls: usize,
    relations: usize,
}
```

### 6.2 Example 2: `db query` Command

```rust
// xtask/src/commands/db.rs
use crate::cli::db::DbCommand;
use crate::commands::{CommandHandler, CommandResult, CommandOutput};
use crate::context::CommandContext;
use crate::error::{CommandError, RecoveryHint};
use ploke_db::Database;
use cozo::{ScriptMutability, DataValue};
use std::collections::BTreeMap;

pub struct DbQueryHandler;

#[async_trait]
impl CommandHandler for DbQueryHandler {
    type Args = DbCommand;
    
    async fn execute(&self, args: &Self::Args, ctx: &mut CommandContext) -> CommandResult {
        let DbCommand::Query { query, db, param, mutable } = args else {
            unreachable!()
        };
        
        // Get or initialize database
        let db = ctx.get_or_init_db(db.clone()).await
            .map_err(|e| CommandError::database(e)
                .with_recovery(RecoveryHint::db_init_failed()))?;
        
        // Parse parameters
        let params: BTreeMap<String, DataValue> = param
            .iter()
            .map(|(k, v)| (k.clone(), DataValue::from(v.as_str())))
            .collect();
        
        // Execute query with tracing
        let _span = tracing::info_span!("db_query", 
            query_length = query.len(),
            mutable = *mutable
        ).entered();
        
        let result = if *mutable {
            db.run_script(query, params, ScriptMutability::Mutable)
        } else {
            db.run_script(query, params, ScriptMutability::Immutable)
        };
        
        match result {
            Ok(named_rows) => {
                let output = QueryOutput {
                    headers: named_rows.headers.clone(),
                    rows: named_rows.rows.iter()
                        .map(|r| r.iter().map(|d| format!("{:?}", d)).collect())
                        .collect(),
                    row_count: named_rows.rows.len(),
                };
                
                Ok(CommandOutput::Data(serde_json::to_value(output).unwrap()))
            }
            Err(e) => {
                Err(CommandError::cozo_query(e, query.clone())
                    .with_recovery(RecoveryHint::cozo_query_help()))
            }
        }
    }
    
    fn metadata(&self) -> CommandMetadata {
        CommandMetadata {
            name: "db query".to_string(),
            description: "Execute a CozoDB query".to_string(),
            examples: vec![
                CommandExample {
                    command: r#"cargo xtask db query '?[count(id)] := *function { id }'"#.to_string(),
                    description: "Count all functions".to_string(),
                },
                CommandExample {
                    command: r#"cargo xtask db query '?[id] := *function { id, name }, name = $name' --param name=main"#.to_string(),
                    description: "Query with parameters".to_string(),
                },
            ],
            related_commands: vec![
                "db stats".to_string(),
                "db list-relations".to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize)]
struct QueryOutput {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    row_count: usize,
}
```

### 6.3 Example 3: `pipeline parse-transform` Command

```rust
// xtask/src/commands/pipeline.rs
use crate::cli::pipeline::PipelineCommand;
use crate::commands::{CommandHandler, CommandResult, CommandOutput};
use crate::context::CommandContext;
use crate::error::{CommandError, RecoveryHint};
use ploke_db::Database;
use ploke_transform::{transform::transform_parsed_graph, schema::create_schema_all};
use syn_parser::try_run_phases_and_merge;
use std::sync::Arc;

pub struct PipelineParseTransformHandler;

#[async_trait]
impl CommandHandler for PipelineParseTransformHandler {
    type Args = PipelineCommand;
    
    async fn execute(&self, args: &Self::Args, ctx: &mut CommandContext) -> CommandResult {
        let PipelineCommand::ParseTransform { path, output, keep_intermediate } = args else {
            unreachable!()
        };
        
        let resolved = ctx.resolve_path(path);
        let canonical = validate_crate_path(&resolved)
            .map_err(|e| CommandError::invalid_input(e)
                .with_recovery(RecoveryHint::check_path()))?;
        
        // Stage 1: Parse
        ctx.output_status("Parsing...", 1);
        let parse_span = tracing::info_span!("pipeline_parse", path = %canonical.display()).entered();
        
        let mut output = try_run_phases_and_merge(&canonical)
            .map_err(|e| CommandError::from_syn_parser(e)
                .with_recovery(RecoveryHint::check_crate_structure()))?;
        
        drop(parse_span);
        
        let graph = output.extract_merged_graph()
            .ok_or_else(|| CommandError::internal("Missing merged graph"))?;
        let tree = output.extract_module_tree()
            .ok_or_else(|| CommandError::internal("Missing module tree"))?;
        
        // Stage 2: Transform
        ctx.output_status("Transforming...", 2);
        let transform_span = tracing::info_span!("pipeline_transform").entered();
        
        let db = Arc::new(Database::init_with_schema()
            .map_err(|e| CommandError::database(e)
                .with_recovery(RecoveryHint::db_init_failed()))?);
        
        create_schema_all(&db)
            .map_err(|e| CommandError::transform(e)
                .with_recovery(RecoveryHint::schema_creation_failed()))?;
        
        transform_parsed_graph(&db, graph, &tree)
            .map_err(|e| CommandError::transform(e)
                .with_recovery(RecoveryHint::transform_failed()))?;
        
        drop(transform_span);
        
        // Stage 3: Save if output path specified
        if let Some(output_path) = output {
            ctx.output_status("Saving database...", 3);
            db.db.backup_db(output_path)
                .map_err(|e| CommandError::database(e)
                    .with_recovery(RecoveryHint::backup_failed()))?;
        }
        
        let stats = PipelineStats {
            functions: db.count_pending_embeddings().unwrap_or(0),
            relations: db.relations_vec().map(|r| r.len()).unwrap_or(0),
            output_path: output.clone(),
        };
        
        Ok(CommandOutput::Data(serde_json::to_value(stats).unwrap()))
    }
    
    fn metadata(&self) -> CommandMetadata {
        CommandMetadata {
            name: "pipeline parse-transform".to_string(),
            description: "Parse and transform a crate in one step".to_string(),
            examples: vec![
                CommandExample {
                    command: "cargo xtask pipeline parse-transform ./my-crate".to_string(),
                    description: "Parse and transform to in-memory DB".to_string(),
                },
                CommandExample {
                    command: "cargo xtask pipeline parse-transform ./my-crate -o my-db.sqlite".to_string(),
                    description: "Parse and save to file".to_string(),
                },
            ],
            related_commands: vec![
                "parse phases-merge".to_string(),
                "transform graph".to_string(),
            ],
        }
    }
}

#[derive(Debug, Serialize)]
struct PipelineStats {
    functions: usize,
    relations: usize,
    output_path: Option<PathBuf>,
}
```

---

## 7. Error Handling with Recovery Paths

### 7.1 Error Type Design

```rust
// xtask/src/error.rs
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
pub struct CommandError {
    pub message: String,
    pub code: ErrorCode,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
    pub recovery_hint: Option<RecoveryHint>,
    pub suggestions: Vec<String>,
    pub context: ErrorContext,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Input errors (1xxx)
    InvalidInput = 1001,
    MissingArgument = 1002,
    InvalidPath = 1003,
    PathNotFound = 1004,
    
    // Parse errors (2xxx)
    ParseFailed = 2001,
    DiscoveryFailed = 2002,
    InvalidCrate = 2003,
    
    // Transform errors (3xxx)
    TransformFailed = 3001,
    SchemaCreationFailed = 3002,
    
    // Database errors (4xxx)
    DatabaseInitFailed = 4001,
    QueryFailed = 4002,
    BackupFailed = 4003,
    RestoreFailed = 4004,
    
    // Embedding errors (5xxx)
    EmbeddingFailed = 5001,
    IndexerFailed = 5002,
    ApiKeyMissing = 5003,
    
    // Internal errors (9xxx)
    Internal = 9001,
    NotImplemented = 9002,
}

#[derive(Debug, Clone)]
pub struct ErrorContext {
    pub command: String,
    pub workspace_root: PathBuf,
    pub traced_file: Option<PathBuf>,
}

impl CommandError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            code,
            source: None,
            recovery_hint: None,
            suggestions: Vec::new(),
            context: ErrorContext::default(),
        }
    }
    
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }
    
    pub fn database<E: std::error::Error + Send + Sync + 'static>(e: E) -> Self {
        Self {
            message: format!("Database error: {}", e),
            code: ErrorCode::DatabaseInitFailed,
            source: Some(Box::new(e)),
            recovery_hint: None,
            suggestions: Vec::new(),
            context: ErrorContext::default(),
        }
    }
    
    pub fn from_syn_parser(e: syn_parser::SynParserError) -> Self {
        let message = format!("Parsing failed: {}", e);
        let mut error = Self::new(ErrorCode::ParseFailed, message);
        error.source = Some(Box::new(e));
        error
    }
    
    pub fn cozo_query(e: cozo::Error, query: String) -> Self {
        let message = format!("Query execution failed: {}", e);
        let mut error = Self::new(ErrorCode::QueryFailed, message);
        error.suggestions.push(format!("Your query: {}", query));
        error.suggestions.push(
            "Try checking relation names with: cargo xtask db list-relations".to_string()
        );
        error
    }
    
    pub fn with_recovery(mut self, hint: RecoveryHint) -> Self {
        self.recovery_hint = Some(hint);
        self
    }
    
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }
}

/// Recovery hints for common error scenarios
#[derive(Debug, Clone)]
pub struct RecoveryHint {
    pub title: String,
    pub steps: Vec<String>,
    pub doc_link: Option<String>,
}

impl RecoveryHint {
    pub fn check_path() -> Self {
        Self {
            title: "Check your path".to_string(),
            steps: vec![
                "Verify the path exists and is accessible".to_string(),
                "For relative paths, ensure you're in the correct directory".to_string(),
                "Use absolute paths if unsure".to_string(),
            ],
            doc_link: Some("https://doc.rust-lang.org/cargo/guide/project-layout.html".to_string()),
        }
    }
    
    pub fn check_crate_structure() -> Self {
        Self {
            title: "Check crate structure".to_string(),
            steps: vec![
                "Ensure Cargo.toml exists and is valid".to_string(),
                "Check that src/ directory contains .rs files".to_string(),
                "Verify Cargo.lock is up to date".to_string(),
            ],
            doc_link: None,
        }
    }
    
    pub fn db_init_failed() -> Self {
        Self {
            title: "Database initialization failed".to_string(),
            steps: vec![
                "Check available disk space".to_string(),
                "Verify write permissions to output directory".to_string(),
                "Try using --memory flag for in-memory database".to_string(),
            ],
            doc_link: None,
        }
    }
    
    pub fn cozo_query_help() -> Self {
        Self {
            title: "CozoDB Query Help".to_string(),
            steps: vec![
                "List available relations: cargo xtask db list-relations".to_string(),
                "Check query syntax at https://docs.cozodb.io".to_string(),
                "Use --mutable flag for write queries".to_string(),
            ],
            doc_link: Some("https://docs.cozodb.io".to_string()),
        }
    }
    
    pub fn report_bug() -> Self {
        Self {
            title: "This may be a bug".to_string(),
            steps: vec![
                "Try running with --verbose for more details".to_string(),
                "Check if a newer version is available".to_string(),
                "Report this issue with the full error output".to_string(),
            ],
            doc_link: Some("https://github.com/ploke/ploke/issues".to_string()),
        }
    }
}
```

---

## 8. Pros and Cons of This Approach

### 8.1 Pros

| Advantage | Description |
|-----------|-------------|
| **Type Safety** | Extensive use of clap derive macros ensures compile-time validation of CLI structure |
| **Extensibility** | New commands can be added by implementing the `CommandHandler` trait without modifying existing code |
| **Testability** | Trait-based design allows for easy mocking of command handlers and formatters |
| **Consistent UX** | Shared argument types ensure consistent flags across all commands |
| **Rich Error Messages** | Recovery hints provide actionable guidance for common failure scenarios |
| **Multiple Output Formats** | Pluggable formatters support human, JSON, table, and compact outputs |
| **Async-First** | Built with async in mind for embedding and I/O operations |
| **Tracing Integration** | First-class support for tracing spans and structured logging |

### 8.2 Cons

| Disadvantage | Description | Mitigation |
|--------------|-------------|------------|
| **Boilerplate** | Trait implementations require more code than simple functions | Use macros for repetitive patterns |
| **Complexity** | Multiple layers (CLI → Command → Handler → Formatter) adds cognitive load | Clear documentation and examples |
| **Compilation Time** | Heavy use of generics and traits may increase compile times | Consider boxing in non-hot paths |
| **Binary Size** | Multiple formatter implementations increase binary size | Feature flags for optional formatters |

### 8.3 Comparison with Alternatives

| Approach | Pros | Cons | Best For |
|----------|------|------|----------|
| **This Proposal (Traits)** | Type-safe, extensible, testable | More boilerplate | Long-term maintainability |
| **Simple Functions** | Less code, easier to understand | Harder to extend, less consistent | Quick prototypes |
| **Macro-heavy** | Less boilerplate | Harder to debug, opaque | Repetitive CRUD operations |
| **Enum-based dispatch** | Simple pattern matching | Less modular, harder to test | Small command sets |

---

## 9. Implementation Recommendations

### 9.1 Phased Implementation

1. **Phase 1: Foundation** (M.3)
   - Set up module structure
   - Implement core traits (`CommandHandler`, `OutputFormatter`)
   - Add error types with recovery hints
   - Implement 2-3 simple commands as proofs of concept

2. **Phase 2: Core Commands** (M.4)
   - Implement all parse commands
   - Implement all db commands
   - Add shared argument validation
   - Complete output formatters

3. **Phase 3: Advanced Features** (M.5-M.6)
   - Pipeline commands
   - TUI headless mode
   - Tool execution
   - Cross-crate validation commands

### 9.2 Testing Strategy

```rust
// Example test structure
#[cfg(test)]
mod tests {
    use super::*;
    
    #[tokio::test]
    async fn test_parse_stats_command() {
        let handler = ParseStatsHandler;
        let args = ParseCommand::Stats {
            path: PathBuf::from("tests/fixture_crates/fixture_nodes"),
            node_type: None,
        };
        let mut ctx = CommandContext::new(workspace_root());
        ctx.format = OutputFormat::Json; // For easy assertion
        
        let result = handler.execute(&args, &mut ctx).await;
        assert!(result.is_ok());
        
        // Verify output structure
        if let Ok(CommandOutput::Data(data)) = result {
            assert!(data.get("functions").is_some());
        }
    }
}
```

---

## 10. Summary

This architecture proposal provides a **robust, extensible foundation** for the xtask commands feature by:

1. **Leveraging clap derive macros** for type-safe CLI parsing
2. **Using trait-based design** for modularity and testability
3. **Supporting multiple output formats** via pluggable formatters
4. **Providing rich error handling** with recovery paths
5. **Organizing code by concern** (CLI → Commands → Output → Config)

The design prioritizes **long-term maintainability** and **agent productivity** while keeping the door open for future enhancements like additional output formats, new command categories, and integration with external tools.
