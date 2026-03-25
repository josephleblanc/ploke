# Architecture Proposal 1: Modular Xtask Commands System

**Agent:** Architecture Agent 1  
**Date:** 2026-03-25  
**Milestone:** M.2.1 - Design Architecture + Documentation  

---

## Executive Summary

This proposal presents a **layered, modular architecture** for the xtask commands feature that emphasizes:

1. **Clear separation of concerns** via per-crate modules
2. **Type-safe command dispatch** using enums and traits
3. **Unified error handling** with `ploke_error::Error`
4. **Comprehensive tracing** for observability
5. **Extensibility** for future commands

---

## 1. Module Structure Diagram

```
xtask/
├── src/
│   ├── main.rs                 # Entry point, CLI parsing, dispatch
│   ├── lib.rs                  # Public exports (for testing)
│   ├── cli.rs                  # Command enum definitions (clap)
│   ├── error.rs                # Error context helpers
│   ├── tracing_setup.rs        # Tracing subscriber configuration
│   ├── feedback.rs             # User feedback/output helpers
│   ├── commands/
│   │   ├── mod.rs              # Command trait and registry
│   │   ├── parse.rs            # syn_parser commands (A.1)
│   │   ├── transform.rs        # ploke_transform commands (A.2)
│   │   ├── ingest.rs           # ploke_embed commands (A.3)
│   │   ├── db.rs               # ploke_db commands (A.4)
│   │   ├── pipeline.rs         # Cross-crate pipeline commands
│   │   ├── validate.rs         # Validation commands
│   │   └── setup.rs            # Setup/test-env commands
│   ├── tui_commands/           # Headless TUI (A.5) - separate module
│   │   ├── mod.rs
│   │   ├── headless.rs         # TestBackend harness
│   │   └── tools_direct.rs     # Direct tool execution (A.6)
│   └── utils/
│       ├── mod.rs
│       ├── db_setup.rs         # Common database initialization
│       ├── output.rs           # Output formatting (JSON/table)
│       └── paths.rs            # Path resolution helpers
```

### Module Responsibilities

| Module | Responsibility | Crates Used |
|--------|---------------|-------------|
| `commands::parse` | Discovery, parsing, graph inspection | syn_parser |
| `commands::transform` | Graph-to-DB transformation | ploke_transform |
| `commands::ingest` | Embedding generation and indexing | ploke_embed |
| `commands::db` | Database operations (backup, query, index) | ploke_db |
| `commands::pipeline` | Multi-stage workflows | All above |
| `commands::validate` | Integrity checks and diagnostics | syn_parser, ploke_db |
| `commands::setup` | Test environment setup | ploke_test_utils |
| `tui_commands` | Headless TUI and direct tool calls | ploke_tui |

---

## 2. Core Types and Structs

### 2.1 Command Enum (CLI Parsing)

```rust
// src/cli.rs
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "xtask")]
#[command(about = "Ploke workspace automation and diagnostic commands")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    
    /// Output format
    #[arg(global = true, long, value_enum, default_value = "human")]
    pub format: OutputFormat,
    
    /// Enable verbose tracing output
    #[arg(global = true, short, long)]
    pub verbose: bool,
    
    /// Write tracing output to file
    #[arg(global = true, long, value_name = "PATH")]
    pub trace_log: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Compact,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Parsing and discovery commands (syn_parser)
    #[command(subcommand)]
    Parse(ParseCommands),
    
    /// Transform commands (ploke_transform)
    #[command(subcommand)]
    Transform(TransformCommands),
    
    /// Ingestion and embedding commands (ploke_embed)
    #[command(subcommand)]
    Ingest(IngestCommands),
    
    /// Database operations (ploke_db)
    #[command(subcommand)]
    Db(DbCommands),
    
    /// Cross-crate pipeline commands
    #[command(subcommand)]
    Pipeline(PipelineCommands),
    
    /// Validation and diagnostic commands
    #[command(subcommand)]
    Validate(ValidateCommands),
    
    /// Setup and test environment commands
    #[command(subcommand)]
    Setup(SetupCommands),
    
    /// Headless TUI commands (ploke_tui)
    #[command(subcommand)]
    Tui(TuiCommands),
    
    /// Direct tool execution commands
    #[command(subcommand)]
    Tool(ToolCommands),
    
    /// Show help and documentation
    Help(HelpArgs),
}

// Example: Parse subcommands
#[derive(Subcommand, Debug)]
pub enum ParseCommands {
    /// Run discovery phase
    Discovery {
        #[arg(value_name = "PATH")]
        crate_path: PathBuf,
    },
    /// Parse and resolve without merging
    PhasesResolve {
        #[arg(value_name = "PATH")]
        crate_path: PathBuf,
    },
    /// Parse, resolve, and merge graphs
    PhasesMerge {
        #[arg(value_name = "PATH")]
        crate_path: PathBuf,
        /// Output graph statistics
        #[arg(long)]
        stats: bool,
    },
    /// Parse entire workspace
    Workspace {
        #[arg(value_name = "PATH")]
        workspace_path: PathBuf,
        /// Specific crates to parse
        #[arg(long, value_name = "CRATE")]
        crates: Vec<String>,
    },
    /// Show parsing statistics
    Stats {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// List discovered crates
    DiscoveryList {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
    /// Validate graph relations
    ValidateRelations {
        #[arg(value_name = "PATH")]
        path: PathBuf,
    },
}

// Additional command enums defined similarly for Transform, Ingest, Db, etc.
```

