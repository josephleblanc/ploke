use chrono::{SecondsFormat, Utc};
use ploke_common::{
    LEGACY_FIXTURE_BACKUP_REL_PATH, LEGACY_FIXTURE_METADATA_REL_PATH,
    MULTI_EMBED_FIXTURE_BACKUP_REL_PATH, MULTI_EMBED_FIXTURE_METADATA_REL_PATH,
    MULTI_EMBED_SCHEMA_TAG,
};
use ploke_db::{
    Database, DbError,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::{Command, ExitCode},
};
use walkdir::WalkDir;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("embedding:collect-evidence") => collect_embedding_evidence(args.collect()),
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
         Commands:\n  verify-fixtures           Ensure required local test assets are staged [--multi-embedding]\n  embedding:collect-evidence  Collect embedding slice evidence [--slice 2]\n  help                      Show this message"
    );
}

struct FixtureCheck {
    id: &'static str,
    rel_path: &'static str,
    description: &'static str,
    remediation: &'static str,
    integrity: Option<FixtureIntegrity>,
    auto_regen: Option<&'static FixtureRegen>,
}

#[derive(Clone, Copy)]
struct FixtureIntegrity {
    metadata_rel_path: &'static str,
}

struct FixtureRegen {
    program: &'static str,
    args: &'static [&'static str],
    description: &'static str,
}

#[derive(Default)]
struct VerifyOptions {
    multi_embedding: bool,
}

const LEGACY_FIXTURE_REGEN: FixtureRegen = FixtureRegen {
    program: "cargo",
    args: &[
        "run",
        "-p",
        "ploke-test-utils",
        "--bin",
        "regenerate_fixture",
        "--features",
        "test_setup",
        "--",
        "--schema",
        "legacy",
    ],
    description: "Regenerating legacy fixture backup",
};

const MULTI_FIXTURE_REGEN: FixtureRegen = FixtureRegen {
    program: "cargo",
    args: &[
        "run",
        "-p",
        "ploke-test-utils",
        "--bin",
        "regenerate_fixture",
        "--features",
        "test_setup multi_embedding_schema",
        "--",
        "--schema",
        "multi",
    ],
    description: "Regenerating multi-embedding fixture backup",
};

const FIXTURE_DB_CHECK: FixtureCheck = FixtureCheck {
    id: "fixture_db_backup",
    rel_path: LEGACY_FIXTURE_BACKUP_REL_PATH,
    description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
    remediation: "Restore via `/save db tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`.",
    integrity: Some(FixtureIntegrity {
        metadata_rel_path: LEGACY_FIXTURE_METADATA_REL_PATH,
    }),
    // NOTE: The legacy backup is treated as a golden artifact built via the canonical
    // TUI indexing + `/save db` flow. We intentionally do NOT auto-regenerate it here
    // so tests depending on real embedding semantics (e.g. ploke-rag dense search)
    // continue to use the vetted backup instead of a synthetic seed.
    auto_regen: None,
};

const PRICING_CHECK: FixtureCheck = FixtureCheck {
    id: "pricing_json",
    rel_path: "crates/ploke-tui/data/models/all_pricing_parsed.json",
    description: "Pricing metadata consumed by llm::request::pricing tests.",
    remediation: "Run `./scripts/openrouter_pricing_sync.py` to fetch the latest OpenRouter pricing payload.",
    integrity: None,
    auto_regen: None,
};

const MULTI_FIXTURE_DB_CHECK: FixtureCheck = FixtureCheck {
    id: "fixture_db_backup_multi",
    rel_path: MULTI_EMBED_FIXTURE_BACKUP_REL_PATH,
    description: "Multi-embedding CozoDB backup used by schema/db feature-flag tests.",
    remediation: "Regenerate via `cargo run -p ploke-test-utils --bin regenerate_fixture --features \"multi_embedding_schema\" -- --schema multi`.",
    integrity: Some(FixtureIntegrity {
        metadata_rel_path: MULTI_EMBED_FIXTURE_METADATA_REL_PATH,
    }),
    auto_regen: Some(&MULTI_FIXTURE_REGEN),
};

