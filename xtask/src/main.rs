use chrono::Utc;
use cozo::ScriptMutability;
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexerTask},
    local::{DevicePreference, EmbeddingConfig, LocalEmbedder},
    runtime::EmbeddingRuntime,
};
use ploke_io::IoManagerHandle;
use ploke_transform::schema::crate_node::WorkspaceMetadataSchema;
use ploke_db::Database;
use ploke_test_utils::{
    backup_db_fixture, fresh_backup_fixture_db, setup_db_full_multi_embedding,
    validate_backup_fixture_contract, FixtureAutomation, FixtureCreationStrategy, FixtureDb,
    FixtureImportMode, FixtureManualRecreation, FIXTURE_NODES_LOCAL_EMBEDDINGS,
};
use ploke_test_utils::fixture_dbs::{active_backup_db_fixtures, all_backup_db_fixtures};
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::ExitCode,
    sync::Arc,
    time::Duration,
};
use tempfile::tempdir;
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{broadcast, mpsc},
};
use walkdir::WalkDir;

const EMBEDDING_MODELS_URL: &str = "https://openrouter.ai/api/v1/embeddings/models";
const EMBEDDING_MODELS_FIXTURE: &str = "fixtures/openrouter/embeddings_models.json";
const EMBEDDING_MODELS_META: &str = "fixtures/openrouter/embeddings_models.meta.json";
const RAG_FIXTURE_PREFIX: &str = "fixture_nodes_";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("verify-fixtures") => verify_fixtures(),
        Some("verify-backup-dbs") => verify_backup_dbs(args.collect()),
        Some("recreate-backup-db") => recreate_backup_db(args.collect()),
        Some("repair-backup-db-schema") => repair_backup_db_schema(args.collect()),
        Some("setup-rag-fixtures") => setup_rag_fixtures(),
        Some("regen-embedding-models") => regen_embedding_models(),
        Some("extract-tokens-log") => extract_tokens_log(args.collect()),
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
         Commands:\n  verify-fixtures          Ensure required local test assets are staged\n  verify-backup-dbs       Validate registered backup DB fixtures used by tests\n  recreate-backup-db      Recreate or print regeneration steps for a registered backup DB fixture\n  repair-backup-db-schema Add the missing workspace_metadata relation to a stale backup fixture in place\n  setup-rag-fixtures       Stage the canonical local fixture_nodes backup into the config dir used by load_db\n  regen-embedding-models   Refresh fixtures/openrouter/embeddings_models.json from OpenRouter\n  extract-tokens-log       Copy filtered token diagnostics into tests/fixture_chat/tokens_sample.log\n  help                     Show this message"
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

const FIXTURE_CHECKS: &[FixtureCheck] = &[
    FixtureCheck {
        id: "fixture_db_backup",
        rel_path: FIXTURE_NODES_LOCAL_EMBEDDINGS.rel_path,
        description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
        remediation: "Run `cargo xtask recreate-backup-db --fixture fixture_nodes_local_embeddings` and then `cargo xtask setup-rag-fixtures`.",
        integrity: None,
    },
    FixtureCheck {
        id: "embedding_models_json",
        rel_path: "fixtures/openrouter/embeddings_models.json",
        description: "Embedding models fixture used by OpenRouter embeddings tests.",
        remediation: "Run `cargo xtask regen-embedding-models` (requires network) to refresh the fixture.",
        integrity: Some(FixtureIntegrity {
            metadata_rel_path: "fixtures/openrouter/embeddings_models.meta.json",
        }),
    },
    FixtureCheck {
        id: "pricing_json",
        rel_path: "crates/ploke-tui/data/models/all_pricing_parsed.json",
        description: "Pricing metadata consumed by llm::request::pricing tests.",
        remediation: "Run `./scripts/openrouter_pricing_sync.py` to fetch the latest OpenRouter pricing payload.",
        integrity: None,
    },
];