### 2.2 Command Context

```rust
// src/commands/mod.rs
use std::path::PathBuf;
use ploke_error::Error;

/// Context passed to all command executions
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// Output format preference
    pub format: OutputFormat,
    
    /// Verbosity level (0-3)
    pub verbosity: u8,
    
    /// Tracing span for the command
    pub trace_span: tracing::Span,
    
    /// Workspace root path
    pub workspace_root: PathBuf,
    
    /// Optional trace log path
    pub trace_log: Option<PathBuf>,
}

impl CommandContext {
    pub fn new(format: OutputFormat, verbosity: u8, workspace_root: PathBuf) -> Self {
        let span = tracing::info_span!("xtask_command", format = ?format, verbosity);
        Self {
            format,
            verbosity,
            trace_span: span,
            workspace_root,
            trace_log: None,
        }
    }
}

/// Trait for all xtask commands
#[async_trait::async_trait]
pub trait XtaskCommand: std::fmt::Debug + Send + Sync {
    /// Execute the command, returning structured output
    async fn execute(&self, ctx: &CommandContext) -> Result<CommandOutput, Error>;
    
    /// Get command name for help/documentation
    fn name(&self) -> &'static str;
    
    /// Get command description
    fn description(&self) -> &'static str;
}

/// Structured command output
#[derive(Debug, Clone)]
pub enum CommandOutput {
    /// Success with message
    Success(String),
    /// Success with structured data (JSON serializable)
    Data(Box<dyn erased_serde::Serialize + Send>),
    /// Success with table output
    Table(TableOutput),
    /// Partial success with warnings
    Partial { message: String, warnings: Vec<String> },
}

/// Table output structure
#[derive(Debug, Clone)]
pub struct TableOutput {
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub title: Option<String>,
}
```

### 2.3 Database Handle Wrapper

```rust
// src/utils/db_setup.rs
use ploke_db::Database;
use ploke_error::{Error, DomainError};
use std::path::PathBuf;
use std::sync::Arc;

/// Database configuration for commands
#[derive(Debug, Clone)]
pub struct DbConfig {
    pub in_memory: bool,
    pub backup_path: Option<PathBuf>,
    pub fixture_id: Option<String>,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            in_memory: true,
            backup_path: None,
            fixture_id: None,
        }
    }
}

/// Initialize database with proper schema
#[tracing::instrument(skip(config))]
pub fn init_database(config: &DbConfig) -> Result<Arc<Database>, Error> {
    let db = if let Some(fixture_id) = &config.fixture_id {
        // Load from fixture
        use ploke_test_utils::{backup_db_fixture, fresh_backup_fixture_db};
        let fixture = backup_db_fixture(fixture_id)
            .ok_or_else(|| DomainError::Config { 
                message: format!("Unknown fixture: {}", fixture_id) 
            })?;
        fresh_backup_fixture_db(fixture)
            .map_err(|e| DomainError::Database { 
                message: format!("Failed to load fixture: {}", e) 
            })?
    } else if let Some(backup_path) = &config.backup_path {
        // Restore from backup
        let db = Database::init_with_schema()
            .map_err(|e| DomainError::Database { message: e.to_string() })?;
        db.restore_backup(backup_path)
            .map_err(|e| DomainError::Database { 
                message: format!("Failed to restore backup: {}", e) 
            })?;
        db
    } else {
        // Fresh in-memory database
        Database::init_with_schema()
            .map_err(|e| DomainError::Database { message: e.to_string() })?
    };
    
    Ok(Arc::new(db))
}
```

