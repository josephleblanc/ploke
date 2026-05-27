use chrono::Utc;
use cozo::ScriptMutability;
use ploke_db::Database;
use ploke_embed::{
    cancel_token::CancellationToken,
    config::OpenRouterConfig,
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexerTask},
    local::{DevicePreference, EmbeddingConfig, LocalEmbedder},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};
use ploke_io::IoManagerHandle;
use ploke_test_utils::fixture_dbs::{
    active_backup_db_fixtures, all_backup_db_fixtures, import_backup_with_embeddings_for_fixture,
    plain_backup_import_relations,
};
use ploke_test_utils::{
    CheckedFixturePath, FIXTURE_NODES_LOCAL_EMBEDDINGS, FixtureAutomation, FixtureCreationStrategy,
    FixtureDb, FixtureImportMode, FixtureManualRecreation, FixturePathScope, FixtureStatus,
    backup_db_fixture, backup_db_snapshot_fixture_dir, load_backup_fixture_db, setup_db_full_crate,
    setup_db_full_multi_embedding, setup_db_full_workspace_fixture,
    setup_db_full_workspace_member_fixture, validate_backup_fixture_contract,
};
use ploke_transform::schema::crate_node::WorkspaceMetadataSchema;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    env,
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command as ProcessCommand, ExitCode},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tempfile::tempdir;
use tokio::{
    runtime::Builder as RuntimeBuilder,
    sync::{broadcast, mpsc},
};
use walkdir::WalkDir;

use clap::Parser;

const EMBEDDING_MODELS_URL: &str = "https://openrouter.ai/api/v1/embeddings/models";
const EMBEDDING_MODELS_FIXTURE: &str = "fixtures/openrouter/embeddings_models.json";
const EMBEDDING_MODELS_META: &str = "fixtures/openrouter/embeddings_models.meta.json";
const OPENROUTER_MODELS_URL: &str = "https://openrouter.ai/api/v1/models";
const MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/all_raw.json";
const MODELS_JSON_RAW_PRETTY: &str = "crates/ploke-tui/data/models/all_raw_pretty.json";
const MODELS_TXT_IDS: &str = "crates/ploke-tui/data/models/all_ids_parsed.txt";
const MODELS_JSON_ARCH: &str = "crates/ploke-tui/data/models/all_arch_parsed.json";
const MODELS_JSON_TOP: &str = "crates/ploke-tui/data/models/all_top_parsed.json";
const MODELS_JSON_PRICING: &str = "crates/ploke-tui/data/models/all_pricing_parsed.json";
const OPENROUTER_MODEL_CATALOG_PATHS: &[&str] = &[
    MODELS_JSON_RAW,
    MODELS_JSON_RAW_PRETTY,
    MODELS_TXT_IDS,
    MODELS_JSON_ARCH,
    MODELS_JSON_TOP,
    MODELS_JSON_PRICING,
];
const GOOGLE_MODELS_JSON_RAW: &str = "crates/ploke-tui/data/models/google_raw.json";
const GOOGLE_MODELS_JSON_PARSED: &str = "crates/ploke-tui/data/models/google_parsed.json";
const GOOGLE_MODELS_TXT_IDS: &str = "crates/ploke-tui/data/models/google_ids_parsed.txt";
const GOOGLE_MODELS_JSON_ARCH: &str = "crates/ploke-tui/data/models/google_arch_parsed.json";
const GOOGLE_MODELS_JSON_TOP: &str = "crates/ploke-tui/data/models/google_top_parsed.json";
const GOOGLE_MODELS_JSON_PRICING: &str = "crates/ploke-tui/data/models/google_pricing_parsed.json";
const GOOGLE_MODEL_CATALOG_PATHS: &[&str] = &[
    GOOGLE_MODELS_JSON_RAW,
    GOOGLE_MODELS_JSON_PARSED,
    GOOGLE_MODELS_TXT_IDS,
    GOOGLE_MODELS_JSON_ARCH,
    GOOGLE_MODELS_JSON_TOP,
    GOOGLE_MODELS_JSON_PRICING,
];
const GITHUB_FIXTURE_SERDE_REPO_URL: &str = "https://github.com/serde-rs/serde.git";
const GITHUB_FIXTURE_SERDE_REF: &str = "fa7da4a93567ed347ad0735c28e439fca688ef26";
const GITHUB_FIXTURE_SERDE_DIR: &str = "tests/fixture_github_clones/corpus/serde";
const GITHUB_FIXTURE_SERDE_MANIFEST: &str = "tests/fixture_github_clones/corpus/serde/Cargo.toml";
const GITHUB_FIXTURE_SERDE_DERIVE_INTERNALS_MANIFEST: &str =
    "tests/fixture_github_clones/corpus/serde/serde_derive_internals/Cargo.toml";
const GITHUB_FIXTURE_SERDE_DERIVE_INTERNALS_LIB: &str =
    "tests/fixture_github_clones/corpus/serde/serde_derive_internals/lib.rs";
const GITHUB_FIXTURE_SERDE_REQUIRED_REL_PATHS: &[&str] = &[
    "Cargo.toml",
    "serde_derive_internals/Cargo.toml",
    "serde_derive_internals/lib.rs",
];
const GITHUB_FIXTURE_SERDE_REQUIRED_PATHS: &[&str] = &[
    GITHUB_FIXTURE_SERDE_MANIFEST,
    GITHUB_FIXTURE_SERDE_DERIVE_INTERNALS_MANIFEST,
    GITHUB_FIXTURE_SERDE_DERIVE_INTERNALS_LIB,
];
const GITHUB_FIXTURE_AXUM_REPO_URL: &str = "https://github.com/tokio-rs/axum.git";
const GITHUB_FIXTURE_AXUM_REF: &str = "b99e25d01f2ffa29e5138a0cc337b6b831a41285";
const GITHUB_FIXTURE_AXUM_DIR: &str = "tests/fixture_github_clones/corpus/axum";
const GITHUB_FIXTURE_AXUM_MANIFEST: &str = "tests/fixture_github_clones/corpus/axum/Cargo.toml";
const GITHUB_FIXTURE_AXUM_CRATE_MANIFEST: &str =
    "tests/fixture_github_clones/corpus/axum/axum/Cargo.toml";
const GITHUB_FIXTURE_AXUM_LIB: &str = "tests/fixture_github_clones/corpus/axum/axum/src/lib.rs";
const GITHUB_FIXTURE_AXUM_REQUIRED_REL_PATHS: &[&str] =
    &["Cargo.toml", "axum/Cargo.toml", "axum/src/lib.rs"];
const GITHUB_FIXTURE_AXUM_REQUIRED_PATHS: &[&str] = &[
    GITHUB_FIXTURE_AXUM_MANIFEST,
    GITHUB_FIXTURE_AXUM_CRATE_MANIFEST,
    GITHUB_FIXTURE_AXUM_LIB,
];
const RAG_FIXTURE_PREFIX: &str = "fixture_nodes_";
const PLOKE_FIXTURE_HOME_ENV: &str = "PLOKE_FIXTURE_HOME";
const GITHUB_CORPUS_LOCK_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const GITHUB_CORPUS_LOCK_POLL: Duration = Duration::from_millis(250);

#[derive(Clone, Copy)]
struct GitHubFixture {
    label: &'static str,
    repo_url: &'static str,
    pinned_ref: &'static str,
    checkout_dir: &'static str,
    required_rel_paths: &'static [&'static str],
}

const GITHUB_FIXTURES: &[GitHubFixture] = &[
    GitHubFixture {
        label: "serde",
        repo_url: GITHUB_FIXTURE_SERDE_REPO_URL,
        pinned_ref: GITHUB_FIXTURE_SERDE_REF,
        checkout_dir: GITHUB_FIXTURE_SERDE_DIR,
        required_rel_paths: GITHUB_FIXTURE_SERDE_REQUIRED_REL_PATHS,
    },
    GitHubFixture {
        label: "axum",
        repo_url: GITHUB_FIXTURE_AXUM_REPO_URL,
        pinned_ref: GITHUB_FIXTURE_AXUM_REF,
        checkout_dir: GITHUB_FIXTURE_AXUM_DIR,
        required_rel_paths: GITHUB_FIXTURE_AXUM_REQUIRED_REL_PATHS,
    },
];

// Library modules
mod cli;
mod commands;
mod context;
mod error;
mod executor;
mod profile_ingest;

enum DispatchError {
    Xtask(XtaskError),
    Clap(clap::Error),
}

fn main() -> ExitCode {
    match dispatch() {
        Ok(()) => ExitCode::SUCCESS,
        Err(DispatchError::Xtask(err)) => {
            eprintln!("{err}");
            if err.is_validation() {
                if let Some(recovery) = err.recovery_suggestion() {
                    eprintln!("\nRecovery: {recovery}");
                }
            }
            ExitCode::FAILURE
        }
        Err(DispatchError::Clap(err)) => {
            let _ = err.print();
            clap_exit_code(&err)
        }
    }
}

fn clap_exit_code(err: &clap::Error) -> ExitCode {
    let code = err.exit_code();
    if code == 0 {
        ExitCode::SUCCESS
    } else if (1..=255).contains(&code) {
        ExitCode::from(code as u8)
    } else {
        ExitCode::FAILURE
    }
}

fn dispatch() -> Result<(), DispatchError> {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 {
        print_combined_usage();
        return Ok(());
    }

    let tail: Vec<String> = args.iter().skip(2).cloned().collect();
    match args[1].as_str() {
        "setup-fixtures" => setup_fixtures().map_err(DispatchError::Xtask),
        "verify-fixtures" => verify_fixtures().map_err(DispatchError::Xtask),
        "fixtures" => fixtures_command(tail).map_err(DispatchError::Xtask),
        "verify-backup-dbs" => verify_backup_dbs(tail).map_err(DispatchError::Xtask),
        "recreate-backup-db" => recreate_backup_db(tail).map_err(DispatchError::Xtask),
        "repair-backup-db-schema" => repair_backup_db_schema(tail).map_err(DispatchError::Xtask),
        "setup-rag-fixtures" => setup_rag_fixtures().map_err(DispatchError::Xtask),
        "setup-github-fixtures" => setup_github_fixtures().map_err(DispatchError::Xtask),
        "regen-embedding-models" => regen_embedding_models().map_err(DispatchError::Xtask),
        "regen-model-catalog" => regen_model_catalog().map_err(DispatchError::Xtask),
        "regen-google-model-catalog" => regen_google_model_catalog().map_err(DispatchError::Xtask),
        "extract-tokens-log" => extract_tokens_log(tail).map_err(DispatchError::Xtask),
        "profile-ingest" => profile_ingest::parse_profile_ingest_args(tail)
            .and_then(profile_ingest::run_profile_ingest)
            .map_err(DispatchError::Xtask),
        "profile-ingest-help" => {
            print_profile_ingest_help();
            Ok(())
        }
        _ => {
            let cli = cli::Cli::try_parse_from(args).map_err(DispatchError::Clap)?;
            cli.execute().map_err(DispatchError::Xtask)
        }
    }
}