fn extract_tokens_log(args: Vec<String>) -> ExitCode {
    let mut input_override: Option<PathBuf> = None;
    let mut output_override: Option<PathBuf> = None;
    let mut include_api = false;

    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--input" => {
                input_override = iter.next().map(PathBuf::from);
            }
            "--output" => {
                output_override = iter.next().map(PathBuf::from);
            }
            "--include-api" => include_api = true,
            other => {
                eprintln!(
                    "Unknown flag '{other}'. Usage: cargo xtask extract-tokens-log [--input PATH] [--output PATH] [--include-api]"
                );
                return ExitCode::FAILURE;
            }
        }
    }

    let root = workspace_root();
    let input_path = match input_override {
        Some(path) => path,
        None => match latest_tokens_log(&root) {
            Ok(path) => path,
            Err(err) => {
                eprintln!("{err}");
                return ExitCode::FAILURE;
            }
        },
    };

    if !input_path.exists() {
        eprintln!("Input log {} does not exist", input_path.display());
        return ExitCode::FAILURE;
    }

    let output_path =
        output_override.unwrap_or_else(|| root.join("tests/fixture_chat/tokens_sample.log"));
    if let Some(parent) = output_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Failed to create output directory {}: {err}",
                parent.display()
            );
            return ExitCode::FAILURE;
        }
    }

    let Ok(input_contents) = fs::read_to_string(&input_path) else {
        eprintln!("Unable to read {}", input_path.display());
        return ExitCode::FAILURE;
    };

    let mut patterns = vec!["kind=\"estimate_input\"", "kind=\"actual_usage\""];
    if include_api {
        patterns.push("kind=\"api_request\"");
    }

    let filtered: Vec<&str> = input_contents
        .lines()
        .filter(|line| patterns.iter().any(|pat| line.contains(pat)))
        .collect();

    if filtered.is_empty() {
        eprintln!(
            "No matching token diagnostics found in {}. Did you run with PLOKE_LOG_TOKENS=1?",
            input_path.display()
        );
        return ExitCode::FAILURE;
    }

    if let Err(err) = fs::write(&output_path, filtered.join("\n")) {
        eprintln!(
            "Failed to write filtered log to {}: {err}",
            output_path.display()
        );
        return ExitCode::FAILURE;
    }

    println!(
        "Wrote {} lines to {}",
        filtered.len(),
        display_relative(&output_path, &root)
    );
    ExitCode::SUCCESS
}

fn latest_tokens_log(root: &Path) -> Result<PathBuf, String> {
    let log_dir = root.join("crates/ploke-tui/logs");
    if !log_dir.exists() {
        return Err(format!(
            "Log dir {} not found; run the TUI once to generate logs.",
            display_relative(&log_dir, root)
        ));
    }
    let mut entries: Vec<PathBuf> = fs::read_dir(&log_dir)
        .map_err(|err| format!("Failed to read {}: {err}", display_relative(&log_dir, root)))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|name| name.starts_with("tokens_") && name.ends_with(".log"))
                .unwrap_or(false)
        })
        .collect();

    entries.sort();
    match entries.pop() {
        Some(path) => Ok(path),
        None => Err(format!(
            "No tokens_*.log found under {}",
            display_relative(&log_dir, root)
        )),
    }
}