---

## 3. Key Traits

### 3.1 Command Trait (Primary)

```rust
// src/commands/mod.rs

/// Core trait for executable commands
#[async_trait::async_trait]
pub trait Command: Send + Sync {
    /// Execute the command
    async fn execute(&self, ctx: &CommandContext) -> Result<CommandOutput, Error>;
    
    /// Command name for logging/help
    fn name(&self) -> &str;
    
    /// One-line description
    fn description(&self) -> &str;
}

/// Trait for commands that require database access
#[async_trait::async_trait]
pub trait DbCommand: Command {
    /// Get database configuration
    fn db_config(&self) -> DbConfig;
    
    /// Execute with initialized database
    async fn execute_with_db(
        &self, 
        ctx: &CommandContext, 
        db: Arc<Database>
    ) -> Result<CommandOutput, Error>;
}

/// Trait for commands that require async runtime
pub trait AsyncCommand: Command {
    /// Whether this command needs a tokio runtime
    fn requires_runtime(&self) -> bool { true }
}

/// Trait for pipeline stages (composable commands)
#[async_trait::async_trait]
pub trait PipelineStage: Send + Sync {
    /// Stage name
    fn stage_name(&self) -> &str;
    
    /// Execute stage with input/output
    async fn execute(
        &self, 
        ctx: &CommandContext,
        input: StageInput,
    ) -> Result<StageOutput, Error>;
}

/// Pipeline stage input
pub enum StageInput {
    None,
    ParsedGraph(ParsedCodeGraph),
    ParsedWorkspace(ParsedWorkspace),
    Database(Arc<Database>),
}

/// Pipeline stage output
pub enum StageOutput {
    None,
    ParsedGraph(ParsedCodeGraph),
    ParsedWorkspace(ParsedWorkspace),
    Database(Arc<Database>),
    ModuleTree(ModuleTree),
}
```

### 3.2 Output Formatter Trait

```rust
// src/utils/output.rs

/// Format command output for different targets
pub trait OutputFormatter: Send + Sync {
    /// Format success message
    fn format_success(&self, message: &str) -> String;
    
    /// Format table output
    fn format_table(&self, table: &TableOutput) -> String;
    
    /// Format error with recovery hints
    fn format_error(&self, error: &Error, command: &str) -> String;
    
    /// Format structured data as JSON
    fn format_json<T: serde::Serialize>(&self, data: &T) -> Result<String, Error>;
}

/// Human-readable formatter
pub struct HumanFormatter;

impl OutputFormatter for HumanFormatter {
    fn format_success(&self, message: &str) -> String {
        format!("✔ {}", message)
    }
    
    fn format_table(&self, table: &TableOutput) -> String {
        // Use comfy-table or similar for nice terminal output
        todo!()
    }
    
    fn format_error(&self, error: &Error, command: &str) -> String {
        format!(
            "✘ Command '{}' failed:\n  {}\n\n  Recovery: See --help for usage examples",
            command, error
        )
    }
    
    fn format_json<T: serde::Serialize>(&self, data: &T) -> Result<String, Error> {
        serde_json::to_string_pretty(data)
            .map_err(|e| DomainError::Transform { message: e.to_string() }.into())
    }
}
```

### 3.3 Tracing Integration Trait

```rust
// src/tracing_setup.rs

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Configure tracing for command execution
pub fn setup_tracing(
    verbosity: u8,
    log_path: Option<&Path>,
) -> Result<tracing_appender::non_blocking::WorkerGuard, Error> {
    let env_filter = match verbosity {
        0 => EnvFilter::new("warn"),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };
    
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(true);
    
    let (file_guard, file_layer) = if let Some(path) = log_path {
        let file_appender = tracing_appender::rolling::never(
            path.parent().unwrap_or(Path::new(".")),
            path.file_name().unwrap_or_default(),
        );
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let layer = tracing_subscriber::fmt::layer()
            .with_writer(non_blocking)
            .with_ansi(false);
        (Some(guard), Some(layer))
    } else {
        (None, None)
    };
    
    let registry = tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt_layer);
    
    if let Some(file_layer) = file_layer {
        registry.with(file_layer).init();
    } else {
        registry.init();
    }
    
    file_guard.ok_or_else(|| {
        DomainError::Internal { message: "Failed to initialize tracing".to_string() }.into()
    })
}
```

