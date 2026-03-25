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

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use std::time::Instant;

use cozo::DataValue;
use ploke_db::Database;
use ploke_test_utils::fixture_dbs::backup_db_fixture;

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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;
        if let Some(parent) = self.output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        db.write_backup_to_path(&self.output)?;
        let msg = if self.compress {
            format!(
                "Wrote Cozo backup to {} (compress flag not yet applied to export format)",
                self.output.display()
            )
        } else {
            format!("Wrote Cozo backup to {}", self.output.display())
        };
        Ok(DbOutput::Success {
            message: msg,
            path: Some(self.output.clone()),
        })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let _db = ctx.get_database(Some(self.path.as_path()))?;
        if self.verify {
            let rels = _db.relations_vec()?;
            if rels.is_empty() {
                return Err(XtaskError::Database(
                    "Load verify failed: database has no relations after import".into(),
                ));
            }
        }
        Ok(DbOutput::Success {
            message: format!("restored database from {}", self.path.display()),
            path: Some(self.path.clone()),
        })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let fixture = backup_db_fixture(self.fixture.trim()).ok_or_else(|| {
            let ids = ploke_test_utils::fixture_dbs::BACKUP_DB_FIXTURES
                .iter()
                .map(|f| f.id)
                .collect::<Vec<_>>()
                .join(", ");
            XtaskError::validation(format!("Unknown fixture id `{}`", self.fixture)).with_recovery(
                format!("Use a registered fixture id from docs/testing/BACKUP_DB_FIXTURES.md (examples: {ids})."),
            )
        })?;

        let _db = ctx.get_database_from_fixture(fixture)?;
        if self.verify {
            let rels = _db.relations_vec()?;
            if rels.is_empty() {
                return Err(XtaskError::Database(
                    "Fixture verify failed: no relations after import".into(),
                ));
            }
        }

        let index_note = if self.index {
            " HNSW primary index was ensured during fixture import."
        } else {
            ""
        };
        Ok(DbOutput::Success {
            message: format!("Loaded fixture `{}`.{}", self.fixture, index_note),
            path: Some(fixture.path()),
        })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;

        let pending_embeddings = if self.pending {
            Some(db.count_pending_embeddings().unwrap_or(0))
        } else {
            None
        };

        let mut by_kind: HashMap<String, usize> = HashMap::new();

        let total = match self.kind {
            None | Some(NodeKind::All) => {
                let mut sum = 0usize;
                for (label, rel) in node_relation_pairs() {
                    if let Ok(n) = count_relation_rows(&db, rel) {
                        if n > 0 {
                            by_kind.insert((*label).to_string(), n);
                            sum += n;
                        }
                    }
                }
                sum
            }
            Some(NodeKind::Function) => {
                let n = count_relation_rows(&db, "function")?;
                by_kind.insert("Function".to_string(), n);
                n
            }
            Some(NodeKind::Type) => {
                let mut sum = 0usize;
                for (label, rel) in [
                    ("Struct", "struct"),
                    ("Enum", "enum"),
                    ("Union", "union"),
                    ("TypeAlias", "type_alias"),
                ] {
                    if let Ok(n) = count_relation_rows(&db, rel) {
                        if n > 0 {
                            by_kind.insert(label.to_string(), n);
                            sum += n;
                        }
                    }
                }
                sum
            }
            Some(NodeKind::Module) => {
                let n = count_relation_rows(&db, "module")?;
                by_kind.insert("Module".to_string(), n);
                n
            }
        };

        Ok(DbOutput::NodeCount {
            total,
            by_kind,
            pending_embeddings,
        })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;
        let params = query_params_map(&self.param);
        let start = Instant::now();
        let qr = if self.mutable {
            if params.is_empty() {
                db.raw_query_mut(&self.query)
            } else {
                db.raw_query_mut_params(&self.query, params)
            }
        } else if params.is_empty() {
            db.raw_query(&self.query)
        } else {
            db.raw_query_params(&self.query, params)
        }
        .map_err(|e| cozo_error_with_query(e, &self.query))?;
        let duration_ms = start.elapsed().as_millis() as u64;
        let columns = qr.headers.clone();
        let rows = data_values_to_json_rows(&qr);
        Ok(DbOutput::QueryResult {
            rows,
            columns,
            duration_ms,
        })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;
        let rels = db.relations_vec()?;
        let category = format!("{:?}", self.category);
        let data = match self.category {
            StatsCategory::All => serde_json::json!({
                "relation_count": rels.len(),
            }),
            StatsCategory::Nodes => serde_json::json!({
                "note": "Use db count-nodes for node counts",
            }),
            StatsCategory::Relations => serde_json::json!({
                "relation_count": rels.len(),
            }),
            StatsCategory::Embeddings => serde_json::json!({
                "pending_embeddings": db.count_pending_embeddings().unwrap_or(0),
            }),
            StatsCategory::Indexes => serde_json::json!({
                "hnsw_indices": rels.iter().filter(|r| r.ends_with(":hnsw_idx")).count(),
            }),
        };
        Ok(DbOutput::DatabaseStats { category, data })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;
        let names = db.relations_vec()?;
        let relations = names
            .into_iter()
            .filter(|n| !self.no_hnsw || !n.ends_with(":hnsw_idx"))
            .map(|name| {
                let row_count = if self.counts {
                    count_relation_rows(&db, &name).ok()
                } else {
                    None
                };
                RelationInfo {
                    is_hnsw: name.ends_with(":hnsw_idx"),
                    name,
                    row_count,
                }
            })
            .collect();
        Ok(DbOutput::RelationsList { relations })
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

    fn execute(&self, ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let db = open_db(ctx, self.db.as_ref())?;
        let pending = db.count_pending_embeddings()?;
        let functions = count_relation_rows(&db, "function").unwrap_or(0);
        Ok(DbOutput::EmbeddingStatus {
            total_nodes: functions,
            embedded: functions.saturating_sub(pending),
            pending,
            sets: vec![],
        })
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

fn open_db(ctx: &CommandContext, db: Option<&PathBuf>) -> Result<Arc<Database>, XtaskError> {
    ctx.get_database(db.map(PathBuf::as_path))
}

fn count_relation_rows(db: &Database, relation: &str) -> Result<usize, XtaskError> {
    let script = format!("?[id] := *{relation} {{ id }}");
    let qr = db
        .raw_query(&script)
        .map_err(|e| XtaskError::Database(e.to_string()))?;
    Ok(qr.rows.len())
}

fn node_relation_pairs() -> &'static [(&'static str, &'static str)] {
    &[
        ("Function", "function"),
        ("Module", "module"),
        ("Enum", "enum"),
        ("Trait", "trait"),
        ("Impl", "impl"),
        ("Const", "const"),
        ("Static", "static"),
        ("Macro", "macro"),
        ("Struct", "struct"),
        ("TypeAlias", "type_alias"),
        ("Union", "union"),
        ("Import", "import"),
    ]
}

fn cozo_error_with_query(e: impl std::fmt::Display, query: &str) -> XtaskError {
    XtaskError::Database(format!(
        "cozo query error: {e} | your input query: {query:?} | underlying: ploke_db::Database::raw_query / raw_query_mut"
    ))
}

fn query_params_map(param: &[(String, String)]) -> BTreeMap<String, DataValue> {
    param
        .iter()
        .map(|(k, v)| (k.clone(), DataValue::Str(v.clone().into())))
        .collect()
}

fn data_values_to_json_rows(qr: &ploke_db::QueryResult) -> Vec<serde_json::Value> {
    qr.rows
        .iter()
        .map(|row| {
            serde_json::Value::Array(
                row.iter()
                    .map(|c| serde_json::Value::String(format!("{c:?}")))
                    .collect(),
            )
        })
        .collect()
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