// TODO: add a dedicated command that regenerates or guides regeneration of pricing/state fixtures (e.g., `cargo xtask regen-pricing`) so this check can auto-heal.
fn verify_fixtures() -> ExitCode {
    let root = workspace_root();
    println!("Verifying fixtures under {}", root.display());
    let mut missing: Vec<(&FixtureCheck, PathBuf)> = Vec::new();
    let mut drift: Vec<(&FixtureCheck, String)> = Vec::new();

    for check in FIXTURE_CHECKS {
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

    if missing.is_empty() && drift.is_empty() {
        println!("All required fixtures are present.");
        ExitCode::SUCCESS
    } else {
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
        ExitCode::FAILURE
    }
}

fn verify_backup_dbs(args: Vec<String>) -> ExitCode {
    let fixture_filter = match parse_fixture_arg(&args, "verify-backup-dbs") {
        Ok(filter) => filter,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let fixtures = match selected_fixtures(fixture_filter.as_deref()) {
        Ok(fixtures) => fixtures,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    println!("Verifying registered backup DB fixtures:");
    let mut failures = Vec::new();

    for fixture in fixtures {
        match verify_registered_backup_fixture(fixture) {
            Ok(summary) => {
                let root = workspace_root();
                println!(
                    "✔ {:<32} {} | relations={} | roundtrip={}",
                    fixture.id,
                    display_relative(&fixture.path(), &root),
                    summary.relation_count,
                    if summary.roundtrip_ok { "ok" } else { "failed" }
                );
            }
            Err(err) => {
                let root = workspace_root();
                println!(
                    "✘ {:<32} {}",
                    fixture.id,
                    display_relative(&fixture.path(), &root)
                );
                failures.push((fixture, err));
            }
        }
    }

    if failures.is_empty() {
        ExitCode::SUCCESS
    } else {
        eprintln!("\nBackup DB fixture validation failed:");
        for (fixture, err) in failures {
            eprintln!("- {}: {err}", fixture.id);
            eprintln!("  {}", recreation_hint(fixture));
        }
        ExitCode::FAILURE
    }
}

fn recreate_backup_db(args: Vec<String>) -> ExitCode {
    let fixture_id = match parse_required_fixture_arg(&args, "recreate-backup-db") {
        Ok(id) => id,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let fixture = match backup_db_fixture(&fixture_id) {
        Some(fixture) => fixture,
        None => {
            eprintln!(
                "Unknown fixture id '{}'. Available ids: {}",
                fixture_id,
                available_fixture_ids()
            );
            return ExitCode::FAILURE;
        }
    };

    let root = workspace_root();
    let output_path = root
        .join("tests/backup_dbs")
        .join(dated_output_filename(fixture));

    match fixture.creation {
        FixtureCreationStrategy::Automated(strategy) => {
            if let Err(err) = recreate_automated_fixture(fixture, strategy, &output_path) {
                eprintln!("Failed to recreate {}: {err}", fixture.id);
                return ExitCode::FAILURE;
            }
            println!(
                "Recreated {} at {}",
                fixture.id,
                display_relative(&output_path, &root)
            );
            println!(
                "Next: update {} and {} if you intend tests to use this new dated fixture.",
                "/home/brasides/code/ploke/crates/test-utils/src/fixture_dbs.rs",
                "/home/brasides/code/ploke/docs/testing/BACKUP_DB_FIXTURES.md"
            );
            ExitCode::SUCCESS
        }
        FixtureCreationStrategy::Manual(help) => {
            print_manual_recreation_help(fixture, help, &output_path);
            ExitCode::FAILURE
        }
    }
}

fn repair_backup_db_schema(args: Vec<String>) -> ExitCode {
    let fixture_id = match parse_required_fixture_arg(&args, "repair-backup-db-schema") {
        Ok(id) => id,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };

    let fixture = match backup_db_fixture(&fixture_id) {
        Some(fixture) => fixture,
        None => {
            eprintln!(
                "Unknown fixture id '{}'. Available ids: {}",
                fixture_id,
                available_fixture_ids()
            );
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = repair_workspace_metadata_relation(fixture) {
        eprintln!("Failed to repair {}: {err}", fixture.id);
        return ExitCode::FAILURE;
    }

    let root = workspace_root();
    println!(
        "Repaired {} in place by adding the workspace_metadata relation.\n  path: {}",
        fixture.id,
        display_relative(&fixture.path(), &root)
    );
    ExitCode::SUCCESS
}

struct BackupFixtureVerification {
    relation_count: usize,
    roundtrip_ok: bool,
}

fn verify_registered_backup_fixture(
    fixture: &'static FixtureDb,
) -> Result<BackupFixtureVerification, String> {
    let db = fresh_backup_fixture_db(fixture).map_err(|err| err.to_string())?;
    let relation_count = db.relations_vec().map_err(|err| err.to_string())?.len();
    if relation_count == 0 {
        return Err("imported fixture has zero relations".to_string());
    }

    let tmp_dir = tempdir().map_err(|err| format!("create temp dir: {err}"))?;
    let backup_path = tmp_dir.path().join("roundtrip.sqlite");
    db.backup_db(&backup_path)
        .map_err(|err| format!("save roundtrip backup: {err}"))?;

    let reloaded = Database::init_with_schema().map_err(|err| err.to_string())?;
    match fixture.import_mode {
        FixtureImportMode::PlainBackup => {
            let relations = reloaded.relations_vec().map_err(|err| err.to_string())?;
            reloaded
                .import_from_backup(&backup_path, &relations)
                .map_err(|err| format!("roundtrip import: {err}"))?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            reloaded
                .import_backup_with_embeddings(&backup_path)
                .map_err(|err| format!("roundtrip import with embeddings: {err}"))?;
        }
    }
    validate_backup_fixture_contract(fixture, &reloaded)
        .map_err(|err| format!("roundtrip fixture validation: {err}"))?;

    Ok(BackupFixtureVerification {
        relation_count,
        roundtrip_ok: true,
    })
}

fn recreate_automated_fixture(
    fixture: &'static FixtureDb,
    strategy: FixtureAutomation,
    output_path: &Path,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create output dir {}: {err}", parent.display()))?;
    }

    let db = match strategy {
        FixtureAutomation::FixtureCrateMultiEmbedding { fixture_name, .. } => {
            let cozo_db = setup_db_full_multi_embedding(fixture_name)
                .map_err(|err| format!("build source fixture database: {err}"))?;
            Arc::new(Database::new(cozo_db))
        }
        FixtureAutomation::FixtureCrateLocalEmbeddings { fixture_name, .. } => {
            recreate_local_embedding_fixture_db(fixture, fixture_name)?
        }
    };

    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|err| format!("remove existing output {}: {err}", output_path.display()))?;
    }

    db.backup_db(output_path)
        .map_err(|err| format!("write backup {}: {err}", output_path.display()))?;

    verify_output_backup(fixture, output_path)?;
    Ok(())
}

fn verify_output_backup(fixture: &'static FixtureDb, output_path: &Path) -> Result<(), String> {
    let reloaded = Database::init_with_schema().map_err(|err| err.to_string())?;
    match fixture.import_mode {
        FixtureImportMode::PlainBackup => {
            let relations = reloaded.relations_vec().map_err(|err| err.to_string())?;
            reloaded
                .import_from_backup(output_path, &relations)
                .map_err(|err| format!("validate generated backup import: {err}"))?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            reloaded
                .import_backup_with_embeddings(output_path)
                .map_err(|err| format!("validate generated backup import with embeddings: {err}"))?;
        }
    }
    validate_backup_fixture_contract(fixture, &reloaded)
        .map_err(|err| format!("validate generated backup fixture contract: {err}"))?;
    Ok(())
}

fn recreate_local_embedding_fixture_db(
    fixture: &'static FixtureDb,
    fixture_name: &'static str,
) -> Result<Arc<Database>, String> {
    let expected_set = fixture
        .expected_embedding_set()
        .ok_or_else(|| format!("fixture {} is missing embedding metadata", fixture.id))?;

    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("build tokio runtime for local fixture recreation: {err}"))?;

    runtime.block_on(async move {
        let cozo_db = setup_db_full_multi_embedding(fixture_name)
            .map_err(|err| format!("build source fixture database: {err}"))?;
        let db = Arc::new(Database::new(cozo_db));

        db.set_active_set(expected_set)
            .map_err(|err| format!("set active embedding set: {err}"))?;

        let local_embedder = LocalEmbedder::new(EmbeddingConfig {
            device_preference: DevicePreference::ForceCpu,
            ..EmbeddingConfig::default()
        })
            .map_err(|err| format!("initialize local embedder: {err}"))?;
        let processor = EmbeddingProcessor::new(EmbeddingSource::Local(local_embedder));
        let embedding_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            processor,
        ));
        let (cancellation_token, cancel_handle) = CancellationToken::new();
        let indexer = IndexerTask::new(
            Arc::clone(&db),
            IoManagerHandle::new(),
            embedding_runtime,
            cancellation_token,
            cancel_handle,
            8,
        );
        let (progress_tx, _progress_rx) = broadcast::channel(32);
        let (_control_tx, control_rx) = mpsc::channel(1);
        indexer
            .run(Arc::new(progress_tx), control_rx)
            .await
            .map_err(|err| format!("run local embedding indexer: {err}"))?;

        let remaining_unembedded = db
            .count_unembedded_nonfiles()
            .map_err(|err| format!("count remaining unembedded nodes: {err}"))?;
        if remaining_unembedded != 0 {
            return Err(format!(
                "local embedding recreation left {remaining_unembedded} unembedded nodes"
            ));
        }

        let active_set = db
            .with_active_set(|set| set.clone())
            .map_err(|err| format!("read active embedding set after indexing: {err}"))?;
        db.put_active_embedding_set_meta(fixture_name, &active_set)
            .map_err(|err| format!("persist active embedding set metadata: {err}"))?;

        Ok(db)
    })
}

