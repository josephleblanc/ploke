use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use clap::Parser;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

/// CLI for running semantic (HNSW) queries against a restored Cozo DB snapshot.
/// Initial iteration: restore DB, load embedding vector (optional), and write a Markdown scaffold.
#[derive(Parser, Debug)]
#[command(name = "emb_query", version, about = "Semantic query harness for ploke-db")]
struct Args {
    /// Path to the SQLite backup that will be restored into an in-memory Cozo database
    #[arg(long)]
    db: PathBuf,

    /// Path to a JSON file containing a single embedding vector (e.g., [0.1, 0.2, ...])
    #[arg(long)]
    embedding_file: Option<PathBuf>,

    /// Comma-separated list of node types to search (e.g., function,struct,enum)
    #[arg(long)]
    types: Option<String>,

    /// Number of results to return
    #[arg(long, default_value_t = 20)]
    top_k: usize,

    /// Output Markdown file path (default: outputs/semantic/<unix_ts>.md)
    #[arg(long)]
    out: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    init_tracing();

    let args = Args::parse();
    info!("Starting emb_query with args: {:?}", args);

    // Determine output path
    let out_path = match args.out {
        Some(p) => p,
        None => {
            let ts = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            PathBuf::from(format!("outputs/semantic/{}.md", ts))
        }
    };

    if let Some(parent) = out_path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            error!("Failed to create output directory {}: {}", parent.display(), e);
            std::process::exit(1);
        }
    }

    // Load embedding if provided
    let mut embedding_dim: Option<usize> = None;
    if let Some(path) = &args.embedding_file {
        match fs::read_to_string(path) {
            Ok(s) => match serde_json::from_str::<Vec<f32>>(&s) {
                Ok(vec) => {
                    embedding_dim = Some(vec.len());
                    info!("Loaded embedding vector with dimension {}", vec.len());
                }
                Err(e) => {
                    error!("Failed to parse embedding JSON at {}: {}", path.display(), e);
                    std::process::exit(1);
                }
            },
            Err(e) => {
                error!("Failed to read embedding file {}: {}", path.display(), e);
                std::process::exit(1);
            }
        }
    } else {
        info!("No embedding file provided; report will be created without query execution.");
    }

    // Restore the in-memory DB from the provided backup
    // Note: Based on repository search, Database::create_new_backup exists and performs a restore.
    let db_res = ploke_db::database::Database::create_new_backup(&args.db).await;
    if let Err(e) = db_res {
        error!("Failed to restore database from backup {}: {:?}", args.db.display(), e);
        std::process::exit(1);
    }
    info!("Database restored successfully from {}", args.db.display());

    // Write a Markdown report scaffold (query execution will be wired in next iteration)
    let mut md = String::new();
    md.push_str("# Semantic Query Report (Prototype)\n\n");
    md.push_str(&format!("- Database backup: {}\n", args.db.display()));
    if let Some(t) = &args.types {
        md.push_str(&format!("- Types: {}\n", t));
    } else {
        md.push_str("- Types: (default — all primary)\n");
    }
    md.push_str(&format!("- Top-K: {}\n", args.top_k));
    if let Some(dim) = embedding_dim {
        md.push_str(&format!("- Embedding dimension: {}\n", dim));
    } else {
        md.push_str("- Embedding: not provided\n");
    }
    md.push_str("\n---\n\n");
    md.push_str("> Note: This is the initial harness. The HNSW query execution and snippet rendering\n");
    md.push_str("> will be connected in the next step once we integrate the existing query functions.\n");

    if let Err(e) = write_string_file(&out_path, &md) {
        error!("Failed to write report to {}: {}", out_path.display(), e);
        std::process::exit(1);
    }
    info!("Wrote report to {}", out_path.display());
}

fn write_string_file(path: &PathBuf, contents: &str) -> std::io::Result<()> {
    let mut f = fs::File::create(path)?;
    f.write_all(contents.as_bytes())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