// Use XtaskError from the error module
use error::XtaskError;

fn print_combined_usage() {
    print_usage();
    eprintln!();
    eprintln!("For `parse`, `db`, `check`, `orchestrate`, and other structured commands:");
    eprintln!("  cargo xtask --help");
}

fn print_usage() {
    eprintln!(
        "xtask helpers\n\
         Usage: cargo xtask <command>\n\
         Commands:\n  setup-fixtures          Prepare all ignored/generated fixtures for this checkout\n  verify-fixtures         Ensure required local test assets are staged\n  fixtures ensure --snapshots Ensure DB fixture snapshots without sharing checkout-local DBs\n  fixtures ensure --typed Prepare shared typed corpus fixture sources and snapshots\n  verify-backup-dbs      Validate registered backup DB fixtures used by tests\n  recreate-backup-db     Recreate or print regeneration steps for a registered backup DB fixture\n  repair-backup-db-schema Add the missing workspace_metadata relation to a stale backup fixture in place\n  setup-rag-fixtures      Stage the canonical local fixture_nodes backup into the config dir used by load_db\n  setup-github-fixtures   Clone ignored GitHub checkout fixtures required by parser tests\n  regen-embedding-models  Refresh fixtures/openrouter/embeddings_models.json from OpenRouter\n  regen-model-catalog     Refresh crates/ploke-tui/data/models from OpenRouter /models\n  regen-google-model-catalog Materialize direct Google model catalog fixtures\n  extract-tokens-log      Copy filtered token diagnostics into tests/fixture_chat/tokens_sample.log\n  profile-ingest          Cold-start parse/transform/embed timing (see --target, --stages, --verbosity, --loops)\n  profile-ingest-help     Show detailed help for profile-ingest command"
    );
}

fn print_profile_ingest_help() {
    eprintln!(
        "profile-ingest: Cold-start performance profiling for ploke ingestion pipeline\n\n\
         Usage: cargo xtask profile-ingest --target <path> [OPTIONS]\n\n\
         Required:\n\
         --target <path>          Path to crate or workspace to profile\n\n\
         Options:\n\
         --stages <list>          Comma-separated: parse,transform,embed (default: parse,transform)\n\
         --verbosity 1|2|3        Output detail level (default: 2)\n\
         --loops <N>              Number of iterations to run (default: 1)\n\
         --json                   Output full JSON report to stdout\n\n\
         Environment:\n\
         OPENROUTER_API_KEY       Required for --stages embed\n\
         PLOKE_PROFILE_LOG=1      Enable tracing output during profiling\n\n\
         Examples:\n\
         cargo xtask profile-ingest --target tests/fixture_crates/fixture_nodes\n\
         cargo xtask profile-ingest --target ./my-crate --stages parse,transform,embed\n\
         cargo xtask profile-ingest --target ./my-crate --loops 10 --stages parse,transform\n\n\
         When --loops > 1, the command runs multiple iterations and reports statistics\n\
         (min, max, avg, p50, p90, p95, p99, std dev) for each stage and total time."
    );
}

#[derive(Clone, Copy)]
enum FixtureTarget {
    Path(&'static str),
    RequiredPaths {
        primary: &'static str,
        required: &'static [&'static str],
    },
    BackupFixture(&'static FixtureDb),
}

struct FixtureCheck {
    id: &'static str,
    target: FixtureTarget,
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
        target: FixtureTarget::BackupFixture(&FIXTURE_NODES_LOCAL_EMBEDDINGS),
        description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
        remediation: "Run `cargo xtask fixtures ensure --snapshots` and then `cargo xtask setup-rag-fixtures`.",
        integrity: None,
    },
    FixtureCheck {
        id: "github_serde_fixture",
        target: FixtureTarget::RequiredPaths {
            primary: GITHUB_FIXTURE_SERDE_MANIFEST,
            required: GITHUB_FIXTURE_SERDE_REQUIRED_PATHS,
        },
        description: "Ignored serde checkout used by syn_parser workspace/config regression tests.",
        remediation: "Run `cargo xtask setup-github-fixtures` to clone the required serde fixture checkout.",
        integrity: None,
    },
    FixtureCheck {
        id: "github_axum_fixture",
        target: FixtureTarget::RequiredPaths {
            primary: GITHUB_FIXTURE_AXUM_MANIFEST,
            required: GITHUB_FIXTURE_AXUM_REQUIRED_PATHS,
        },
        description: "Ignored axum checkout used by syn_parser workspace regression tests.",
        remediation: "Run `cargo xtask setup-github-fixtures` to clone the required axum fixture checkout.",
        integrity: None,
    },
    FixtureCheck {
        id: "embedding_models_json",
        target: FixtureTarget::Path(EMBEDDING_MODELS_FIXTURE),
        description: "Embedding models fixture used by OpenRouter embeddings tests.",
        remediation: "Run `cargo xtask regen-embedding-models` (requires network) to refresh the fixture.",
        integrity: Some(FixtureIntegrity {
            metadata_rel_path: "fixtures/openrouter/embeddings_models.meta.json",
        }),
    },
    FixtureCheck {
        id: "model_catalog_raw",
        target: FixtureTarget::Path(MODELS_JSON_RAW),
        description: "Raw OpenRouter /models catalog used by ploke-llm router tests.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "model_catalog_pretty",
        target: FixtureTarget::Path(MODELS_JSON_RAW_PRETTY),
        description: "Pretty-printed OpenRouter /models catalog for local inspection.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "model_ids_txt",
        target: FixtureTarget::Path(MODELS_TXT_IDS),
        description: "Model id list consumed by ploke-llm ModelId tests.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "model_arch_json",
        target: FixtureTarget::Path(MODELS_JSON_ARCH),
        description: "Model architecture slice consumed by ploke-llm Architecture tests.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "model_top_json",
        target: FixtureTarget::Path(MODELS_JSON_TOP),
        description: "Model top-provider slice generated from the OpenRouter catalog.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "pricing_json",
        target: FixtureTarget::Path(MODELS_JSON_PRICING),
        description: "Pricing metadata consumed by llm::request::pricing tests.",
        remediation: "Run `cargo xtask regen-model-catalog` (requires network) to refresh the model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_catalog_raw",
        target: FixtureTarget::Path(GOOGLE_MODELS_JSON_RAW),
        description: "Raw direct Google model catalog used by route-provenance tests.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_catalog_parsed",
        target: FixtureTarget::Path(GOOGLE_MODELS_JSON_PARSED),
        description: "Shared registry rows adapted from the direct Google model catalog.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_ids_txt",
        target: FixtureTarget::Path(GOOGLE_MODELS_TXT_IDS),
        description: "Direct Google model id list for setup diagnostics.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_arch_json",
        target: FixtureTarget::Path(GOOGLE_MODELS_JSON_ARCH),
        description: "Direct Google model architecture slice for setup diagnostics.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_top_json",
        target: FixtureTarget::Path(GOOGLE_MODELS_JSON_TOP),
        description: "Direct Google top-provider slice for setup diagnostics.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
    FixtureCheck {
        id: "google_pricing_json",
        target: FixtureTarget::Path(GOOGLE_MODELS_JSON_PRICING),
        description: "Direct Google pricing slice; zero-valued until explicit Google pricing is modeled.",
        remediation: "Run `cargo xtask regen-google-model-catalog` to refresh the Google model catalog fixtures.",
        integrity: None,
    },
];

fn extract_tokens_log(args: Vec<String>) -> Result<(), XtaskError> {
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
                return Err(XtaskError::new(format!(
                    "Unknown flag '{other}'. Usage: cargo xtask extract-tokens-log [--input PATH] [--output PATH] [--include-api]"
                )));
            }
        }
    }

    let root = workspace_root();
    let input_path = match input_override {
        Some(path) => path,
        None => latest_tokens_log(&root)?,
    };

    if !input_path.exists() {
        return Err(XtaskError::new(format!(
            "Input log {} does not exist",
            input_path.display()
        )));
    }

    let output_path =
        output_override.unwrap_or_else(|| root.join("tests/fixture_chat/tokens_sample.log"));
    if let Some(parent) = output_path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        return Err(XtaskError::new(format!(
            "Failed to create output directory {}: {err}",
            parent.display()
        )));
    }

    let Ok(input_contents) = fs::read_to_string(&input_path) else {
        return Err(XtaskError::new(format!(
            "Unable to read {}",
            input_path.display()
        )));
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
        return Err(XtaskError::new(format!(
            "No matching token diagnostics found in {}. Did you run with PLOKE_LOG_TOKENS=1?",
            input_path.display()
        )));
    }

    if let Err(err) = fs::write(&output_path, filtered.join("\n")) {
        return Err(XtaskError::new(format!(
            "Failed to write filtered log to {}: {err}",
            output_path.display()
        )));
    }

    println!(
        "Wrote {} lines to {}",
        filtered.len(),
        display_relative(&output_path, &root)
    );
    Ok(())
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

fn setup_fixtures() -> Result<(), XtaskError> {
    println!("Preparing all required local fixtures for this checkout.");
    setup_backup_db_fixtures()?;
    setup_rag_fixtures()?;
    setup_github_fixtures()?;

    let root = workspace_root();
    let embedding_integrity = FixtureIntegrity {
        metadata_rel_path: EMBEDDING_MODELS_META,
    };
    if !root.join(EMBEDDING_MODELS_FIXTURE).exists()
        || verify_integrity(&root, &embedding_integrity).is_err()
    {
        regen_embedding_models()?;
    } else {
        println!(
            "Reused embedding model fixture at {}",
            EMBEDDING_MODELS_FIXTURE
        );
    }

    if missing_any_rel_path(&root, OPENROUTER_MODEL_CATALOG_PATHS) {
        regen_model_catalog()?;
    } else {
        println!("Reused OpenRouter model catalog fixtures.");
    }

    if missing_any_rel_path(&root, GOOGLE_MODEL_CATALOG_PATHS) {
        regen_google_model_catalog()?;
    } else {
        println!("Reused direct Google model catalog fixtures.");
    }

    verify_fixtures()
}