const FIXTURE_CHECKS: &[FixtureCheck] = &[FIXTURE_DB_CHECK, PRICING_CHECK];

const MULTI_FIXTURE_ARTIFACT_REL_PATH: &str =
    "target/test-output/embedding/fixtures/multi_embedding_fixture_verification.json";
const SLICE1_ARTIFACT_REL_PATH: &str = "target/test-output/embedding/slice1-schema.json";
const SLICE2_ARTIFACT_REL_PATH: &str = "target/test-output/embedding/slice2-db.json";
const SLICE3_ARTIFACT_REL_PATH: &str = "target/test-output/embedding/slice3-runtime.json";


fn attempt_auto_regen(
    check: &FixtureCheck,
    root: &Path,
    regen_attempted: &mut bool,
    reason: &str,
) -> bool {
    if *regen_attempted {
        return false;
    }
    let regen = match check.auto_regen {
        Some(regen) => regen,
        None => return false,
    };
    println!("↺ {:<18} {} ({})", check.id, regen.description, reason);
    match run_regen_command(root, regen) {
        Ok(()) => {
            *regen_attempted = true;
            true
        }
        Err(err) => {
            *regen_attempted = true;
            eprintln!(
                "Auto-regeneration for {} failed: {}.\nManual fix: {}",
                check.id, err, check.remediation
            );
            false
        }
    }
}

fn run_regen_command(root: &Path, regen: &FixtureRegen) -> Result<(), String> {
    let mut cmd = Command::new(regen.program);
    cmd.args(regen.args).current_dir(root);
    let status = cmd
        .status()
        .map_err(|err| format!("failed to spawn {}: {}", regen.program, err))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{} exited with status {}", regen.program, status))
    }
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