fn print_manual_recreation_help(
    fixture: &'static FixtureDb,
    help: FixtureManualRecreation,
    output_path: &Path,
) {
    let root = workspace_root();
    eprintln!("{} cannot be recreated hermetically yet.", fixture.id);
    eprintln!("{}", help.summary);
    eprintln!(
        "Suggested dated output path: {}",
        display_relative(output_path, &root)
    );
    eprintln!("Manual steps:");
    for (idx, step) in help.steps.iter().enumerate() {
        eprintln!("  {}. {}", idx + 1, step);
    }
}

fn repair_workspace_metadata_relation(fixture: &'static FixtureDb) -> Result<(), String> {
    let fixture_path = fixture.path();
    if !fixture_path.exists() {
        return Err(format!(
            "Backup fixture {} is missing at {}",
            fixture.id,
            fixture_path.display()
        ));
    }

    let db = Database::new_init().map_err(|err| err.to_string())?;
    db.restore_backup(&fixture_path)
        .map_err(|err| format!("restore backup: {err}"))?;

    let script = WorkspaceMetadataSchema::SCHEMA.script_create();
    db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
        .map_err(|err| {
            format!(
                "create workspace_metadata relation with current schema script: {err}"
            )
        })?;

    db.backup_db(&fixture_path)
        .map_err(|err| format!("write repaired backup: {err}"))?;
    Ok(())
}