fn setup_backup_db_fixtures() -> Result<(), XtaskError> {
    let root = workspace_root();
    println!("Preparing active backup DB fixtures:");

    for fixture in active_backup_db_fixtures() {
        let required_local_output = match fixture.path_scope {
            FixturePathScope::CheckoutLocal => Some(
                fixture
                    .checkout_local_path()
                    .expect("checkout-local fixtures should resolve a scoped output path"),
            ),
            FixturePathScope::SharedSnapshot | FixturePathScope::LegacyAbsolute => None,
        };
        if let Some(output_path) = required_local_output.as_ref()
            && !output_path.exists()
        {
            recreate_active_backup_fixture(fixture, output_path).map_err(|recreate| {
                XtaskError::new(format!(
                    "Failed to create checkout-local fixture {} at {}: {recreate}",
                    fixture.id,
                    output_path.display()
                ))
            })?;
            let summary = verify_registered_backup_fixture(fixture).map_err(|err| {
                XtaskError::new(format!(
                    "Recreated checkout-local fixture {} failed strict validation from {}: {err}",
                    fixture.id,
                    output_path.display()
                ))
            })?;
            println!(
                "  recreated {:<29} {} | relations={}",
                fixture.id,
                display_relative(summary.path.path(), &root),
                summary.relation_count
            );
            continue;
        }

        match verify_registered_backup_fixture(fixture) {
            Ok(summary) => {
                let path_note =
                    fixture_path_note(summary.path.path(), summary.path.registered_path(), &root);
                println!(
                    "  reused {:<32} {}{}",
                    fixture.id,
                    display_relative(summary.path.path(), &root),
                    path_note
                );
            }
            Err(err) => {
                let output_path = recreation_output_path(fixture, &root);
                recreate_active_backup_fixture(fixture, &output_path).map_err(|recreate| {
                    XtaskError::new(format!(
                        "Failed to recreate {} after validation error ({err}): {recreate}",
                        fixture.id
                    ))
                })?;
                println!(
                    "  recreated {:<29} {}",
                    fixture.id,
                    display_relative(&output_path, &root)
                );
            }
        }
    }

    Ok(())
}

fn recreate_active_backup_fixture(
    fixture: &'static FixtureDb,
    output_path: &Path,
) -> Result<(), String> {
    let FixtureCreationStrategy::Automated(strategy) = fixture.creation else {
        return Err(format!("{} cannot be recreated automatically", fixture.id));
    };
    recreate_automated_fixture(fixture, strategy, output_path)
}

fn missing_any_rel_path(root: &Path, rel_paths: &[&str]) -> bool {
    rel_paths
        .iter()
        .any(|rel_path| !root.join(rel_path).exists())
}

fn verify_fixtures() -> Result<(), XtaskError> {
    let root = workspace_root();
    println!("Verifying fixtures under {}", root.display());
    let mut missing: Vec<(&FixtureCheck, PathBuf)> = Vec::new();
    let mut drift: Vec<(&FixtureCheck, String)> = Vec::new();
    let mut invalid: Vec<(&FixtureCheck, String)> = Vec::new();

    for check in FIXTURE_CHECKS {
        match check.target {
            FixtureTarget::Path(rel_path) => {
                let full_path = root.join(rel_path);
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
                        Ok(_) => {
                            println!("✔ {:<18} {}", check.id, display_relative(&full_path, &root))
                        }
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
            FixtureTarget::RequiredPaths { primary, required } => {
                let primary_path = root.join(primary);
                let missing_required = required.iter().find_map(|rel_path| {
                    let full_path = root.join(rel_path);
                    (!full_path.exists()).then_some(full_path)
                });
                if let Some(missing_path) = missing_required {
                    println!(
                        "✘ {:<18} {} (missing {})",
                        check.id,
                        display_relative(&primary_path, &root),
                        display_relative(&missing_path, &root)
                    );
                    missing.push((check, missing_path));
                } else {
                    println!(
                        "✔ {:<18} {}",
                        check.id,
                        display_relative(&primary_path, &root)
                    );
                }
            }
            FixtureTarget::BackupFixture(fixture) => {
                match verify_registered_backup_fixture(fixture) {
                    Ok(summary) => {
                        let path_note = fixture_path_note(
                            summary.path.path(),
                            summary.path.registered_path(),
                            &root,
                        );
                        println!(
                            "✔ {:<18} {}{} | relations={} | roundtrip={}",
                            check.id,
                            display_relative(summary.path.path(), &root),
                            path_note,
                            summary.relation_count,
                            if summary.roundtrip_ok { "ok" } else { "failed" }
                        );
                    }
                    Err(err) => {
                        let path = fixture.path();
                        let registered_path = fixture.registered_path();
                        let path_note = fixture_path_note(&path, &registered_path, &root);
                        println!(
                            "✘ {:<18} {}{} (invalid)",
                            check.id,
                            display_relative(&path, &root),
                            path_note
                        );
                        invalid.push((check, err));
                    }
                }
            }
        }
    }

    if missing.is_empty() && drift.is_empty() && invalid.is_empty() {
        println!("All required fixtures are present.");
        Ok(())
    } else {
        let mut message = String::new();
        if !missing.is_empty() {
            message.push_str("\nMissing fixtures detected:\n");
            for (check, path) in &missing {
                message.push_str(&format!(
                    "- {id}: {desc}\n  Path: {path}\n  Fix:  {remedy}",
                    id = check.id,
                    desc = check.description,
                    path = path.display(),
                    remedy = check.remediation
                ));
                message.push('\n');
            }
        }
        if !drift.is_empty() {
            message.push_str("\nFixture drift detected (backup no longer matches source files):\n");
            for (check, err) in &drift {
                message.push_str(&format!(
                    "- {id}: {desc}\n  Issue: {err}\n  Fix:   {remedy}",
                    id = check.id,
                    desc = check.description,
                    err = err,
                    remedy = check.remediation
                ));
                message.push('\n');
            }
        }
        if !invalid.is_empty() {
            message.push_str("\nFixture validation failed:\n");
            for (check, err) in &invalid {
                message.push_str(&format!(
                    "- {id}: {desc}\n  Issue: {err}\n  Fix:   {remedy}",
                    id = check.id,
                    desc = check.description,
                    err = err,
                    remedy = check.remediation
                ));
                message.push('\n');
            }
        }
        Err(XtaskError::new(message.trim_end().to_string()))
    }
}

fn verify_backup_dbs(args: Vec<String>) -> Result<(), XtaskError> {
    let fixture_filter = parse_fixture_arg(&args, "verify-backup-dbs")?;
    let fixtures = selected_fixtures(fixture_filter.as_deref())?;

    println!("Verifying registered backup DB fixtures:");
    let mut failures = Vec::new();
    let root = workspace_root();

    for fixture in fixtures {
        match verify_registered_backup_fixture(fixture) {
            Ok(summary) => {
                let path_note =
                    fixture_path_note(summary.path.path(), summary.path.registered_path(), &root);
                println!(
                    "✔ {:<32} {}{} | scope={} | relations={} | roundtrip={}",
                    fixture.id,
                    display_relative(summary.path.path(), &root),
                    path_note,
                    fixture_scope_label(fixture.path_scope),
                    summary.relation_count,
                    if summary.roundtrip_ok { "ok" } else { "failed" }
                );
            }
            Err(err) => {
                let path = fixture.path();
                let registered_path = fixture.registered_path();
                let path_note = fixture_path_note(&path, &registered_path, &root);
                println!(
                    "✘ {:<32} {}{} | scope={}",
                    fixture.id,
                    display_relative(&path, &root),
                    path_note,
                    fixture_scope_label(fixture.path_scope)
                );
                failures.push((fixture, err));
            }
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        let mut message = String::from("\nBackup DB fixture validation failed:\n");
        for (fixture, err) in failures {
            message.push_str(&format!("- {}: {err}\n", fixture.id));
            message.push_str(&format!("  {}\n", recreation_hint(fixture)));
        }
        Err(XtaskError::new(message.trim_end().to_string()))
    }
}

fn fixtures_command(args: Vec<String>) -> Result<(), XtaskError> {
    match args.as_slice() {
        [command, flag] if command == "ensure" && flag == "--snapshots" => {
            ensure_all_snapshot_fixture_profiles()
        }
        [command, flag] if command == "ensure" && flag == "--typed" => {
            if !cfg!(feature = "typed_type_graph") {
                return Err(XtaskError::new(
                    "Typed type graph fixture validation requires xtask's `typed_type_graph` feature. Run `cargo run -p xtask --features typed_type_graph -- fixtures ensure --typed`.",
                ));
            }
            ensure_typed_fixture_sources()?;
            ensure_snapshot_fixtures(SnapshotFixtureSelection::TypedTypeGraph)
        }
        [command, flag] if command == "regenerate" && flag == "--all" => {
            regenerate_all_snapshot_fixture_profiles()
        }
        [command, flag] if command == "regenerate" && flag == "--active" => {
            regenerate_snapshot_fixtures(SnapshotFixtureSelection::Active)
        }
        [command, flag] if command == "regenerate" && flag == "--typed" => {
            regenerate_snapshot_fixtures(SnapshotFixtureSelection::TypedTypeGraph)
        }
        _ => Err(XtaskError::new(
            "Usage: cargo xtask fixtures ensure --snapshots | --typed\n       cargo xtask fixtures regenerate --all | --active | --typed".to_string(),
        )),
    }
}

#[derive(Debug, Clone, Copy)]
enum SnapshotFixtureSelection {
    Active,
    TypedTypeGraph,
}

fn snapshot_fixture_matches_selection(
    fixture: &'static FixtureDb,
    selection: SnapshotFixtureSelection,
) -> bool {
    match selection {
        SnapshotFixtureSelection::Active => fixture.status == FixtureStatus::Active,
        SnapshotFixtureSelection::TypedTypeGraph => fixture.status == FixtureStatus::TypedTypeGraph,
    }
}

fn regenerate_all_snapshot_fixture_profiles() -> Result<(), XtaskError> {
    if cfg!(feature = "typed_type_graph") {
        return Err(XtaskError::new(
            "`fixtures regenerate --all` must be run without `typed_type_graph`; it regenerates active fixtures in the normal profile, then invokes the typed-only pass itself.",
        ));
    }

    regenerate_snapshot_fixtures(SnapshotFixtureSelection::Active)?;
    run_typed_fixture_regeneration_pass()?;
    Ok(())
}

fn ensure_all_snapshot_fixture_profiles() -> Result<(), XtaskError> {
    if cfg!(feature = "typed_type_graph") {
        return Err(XtaskError::new(
            "`fixtures ensure --snapshots` must be run without `typed_type_graph`; it validates active fixtures in the normal profile, then invokes the typed-only pass itself.",
        ));
    }

    ensure_snapshot_fixtures(SnapshotFixtureSelection::Active)?;
    run_typed_fixture_ensure_pass()?;
    Ok(())
}

fn run_typed_fixture_regeneration_pass() -> Result<(), XtaskError> {
    println!("Regenerating typed graph fixtures with xtask's typed_type_graph feature");
    let status = ProcessCommand::new("cargo")
        .current_dir(workspace_root())
        .args([
            "run",
            "-p",
            "xtask",
            "--features",
            "typed_type_graph",
            "--",
            "fixtures",
            "regenerate",
            "--typed",
        ])
        .status()
        .map_err(|err| XtaskError::new(format!("spawn typed fixture regeneration pass: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::new(format!(
            "typed fixture regeneration pass exited with status {status}"
        )))
    }
}

fn run_typed_fixture_ensure_pass() -> Result<(), XtaskError> {
    println!("Ensuring typed graph fixtures with xtask's typed_type_graph feature");
    let status = ProcessCommand::new("cargo")
        .current_dir(workspace_root())
        .args([
            "run",
            "-p",
            "xtask",
            "--features",
            "typed_type_graph",
            "--",
            "fixtures",
            "ensure",
            "--typed",
        ])
        .status()
        .map_err(|err| XtaskError::new(format!("spawn typed fixture ensure pass: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(XtaskError::new(format!(
            "typed fixture ensure pass exited with status {status}"
        )))
    }
}

