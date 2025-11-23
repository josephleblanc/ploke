#![cfg(feature = "test_setup")]

//! Helper binary to rebuild fixture backups directly from the canonical fixture crates.

use std::{
    env,
    error::Error,
    fmt, fs,
    io::Read,
    path::{Path, PathBuf},
};

use chrono::Utc;
use ploke_db::Database;
use ploke_test_utils::{ setup_db_full, fixtures_crates_dir };
#[cfg(feature = "multi_embedding_schema")]
use ploke_test_utils::{ MULTI_EMBED_SCHEMA_TAG,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

const DEFAULT_FIXTURE: &str = "fixture_nodes";
const FIXTURE_CHOICES: &[&str] = &[
    "duplicate_name_fixture_1",
    "duplicate_name_fixture_2",
    "example_crate",
    "file_dir_detection",
    "fixture_attributes",
    "fixture_conflation",
    "fixture_cyclic_types",
    "fixture_edge_cases",
    "fixture_generics",
    "fixture_macros",
    "fixture_nodes",
    "fixture_path_resolution",
    "fixture_spp_edge_cases",
    "fixture_spp_edge_cases_no_cfg",
    "fixture_tracking_hash",
    "fixture_types",
    "fixture_update_embed",
    "simple_crate",
    "subdir",
];

fn main() -> Result<(), Box<dyn Error>> {
    let cli = CliArgs::parse()?;
    let fixture = resolve_fixture(cli.fixture_name.as_deref().unwrap_or(DEFAULT_FIXTURE))?;
    let output_root = env::var("PLOKE_FIXTURE_BACKUP_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("tests/backup_dbs"));

    println!(
        "Building `{variant}` fixture database for `{fixture}` …",
        variant = cli.schema_variant
    );
    let raw_db = setup_db_full(fixture)?;
    let database = Database::new(raw_db);

    let base_name = database.get_crate_name_id(fixture)?;
    let backup_name = format!("{}-{}", cli.schema_variant, &base_name);
    fs::create_dir_all(&output_root)?;
    let output_path = output_root.join(&backup_name);
    if output_path.exists() {
        println!("Removing existing backup {}", output_path.display());
        fs::remove_file(&output_path)?;
    }
    println!("Writing backup to {}", output_path.display());
    database
        .backup_db(&output_path)
        .map_err(|err| format!("failed to write backup: {err}"))?;

    write_metadata(fixture, &output_root, &backup_name, cli.schema_variant)?;
    println!("Done. Backup file updated at {}", output_path.display());
    Ok(())
}

fn resolve_fixture(name: &str) -> Result<&'static str, Box<dyn Error>> {
    FIXTURE_CHOICES
        .iter()
        .copied()
        .find(|candidate| *candidate == name)
        .ok_or_else(|| {
            format!(
                "Unknown fixture `{name}`. Available options: {choices}",
                choices = FIXTURE_CHOICES.join(", ")
            )
            .into()
        })
}

struct CliArgs {
    fixture_name: Option<String>,
    schema_variant: SchemaVariant,
}

impl CliArgs {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut args = env::args().skip(1);
        let mut fixture_name = None;
        let mut schema_variant = SchemaVariant::MultiEmbedding;

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--schema" => {
                    let value = args.next().ok_or("Expected value after --schema")?;
                    schema_variant = SchemaVariant::from_str(&value)?;
                }
                "--legacy" => schema_variant = SchemaVariant::Legacy,
                "--multi" | "--multi-embedding" => schema_variant = SchemaVariant::MultiEmbedding,
                "--help" | "-h" => {
                    CliArgs::print_usage();
                    std::process::exit(0);
                }
                _ if fixture_name.is_none() => fixture_name = Some(arg),
                _ => {
                    return Err(format!("Unexpected argument `{arg}`").into());
                }
            }
        }

        Ok(Self {
            fixture_name,
            schema_variant,
        })
    }

    fn print_usage() {
        eprintln!(
            "Usage: cargo run -p ploke-test-utils --bin regenerate_fixture [fixture_name] [--schema <legacy|multi>]\n\
             Defaults: fixture_name={DEFAULT_FIXTURE}, schema=multi"
        );
    }
}

#[derive(Clone, Copy)]
enum SchemaVariant {
    Legacy,
    MultiEmbedding,
}

impl SchemaVariant {
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "legacy" => Ok(Self::Legacy),
            "multi" | "multi-embedding" | "multi_embedding" => Ok(Self::MultiEmbedding),
            other => Err(format!(
                "Unknown schema variant `{other}`. Use `legacy` or `multi`."
            )),
        }
    }

    fn metadata_notes(self) -> &'static str {
        match self {
            SchemaVariant::Legacy => {
                "Legacy backup containing single-embedding columns for compatibility tests."
            }
            SchemaVariant::MultiEmbedding => {
                "Multi-embedding backup with per-dimension relations seeded from experiment specs."
            }
        }
    }
}

impl fmt::Display for SchemaVariant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaVariant::Legacy => write!(f, "legacy"),
            SchemaVariant::MultiEmbedding => write!(f, "newer"),
        }
    }
}

#[derive(Serialize)]
struct FixtureMetadata {
    fixture_dir: String,
    tree_sha256: String,
    generated_at: String,
    schema_variant: String,
    notes: String,
}

fn write_metadata(
    fixture: &str,
    output_root: &Path,
    backup_name: &str,
    schema_variant: SchemaVariant,
) -> Result<(), Box<dyn Error>> {
    let fixture_dir = fixtures_crates_dir().join(fixture);
    let hash = compute_directory_hash(&fixture_dir)?;
    let metadata = FixtureMetadata {
        fixture_dir: format!("tests/fixture_crates/{fixture}"),
        tree_sha256: hash,
        generated_at: Utc::now().to_rfc3339(),
        schema_variant: schema_variant.to_string(),
        notes: schema_variant.metadata_notes().to_string(),
    };

    let metadata_path = output_root.join(format!("{backup_name}.meta.json"));
    fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;
    println!("Recorded metadata at {}", metadata_path.display());
    Ok(())
}

fn compute_directory_hash(dir: &Path) -> Result<String, Box<dyn Error>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter() {
        let entry = entry?;
        if entry.file_type().is_file() {
            let rel = entry.path().strip_prefix(dir)?.to_path_buf();
            files.push(rel);
        }
    }
    files.sort();

    let mut hasher = Sha256::new();
    for rel in files {
        let rel_str = rel.to_string_lossy();
        hasher.update(rel_str.as_bytes());
        hasher.update([0]);

        let full_path = dir.join(&rel);
        let mut file = fs::File::open(&full_path)?;

        let mut buffer = [0u8; 8192];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        hasher.update([0xFF]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
