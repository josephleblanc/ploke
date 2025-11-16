use ploke_common::{
    LEGACY_FIXTURE_BACKUP_REL_PATH, LEGACY_FIXTURE_METADATA_REL_PATH,
    MULTI_EMBED_FIXTURE_BACKUP_REL_PATH, MULTI_EMBED_FIXTURE_METADATA_REL_PATH,
};
use ploke_db::{
    multi_embedding::{
        experimental_node_relation_specs, vector_dimension_specs, ExperimentalEmbeddingDbExt,
        ExperimentalVectorRelation,
    },
    Database, DbError,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::ExitCode,
};
use walkdir::WalkDir;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("verify-fixtures") => verify_fixtures(args.collect()),
        Some("help") | Some("-h") | Some("--help") => {
            print_usage();
            ExitCode::SUCCESS
        }
        None => {
            print_usage();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("Unknown command '{other}'.");
            print_usage();
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    eprintln!(
        "xtask helpers\n\
         Usage: cargo xtask <command>\n\
         Commands:\n  verify-fixtures    Ensure required local test assets are staged [--multi-embedding]\n  help               Show this message"
    );
}

struct FixtureCheck {
    id: &'static str,
    rel_path: &'static str,
    description: &'static str,
    remediation: &'static str,
    integrity: Option<FixtureIntegrity>,
}

#[derive(Clone, Copy)]
struct FixtureIntegrity {
    metadata_rel_path: &'static str,
}

#[derive(Default)]
struct VerifyOptions {
    multi_embedding: bool,
}

const FIXTURE_DB_CHECK: FixtureCheck = FixtureCheck {
    id: "fixture_db_backup",
    rel_path: LEGACY_FIXTURE_BACKUP_REL_PATH,
    description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
    remediation: "Restore via `/save db tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`.",
    integrity: Some(FixtureIntegrity {
        metadata_rel_path: LEGACY_FIXTURE_METADATA_REL_PATH,
    }),
};

const PRICING_CHECK: FixtureCheck = FixtureCheck {
    id: "pricing_json",
    rel_path: "crates/ploke-tui/data/models/all_pricing_parsed.json",
    description: "Pricing metadata consumed by llm::request::pricing tests.",
    remediation:
        "Run `./scripts/openrouter_pricing_sync.py` to fetch the latest OpenRouter pricing payload.",
    integrity: None,
};

const MULTI_FIXTURE_DB_CHECK: FixtureCheck = FixtureCheck {
    id: "fixture_db_backup_multi",
    rel_path: MULTI_EMBED_FIXTURE_BACKUP_REL_PATH,
    description: "Multi-embedding CozoDB backup used by schema/db feature-flag tests.",
    remediation: "Regenerate via `cargo run -p ploke-test-utils --bin regenerate_fixture --features \"multi_embedding_schema\" -- --schema multi`.",
    integrity: Some(FixtureIntegrity {
        metadata_rel_path: MULTI_EMBED_FIXTURE_METADATA_REL_PATH,
    }),
};

const FIXTURE_CHECKS: &[FixtureCheck] = &[FIXTURE_DB_CHECK, PRICING_CHECK];