fn regenerate_snapshot_fixtures(selection: SnapshotFixtureSelection) -> Result<(), XtaskError> {
    let fixtures = all_backup_db_fixtures()
        .iter()
        .copied()
        .filter(|fixture| snapshot_fixture_matches_selection(fixture, selection))
        .filter_map(|fixture| match fixture.creation {
            FixtureCreationStrategy::Automated(strategy) => Some((fixture, strategy)),
            FixtureCreationStrategy::Manual(_) => None,
        })
        .collect::<Vec<_>>();

    if fixtures.is_empty() {
        println!("No automated fixtures matched the requested regeneration selection.");
        return Ok(());
    }

    let typed_selected = fixtures
        .iter()
        .any(|(fixture, _)| fixture.status == FixtureStatus::TypedTypeGraph);
    if typed_selected && !cfg!(feature = "typed_type_graph") {
        return Err(XtaskError::new(
            "Typed type graph fixture regeneration requires xtask's `typed_type_graph` feature. Run `cargo run -p xtask --features typed_type_graph -- fixtures regenerate --typed`.",
        ));
    }

    let shared_count = fixtures
        .iter()
        .filter(|(fixture, _)| fixture.path_scope == FixturePathScope::SharedSnapshot)
        .count();
    if shared_count > 0 {
        let snapshot_dir = backup_db_snapshot_fixture_dir();
        fs::create_dir_all(&snapshot_dir).map_err(|err| {
            XtaskError::new(format!(
                "Unable to create shared DB snapshot fixture dir {}: {err}",
                snapshot_dir.display()
            ))
        })?;
        println!(
            "Regenerating shared DB snapshot fixtures under {}",
            snapshot_dir.display()
        );
    }

    let root = workspace_root();
    let mut regenerated = Vec::new();
    for (fixture, strategy) in fixtures {
        let output_path = bulk_fixture_output_path(fixture, &root);
        recreate_automated_fixture(fixture, strategy, &output_path)
            .map_err(|err| XtaskError::new(format!("Failed to recreate {}: {err}", fixture.id)))?;
        let summary = verify_registered_backup_fixture(fixture).map_err(|err| {
            XtaskError::new(format!(
                "Regenerated fixture {} failed strict validation from {}: {err}",
                fixture.id,
                output_path.display()
            ))
        })?;
        regenerated.push((fixture, output_path, summary));
    }

    for (fixture, path, summary) in regenerated {
        println!(
            "✔ {:<32} {} | scope={} | relations={} | roundtrip={}",
            fixture.id,
            display_relative(&path, &root),
            fixture_scope_label(fixture.path_scope),
            summary.relation_count,
            if summary.roundtrip_ok { "ok" } else { "failed" }
        );
    }

    Ok(())
}

fn ensure_snapshot_fixtures(selection: SnapshotFixtureSelection) -> Result<(), XtaskError> {
    let fixtures = all_backup_db_fixtures()
        .iter()
        .copied()
        .filter(|fixture| snapshot_fixture_matches_selection(fixture, selection))
        .collect::<Vec<_>>();

    if fixtures
        .iter()
        .any(|fixture| fixture.path_scope == FixturePathScope::SharedSnapshot)
    {
        let snapshot_dir = backup_db_snapshot_fixture_dir();
        fs::create_dir_all(&snapshot_dir).map_err(|err| {
            XtaskError::new(format!(
                "Unable to create shared DB snapshot fixture dir {}: {err}",
                snapshot_dir.display()
            ))
        })?;

        println!(
            "Ensuring shared DB snapshot fixtures under {}",
            snapshot_dir.display()
        );
    } else {
        println!("Ensuring checkout-local DB fixtures without staging shared snapshots");
    }

    let root = workspace_root();
    let mut missing_seeds = Vec::new();
    let mut staged = Vec::new();
    for fixture in fixtures {
        if fixture.path_scope == FixturePathScope::CheckoutLocal {
            let output_path = fixture
                .checkout_local_path()
                .expect("checkout-local fixtures should resolve a scoped output path");
            if !output_path.exists() {
                recreate_active_backup_fixture(fixture, &output_path).map_err(|err| {
                    XtaskError::new(format!(
                        "Failed to create checkout-local fixture {} at {}: {err}",
                        fixture.id,
                        output_path.display()
                    ))
                })?;
            }
            let summary = match verify_registered_backup_fixture(fixture) {
                Ok(summary) => summary,
                Err(err) => {
                    recreate_active_backup_fixture(fixture, &output_path).map_err(|recreate| {
                        XtaskError::new(format!(
                            "Failed to recreate checkout-local fixture {} after validation error ({err}): {recreate}",
                            fixture.id
                        ))
                    })?;
                    verify_registered_backup_fixture(fixture).map_err(|verify| {
                        XtaskError::new(format!(
                            "Checkout-local fixture {} failed strict validation from {}: {verify}\n  {}",
                            fixture.id,
                            output_path.display(),
                            recreation_hint(fixture)
                        ))
                    })?
                }
            };
            staged.push((fixture, summary.path.path().to_path_buf(), summary));
            continue;
        }

        let seed_path = fixture.repo_path();
        let snapshot_path = bulk_fixture_output_path(fixture, &root);
        if snapshot_path.exists() {
            let summary = verify_registered_backup_fixture(fixture).map_err(|err| {
                XtaskError::new(format!(
                    "Shared snapshot fixture {} failed strict validation from {}: {err}\n  {}",
                    fixture.id,
                    snapshot_path.display(),
                    recreation_hint(fixture)
                ))
            })?;
            if seed_path.exists() {
                let seed_hash = compute_file_hash(&seed_path).map_err(XtaskError::new)?;
                let snapshot_hash = compute_file_hash(&snapshot_path).map_err(XtaskError::new)?;
                if seed_hash != snapshot_hash {
                    println!(
                        "WARN {:<29} shared snapshot differs from committed seed {}; update the seed if this regenerated snapshot should be shared",
                        fixture.id,
                        display_relative(&seed_path, &root)
                    );
                }
            }
            staged.push((fixture, snapshot_path, summary));
            continue;
        }

        if !seed_path.exists() {
            missing_seeds.push((fixture, seed_path));
            continue;
        }

        stage_snapshot_fixture_file(fixture, &seed_path, &snapshot_path)?;
        let summary = match verify_registered_backup_fixture(fixture) {
            Ok(summary) => summary,
            Err(err) => {
                let _ = fs::remove_file(&snapshot_path);
                return Err(XtaskError::new(format!(
                    "Staged fixture {} failed strict validation from {}: {err}\n  {}",
                    fixture.id,
                    snapshot_path.display(),
                    recreation_hint(fixture)
                )));
            }
        };
        staged.push((fixture, snapshot_path, summary));
    }

    for (fixture, path, summary) in staged {
        println!(
            "✔ {:<32} {} | scope={} | relations={} | roundtrip={}",
            fixture.id,
            display_relative(&path, &root),
            fixture_scope_label(fixture.path_scope),
            summary.relation_count,
            if summary.roundtrip_ok { "ok" } else { "failed" }
        );
    }

    if missing_seeds.is_empty() {
        Ok(())
    } else {
        let mut message = String::from("Committed seed backup fixtures are missing:\n");
        for (fixture, seed_path) in missing_seeds {
            message.push_str(&format!(
                "- {} seed missing at {}\n  {}\n",
                fixture.id,
                display_relative(&seed_path, &root),
                recreation_hint(fixture)
            ));
        }
        Err(XtaskError::new(message.trim_end().to_string()))
    }
}

fn stage_snapshot_fixture_file(
    fixture: &'static FixtureDb,
    seed_path: &Path,
    snapshot_path: &Path,
) -> Result<(), XtaskError> {
    if fixture.path_scope == FixturePathScope::CheckoutLocal {
        return Err(XtaskError::new(format!(
            "{} is checkout-local; refusing to stage it into the shared snapshot directory",
            fixture.id
        )));
    }

    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            XtaskError::new(format!(
                "Unable to create snapshot fixture directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    fs::copy(seed_path, snapshot_path).map_err(|err| {
        XtaskError::new(format!(
            "Copy fixture {} from {} to {}: {err}",
            fixture.id,
            seed_path.display(),
            snapshot_path.display()
        ))
    })?;
    Ok(())
}

fn ensure_typed_fixture_sources() -> Result<(), XtaskError> {
    let typed_corpus_fixtures = all_backup_db_fixtures()
        .iter()
        .copied()
        .filter(|fixture| fixture.status == FixtureStatus::TypedTypeGraph)
        .filter_map(|fixture| match fixture.creation {
            FixtureCreationStrategy::Automated(FixtureAutomation::GithubCorpusCrate {
                normalized_repo,
                checkout_slug,
                clone_url,
                rev,
                ..
            })
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
                    normalized_repo,
                    checkout_slug,
                    clone_url,
                    rev,
                    ..
                },
            )
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusWorkspaceTargets {
                    normalized_repo,
                    checkout_slug,
                    clone_url,
                    rev,
                    ..
                },
            )
            | FixtureCreationStrategy::Automated(
                FixtureAutomation::GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
                    normalized_repo,
                    checkout_slug,
                    clone_url,
                    rev,
                    ..
                },
            ) => Some((fixture, normalized_repo, checkout_slug, clone_url, rev)),
            _ => None,
        })
        .collect::<Vec<_>>();

    if typed_corpus_fixtures.is_empty() {
        println!("No typed GitHub corpus fixtures are registered.");
        return Ok(());
    }

    println!(
        "Ensuring typed corpus fixture sources under {}",
        fixture_cache_home().map_err(XtaskError::new)?.display()
    );

    let root = workspace_root();
    for (fixture, normalized_repo, checkout_slug, clone_url, rev) in typed_corpus_fixtures {
        let checkout = match ensure_github_corpus_fixture_checkout(
            normalized_repo,
            checkout_slug,
            clone_url,
            rev,
        ) {
            Ok(checkout) => checkout,
            Err(err) => {
                return Err(XtaskError::new(format!(
                    "Failed to prepare source checkout for {}: {err}",
                    fixture.id
                )));
            }
        };
        println!(
            "✔ {:<32} source={}",
            fixture.id,
            display_relative(&checkout, &root)
        );
    }

    Ok(())
}