---

## 4. Error Handling Strategy

### 4.1 Unified Error Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                      ERROR HANDLING FLOW                        │
└─────────────────────────────────────────────────────────────────┘

Command Execution:
    ┌─────────────────┐
    │  User Input     │
    └────────┬────────┘
             │
             ▼ Validation Error
    ┌─────────────────┐     ┌─────────────────────────────┐
    │ CLI Parsing     │────▶│ ploke_error::DomainError    │
    │ (clap)          │     │ - InvalidInput variant      │
    └─────────────────┘     └─────────────────────────────┘
             │ OK
             ▼
    ┌─────────────────┐
    │ Command::execute│
    └────────┬────────┘
             │
    ┌────────┴────────┐
    │                 │
    ▼                 ▼
┌─────────┐    ┌─────────────┐
│ syn_parser│   │ ploke_db    │
│ errors   │    │ errors      │
└────┬────┘    └──────┬──────┘
     │                │
     ▼                ▼
┌─────────────────────────────┐
│ Map to ploke_error::Error   │
│ - Preserve context          │
│ - Add recovery hints        │
└─────────────────────────────┘
             │
             ▼
    ┌─────────────────┐
    │ Recovery Advice │────▶ stdout with helpful message
    │ Output          │       and pointer to tracing log
    └─────────────────┘
```

### 4.2 Error Context Enrichment

```rust
// src/error.rs
use ploke_error::{Error, DomainError, ContextExt};

/// Extension trait for enriching errors with command context
pub trait CommandErrorExt<T> {
    /// Add context about which command failed
    fn with_command_context(self, command: &str, args: &[String]) -> Result<T, Error>;
    
    /// Add recovery suggestion
    fn with_recovery_hint(self, hint: &str) -> Result<T, Error>;
}

impl<T> CommandErrorExt<T> for Result<T, Error> {
    fn with_command_context(self, command: &str, args: &[String]) -> Result<T, Error> {
        self.map_err(|e| {
            let ctx = format!("Command: {} {:?}", command, args);
            e.with_context(ctx)
        })
    }
    
    fn with_recovery_hint(self, hint: &str) -> Result<T, Error> {
        self.map_err(|e| {
            // Wrap in domain error with recovery info
            DomainError::Internal { 
                message: format!("{}\n\nRecovery: {}", e, hint) 
            }.into()
        })
    }
}

/// Map external crate errors to ploke_error
pub fn map_syn_parser_error(e: syn_parser::SynParserError) -> Error {
    match e {
        SynParserError::PartialParsing { successes, errors } => {
            DomainError::Transform {
                message: format!(
                    "Partial parsing: {} succeeded, {} failed\nErrors: {:?}",
                    successes.0.len(), errors.len(), errors
                ),
            }.into()
        }
        other => DomainError::Transform { message: other.to_string() }.into()
    }
}

pub fn map_db_error(e: ploke_db::DbError, operation: &str) -> Error {
    DomainError::Database {
        message: format!("Database operation '{}' failed: {}", operation, e),
    }.into()
}

pub fn map_transform_error(e: ploke_transform::TransformError) -> Error {
    DomainError::Transform { message: e.to_string() }.into()
}
```

### 4.3 Recovery Path Mapping

| Error Source | Recovery Hint Example |
|--------------|----------------------|
| Missing fixture | `"Run 'cargo xtask setup test-env --fixture <id>' to initialize"` |
| Parse failure | `"Check that path contains a valid Cargo.toml with [package] section"` |
| DB query error | `"Check database schema with 'cargo xtask db check-schema'"` |
| Missing API key | `"Set TEST_OPENROUTER_API_KEY environment variable"` |
| Invalid node ID | `"Use 'cargo xtask parse list-items <path>' to see valid IDs"` |

---

## 5. Example: Command Flow Through Architecture

### 5.1 Parse Command Flow

```
User runs: cargo xtask parse phases-merge ./my-crate --stats

