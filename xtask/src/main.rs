use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("verify-fixtures") => verify_fixtures(),
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
         Commands:\n  verify-fixtures    Ensure required local test assets are staged\n  help               Show this message"
    );
}

struct FixtureCheck {
    id: &'static str,
    rel_path: &'static str,
    description: &'static str,
    remediation: &'static str,
}

const FIXTURE_CHECKS: &[FixtureCheck] = &[
    FixtureCheck {
        id: "fixture_db_backup",
        rel_path: "tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92",
        description: "Required CozoDB backup used by AppHarness/apply_code_edit tests.",
        remediation: "Restore via `/save db tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92`.",
    },
    FixtureCheck {
        id: "pricing_json",
        rel_path: "crates/ploke-tui/data/models/all_pricing_parsed.json",
        description: "Pricing metadata consumed by llm::request::pricing tests.",
        remediation: "Run `./scripts/openrouter_pricing_sync.py` to fetch the latest OpenRouter pricing payload.",
    },
];

// TODO: add a dedicated command that regenerates or guides regeneration of pricing/state fixtures (e.g., `cargo xtask regen-pricing`) so this check can auto-heal.
fn verify_fixtures() -> ExitCode {
    let root = workspace_root();
    println!("Verifying fixtures under {}", root.display());
    let mut missing: Vec<(&FixtureCheck, PathBuf)> = Vec::new();

    for check in FIXTURE_CHECKS {
        let full_path = root.join(check.rel_path);
        if full_path.exists() {
            println!("✔ {:<18} {}", check.id, display_relative(&full_path, &root));
        } else {
            println!("✘ {:<18} {}", check.id, display_relative(&full_path, &root));
            missing.push((check, full_path));
        }
    }

    if missing.is_empty() {
        println!("All required fixtures are present.");
        ExitCode::SUCCESS
    } else {
        eprintln!("\nMissing fixtures detected:");
        for (check, path) in missing {
            eprintln!(
                "- {id}: {desc}\n  Path: {path}\n  Fix:  {remedy}",
                id = check.id,
                desc = check.description,
                path = path.display(),
                remedy = check.remediation
            );
        }
        ExitCode::FAILURE
    }
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