fn recreate_backup_db(args: Vec<String>) -> Result<(), XtaskError> {
    let fixture_id = parse_required_fixture_arg(&args, "recreate-backup-db")?;
    let fixture = resolve_fixture(&fixture_id)?;

    let root = workspace_root();
    let output_path = recreation_output_path(fixture, &root);

    let fixture_db_path = "crates/test-utils/src/fixture_dbs.rs";
    let fixture_db_doc_path = "docs/testing/BACKUP_DB_FIXTURES.md";
    match fixture.creation {
        FixtureCreationStrategy::Automated(strategy) => {
            recreate_automated_fixture(fixture, strategy, &output_path).map_err(|err| {
                XtaskError::new(format!("Failed to recreate {}: {err}", fixture.id))
            })?;
            println!(
                "Recreated {} at {}",
                fixture.id,
                display_relative(&output_path, &root)
            );
            match fixture.path_scope {
                FixturePathScope::CheckoutLocal => {
                    println!(
                        "This checkout-local fixture is now the effective path for \
                        registry loads in this workspace."
                    );
                }
                FixturePathScope::SharedSnapshot => {
                    println!(
                        "Next: update {fixture_db_path} and {fixture_db_doc_path} \
                        if you intend tests to use this new dated shared snapshot. Copy the \
                        reviewed snapshot into tests/backup_dbs/ only when you want to commit it \
                        as a seed artifact."
                    );
                }
                FixturePathScope::LegacyAbsolute => {
                    println!(
                        "Next: update {fixture_db_path} and {fixture_db_doc_path} \
                        if you intend tests to use this new dated fixture."
                    );
                }
            }
            Ok(())
        }
        FixtureCreationStrategy::Manual(help) => {
            Err(print_manual_recreation_help(fixture, help, &output_path))
        }
    }
}

fn repair_backup_db_schema(args: Vec<String>) -> Result<(), XtaskError> {
    let fixture_id = parse_required_fixture_arg(&args, "repair-backup-db-schema")?;
    let fixture = resolve_fixture(&fixture_id)?;

    let repaired_path = repair_workspace_metadata_relation(fixture)
        .map_err(|err| XtaskError::new(format!("Failed to repair {}: {err}", fixture.id)))?;

    let root = workspace_root();
    println!(
        "Repaired {} in place by adding the workspace_metadata relation.\n  path: {}",
        fixture.id,
        display_relative(&repaired_path, &root)
    );
    Ok(())
}

struct BackupFixtureVerification {
    path: CheckedFixturePath,
    relation_count: usize,
    roundtrip_ok: bool,
}

fn verify_registered_backup_fixture(
    fixture: &'static FixtureDb,
) -> Result<BackupFixtureVerification, String> {
    let loaded = load_backup_fixture_db(fixture).map_err(|err| err.to_string())?;
    let (path, db) = loaded.into_parts();
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
            let relations =
                plain_backup_import_relations(fixture, &reloaded).map_err(|err| err.to_string())?;
            reloaded
                .import_from_backup(&backup_path, &relations)
                .map_err(|err| format!("roundtrip import: {err}"))?;
            reloaded
                .ensure_compilation_unit_relations()
                .map_err(|err| err.to_string())?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            import_backup_with_embeddings_for_fixture(fixture, &reloaded, &backup_path)
                .map_err(|err| format!("roundtrip import with embeddings: {err}"))?;
        }
    }
    validate_backup_fixture_contract(fixture, &reloaded)
        .map_err(|err| format!("roundtrip fixture validation: {err}"))?;

    Ok(BackupFixtureVerification {
        path,
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
        FixtureAutomation::WorkspaceFixture { fixture_name, .. } => {
            let cozo_db = setup_db_full_workspace_fixture(fixture_name)
                .map_err(|err| format!("build workspace fixture database: {err}"))?;
            Arc::new(Database::new(cozo_db))
        }
        FixtureAutomation::WorkspaceCrate { crate_name, .. } => {
            let cozo_db = setup_db_full_crate(crate_name)
                .map_err(|err| format!("build workspace crate fixture database: {err}"))?;
            Arc::new(Database::new(cozo_db))
        }
        FixtureAutomation::GithubCorpusCrate {
            normalized_repo,
            checkout_slug,
            clone_url,
            rev,
            ..
        } => {
            let checkout_path = ensure_github_corpus_fixture_checkout(
                normalized_repo,
                checkout_slug,
                clone_url,
                rev,
            )
            .map_err(|err| format!("prepare corpus checkout: {err}"))?;
            let cozo_db = setup_db_full_parse_target(&checkout_path).map_err(|err| {
                format!(
                    "build corpus fixture database from {}: {err}",
                    display_relative(&checkout_path, &workspace_root())
                )
            })?;
            Arc::new(Database::new(cozo_db))
        }
        FixtureAutomation::GithubCorpusCrateOpenRouterEmbeddings {
            normalized_repo,
            checkout_slug,
            clone_url,
            rev,
            ..
        } => {
            let checkout_path = ensure_github_corpus_fixture_checkout(
                normalized_repo,
                checkout_slug,
                clone_url,
                rev,
            )
            .map_err(|err| format!("prepare corpus checkout: {err}"))?;
            recreate_openrouter_embedding_corpus_db(fixture, &checkout_path)?
        }
        FixtureAutomation::GithubCorpusWorkspaceTargets {
            normalized_repo,
            checkout_slug,
            clone_url,
            rev,
            target_relative_paths,
            ..
        } => {
            let checkout_path = ensure_github_corpus_fixture_checkout(
                normalized_repo,
                checkout_slug,
                clone_url,
                rev,
            )
            .map_err(|err| format!("prepare corpus checkout: {err}"))?;
            let cozo_db =
                setup_db_full_parse_workspace_targets(&checkout_path, target_relative_paths)
                    .map_err(|err| {
                        format!(
                            "build corpus workspace fixture database from {}: {err}",
                            display_relative(&checkout_path, &workspace_root())
                        )
                    })?;
            Arc::new(Database::new(cozo_db))
        }
        FixtureAutomation::GithubCorpusWorkspaceTargetsOpenRouterEmbeddings {
            normalized_repo,
            checkout_slug,
            clone_url,
            rev,
            target_relative_paths,
            ..
        } => {
            let checkout_path = ensure_github_corpus_fixture_checkout(
                normalized_repo,
                checkout_slug,
                clone_url,
                rev,
            )
            .map_err(|err| format!("prepare corpus checkout: {err}"))?;
            recreate_openrouter_embedding_corpus_workspace_db(
                fixture,
                &checkout_path,
                target_relative_paths,
            )?
        }
        FixtureAutomation::FixtureWorkspaceMember {
            fixture_name,
            member_crate,
            ..
        } => {
            let cozo_db = setup_db_full_workspace_member_fixture(fixture_name, member_crate)
                .map_err(|err| format!("build workspace member fixture database: {err}"))?;
            Arc::new(Database::new(cozo_db))
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

fn setup_db_full_parse_target(target: &Path) -> Result<cozo::Db<cozo::MemStorage>, String> {
    let db = cozo::Db::new(cozo::MemStorage::default())
        .map_err(|err| format!("create in-memory cozo db: {err}"))?;
    db.initialize()
        .map_err(|err| format!("initialize in-memory cozo db: {err}"))?;
    ploke_transform::schema::create_schema_all(&db)
        .map_err(|err| format!("create transform schema: {err}"))?;

    let mut output = syn_parser::try_run_phases_and_merge(target)
        .map_err(|err| format!("parse and merge target {}: {err}", target.display()))?;
    let mut merged = output
        .extract_merged_graph()
        .ok_or_else(|| "parser output missing merged graph".to_string())?;
    let tree = match output.extract_module_tree() {
        Some(tree) => tree,
        None => merged
            .build_tree_and_prune()
            .map_err(|err| format!("build module tree: {err}"))?,
    };

    ploke_transform::transform::transform_parsed_graph(&db, merged, &tree)
        .map_err(|err| format!("transform parsed graph: {err}"))?;
    Ok(db)
}

fn setup_db_full_parse_workspace_targets(
    workspace_path: &Path,
    target_relative_paths: &[&str],
) -> Result<cozo::Db<cozo::MemStorage>, String> {
    let db = cozo::Db::new(cozo::MemStorage::default())
        .map_err(|err| format!("create in-memory cozo db: {err}"))?;
    db.initialize()
        .map_err(|err| format!("initialize in-memory cozo db: {err}"))?;
    ploke_transform::schema::create_schema_all(&db)
        .map_err(|err| format!("create transform schema: {err}"))?;

    let selected = target_relative_paths
        .iter()
        .map(|relative| workspace_path.join(relative))
        .collect::<Vec<_>>();
    let selected_refs = selected.iter().map(PathBuf::as_path).collect::<Vec<_>>();
    let parsed_workspace = syn_parser::parse_workspace(workspace_path, Some(&selected_refs))
        .map_err(|err| {
            format!(
                "parse workspace {} with targets {:?}: {err}",
                workspace_path.display(),
                target_relative_paths
            )
        })?;

    ploke_transform::transform::transform_parsed_workspace(&db, parsed_workspace)
        .map_err(|err| format!("transform parsed workspace: {err}"))?;
    Ok(db)
}

fn ensure_github_corpus_fixture_checkout(
    normalized_repo: &str,
    checkout_slug: &str,
    clone_url: &str,
    rev: &str,
) -> Result<PathBuf, String> {
    let cache = GithubCorpusFixtureCache::new()?;
    let lock = acquire_fixture_cache_lock(&cache.lock_path(checkout_slug, rev))?;

    let mirror_path = cache.mirror_path(checkout_slug);
    ensure_github_corpus_mirror(normalized_repo, clone_url, rev, &mirror_path)?;

    let checkout_path = cache.checkout_path(checkout_slug, rev);
    ensure_github_corpus_revision_checkout(
        normalized_repo,
        checkout_slug,
        rev,
        &mirror_path,
        &checkout_path,
    )?;

    drop(lock);

    let actual = git_stdout(Some(&checkout_path), &["rev-parse", "HEAD"])
        .map_err(|err| format!("read checked-out commit for {normalized_repo}: {err}"))?;
    if actual.trim() != rev {
        return Err(format!(
            "checkout {} resolved to {}, expected {}",
            checkout_path.display(),
            actual.trim(),
            rev
        ));
    }

    Ok(checkout_path)
}

struct GithubCorpusFixtureCache {
    root: PathBuf,
}

impl GithubCorpusFixtureCache {
    fn new() -> Result<Self, String> {
        Ok(Self {
            root: fixture_cache_home()?,
        })
    }

    fn mirror_path(&self, checkout_slug: &str) -> PathBuf {
        self.root
            .join("corpus")
            .join("mirrors")
            .join(format!("{checkout_slug}.git"))
    }

    fn checkout_path(&self, checkout_slug: &str, rev: &str) -> PathBuf {
        self.root
            .join("corpus")
            .join("checkouts")
            .join(checkout_slug)
            .join(rev)
    }

    fn lock_path(&self, checkout_slug: &str, rev: &str) -> PathBuf {
        self.root
            .join("locks")
            .join(format!("github-corpus-{checkout_slug}-{rev}.lock"))
    }
}

struct FixtureCacheLock {
    path: PathBuf,
}

impl Drop for FixtureCacheLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn acquire_fixture_cache_lock(lock_path: &Path) -> Result<FixtureCacheLock, String> {
    if let Some(parent) = lock_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create fixture lock directory {}: {err}", parent.display()))?;
    }

    let start = Instant::now();
    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(lock_path)
        {
            Ok(mut file) => {
                writeln!(
                    file,
                    "pid={}\ncreated_at={}",
                    std::process::id(),
                    Utc::now().to_rfc3339()
                )
                .map_err(|err| format!("write fixture lock {}: {err}", lock_path.display()))?;
                return Ok(FixtureCacheLock {
                    path: lock_path.to_path_buf(),
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                if start.elapsed() >= GITHUB_CORPUS_LOCK_TIMEOUT {
                    return Err(format!(
                        "Timed out waiting for fixture cache lock {}. Another process may be preparing the same corpus fixture. If no such process is running, remove the stale lock and retry.",
                        lock_path.display()
                    ));
                }
                thread::sleep(GITHUB_CORPUS_LOCK_POLL);
            }
            Err(err) => {
                return Err(format!(
                    "create fixture cache lock {}: {err}",
                    lock_path.display()
                ));
            }
        }
    }
}

fn ensure_github_corpus_mirror(
    normalized_repo: &str,
    clone_url: &str,
    rev: &str,
    mirror_path: &Path,
) -> Result<(), String> {
    if mirror_path.exists() && !mirror_path.join("HEAD").is_file() {
        return Err(format!(
            "corpus mirror path {} exists but does not look like a bare git repository",
            mirror_path.display()
        ));
    }

    if !mirror_path.exists() {
        let mirror_parent = mirror_path.parent().ok_or_else(|| {
            format!(
                "corpus mirror path {} has no parent directory",
                mirror_path.display()
            )
        })?;
        fs::create_dir_all(mirror_parent).map_err(|err| {
            format!(
                "create corpus mirror directory {}: {err}",
                mirror_parent.display()
            )
        })?;
        let mirror_arg = path_to_str(mirror_path)?;
        run_git(None, &["clone", "--mirror", clone_url, mirror_arg])
            .map_err(|err| format!("clone mirror for {normalized_repo}: {err}"))?;
    }

    if !git_has_commit(mirror_path, rev) {
        run_git(Some(mirror_path), &["fetch", "origin", rev])
            .map_err(|err| format!("fetch {normalized_repo}@{rev} into mirror: {err}"))?;
    }

    if !git_has_commit(mirror_path, rev) {
        return Err(format!(
            "corpus mirror {} does not contain expected revision {} after fetch",
            mirror_path.display(),
            rev
        ));
    }

    Ok(())
}

fn ensure_github_corpus_revision_checkout(
    normalized_repo: &str,
    checkout_slug: &str,
    rev: &str,
    mirror_path: &Path,
    checkout_path: &Path,
) -> Result<(), String> {
    if checkout_path.exists() {
        if !checkout_path.join(".git").exists() {
            return Err(format!(
                "revision checkout path {} exists but is not a git checkout",
                checkout_path.display()
            ));
        }
        let actual = git_stdout(Some(checkout_path), &["rev-parse", "HEAD"]).map_err(|err| {
            format!("read existing checkout revision for {normalized_repo}: {err}")
        })?;
        if actual.trim() != rev {
            return Err(format!(
                "revision checkout {} is for {}, expected {}. Because revision checkout paths are immutable cache entries, remove this corrupt checkout and retry.",
                checkout_path.display(),
                actual.trim(),
                rev
            ));
        }
        return Ok(());
    }

    let checkout_parent = checkout_path.parent().ok_or_else(|| {
        format!(
            "corpus checkout path {} has no parent directory",
            checkout_path.display()
        )
    })?;
    fs::create_dir_all(checkout_parent).map_err(|err| {
        format!(
            "create corpus checkout directory {}: {err}",
            checkout_parent.display()
        )
    })?;

    let mirror_arg = path_to_str(mirror_path)?;
    let checkout_arg = path_to_str(checkout_path)?;
    run_git(
        None,
        &[
            "clone",
            "--shared",
            "--no-checkout",
            mirror_arg,
            checkout_arg,
        ],
    )
    .map_err(|err| format!("clone cached checkout for {checkout_slug}: {err}"))?;
    run_git(Some(checkout_path), &["checkout", "--detach", rev])
        .map_err(|err| format!("checkout {normalized_repo}@{rev}: {err}"))?;

    Ok(())
}

fn fixture_cache_home() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os(PLOKE_FIXTURE_HOME_ENV) {
        return Ok(PathBuf::from(path));
    }
    Ok(backup_db_snapshot_fixture_dir().join("_source_cache"))
}

fn path_to_str(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("non-utf8 path {}", path.display()))
}

fn git_has_commit(repo: &Path, rev: &str) -> bool {
    let commitish = format!("{rev}^{{commit}}");
    git_output(Some(repo), &["cat-file", "-e", &commitish])
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn run_git(cwd: Option<&Path>, args: &[&str]) -> Result<(), String> {
    let output = git_output(cwd, args)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(stderr_or_status(&output))
    }
}

fn git_stdout(cwd: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let output = git_output(cwd, args)?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(stderr_or_status(&output))
    }
}