#[derive(Serialize, Deserialize, Clone)]
struct TestRun {
    name: String,
    command: String,
    status: String,
    pass_count: u32,
    fail_count: u32,
    ignored: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct FlagValidationEntry {
    tier: String,
    command: String,
    outcome: String,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SliceEvidence {
    slice: u8,
    generated_at: String,
    feature_flags: Vec<String>,
    tests: Vec<TestRun>,
    artifacts: Vec<String>,
    live: bool,
    tool_calls_observed: u32,
    notes: String,
    #[serde(default)]
    flag_validation: Vec<FlagValidationEntry>,
}

#[derive(Serialize)]
struct MultiEmbeddingFixtureTelemetry {
    slice: u8,
    generated_at: String,
    command: String,
    feature_flags: Vec<String>,
    tests: Vec<TestRun>,
    artifacts: Vec<String>,
    live: bool,
    tool_calls_observed: u32,
    notes: String,
    metadata_relations: usize,
    vector_relations: usize,
    metadata_rows: usize,
    vector_rows: usize,
}


fn persist_multi_embedding_evidence(
    root: &Path,
    report: &MultiEmbeddingReport,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let fixture_test = TestRun {
        name: "Multi-embedding fixture verification".into(),
        command: "cargo xtask verify-fixtures --multi-embedding".into(),
        status: "pass".into(),
        pass_count: 1,
        fail_count: 0,
        ignored: 0,
    };
    let feature_flags = default_feature_flags();
    let fixture_artifact = MultiEmbeddingFixtureTelemetry {
        slice: 1,
        generated_at: now.clone(),
        command: fixture_test.command.clone(),
        feature_flags: feature_flags.clone(),
        tests: vec![fixture_test.clone()],
        artifacts: vec![MULTI_FIXTURE_ARTIFACT_REL_PATH.to_string()],
        live: false,
        tool_calls_observed: 0,
        notes: format!(
            "Verified {} metadata rows and {} vector rows across {} relations (schema tag {}).",
            report.total_metadata_rows,
            report.total_vector_rows,
            report.metadata_relations + report.vector_relations,
            MULTI_EMBED_SCHEMA_TAG
        ),
        metadata_relations: report.metadata_relations,
        vector_relations: report.vector_relations,
        metadata_rows: report.total_metadata_rows,
        vector_rows: report.total_vector_rows,
    };
    write_pretty_json(root, MULTI_FIXTURE_ARTIFACT_REL_PATH, &fixture_artifact)?;
    update_slice1_artifact(root, &now, fixture_test, feature_flags, report)
}

fn update_slice1_artifact(
    root: &Path,
    generated_at: &str,
    verify_test: TestRun,
    feature_flags: Vec<String>,
    report: &MultiEmbeddingReport,
) -> Result<(), String> {
    let path = root.join(SLICE1_ARTIFACT_REL_PATH);
    let mut evidence = if path.exists() {
        let contents = fs::read_to_string(&path)
            .map_err(|err| format!("Failed to read {}: {err}", display_relative(&path, root)))?;
        serde_json::from_str::<SliceEvidence>(&contents)
            .map_err(|err| format!("Failed to parse {}: {err}", display_relative(&path, root)))?
    } else {
        default_slice1_evidence(generated_at)
    };
    evidence.generated_at = generated_at.to_string();
    merge_unique(&mut evidence.feature_flags, feature_flags);
    merge_unique(
        &mut evidence.artifacts,
        vec![MULTI_FIXTURE_ARTIFACT_REL_PATH.to_string()],
    );
    upsert_test(&mut evidence.tests, verify_test);
    let new_note = format!(
        "Multi-embedding fixtures refreshed on {} ({} metadata rows / {} vector rows).",
        generated_at, report.total_metadata_rows, report.total_vector_rows
    );
    evidence.notes = append_note(&evidence.notes, &new_note);
    write_pretty_json(root, SLICE1_ARTIFACT_REL_PATH, &evidence)
}

fn default_slice1_evidence(generated_at: &str) -> SliceEvidence {
    SliceEvidence {
        slice: 1,
        generated_at: generated_at.to_string(),
        feature_flags: default_feature_flags(),
        tests: Vec::new(),
        artifacts: vec![MULTI_FIXTURE_ARTIFACT_REL_PATH.to_string()],
        live: false,
        tool_calls_observed: 0,
        notes: String::new(),
        flag_validation: Vec::new(),
    }
}

fn default_feature_flags() -> Vec<String> {
    vec![
        "ploke-db:multi_embedding_schema".into(),
        format!("fixtures:{}", MULTI_EMBED_SCHEMA_TAG),
    ]
}

fn merge_unique(list: &mut Vec<String>, additions: Vec<String>) {
    for value in additions {
        if !list.iter().any(|existing| existing == &value) {
            list.push(value);
        }
    }
}

fn upsert_test(tests: &mut Vec<TestRun>, new_test: TestRun) {
    if let Some(existing) = tests
        .iter_mut()
        .find(|test| test.command == new_test.command)
    {
        *existing = new_test;
    } else {
        tests.push(new_test);
    }
}

fn append_note(existing: &str, new_note: &str) -> String {
    if existing.trim().is_empty() {
        new_note.to_string()
    } else if existing.contains(new_note) {
        existing.to_string()
    } else {
        format!("{existing}\n{new_note}")
    }
}

fn write_pretty_json<T: ?Sized + Serialize>(
    root: &Path,
    rel_path: &str,
    value: &T,
) -> Result<(), String> {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "Failed to create artifact directory {}: {err}",
                display_relative(parent, root)
            )
        })?;
    }
    let json = serde_json::to_string_pretty(value)
        .map_err(|err| format!("Failed to serialize JSON: {err}"))?;
    fs::write(&path, json)
        .map_err(|err| format!("Failed to write {}: {err}", display_relative(&path, root)))
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