fn parse_fixture_arg(args: &[String], command: &str) -> Result<Option<String>, String> {
    let mut iter = args.iter();
    let mut fixture = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fixture" => {
                let Some(value) = iter.next() else {
                    return Err(format!("Missing value for --fixture. Usage: cargo xtask {command} [--fixture <id>]"));
                };
                fixture = Some(value.clone());
            }
            other => {
                return Err(format!(
                    "Unknown flag '{other}'. Usage: cargo xtask {command} [--fixture <id>]"
                ));
            }
        }
    }
    Ok(fixture)
}

fn parse_required_fixture_arg(args: &[String], command: &str) -> Result<String, String> {
    match parse_fixture_arg(args, command)? {
        Some(id) => Ok(id),
        None => Err(format!(
            "Missing required --fixture <id>. Usage: cargo xtask {command} --fixture <id>"
        )),
    }
}

fn selected_fixtures(fixture_id: Option<&str>) -> Result<Vec<&'static FixtureDb>, String> {
    match fixture_id {
        Some(id) => backup_db_fixture(id)
            .map(|fixture| vec![fixture])
            .ok_or_else(|| format!("Unknown fixture id '{}'. Available ids: {}", id, available_fixture_ids())),
        None => Ok(active_backup_db_fixtures().collect()),
    }
}