fn git_output(cwd: Option<&Path>, args: &[&str]) -> Result<std::process::Output, String> {
    let mut command = ProcessCommand::new("git");
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .args(args)
        .output()
        .map_err(|err| format!("spawn git {}: {err}", args.join(" ")))
}

fn stderr_or_status(output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.is_empty() {
        format!("process exited with status {}", output.status)
    } else {
        stderr
    }
}

fn verify_output_backup(fixture: &'static FixtureDb, output_path: &Path) -> Result<(), String> {
    let reloaded = Database::init_with_schema().map_err(|err| err.to_string())?;
    match fixture.import_mode {
        FixtureImportMode::PlainBackup => {
            let relations =
                plain_backup_import_relations(fixture, &reloaded).map_err(|err| err.to_string())?;
            reloaded
                .import_from_backup(output_path, &relations)
                .map_err(|err| format!("validate generated backup import: {err}"))?;
            reloaded
                .ensure_compilation_unit_relations()
                .map_err(|err| err.to_string())?;
        }
        FixtureImportMode::BackupWithEmbeddings => {
            import_backup_with_embeddings_for_fixture(fixture, &reloaded, output_path).map_err(
                |err| format!("validate generated backup import with embeddings: {err}"),
            )?;
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
    let cozo_db = setup_db_full_multi_embedding(fixture_name)
        .map_err(|err| format!("build source fixture database: {err}"))?;
    let local_embedder = LocalEmbedder::new(EmbeddingConfig {
        device_preference: DevicePreference::ForceCpu,
        ..EmbeddingConfig::default()
    })
    .map_err(|err| format!("initialize local embedder: {err}"))?;
    let processor = EmbeddingProcessor::new(EmbeddingSource::Local(local_embedder));
    recreate_embedding_db_with_processor(fixture, fixture_name, cozo_db, processor, "local")
}

fn recreate_openrouter_embedding_corpus_db(
    fixture: &'static FixtureDb,
    checkout_path: &Path,
) -> Result<Arc<Database>, String> {
    let cozo_db = setup_db_full_parse_target(checkout_path).map_err(|err| {
        format!(
            "build corpus fixture database from {}: {err}",
            display_relative(checkout_path, &workspace_root())
        )
    })?;
    let expected_set = fixture
        .expected_embedding_set()
        .ok_or_else(|| format!("fixture {} is missing embedding metadata", fixture.id))?;
    let or_cfg = OpenRouterConfig {
        model: expected_set.model.to_string(),
        dimensions: Some(expected_set.dims() as usize),
        request_dimensions: None,
        snippet_batch_size: 16,
        input_type: Some("code-snippet".into()),
        ..Default::default()
    };
    let backend = OpenRouterBackend::new(&or_cfg)
        .map_err(|err| format!("initialize OpenRouter embedder: {err}"))?;
    let processor = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend));
    recreate_embedding_db_with_processor(fixture, fixture.id, cozo_db, processor, "OpenRouter")
}

fn recreate_openrouter_embedding_corpus_workspace_db(
    fixture: &'static FixtureDb,
    checkout_path: &Path,
    target_relative_paths: &[&str],
) -> Result<Arc<Database>, String> {
    let cozo_db = setup_db_full_parse_workspace_targets(checkout_path, target_relative_paths)
        .map_err(|err| {
            format!(
                "build corpus workspace fixture database from {}: {err}",
                display_relative(checkout_path, &workspace_root())
            )
        })?;
    let expected_set = fixture
        .expected_embedding_set()
        .ok_or_else(|| format!("fixture {} is missing embedding metadata", fixture.id))?;
    let or_cfg = OpenRouterConfig {
        model: expected_set.model.to_string(),
        dimensions: Some(expected_set.dims() as usize),
        request_dimensions: None,
        snippet_batch_size: 16,
        input_type: Some("code-snippet".into()),
        ..Default::default()
    };
    let backend = OpenRouterBackend::new(&or_cfg)
        .map_err(|err| format!("initialize OpenRouter embedder: {err}"))?;
    let processor = EmbeddingProcessor::new(EmbeddingSource::OpenRouter(backend));
    recreate_embedding_db_with_processor(fixture, fixture.id, cozo_db, processor, "OpenRouter")
}