// TODO: add a dedicated command that regenerates or guides regeneration of pricing/state fixtures (e.g., `cargo xtask regen-pricing`) so this check can auto-heal.
fn verify_fixtures(args: Vec<String>) -> ExitCode {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        print_verify_fixtures_usage();
        return ExitCode::SUCCESS;
    }
    let opts = match parse_verify_args(args) {
        Ok(opts) => opts,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let root = workspace_root();
    println!("Verifying fixtures under {}", root.display());
    let mut missing: Vec<(&FixtureCheck, PathBuf)> = Vec::new();
    let mut drift: Vec<(&FixtureCheck, String)> = Vec::new();
    let mut multi_errors: Vec<String> = Vec::new();

    let mut checks: Vec<&FixtureCheck> = FIXTURE_CHECKS.iter().collect();
    if opts.multi_embedding {
        checks.push(&MULTI_FIXTURE_DB_CHECK);
    }

    for check in checks {
        let full_path = root.join(check.rel_path);
        if !full_path.exists() {
            println!(
                "✘ {:<18} {} (missing)",
                check.id,
                display_relative(&full_path, &root)
            );
            missing.push((check, full_path));
            continue;
        }

        if let Some(integrity) = &check.integrity {
            match verify_integrity(&root, integrity) {
                Ok(_) => println!("✔ {:<18} {}", check.id, display_relative(&full_path, &root)),
                Err(err) => {
                    println!(
                        "✘ {:<18} {} (drift)",
                        check.id,
                        display_relative(&full_path, &root)
                    );
                    drift.push((check, err));
                }
            }
        } else {
            println!("✔ {:<18} {}", check.id, display_relative(&full_path, &root));
        }
    }

    if opts.multi_embedding {
        let multi_missing = missing
            .iter()
            .any(|(check, _)| check.id == MULTI_FIXTURE_DB_CHECK.id);
        let multi_drift = drift
            .iter()
            .any(|(check, _)| check.id == MULTI_FIXTURE_DB_CHECK.id);
        if multi_missing || multi_drift {
            multi_errors.push(
                "Multi-embedding verification skipped: fixture database backup unavailable.".into(),
            );
        } else {
            match verify_multi_embedding_fixture(&root, MULTI_FIXTURE_DB_CHECK.rel_path) {
                Ok(report) => {
                    println!(
                        "✔ {:<18} metadata_relations={}, vector_relations={}, metadata_rows={}, vector_rows={}",
                        "multi_embedding",
                        report.metadata_relations,
                        report.vector_relations,
                        report.total_metadata_rows,
                        report.total_vector_rows
                    );
                }
                Err(mut errs) => multi_errors.append(&mut errs),
            }
        }
    }

    if missing.is_empty() && drift.is_empty() && multi_errors.is_empty() {
        println!("All required fixtures are present.");
        return ExitCode::SUCCESS;
    }

    if !missing.is_empty() {
        eprintln!("\nMissing fixtures detected:");
        for (check, path) in &missing {
            eprintln!(
                "- {id}: {desc}\n  Path: {path}\n  Fix:  {remedy}",
                id = check.id,
                desc = check.description,
                path = path.display(),
                remedy = check.remediation
            );
        }
    }
    if !drift.is_empty() {
        eprintln!("\nFixture drift detected (backup no longer matches source files):");
        for (check, err) in &drift {
            eprintln!(
                "- {id}: {desc}\n  Issue: {err}\n  Fix:   {remedy}",
                id = check.id,
                desc = check.description,
                err = err,
                remedy = check.remediation
            );
        }
    }
    if !multi_errors.is_empty() {
        eprintln!("\nMulti-embedding fixture issues:");
        for err in &multi_errors {
            eprintln!("- {err}");
        }
    }
    ExitCode::FAILURE
}

#[derive(Deserialize)]
struct FixtureIntegrityMetadata {
    fixture_dir: PathBuf,
    tree_sha256: String,
}

fn parse_verify_args(args: Vec<String>) -> Result<VerifyOptions, String> {
    let mut opts = VerifyOptions::default();
    for arg in args {
        match arg.as_str() {
            "--multi-embedding" => opts.multi_embedding = true,
            other => {
                return Err(format!(
                    "Unknown verify-fixtures flag '{other}'. Use `cargo xtask verify-fixtures --help` for usage."
                ));
            }
        }
    }
    Ok(opts)
}

fn print_verify_fixtures_usage() {
    println!("cargo xtask verify-fixtures [--multi-embedding]");
    println!(
        "  --multi-embedding   Validate that fixture backups contain multi-embedding relations"
    );
}

fn verify_integrity(root: &Path, integrity: &FixtureIntegrity) -> Result<(), String> {
    let metadata_path = root.join(integrity.metadata_rel_path);
    if !metadata_path.exists() {
        return Err(format!(
            "Metadata file {} is missing",
            display_relative(&metadata_path, root)
        ));
    }

    let metadata_contents = fs::read_to_string(&metadata_path).map_err(|err| {
        format!(
            "Unable to read {}: {err}",
            display_relative(&metadata_path, root)
        )
    })?;

    let metadata: FixtureIntegrityMetadata =
        serde_json::from_str(&metadata_contents).map_err(|err| {
            format!(
                "Unable to parse {}: {err}",
                display_relative(&metadata_path, root)
            )
        })?;

    let fixture_dir = root.join(&metadata.fixture_dir);
    if !fixture_dir.exists() {
        return Err(format!(
            "Fixture directory {} referenced by {} is missing",
            display_relative(&fixture_dir, root),
            display_relative(&metadata_path, root)
        ));
    }

    let actual_hash = compute_directory_hash(&fixture_dir).map_err(|err| {
        format!(
            "Failed to hash {}: {err}",
            display_relative(&fixture_dir, root)
        )
    })?;

    if actual_hash != metadata.tree_sha256 {
        return Err(format!(
            "Fixture directory {} drifted (expected {}, found {})",
            display_relative(&fixture_dir, root),
            metadata.tree_sha256,
            actual_hash
        ));
    }

    Ok(())
}