fn available_fixture_ids() -> String {
    all_backup_db_fixtures()
        .iter()
        .map(|fixture| fixture.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn dated_output_filename(fixture: &'static FixtureDb) -> String {
    format!("{}_{}.sqlite", fixture.output_stem(), Utc::now().format("%Y-%m-%d"))
}

fn recreation_hint(fixture: &'static FixtureDb) -> String {
    format!(
        "Run `cargo xtask recreate-backup-db --fixture {}` for recreation or instructions.",
        fixture.id
    )
}

fn setup_rag_fixtures() -> ExitCode {
    let root = workspace_root();
    let source = FIXTURE_NODES_LOCAL_EMBEDDINGS.path();
    if !source.exists() {
        eprintln!(
            "Canonical RAG fixture backup is missing: {}",
            display_relative(&source, &root)
        );
        return ExitCode::FAILURE;
    }

    let config_root = match user_config_local_dir() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let data_dir = config_root.join("ploke").join("data");
    if let Err(err) = fs::create_dir_all(&data_dir) {
        eprintln!("Unable to create {}: {err}", data_dir.display());
        return ExitCode::FAILURE;
    }

    let canonical_name = match source.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => {
            eprintln!(
                "Could not determine fixture filename from {}",
                source.display()
            );
            return ExitCode::FAILURE;
        }
    };

    let mut moved_conflicts = Vec::new();
    let quarantine_root = data_dir.join("quarantined_by_xtask").join(format!(
        "setup_rag_fixtures_{}",
        Utc::now().format("%Y%m%dT%H%M%SZ")
    ));

    let read_dir = match fs::read_dir(&data_dir) {
        Ok(entries) => entries,
        Err(err) => {
            eprintln!("Unable to inspect {}: {err}", data_dir.display());
            return ExitCode::FAILURE;
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("Failed to read entry in {}: {err}", data_dir.display());
                return ExitCode::FAILURE;
            }
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !name.starts_with(RAG_FIXTURE_PREFIX) || name == canonical_name {
            continue;
        }

        if let Err(err) = fs::create_dir_all(&quarantine_root) {
            eprintln!(
                "Unable to create quarantine dir {}: {err}",
                quarantine_root.display()
            );
            return ExitCode::FAILURE;
        }

        let dest = quarantine_root.join(name);
        if let Err(err) = fs::rename(&path, &dest) {
            eprintln!(
                "Failed to move conflicting fixture {} to {}: {err}",
                path.display(),
                dest.display()
            );
            return ExitCode::FAILURE;
        }
        moved_conflicts.push((path, dest));
    }

    let dest = data_dir.join(&canonical_name);
    let source_hash = match compute_file_hash(&source) {
        Ok(hash) => hash,
        Err(err) => {
            eprintln!("Failed to hash {}: {err}", source.display());
            return ExitCode::FAILURE;
        }
    };

    let needs_copy = if dest.exists() {
        match compute_file_hash(&dest) {
            Ok(dest_hash) => dest_hash != source_hash,
            Err(err) => {
                eprintln!("Failed to hash {}: {err}", dest.display());
                return ExitCode::FAILURE;
            }
        }
    } else {
        true
    };

    if needs_copy {
        if let Err(err) = fs::copy(&source, &dest) {
            eprintln!(
                "Failed to copy fixture from {} to {}: {err}",
                source.display(),
                dest.display()
            );
            return ExitCode::FAILURE;
        }
    }

    println!(
        "Prepared RAG fixture backup for config-dir loads.\n  source: {}\n  staged: {}",
        display_relative(&source, &root),
        dest.display()
    );
    if moved_conflicts.is_empty() {
        println!(
            "No conflicting {} backups were present.",
            RAG_FIXTURE_PREFIX
        );
    } else {
        println!("Moved conflicting backups out of the load path:");
        for (from, to) in moved_conflicts {
            println!("  {} -> {}", from.display(), to.display());
        }
    }

    ExitCode::SUCCESS
}

#[derive(Deserialize)]
struct FixtureIntegrityMetadata {
    fixture_dir: Option<PathBuf>,
    fixture_file: Option<PathBuf>,
    tree_sha256: String,
}

enum HashSource {
    Directory(PathBuf),
    File(PathBuf),
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

    let hash_source = match (metadata.fixture_dir, metadata.fixture_file) {
        (Some(dir), None) => HashSource::Directory(root.join(dir)),
        (None, Some(file)) => HashSource::File(root.join(file)),
        _ => {
            return Err(format!(
                "Metadata {} missing fixture_dir or fixture_file",
                display_relative(&metadata_path, root)
            ));
        }
    };

    let (target, actual_hash) = match hash_source {
        HashSource::Directory(dir) => {
            if !dir.exists() {
                return Err(format!(
                    "Fixture directory {} referenced by {} is missing",
                    display_relative(&dir, root),
                    display_relative(&metadata_path, root)
                ));
            }
            let hash = compute_directory_hash(&dir)
                .map_err(|err| format!("Failed to hash {}: {err}", display_relative(&dir, root)))?;
            (dir, hash)
        }
        HashSource::File(file) => {
            if !file.exists() {
                return Err(format!(
                    "Fixture file {} referenced by {} is missing",
                    display_relative(&file, root),
                    display_relative(&metadata_path, root)
                ));
            }
            let hash = compute_file_hash(&file).map_err(|err| {
                format!("Failed to hash {}: {err}", display_relative(&file, root))
            })?;
            (file, hash)
        }
    };

    if actual_hash != metadata.tree_sha256 {
        return Err(format!(
            "Fixture {} drifted (expected {}, found {})",
            display_relative(&target, root),
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

fn compute_file_hash(path: &Path) -> Result<String, String> {
    let mut hasher = Sha256::new();
    let rel_str = path_to_unix_string(
        &path
            .file_name()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("")),
    );
    hasher.update(rel_str.as_bytes());
    hasher.update(&[0]);

    let mut file =
        File::open(path).map_err(|err| format!("unable to open {}: {err}", path.display()))?;
    let mut buffer = [0u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    hasher.update(&[0xFF]);

    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Serialize)]
struct EmbeddingFixtureMetadata<'a> {
    fixture_file: &'a str,
    tree_sha256: String,
    generated_at: String,
    source_url: &'a str,
    notes: &'a str,
}

fn regen_embedding_models() -> ExitCode {
    let client = match Client::builder().timeout(Duration::from_secs(20)).build() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Failed to build HTTP client: {err}");
            return ExitCode::FAILURE;
        }
    };

    let resp = match client
        .get(EMBEDDING_MODELS_URL)
        .header("Accept", "application/json")
        .send()
    {
        Ok(r) => r,
        Err(err) => {
            eprintln!("Failed to fetch {}: {err}", EMBEDDING_MODELS_URL);
            return ExitCode::FAILURE;
        }
    };

    if !resp.status().is_success() {
        eprintln!(
            "Unexpected status {} from {}",
            resp.status(),
            EMBEDDING_MODELS_URL
        );
        return ExitCode::FAILURE;
    }

    let parsed: ploke_llm::request::models::Response = match resp.json() {
        Ok(body) => body,
        Err(err) => {
            eprintln!("Failed to parse embeddings response: {err}");
            return ExitCode::FAILURE;
        }
    };

    let pretty = match serde_json::to_string_pretty(&parsed) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Failed to pretty-print embeddings response: {err}");
            return ExitCode::FAILURE;
        }
    };

    let root = workspace_root();
    let out_path = root.join(EMBEDDING_MODELS_FIXTURE);
    if let Some(parent) = out_path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            eprintln!(
                "Unable to create fixture directory {}: {err}",
                parent.display()
            );
            return ExitCode::FAILURE;
        }
    }

    if let Err(err) = fs::write(&out_path, pretty) {
        eprintln!("Failed to write {}: {err}", out_path.display());
        return ExitCode::FAILURE;
    }

    let sha = match compute_file_hash(&out_path) {
        Ok(hash) => hash,
        Err(err) => {
            eprintln!("Failed to hash {}: {err}", out_path.display());
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = write_embedding_metadata(&root, &sha) {
        eprintln!("{err}");
        return ExitCode::FAILURE;
    }

    println!(
        "✔ refreshed {} (sha256={})",
        display_relative(&out_path, &root),
        sha
    );
    ExitCode::SUCCESS
}

fn write_embedding_metadata(root: &Path, sha: &str) -> Result<(), String> {
    let meta = EmbeddingFixtureMetadata {
        fixture_file: EMBEDDING_MODELS_FIXTURE,
        tree_sha256: sha.to_string(),
        generated_at: Utc::now().to_rfc3339(),
        source_url: EMBEDDING_MODELS_URL,
        notes: "Generated via `cargo xtask regen-embedding-models`",
    };

    let meta_path = root.join(EMBEDDING_MODELS_META);
    if let Some(parent) = meta_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Unable to create metadata directory {}: {err}",
                parent.display()
            )
        })?;
    }

    let body =
        serde_json::to_string_pretty(&meta).map_err(|err| format!("Serialize metadata: {err}"))?;
    fs::write(&meta_path, body).map_err(|err| {
        format!(
            "Failed to write metadata {}: {err}",
            display_relative(&meta_path, root)
        )
    })?;
    Ok(())
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

fn user_config_local_dir() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(path));
    }
    if let Some(home) = env::var_os("HOME") {
        return Ok(PathBuf::from(home).join(".config"));
    }
    Err("Could not determine config dir; set XDG_CONFIG_HOME or HOME.".to_string())
}

fn display_relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}