┌─────────────────────────────────────────────────────────────────┐
│ 1. CLI PARSING (main.rs)                                        │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ Cli::parse() → Commands::Parse(ParseCommands::PhasesMerge {     │
│     crate_path: "./my-crate",                                   │
│     stats: true                                                 │
│ })                                                               │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 2. COMMAND DISPATCH (commands/mod.rs)                           │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ let command = ParsePhasesMergeCommand {                         │
│     path: crate_path,                                           │
│     show_stats: stats,                                          │
│ };                                                               │
│ command.execute(&ctx).await                                     │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 3. TRACING SPAN (tracing_setup.rs)                              │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ #[tracing::instrument(skip(self, ctx), fields(                  │
│     command = "parse phases-merge",                             │
│     path = %self.path,                                          │
│     stats = self.show_stats                                     │
│ ))]                                                              │
│ async fn execute(&self, ctx: &CommandContext) -> ...            │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 4. BUSINESS LOGIC (commands/parse.rs)                           │
└─────────────────────────────────────────────────────────────────┘
    │
    ├──▶ Validate path exists
    ├──▶ Call syn_parser::try_run_phases_and_merge(&path)
    │    └── Returns ParserOutput
    ├──▶ If stats: extract graph and compute statistics
    │    └── functions: 42, types: 15, modules: 8, etc.
    └──▶ Format output
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ 5. OUTPUT FORMATTING (feedback.rs)                              │
└─────────────────────────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────────────────────────┐
│ Human format:                                                   │
│ ✔ Successfully parsed crate at ./my-crate                       │
│                                                                 │
│ Statistics:                                                     │
│   Functions: 42                                                 │
│   Types: 15                                                     │
│   Modules: 8                                                    │
│   Relations: 156                                                │
│                                                                 │
│ Tracing log: /tmp/xtask-2026-03-25-abc123.log                   │
└─────────────────────────────────────────────────────────────────┘
```

### 5.2 Pipeline Command Flow (Cross-Crate)

```rust
// Example: pipeline parse-transform

#[derive(Debug)]
pub struct ParseTransformPipeline {
    pub crate_path: PathBuf,
    pub output_path: Option<PathBuf>,
}

#[async_trait::async_trait]
impl Command for ParseTransformPipeline {
    #[tracing::instrument(skip(self, ctx))]
    async fn execute(&self, ctx: &CommandContext) -> Result<CommandOutput, Error> {
        // Stage 1: Parse
        tracing::info!(stage = "parse", "Starting parse phase");
        let mut parser_output = try_run_phases_and_merge(&self.crate_path)
            .map_err(map_syn_parser_error)
            .with_recovery_hint(
                "Ensure the path points to a valid Rust crate with Cargo.toml"
            )?;
        
        let merged_graph = parser_output.extract_merged_graph()
            .ok_or_else(|| DomainError::Transform {
                message: "Failed to extract merged graph".to_string()
            })?;
        let module_tree = parser_output.extract_module_tree()
            .ok_or_else(|| DomainError::Transform {
                message: "Failed to extract module tree".to_string()
            })?;
        
        tracing::info!("Parse complete: {} functions", merged_graph.functions().len());
        
        // Stage 2: Initialize Database
        tracing::info!(stage = "db_init", "Initializing database");
        let db = Arc::new(Database::init_with_schema()
            .map_err(|e| map_db_error(e, "init_with_schema"))?);
        
        // Stage 3: Create Schema
        tracing::info!(stage = "schema", "Creating database schema");
        ploke_transform::schema::create_schema_all(&db)
            .map_err(map_transform_error)?;
        
        // Stage 4: Transform
        tracing::info!(stage = "transform", "Transforming graph to database");
        transform_parsed_graph(&db, merged_graph, &module_tree)
            .map_err(map_transform_error)?;
        
        // Stage 5: Optional save
        if let Some(output) = &self.output_path {
            tracing::info!(path = %output.display(), "Saving database backup");
            db.backup_db(output)
                .map_err(|e| DomainError::Database {
                    message: format!("Failed to save backup: {}", e)
                })?;
        }
        
        // Generate output
        let stats = db_stats(&db).await?;
        Ok(CommandOutput::Data(Box::new(stats)))
    }
}
```

---

## 6. Tracing Integration

### 6.1 Tracing Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    TRACING ARCHITECTURE                         │
└─────────────────────────────────────────────────────────────────┘

Global Subscriber (initialized once in main):
    ┌─────────────────────────────────────────┐
    │  EnvFilter (controls verbosity)         │
    │  - RUST_LOG=info                        │
    │  - xtask=debug                          │
    └─────────────────────────────────────────┘
                    │
        ┌───────────┴───────────┐
        ▼                       ▼
┌───────────────┐       ┌───────────────┐
│  Console Layer│       │  File Layer   │
│  (formatted)  │       │  (JSON/compact│
│               │       │   for agents) │
└───────────────┘       └───────────────┘

Command-Level Spans:
    #[tracing::instrument(
        name = "command",
        fields(
            cmd_name = %self.name(),
            cmd_category = "parse",
        ),
        skip(self, ctx)
    )]
```