fn compute_directory_hash(dir: &Path) -> Result<String, String> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).into_iter() {
        let entry = entry.map_err(|err| {
            let path = err
                .path()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| dir.display().to_string());
            format!("walkdir error near {path}: {err}")
        })?;

        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(dir)
                .map_err(|err| format!("failed to strip prefix: {err}"))?
                .to_path_buf();
            files.push(rel);
        }
    }

    files.sort();
    let mut hasher = Sha256::new();
    for rel in files {
        let rel_str = path_to_unix_string(&rel);
        hasher.update(rel_str.as_bytes());
        hasher.update(&[0]);

        let full_path = dir.join(&rel);
        let mut file = File::open(&full_path)
            .map_err(|err| format!("unable to open {}: {err}", full_path.display()))?;
        let mut buffer = [0u8; 8192];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|err| format!("failed to read {}: {err}", full_path.display()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }

        hasher.update(&[0xFF]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}

struct MultiEmbeddingReport {
    metadata_relations: usize,
    vector_relations: usize,
    total_metadata_rows: usize,
    total_vector_rows: usize,
}

fn verify_multi_embedding_fixture(
    root: &Path,
    rel_path: &str,
) -> Result<MultiEmbeddingReport, Vec<String>> {
    let backup_path = root.join(rel_path);
    let database = Database::load_backup(&backup_path).map_err(|err| {
        vec![format!(
            "Failed to load fixture database {}: {err}",
            backup_path.display()
        )]
    })?;
    let mut errors = Vec::new();
    let mut total_metadata_rows = 0usize;
    let mut total_vector_rows = 0usize;
    let mut metadata_relations = 0usize;
    let mut vector_relations = 0usize;
    let vector_specs = vector_dimension_specs();

    for spec in experimental_node_relation_specs() {
        metadata_relations += 1;
        let metadata_relation = spec.metadata_schema.relation();
        if let Err(err) = database.ensure_relation_registered(metadata_relation) {
            errors.push(format!(
                "Relation `{}` missing from backup: {}",
                metadata_relation, err
            ));
            continue;
        }

        let metadata_count = match count_relation_rows(&database, metadata_relation, "id") {
            Ok(count) => count,
            Err(err) => {
                errors.push(format!(
                    "Unable to count rows in `{}`: {}",
                    metadata_relation, err
                ));
                continue;
            }
        };
        total_metadata_rows += metadata_count;

        for dim_spec in vector_specs {
            vector_relations += 1;
            let relation =
                ExperimentalVectorRelation::new(dim_spec.dims(), spec.vector_relation_base);
            let relation_name = relation.relation_name();
            if let Err(err) = database.ensure_relation_registered(&relation_name) {
                errors.push(format!(
                    "Vector relation `{}` missing from backup: {}",
                    relation_name, err
                ));
                continue;
            }
            if let Err(err) = database.assert_vector_column_layout(&relation_name, dim_spec.dims())
            {
                errors.push(format!(
                    "Vector relation `{}` column layout mismatch: {}",
                    relation_name, err
                ));
                continue;
            }

            let vector_count = match count_relation_rows(&database, &relation_name, "node_id") {
                Ok(count) => count,
                Err(err) => {
                    errors.push(format!(
                        "Unable to count rows in `{}`: {}",
                        relation_name, err
                    ));
                    continue;
                }
            };
            total_vector_rows += vector_count;

            if vector_count != metadata_count {
                errors.push(format!(
                    "Relation `{}`: metadata rows ({metadata_count}) != vector rows ({vector_count}) for dimension {}",
                    relation_name,
                    dim_spec.dims()
                ));
            }
        }
    }

    if errors.is_empty() {
        Ok(MultiEmbeddingReport {
            metadata_relations,
            vector_relations,
            total_metadata_rows,
            total_vector_rows,
        })
    } else {
        Err(errors)
    }
}

fn count_relation_rows(db: &Database, relation: &str, id_column: &str) -> Result<usize, DbError> {
    let script = format!(
        r#"
?[count({id})] :=
    *{relation}{{ {id} @ 'NOW' }}
"#,
        id = id_column,
        relation = relation
    );
    let result = db.raw_query(&script)?;
    result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|value| value.get_int())
        .map(|n| n as usize)
        .ok_or(DbError::NotFound)
}

fn path_to_unix_string(path: &Path) -> String {
    let parts: Vec<String> = path
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect();
    parts.join("/")
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask has a parent directory")
        .to_path_buf()
}

fn display_relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}
