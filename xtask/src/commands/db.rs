//! Database commands for ploke_db integration (A.4)
//!
//! This module provides commands for database operations:
//! - Backup and restore
//! - Fixture loading
//! - Node counting and statistics
//! - Index management (HNSW, BM25)
//! - Query execution
//!
//! ## Commands
//!
//! - `db save` - Save database to backup file
//! - `db load` - Load database from backup file
//! - `db load-fixture` - Load a fixture database
//! - `db count-nodes` - Count nodes in database
//! - `db hnsw-build` - Build HNSW index
//! - `db hnsw-rebuild` - Rebuild HNSW index
//! - `db bm25-rebuild` - Rebuild BM25 index
//! - `db query` - Execute CozoDB query
//! - `db stats` - Show database statistics
//! - `db list-relations` - List relations in database
//! - `db embedding-status` - Show embedding status

use super::{CommandContext, OutputFormat, XtaskError};
use crate::executor::Command;
use std::path::PathBuf;

/// Database command enum with all subcommands
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Db {
    /// Save database to backup file
    Save(Save),

    /// Load database from backup file
    Load(Load),

    /// Load a fixture database
    LoadFixture(LoadFixture),

    /// Count nodes in database
    CountNodes(CountNodes),

    /// Build HNSW index
    HnswBuild(HnswBuild),

    /// Rebuild HNSW index
    HnswRebuild(HnswRebuild),

    /// Rebuild BM25 index
    Bm25Rebuild(Bm25Rebuild),

    /// Execute arbitrary CozoDB query
    Query(Query),

    /// Show database statistics
    Stats(Stats),

    /// List relations in database
    ListRelations(ListRelations),

    /// Show embedding status
    EmbeddingStatus(EmbeddingStatus),
}

impl Db {
    /// Execute the database command
    pub fn execute(&self, ctx: &CommandContext) -> Result<DbOutput, XtaskError> {
        match self {
            Db::Save(cmd) => cmd.execute(ctx),
            Db::Load(cmd) => cmd.execute(ctx),
            Db::LoadFixture(cmd) => cmd.execute(ctx),
            Db::CountNodes(cmd) => cmd.execute(ctx),
            Db::HnswBuild(cmd) => cmd.execute(ctx),
            Db::HnswRebuild(cmd) => cmd.execute(ctx),
            Db::Bm25Rebuild(cmd) => cmd.execute(ctx),
            Db::Query(cmd) => cmd.execute(ctx),
            Db::Stats(cmd) => cmd.execute(ctx),
            Db::ListRelations(cmd) => cmd.execute(ctx),
            Db::EmbeddingStatus(cmd) => cmd.execute(ctx),
        }
    }
}

/// Save database command
///
/// Creates a backup of the current database state.
#[derive(Debug, Clone, clap::Args)]
pub struct Save {
    /// Database path (default: active)
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Output backup file path
    #[arg(value_name = "OUTPUT_PATH")]
    pub output: PathBuf,

    /// Compress the backup
    #[arg(long)]
    pub compress: bool,
}

impl Command for Save {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db save"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Save command implementation")
    }
}

/// Load database command
///
/// Restores a database from a backup file.
#[derive(Debug, Clone, clap::Args)]
pub struct Load {
    /// Backup file path
    #[arg(value_name = "BACKUP_PATH")]
    pub path: PathBuf,

    /// Target database path (default: new in-memory)
    #[arg(short, long, value_name = "PATH")]
    pub target: Option<PathBuf>,

    /// Verify after loading
    #[arg(long)]
    pub verify: bool,
}

impl Command for Load {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db load"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Load command implementation")
    }
}

/// Load fixture command
///
/// Loads a predefined test fixture database.
#[derive(Debug, Clone, clap::Args)]
pub struct LoadFixture {
    /// Fixture identifier
    #[arg(value_name = "FIXTURE_ID")]
    pub fixture: String,

    /// Create HNSW index after loading
    #[arg(long)]
    pub index: bool,