fn recreate_embedding_db_with_processor(
    fixture: &'static FixtureDb,
    active_set_owner: &str,
    cozo_db: cozo::Db<cozo::MemStorage>,
    processor: EmbeddingProcessor,
    backend_label: &str,
) -> Result<Arc<Database>, String> {
    let expected_set = fixture
        .expected_embedding_set()
        .ok_or_else(|| format!("fixture {} is missing embedding metadata", fixture.id))?;

    let runtime = RuntimeBuilder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("build tokio runtime for embedding fixture recreation: {err}"))?;

    runtime.block_on(async move {
        let db = Arc::new(Database::new(cozo_db));

        db.set_active_set(expected_set)
            .map_err(|err| format!("set active embedding set: {err}"))?;

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
            None,
        );
        let (progress_tx, _progress_rx) = broadcast::channel(32);
        let (_control_tx, control_rx) = mpsc::channel(1);
        indexer
            .run(Arc::new(progress_tx), control_rx)
            .await
            .map_err(|err| format!("run {backend_label} embedding indexer: {err}"))?;

        let remaining_unembedded = db
            .count_unembedded_nonfiles()
            .map_err(|err| format!("count remaining unembedded nodes: {err}"))?;
        if remaining_unembedded != 0 {
            return Err(format!(
                "{backend_label} embedding recreation left {remaining_unembedded} unembedded nodes"
            ));
        }

        let active_set = db
            .with_active_set(|set| set.clone())
            .map_err(|err| format!("read active embedding set after indexing: {err}"))?;
        db.put_active_embedding_set_meta(active_set_owner, &active_set)
            .map_err(|err| format!("persist active embedding set metadata: {err}"))?;

        Ok(db)
    })
}

fn print_manual_recreation_help(
    fixture: &'static FixtureDb,
    help: FixtureManualRecreation,
    output_path: &Path,
) -> XtaskError {
    let root = workspace_root();
    let mut message = format!(
        "{} cannot be recreated hermetically yet.\n{}\nSuggested dated output path: {}\nManual steps:",
        fixture.id,
        help.summary,
        display_relative(output_path, &root)
    );
    for (idx, step) in help.steps.iter().enumerate() {
        message.push_str(&format!("\n  {}. {}", idx + 1, step));
    }
    XtaskError::new(message)
}

fn repair_workspace_metadata_relation(fixture: &'static FixtureDb) -> Result<PathBuf, String> {
    if fixture.path_scope != FixturePathScope::LegacyAbsolute {
        return Err(format!(
            "{} is {}; recreate it with `cargo xtask recreate-backup-db --fixture {}` instead of repairing the registered fallback in place",
            fixture.id,
            fixture_scope_label(fixture.path_scope),
            fixture.id
        ));
    }

    let fixture_path = fixture.registered_path();
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
            format!("create workspace_metadata relation with current schema script: {err}")
        })?;

    db.backup_db(&fixture_path)
        .map_err(|err| format!("write repaired backup: {err}"))?;
    Ok(fixture_path)
}

fn parse_fixture_arg(args: &[String], command: &str) -> Result<Option<String>, String> {
    let mut iter = args.iter();
    let mut fixture = None;
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--fixture" => {
                let Some(value) = iter.next() else {
                    return Err(format!(
                        "Missing value for --fixture. Usage: cargo xtask {command} [--fixture <id>]"
                    ));
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
            .ok_or_else(|| {
                format!(
                    "Unknown fixture id '{}'. Available ids: {}",
                    id,
                    available_fixture_ids()
                )
            }),
        None => Ok(active_backup_db_fixtures().collect()),
    }
}

fn resolve_fixture(fixture_id: &str) -> Result<&'static FixtureDb, XtaskError> {
    backup_db_fixture(fixture_id).ok_or_else(|| {
        XtaskError::new(format!(
            "Unknown fixture id '{}'. Available ids: {}",
            fixture_id,
            available_fixture_ids()
        ))
    })
}

fn available_fixture_ids() -> String {
    all_backup_db_fixtures()
        .iter()
        .map(|fixture| fixture.id)
        .collect::<Vec<_>>()
        .join(", ")
}

fn recreation_output_path(fixture: &'static FixtureDb, root: &Path) -> PathBuf {
    match fixture.path_scope {
        FixturePathScope::CheckoutLocal => fixture
            .checkout_local_path()
            .expect("checkout-local fixtures should resolve a scoped output path"),
        FixturePathScope::SharedSnapshot => {
            backup_db_snapshot_fixture_dir().join(dated_output_filename(fixture))
        }
        FixturePathScope::LegacyAbsolute => root
            .join("tests/backup_dbs")
            .join(dated_output_filename(fixture)),
    }
}

fn bulk_fixture_output_path(fixture: &'static FixtureDb, root: &Path) -> PathBuf {
    match fixture.path_scope {
        FixturePathScope::CheckoutLocal => fixture
            .checkout_local_path()
            .expect("checkout-local fixtures should resolve a scoped output path"),
        FixturePathScope::SharedSnapshot => fixture.shared_snapshot_path(),
        FixturePathScope::LegacyAbsolute => root.join(fixture.rel_path),
    }
}

fn dated_output_filename(fixture: &'static FixtureDb) -> String {
    format!(
        "{}_{}.sqlite",
        fixture.output_stem(),
        Utc::now().format("%Y-%m-%d")
    )
}

fn fixture_scope_label(scope: FixturePathScope) -> &'static str {
    match scope {
        FixturePathScope::CheckoutLocal => "checkout-local",
        FixturePathScope::SharedSnapshot => "shared-snapshot",
        FixturePathScope::LegacyAbsolute => "legacy-absolute",
    }
}

fn fixture_path_note(path: &Path, registered_path: &Path, root: &Path) -> String {
    if path == registered_path {
        return String::new();
    }
    format!(" (registered: {})", display_relative(registered_path, root))
}

fn recreation_hint(fixture: &'static FixtureDb) -> String {
    format!(
        "Run `cargo xtask recreate-backup-db --fixture {}` for recreation or instructions.",
        fixture.id
    )
}

fn setup_rag_fixtures() -> Result<(), XtaskError> {
    let root = workspace_root();
    let checked_source = FIXTURE_NODES_LOCAL_EMBEDDINGS.checked_path()?;
    let source = checked_source.path();
    if !source.exists() {
        return Err(XtaskError::new(format!(
            "Canonical RAG fixture backup is missing: {}",
            display_relative(source, &root)
        )));
    }

    let config_root = user_config_local_dir()?;
    let data_dir = config_root.join("ploke").join("data");
    if let Err(err) = fs::create_dir_all(&data_dir) {
        return Err(XtaskError::new(format!(
            "Unable to create {}: {err}",
            data_dir.display()
        )));
    }

    let canonical_name = match source.file_name().and_then(|name| name.to_str()) {
        Some(name) => name.to_string(),
        None => {
            return Err(XtaskError::new(format!(
                "Could not determine fixture filename from {}",
                source.display()
            )));
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
            return Err(XtaskError::new(format!(
                "Unable to inspect {}: {err}",
                data_dir.display()
            )));
        }
    };

    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                return Err(XtaskError::new(format!(
                    "Failed to read entry in {}: {err}",
                    data_dir.display()
                )));
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
            return Err(XtaskError::new(format!(
                "Unable to create quarantine dir {}: {err}",
                quarantine_root.display()
            )));
        }

        let dest = quarantine_root.join(name);
        if let Err(err) = fs::rename(&path, &dest) {
            return Err(XtaskError::new(format!(
                "Failed to move conflicting fixture {} to {}: {err}",
                path.display(),
                dest.display()
            )));
        }
        moved_conflicts.push((path, dest));
    }

    let dest = data_dir.join(&canonical_name);
    let source_hash = compute_file_hash(source)
        .map_err(|err| XtaskError::new(format!("Failed to hash {}: {err}", source.display())))?;

    let needs_copy = if dest.exists() {
        match compute_file_hash(&dest) {
            Ok(dest_hash) => dest_hash != source_hash,
            Err(err) => {
                return Err(XtaskError::new(format!(
                    "Failed to hash {}: {err}",
                    dest.display()
                )));
            }
        }
    } else {
        true
    };

    if needs_copy && let Err(err) = fs::copy(source, &dest) {
        return Err(XtaskError::new(format!(
            "Failed to copy fixture from {} to {}: {err}",
            source.display(),
            dest.display()
        )));
    }

    println!(
        "Prepared RAG fixture backup for config-dir loads.\n  source: {}\n  staged: {}",
        display_relative(source, &root),
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

    Ok(())
}

fn setup_github_fixtures() -> Result<(), XtaskError> {
    let root = workspace_root();
    let mut prepared = Vec::new();

    for fixture in GITHUB_FIXTURES {
        let checkout_path = root.join(fixture.checkout_dir);
        let action = prepare_github_fixture(*fixture, &checkout_path).map_err(XtaskError::new)?;

        if let Some(missing_path) = missing_github_fixture_path(fixture, &checkout_path) {
            return Err(XtaskError::new(format!(
                "{} fixture checkout is incomplete after setup; missing {}",
                fixture.label,
                display_relative(&missing_path, &root)
            )));
        }

        let commit =
            git_output_in(Some(&checkout_path), &["rev-parse", "HEAD"]).map_err(|err| {
                XtaskError::new(format!(
                    "Failed to inspect {} fixture checkout: {err}",
                    fixture.label
                ))
            })?;

        prepared.push((fixture, action, commit, checkout_path));
    }

    println!("Prepared GitHub parser fixtures.");
    for (fixture, action, commit, checkout_path) in prepared {
        println!(
            "  {}: action={} repo={} ref={} commit={} path={}",
            fixture.label,
            action,
            fixture.repo_url,
            fixture.pinned_ref,
            commit,
            display_relative(&checkout_path, &root)
        );
    }
    Ok(())
}