### 6.2 Tracing in Commands

```rust
// Example with comprehensive tracing

#[tracing::instrument(
    level = "info",
    skip(ctx),
    fields(
        command = "db_query",
        query_length = query.len(),
    )
)]
pub async fn execute_db_query(
    ctx: &CommandContext,
    query: &str,
    db: &Database,
) -> Result<QueryResult, Error> {
    tracing::debug!(query = %query, "Executing CozoDB query");
    
    let start = std::time::Instant::now();
    let result = db.run_script(query, Default::default(), ScriptMutability::Immutable)
        .map_err(|e| {
            tracing::error!(error = %e, "Query execution failed");
            map_db_error(e, "run_script")
        })?;
    
    let elapsed = start.elapsed();
    tracing::info!(
        rows = result.rows.len(),
        elapsed_ms = elapsed.as_millis(),
        "Query completed"
    );
    
    Ok(result)
}
```

### 6.3 Agent-Friendly Trace Output

```rust
// File layer outputs structured data for agent consumption
{
    "timestamp": "2026-03-25T12:34:56Z",
    "level": "INFO",
    "target": "xtask::commands::parse",
    "span": {
        "name": "parse_workspace",
        "workspace": "ploke",
        "crates": 12
    },
    "fields": {
        "event": "parse_complete",
        "total_functions": 1542,
        "total_types": 387,
        "elapsed_ms": 2847
    }
}
```

---

## 7. Pros and Cons of This Approach

### 7.1 Pros

| Advantage | Explanation |
|-----------|-------------|
| **Type Safety** | Command enums ensure only valid command combinations can be constructed |
| **Modularity** | Per-crate modules allow parallel development and clear ownership |
| **Testability** | Trait-based design enables easy mocking for unit tests |
| **Extensibility** | New commands just implement `Command` trait; no dispatch code changes |
| **Consistency** | Unified error handling and output formatting across all commands |
| **Observability** | Comprehensive tracing provides debugging info for agents |
| **Documentation** | Clap derives generate help text automatically |

### 7.2 Cons

| Disadvantage | Explanation | Mitigation |
|--------------|-------------|------------|
| **Boilerplate** | Each command requires enum variant + struct + trait impl | Use macros for repetitive patterns |
| **Binary Size** | Clap and async trait add dependencies | Feature-gate heavy dependencies |
| **Complexity** | Multiple layers (CLI → Command → Logic) | Clear module docs, examples |
| **Async Overhead** | Most commands are sync but wrapped in async | Use `block_on` for sync commands |

### 7.3 Comparison with Alternatives

| Approach | Pros | Cons |
|----------|------|------|
| **This Proposal** (enums + traits) | Type-safe, extensible, testable | More boilerplate |
| **Simple Functions** | Simple, less code | No shared behavior, harder to test |
| **Macro-based DSL** | Very concise | Harder to understand/debug |
| **Dynamic Dispatch** | Very flexible | Runtime errors, less IDE support |

---

## 8. Implementation Phases

### Phase 1: Foundation (M.3)
1. Set up module structure
2. Implement `Command` trait and core types
3. Add tracing infrastructure
4. Migrate existing `profile_ingest` as proof of concept

### Phase 2: Core Commands (M.4)
1. Implement parse commands (A.1)
2. Implement transform commands (A.2)
3. Implement db commands (A.4)
4. Add validation commands

### Phase 3: Advanced Features (M.5)
1. Pipeline commands
2. Ingest/embedding commands (A.3)
3. TUI headless commands (A.5)
4. Direct tool execution (A.6)

### Phase 4: Polish
1. Comprehensive testing
2. Documentation
3. Performance optimization

---

## 9. Open Questions

1. **Should we use `clap` derive macros or builder API?** Derive is more ergonomic but less flexible.

2. **How should we handle async for sync commands?** Always use async trait or have separate sync/async traits?

3. **Should pipeline stages be composable at runtime?** This would allow users to build custom pipelines.

4. **What's the story for command history/undo?** Not required for MVP but could be useful.

---

*This architecture proposal is designed to meet the requirements in PRIMARY_TASK_SPEC.md Sections A-G while providing a solid foundation for future extension.*