    /// Verify fixture integrity
    #[arg(long, default_value = "true")]
    pub verify: bool,
}

impl Command for LoadFixture {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db load-fixture"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("LoadFixture command implementation")
    }
}

/// Count nodes command
///
/// Counts nodes in the database, optionally filtered by type.
#[derive(Debug, Clone, clap::Args)]
pub struct CountNodes {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Count specific node types
    #[arg(long, value_enum)]
    pub kind: Option<NodeKind>,

    /// Include pending embeddings count
    #[arg(long)]
    pub pending: bool,
}

impl Command for CountNodes {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db count-nodes"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("CountNodes command implementation")
    }
}

/// HNSW build command
///
/// Builds the HNSW index for vector similarity search.
#[derive(Debug, Clone, clap::Args)]
pub struct HnswBuild {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Embedding set to index
    #[arg(long, value_name = "SET")]
    pub embedding_set: Option<String>,

    /// Number of dimensions (default: auto-detect)
    #[arg(long)]
    pub dimensions: Option<usize>,
}

impl Command for HnswBuild {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db hnsw-build"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("HnswBuild command implementation")
    }
}

/// HNSW rebuild command
///
/// Rebuilds the HNSW index from scratch.
#[derive(Debug, Clone, clap::Args)]
pub struct HnswRebuild {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Force rebuild even if index exists
    #[arg(long)]
    pub force: bool,
}

impl Command for HnswRebuild {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db hnsw-rebuild"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("HnswRebuild command implementation")
    }
}

/// BM25 rebuild command
///
/// Rebuilds the BM25 full-text search index.
#[derive(Debug, Clone, clap::Args)]
pub struct Bm25Rebuild {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Number of documents to process per batch
    #[arg(long, default_value = "1000")]
    pub batch_size: usize,
}

impl Command for Bm25Rebuild {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db bm25-rebuild"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Bm25Rebuild command implementation")
    }
}

/// Query command
///
/// Executes a CozoDB query against the database.
#[derive(Debug, Clone, clap::Args)]
pub struct Query {
    /// CozoScript query string
    #[arg(value_name = "QUERY")]
    pub query: String,

    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Query parameters (key=value)
    #[arg(short, long, value_parser = parse_key_val::<String, String>)]
    pub param: Vec<(String, String)>,

    /// Allow mutating query
    #[arg(long)]
    pub mutable: bool,

    /// Output format (overrides global)
    #[arg(long, value_enum)]
    pub output: Option<OutputFormat>,
}

impl Command for Query {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db query"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Query command implementation")
    }
}

/// Stats command
///
/// Shows comprehensive database statistics.
#[derive(Debug, Clone, clap::Args)]
pub struct Stats {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Stats category
    #[arg(value_enum, default_value = "all")]
    pub category: StatsCategory,
}

impl Command for Stats {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db stats"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("Stats command implementation")
    }
}

/// List relations command
///
/// Lists all relations (tables) in the database.
#[derive(Debug, Clone, clap::Args)]
pub struct ListRelations {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Exclude HNSW indices
    #[arg(long)]
    pub no_hnsw: bool,

    /// Include row counts
    #[arg(long)]
    pub counts: bool,
}

impl Command for ListRelations {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db list-relations"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("ListRelations command implementation")
    }
}

/// Embedding status command
///
/// Shows the status of embeddings in the database.
#[derive(Debug, Clone, clap::Args)]
pub struct EmbeddingStatus {
    /// Database path
    #[arg(short, long, value_name = "PATH")]
    pub db: Option<PathBuf>,

    /// Specific embedding set
    #[arg(long, value_name = "SET")]
    pub set: Option<String>,

    /// Show detailed per-set statistics
    #[arg(long)]
    pub detailed: bool,
}

impl Command for EmbeddingStatus {
    type Output = DbOutput;
    type Error = XtaskError;

    fn name(&self) -> &'static str {
        "db embedding-status"
    }

    fn category(&self) -> crate::executor::CommandCategory {
        crate::executor::CommandCategory::Database
    }

    fn requires_async(&self) -> bool {
        false
    }

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        // Implementation skeleton - full implementation in M.4
        todo!("EmbeddingStatus command implementation")
    }
}

