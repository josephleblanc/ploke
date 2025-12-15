use chrono::Utc;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};
use walkdir::WalkDir;

const EMBEDDING_MODELS_URL: &str = "https://openrouter.ai/api/v1/embeddings/models";
const EMBEDDING_MODELS_FIXTURE: &str = "fixtures/openrouter/embeddings_models.json";
const EMBEDDING_MODELS_META: &str = "fixtures/openrouter/embeddings_models.meta.json";

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("verify-fixtures") => verify_fixtures(),
        Some("regen-embedding-models") => regen_embedding_models(),
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
         Commands:\n  verify-fixtures         Ensure required local test assets are staged\n  regen-embedding-models  Refresh fixtures/openrouter/embeddings_models.json from OpenRouter\n  help                    Show this message"
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
        rel_path: "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92",
        description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
        remediation: "Restore via `/save db tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`.",
        integrity: Some(FixtureIntegrity {
            metadata_rel_path: "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92.meta.json",
        }),
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

fn display_relative(path: &Path, root: &Path) -> String {
    match path.strip_prefix(root) {
        Ok(rel) => rel.display().to_string(),
        Err(_) => path.display().to_string(),
    }
}