fn collect_embedding_evidence(args: Vec<String>) -> ExitCode {
    let root = workspace_root();
    let mut slice: Option<u8> = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        if arg == "--slice" {
            if let Some(value) = iter.next() {
                match value.parse::<u8>() {
                    Ok(n) => slice = Some(n),
                    Err(_) => {
                        eprintln!("Invalid value for --slice: {value}");
                        return ExitCode::FAILURE;
                    }
                }
            } else {
                eprintln!("Missing value for --slice");
                return ExitCode::FAILURE;
            }
        } else if arg == "--help" || arg == "-h" {
            println!("cargo xtask embedding:collect-evidence --slice 2");
            return ExitCode::SUCCESS;
        } else {
            eprintln!("Unknown embedding:collect-evidence flag '{arg}'. Use --help for usage.");
            return ExitCode::FAILURE;
        }
    }

    let slice = slice.unwrap_or(2);

    match slice {
        2 => collect_slice2_evidence(&root),
        3 => collect_slice3_evidence(&root),
        other => {
            eprintln!(
                "Only Slice 2 and Slice 3 evidence collection are currently implemented (requested slice {other})."
            );
            ExitCode::FAILURE
        }
    }
}

fn collect_slice2_evidence(root: &Path) -> ExitCode {
    println!(
        "Collecting Slice 2 embedding evidence under {}",
        root.display()
    );

    let mut tests: Vec<TestRun> = Vec::new();
    let mut flag_validation: Vec<FlagValidationEntry> = Vec::new();
    let mut any_failed = false;

    // Validation Matrix commands for Slice 2 (schema/db/runtime tiers).
    let commands: &[(&str, &str, &[&str])] = &[
        (
            "multi_embedding_schema",
            "ploke-db schema tier",
            &[
                "test",
                "-p",
                "ploke-db",
                "--features",
                "multi_embedding_schema",
            ],
        ),
        (
            "multi_embedding_schema",
            "ploke-test-utils schema tier",
            &[
                "test",
                "-p",
                "ploke-test-utils",
                "--features",
                "multi_embedding_schema",
            ],
        ),
        (
            "multi_embedding_db",
            "ploke-db dual-write tier",
            &["test", "-p", "ploke-db", "--features", "multi_embedding_db"],
        ),
        (
            "multi_embedding_db",
            "ploke-test-utils dual-write tier",
            &[
                "test",
                "-p",
                "ploke-test-utils",
                "--features",
                "multi_embedding_db",
            ],
        ),
        (
            "multi_embedding_runtime",
            "ploke-tui runtime tier (load_db_crate_focus)",
            &[
                "test",
                "-p",
                "ploke-tui",
                "--features",
                "multi_embedding_runtime",
                "--test",
                "load_db_crate_focus",
            ],
        ),
    ];

    for (tier, name, args) in commands {
        let (test_run, validation) = run_validation_command(root, tier, name, args);
        if test_run.status != "pass" {
            any_failed = true;
        }
        println!("[{}] {} -> {}", tier, test_run.command, test_run.status);
        tests.push(test_run);
        flag_validation.push(validation);
    }

    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let feature_flags = vec![
        "ploke-db:multi_embedding_schema".into(),
        "ploke-db:multi_embedding_db".into(),
        "ploke-tui:multi_embedding_runtime".into(),
    ];
    let notes = "Slice 2 dual-write + HNSW helpers validated via ploke-db and ploke-test-utils tests; ploke-tui runtime tier currently compiles but has limited executed coverage."
        .into();
    let evidence = SliceEvidence {
        slice: 2,
        generated_at: now,
        feature_flags,
        tests,
        artifacts: vec![SLICE2_ARTIFACT_REL_PATH.to_string()],
        live: false,
        tool_calls_observed: 0,
        notes,
        flag_validation,
    };

    if let Err(err) = write_pretty_json(root, SLICE2_ARTIFACT_REL_PATH, &evidence) {
        eprintln!("Failed to write Slice 2 evidence: {err}");
        return ExitCode::FAILURE;
    }

    if any_failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn collect_slice3_evidence(root: &Path) -> ExitCode {
    println!(
        "Collecting Slice 3 embedding runtime evidence under {}",
        root.display()
    );

    let mut tests: Vec<TestRun> = Vec::new();
    let mut flag_validation: Vec<FlagValidationEntry> = Vec::new();
    let mut any_failed = false;

    // For Slice 3 we currently focus on runtime/indexer tiers. As additional
    // indexer + TEST_APP harness tests come online they should be added here.
    let commands: &[(&str, &str, &[&str])] = &[
        (
            "multi_embedding_runtime",
            "ploke-tui runtime tier (multi_embedding_runtime_db_tests)",
            &[
                "test",
                "-p",
                "ploke-tui",
                "--features",
                "multi_embedding_runtime",
                "--test",
                "multi_embedding_runtime_db_tests",
            ],
        ),
    ];

    for (tier, name, args) in commands {
        let (test_run, validation) = run_validation_command(root, tier, name, args);
        if test_run.status != "pass" {
            any_failed = true;
        }
        println!("[{}] {} -> {}", tier, test_run.command, test_run.status);
        tests.push(test_run);
        flag_validation.push(validation);
    }

    let now = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let feature_flags = vec!["ploke-tui:multi_embedding_runtime".into()];
    let notes =
        "Slice 3 runtime evidence (offline) collected from ploke-tui multi_embedding_runtime tests."
            .into();
    let evidence = SliceEvidence {
        slice: 3,
        generated_at: now,
        feature_flags,
        tests,
        artifacts: vec![SLICE3_ARTIFACT_REL_PATH.to_string()],
        live: false,
        tool_calls_observed: 0,
        notes,
        flag_validation,
    };

    if let Err(err) = write_pretty_json(root, SLICE3_ARTIFACT_REL_PATH, &evidence) {
        eprintln!("Failed to write Slice 3 evidence: {err}");
        return ExitCode::FAILURE;
    }

    if any_failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run_validation_command(
    root: &Path,
    tier: &str,
    name: &str,
    cargo_args: &[&str],
) -> (TestRun, FlagValidationEntry) {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(root).args(cargo_args);
    let output = cmd.output();

    let (status, status_str, outcome_str, note) = match output {
        Ok(out) => {
            let success = out.status.success();
            let stdout = String::from_utf8_lossy(&out.stdout);

            // Detect whether any tests actually ran by summing the counts from
            // lines like "running N tests". This avoids treating crates with a mix
            // of real tests and empty doc-tests as "zero tests executed".
            let ran_any_tests = stdout
                .lines()
                .filter_map(|line| line.trim().strip_prefix("running "))
                .filter_map(|rest| rest.split_whitespace().next())
                .filter_map(|num| num.parse::<u32>().ok())
                .any(|n| n > 0);

            let mut note = None;
            if success && !ran_any_tests {
                note = Some("compiled but ran zero tests under this flag".into());
            }

            let status_str = if success { "pass" } else { "fail" };
            let outcome_str = status_str;
            (
                success,
                status_str.to_string(),
                outcome_str.to_string(),
                note,
            )
        }
        Err(err) => {
            eprintln!("Failed to run command for tier {tier} ({name}): {err}");
            (false, "compile_error".into(), "compile_error".into(), None)
        }
    };

    let command_str = format!("cargo {}", cargo_args.join(" "));
    let test_run = TestRun {
        name: name.to_string(),
        command: command_str.clone(),
        status: status_str,
        pass_count: if status { 1 } else { 0 },
        fail_count: if status { 0 } else { 1 },
        ignored: 0,
    };

    let validation = FlagValidationEntry {
        tier: tier.to_string(),
        command: command_str,
        outcome: outcome_str,
        note,
    };

    (test_run, validation)
}