/// Node kind for filtering
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum, serde::Serialize)]
pub enum NodeKind {
    /// Functions only
    Function,
    /// Types only (structs, enums, unions)
    Type,
    /// Modules only
    Module,
    /// All node types (default)
    #[default]
    All,
}

/// Stats category for filtering statistics
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum, serde::Serialize)]
pub enum StatsCategory {
    /// All statistics
    #[default]
    All,
    /// Embedding statistics only
    Embeddings,
    /// Node counts only
    Nodes,
    /// Relation counts only
    Relations,
    /// Index information only
    Indexes,
}

/// Output type for database commands
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum DbOutput {
    /// Success with message
    Success {
        message: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        path: Option<PathBuf>,
    },
    /// Node count output
    NodeCount {
        total: usize,
        by_kind: std::collections::HashMap<String, usize>,
        pending_embeddings: Option<usize>,
    },
    /// Query result output
    QueryResult {
        rows: Vec<serde_json::Value>,
        columns: Vec<String>,
        duration_ms: u64,
    },
    /// Statistics output
    DatabaseStats {
        category: String,
        data: serde_json::Value,
    },
    /// Relations list output
    RelationsList {
        relations: Vec<RelationInfo>,
    },
    /// Embedding status output
    EmbeddingStatus {
        total_nodes: usize,
        embedded: usize,
        pending: usize,
        sets: Vec<EmbeddingSetInfo>,
    },
}

/// Relation information
#[derive(Debug, Clone, serde::Serialize)]
pub struct RelationInfo {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_count: Option<usize>,
    pub is_hnsw: bool,
}

/// Embedding set information
#[derive(Debug, Clone, serde::Serialize)]
pub struct EmbeddingSetInfo {
    pub name: String,
    pub dimensions: usize,
    pub model: String,
    pub count: usize,
}

/// Parse a key=value pair for command line arguments
fn parse_key_val<T, E>(s: &str) -> Result<(T, E), Box<dyn std::error::Error + Send + Sync>>
where
    T: std::str::FromStr,
    E: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
    E::Err: std::error::Error + Send + Sync + 'static,
{
    let pos = s
        .find('=')
        .ok_or_else(|| format!("invalid key=value: no `=` found in `{s}`"))?;
    Ok((s[..pos].parse()?, s[pos + 1..].parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_nodes_fields() {
        let cmd = CountNodes {
            db: Some(PathBuf::from("/test.db")),
            kind: Some(NodeKind::Function),
            pending: true,
        };
        assert_eq!(cmd.name(), "db count-nodes");
        assert!(!cmd.requires_async());
    }

    #[test]
    fn test_query_params() {
        let cmd = Query {
            query: "*relation[node]".to_string(),
            db: None,
            param: vec![("limit".to_string(), "10".to_string())],
            mutable: false,
            output: Some(OutputFormat::Json),
        };
        assert_eq!(cmd.param.len(), 1);
    }

    #[test]
    fn test_db_output_serialization() {
        let output = DbOutput::Success {
            message: "Backup created".to_string(),
            path: Some(PathBuf::from("/backup.cozo")),
        };

        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("Backup created"));
    }

    #[test]
    fn test_parse_key_val() {
        let result = parse_key_val::<String, i32>("limit=10");
        assert!(result.is_ok());
        let (k, v) = result.unwrap();
        assert_eq!(k, "limit");
        assert_eq!(v, 10);
    }

    #[test]
    fn test_parse_key_val_invalid() {
        let result = parse_key_val::<String, String>("invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_embedding_set_info() {
        let info = EmbeddingSetInfo {
            name: "default".to_string(),
            dimensions: 384,
            model: "all-MiniLM-L6-v2".to_string(),
            count: 1000,
        };

        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("all-MiniLM-L6-v2"));
        assert!(json.contains("384"));
    }
}