fn prepare_github_fixture(
    fixture: GitHubFixture,
    checkout_path: &Path,
) -> Result<&'static str, String> {
    if checkout_path.exists() {
        if !checkout_path.join(".git").is_dir() {
            return Err(format!(
                "{} exists but is not a git checkout; move it aside before rerunning `cargo xtask setup-github-fixtures`",
                checkout_path.display()
            ));
        }

        let current = git_output_in(Some(checkout_path), &["rev-parse", "HEAD"])?;
        if current == fixture.pinned_ref {
            return Ok("reused");
        }

        let status = git_output_in(Some(checkout_path), &["status", "--porcelain"])?;
        if !status.is_empty() {
            return Err(format!(
                "{} is at {}, expected {}, and has local changes; clean it before rerunning `cargo xtask setup-github-fixtures`",
                checkout_path.display(),
                current,
                fixture.pinned_ref
            ));
        }

        git_output_in(
            Some(checkout_path),
            &["fetch", "--depth", "1", "origin", fixture.pinned_ref],
        )?;
        git_output_in(Some(checkout_path), &["checkout", "--detach", "FETCH_HEAD"])?;
        return Ok("updated");
    }

    let parent = checkout_path.parent().ok_or_else(|| {
        format!(
            "Could not determine parent directory for {}",
            checkout_path.display()
        )
    })?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("create fixture parent {}: {err}", parent.display()))?;

    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("{}-fixture-", fixture.label))
        .tempdir_in(parent)
        .map_err(|err| format!("create temporary fixture checkout dir: {err}"))?;
    let temp_checkout = temp_dir.path().join(fixture.label);

    git_output_in(Some(temp_dir.path()), &["init", fixture.label])?;
    git_output_in(
        Some(&temp_checkout),
        &["remote", "add", "origin", fixture.repo_url],
    )?;
    git_output_in(
        Some(&temp_checkout),
        &["fetch", "--depth", "1", "origin", fixture.pinned_ref],
    )?;
    git_output_in(
        Some(&temp_checkout),
        &["checkout", "--detach", "FETCH_HEAD"],
    )?;

    if let Some(missing_path) = missing_github_fixture_path(&fixture, &temp_checkout) {
        return Err(format!(
            "cloned {} fixture is missing expected path {}",
            fixture.label,
            missing_path.display()
        ));
    }

    fs::rename(&temp_checkout, checkout_path).map_err(|err| {
        format!(
            "move fixture checkout from {} to {}: {err}",
            temp_checkout.display(),
            checkout_path.display()
        )
    })?;
    Ok("cloned")
}

fn missing_github_fixture_path(fixture: &GitHubFixture, checkout_path: &Path) -> Option<PathBuf> {
    fixture
        .required_rel_paths
        .iter()
        .map(|rel_path| checkout_path.join(rel_path))
        .find(|path| !path.exists())
}

fn git_output_in(current_dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
    let mut command = ProcessCommand::new("git");
    if let Some(dir) = current_dir {
        command.current_dir(dir);
    }
    let output = command
        .args(args)
        .output()
        .map_err(|err| format!("spawn git {}: {err}", args.join(" ")))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let mut message = format!("git {} failed", args.join(" "));
        if !stderr.is_empty() {
            message.push_str(&format!(": {stderr}"));
        }
        if !stdout.is_empty() {
            message.push_str(&format!(" stdout: {stdout}"));
        }
        Err(message)
    }
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

fn regen_embedding_models() -> Result<(), XtaskError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| XtaskError::new(format!("Failed to build HTTP client: {err}")))?;

    let resp = match client
        .get(EMBEDDING_MODELS_URL)
        .header("Accept", "application/json")
        .send()
    {
        Ok(r) => r,
        Err(err) => {
            return Err(XtaskError::new(format!(
                "Failed to fetch {}: {err}",
                EMBEDDING_MODELS_URL
            )));
        }
    };

    if !resp.status().is_success() {
        return Err(XtaskError::new(format!(
            "Unexpected status {} from {}",
            resp.status(),
            EMBEDDING_MODELS_URL
        )));
    }

    let parsed: ploke_llm::request::models::Response = match resp.json() {
        Ok(body) => body,
        Err(err) => {
            return Err(XtaskError::new(format!(
                "Failed to parse embeddings response: {err}"
            )));
        }
    };

    let pretty = match serde_json::to_string_pretty(&parsed) {
        Ok(s) => s,
        Err(err) => {
            return Err(XtaskError::new(format!(
                "Failed to pretty-print embeddings response: {err}"
            )));
        }
    };

    let root = workspace_root();
    let out_path = root.join(EMBEDDING_MODELS_FIXTURE);
    if let Some(parent) = out_path.parent()
        && let Err(err) = fs::create_dir_all(parent)
    {
        return Err(XtaskError::new(format!(
            "Unable to create fixture directory {}: {err}",
            parent.display()
        )));
    }

    if let Err(err) = fs::write(&out_path, pretty) {
        return Err(XtaskError::new(format!(
            "Failed to write {}: {err}",
            out_path.display()
        )));
    }

    let sha = compute_file_hash(&out_path)
        .map_err(|err| XtaskError::new(format!("Failed to hash {}: {err}", out_path.display())))?;

    write_embedding_metadata(&root, &sha)?;

    println!(
        "✔ refreshed {} (sha256={})",
        display_relative(&out_path, &root),
        sha
    );
    Ok(())
}

fn regen_model_catalog() -> Result<(), XtaskError> {
    let client = Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| XtaskError::new(format!("Failed to build HTTP client: {err}")))?;

    let resp = match client
        .get(OPENROUTER_MODELS_URL)
        .header("Accept", "application/json")
        .send()
    {
        Ok(r) => r,
        Err(err) => {
            return Err(XtaskError::new(format!(
                "Failed to fetch {}: {err}",
                OPENROUTER_MODELS_URL
            )));
        }
    };

    if !resp.status().is_success() {
        return Err(XtaskError::new(format!(
            "Unexpected status {} from {}",
            resp.status(),
            OPENROUTER_MODELS_URL
        )));
    }

    let raw = resp.text().map_err(|err| {
        XtaskError::new(format!(
            "Failed to read response body from {}: {err}",
            OPENROUTER_MODELS_URL
        ))
    })?;
    let raw_value: serde_json::Value = serde_json::from_str(&raw).map_err(|err| {
        XtaskError::new(format!(
            "Failed to parse raw model catalog from {}: {err}",
            OPENROUTER_MODELS_URL
        ))
    })?;
    let parsed: ploke_llm::request::models::Response = serde_json::from_value(raw_value.clone())
        .map_err(|err| {
            XtaskError::new(format!(
                "Failed to decode model catalog into ploke-llm model types: {err}"
            ))
        })?;

    let root = workspace_root();
    let mut written = Vec::new();

    write_text_fixture(&root, MODELS_JSON_RAW, &raw, &mut written)?;

    let raw_pretty = serde_json::to_string_pretty(&raw_value).map_err(|err| {
        XtaskError::new(format!("Failed to pretty-print raw model catalog: {err}"))
    })?;
    write_text_fixture(&root, MODELS_JSON_RAW_PRETTY, &raw_pretty, &mut written)?;

    let ids = parsed
        .data
        .iter()
        .map(|item| item.id.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    write_text_fixture(&root, MODELS_TXT_IDS, &ids, &mut written)?;

    let architecture = parsed
        .data
        .iter()
        .map(|item| item.architecture.clone())
        .collect::<Vec<_>>();
    write_json_fixture(&root, MODELS_JSON_ARCH, &architecture, &mut written)?;

    let top_provider = parsed
        .data
        .iter()
        .map(|item| item.top_provider.clone())
        .collect::<Vec<_>>();
    write_json_fixture(&root, MODELS_JSON_TOP, &top_provider, &mut written)?;

    let pricing = parsed
        .data
        .iter()
        .map(|item| item.pricing)
        .collect::<Vec<_>>();
    write_json_fixture(&root, MODELS_JSON_PRICING, &pricing, &mut written)?;

    println!(
        "✔ refreshed OpenRouter model catalog from {} (models={})",
        OPENROUTER_MODELS_URL,
        parsed.data.len()
    );
    for path in written {
        println!("  wrote {path}");
    }
    Ok(())
}

fn regen_google_model_catalog() -> Result<(), XtaskError> {
    let rt = RuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| XtaskError::new(format!("Failed to build Tokio runtime: {err}")))?;
    let typed_response = rt
        .block_on(async {
            <ploke_llm::router_only::google::Google as ploke_llm::router_only::HasModels>::fetch_models(
                &reqwest::Client::new(),
            )
            .await
        })
        .map_err(|err| {
            XtaskError::new(format!(
                "Failed to materialize direct Google model catalog: {err}"
            ))
        })?;

    let parsed = ploke_llm::request::models::Response {
        data: typed_response
            .clone()
            .into_iter()
            .map(ploke_llm::request::models::ResponseItem::from)
            .collect(),
    };

    let root = workspace_root();
    let mut written = Vec::new();

    write_json_fixture(&root, GOOGLE_MODELS_JSON_RAW, &typed_response, &mut written)?;
    write_json_fixture(&root, GOOGLE_MODELS_JSON_PARSED, &parsed, &mut written)?;

    let ids = parsed
        .data
        .iter()
        .map(|item| item.id.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    write_text_fixture(&root, GOOGLE_MODELS_TXT_IDS, &ids, &mut written)?;

    let architecture = parsed
        .data
        .iter()
        .map(|item| item.architecture.clone())
        .collect::<Vec<_>>();
    write_json_fixture(&root, GOOGLE_MODELS_JSON_ARCH, &architecture, &mut written)?;

    let top_provider = parsed
        .data
        .iter()
        .map(|item| item.top_provider.clone())
        .collect::<Vec<_>>();
    write_json_fixture(&root, GOOGLE_MODELS_JSON_TOP, &top_provider, &mut written)?;

    let pricing = parsed
        .data
        .iter()
        .map(|item| item.pricing)
        .collect::<Vec<_>>();
    write_json_fixture(&root, GOOGLE_MODELS_JSON_PRICING, &pricing, &mut written)?;

    println!(
        "✔ refreshed direct Google model catalog (models={})",
        parsed.data.len()
    );
    for path in written {
        println!("  wrote {path}");
    }
    Ok(())
}

fn write_json_fixture<T: Serialize + ?Sized>(
    root: &Path,
    rel_path: &str,
    value: &T,
    written: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let body = serde_json::to_string_pretty(value).map_err(|err| {
        XtaskError::new(format!("Failed to serialize fixture {}: {err}", rel_path))
    })?;
    write_text_fixture(root, rel_path, &body, written)
}

fn write_text_fixture(
    root: &Path,
    rel_path: &str,
    body: &str,
    written: &mut Vec<String>,
) -> Result<(), XtaskError> {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            XtaskError::new(format!(
                "Unable to create fixture directory {}: {err}",
                parent.display()
            ))
        })?;
    }
    fs::write(&path, body)
        .map_err(|err| XtaskError::new(format!("Failed to write {}: {err}", path.display())))?;
    written.push(display_relative(&path, root));
    Ok(())
}

fn write_embedding_metadata(root: &Path, sha: &str) -> Result<(), XtaskError> {
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
            XtaskError::new(format!(
                "Unable to create metadata directory {}: {err}",
                parent.display()
            ))
        })?;
    }

    let body = serde_json::to_string_pretty(&meta)
        .map_err(|err| XtaskError::new(format!("Serialize metadata: {err}")))?;
    fs::write(&meta_path, body).map_err(|err| {
        XtaskError::new(format!(
            "Failed to write metadata {}: {err}",
            display_relative(&meta_path, root)
        ))
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
