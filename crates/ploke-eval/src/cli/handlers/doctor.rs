use std::path::PathBuf;

use ploke_llm::Router;
use ploke_llm::router_only::google::Google;
use ploke_llm::router_only::openrouter::OpenRouter;

use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, instances_dir, model_registry_file,
    models_dir, parent_patcher_model_file, repos_dir, starting_db_cache_dir,
};
use crate::model_registry::{
    load_active_model, load_model_registry, load_parent_patcher_model, registry_has_model,
};
use crate::registry::builtin_dataset_registry_entries;
use crate::spec::PrepareError;

pub(crate) fn run_doctor() -> Result<(), PrepareError> {
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut note = 0usize;

    println!("ploke-eval doctor");
    println!();

    let home = crate::layout::ploke_eval_home()?;
    println!("home: {}", home.display());

    let builtins = builtin_dataset_registry_entries();
    println!(
        "built-in datasets: {}{}",
        builtins.len(),
        if builtins.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                builtins
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );

    check_dir(
        "datasets dir",
        datasets_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "models dir",
        models_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "repo cache dir",
        repos_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "run artifacts dir",
        instances_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "batch artifacts dir",
        batches_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "starting-db cache dir",
        starting_db_cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "cache dir",
        cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );

    match load_model_registry() {
        Ok(registry) => {
            ok += 1;
            println!(
                "[ok] model registry: {} models ({})",
                registry.data.len(),
                model_registry_file()?.display()
            );
        }
        Err(PrepareError::MissingModelRegistry(path)) => {
            warn += 1;
            println!("[warn] model registry: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model list",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_active_model() {
        Ok(active) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &active.model_id) {
                    ok += 1;
                    println!(
                        "[ok] active model: {} ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] active model: {} is not present in the current registry ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] active model: {} ({})",
                    active.model_id,
                    active_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingActiveModel(path)) => {
            warn += 1;
            println!("[warn] active model: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_parent_patcher_model() {
        Ok(selected) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &selected.model_id) {
                    ok += 1;
                    println!(
                        "[ok] parent patcher model: {} ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] parent patcher model: {} is not present in the current registry ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] parent patcher model: {} ({})",
                    selected.model_id,
                    parent_patcher_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingParentPatcherModel(path)) => {
            warn += 1;
            println!(
                "[warn] parent patcher model: missing ({}); broad harness will fall back to the active model",
                path.display()
            );
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match OpenRouter::resolve_api_key() {
        Ok(_) => {
            ok += 1;
            println!("[ok] OpenRouter API key: present");
        }
        Err(err) => {
            warn += 1;
            println!("[warn] OpenRouter API key: unavailable ({err})");
        }
    }

    match Google::route_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google route config: GOOGLE_PROJECT_ID and GOOGLE_REGION available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google route config: unavailable ({err})");
        }
    }

    match Google::auth_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google auth: ADC or explicit bearer token available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google auth: unavailable ({err})");
        }
    }

    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            ok += 1;
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("[ok] git: {version}");
        }
        Ok(output) => {
            warn += 1;
            println!(
                "[warn] git: command exited with status {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Err(err) => {
            warn += 1;
            println!("[warn] git: unavailable ({err})");
        }
    }

    println!();
    println!(
        "summary: {ok} ok, {warn} warning{}, {note} note{}",
        if warn == 1 { "" } else { "s" },
        if note == 1 { "" } else { "s" }
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum MissingDirStatus {
    Warn,
    Note,
}

fn check_dir(
    label: &str,
    path: PathBuf,
    ok: &mut usize,
    warn: &mut usize,
    note: &mut usize,
    missing_status: MissingDirStatus,
) {
    if path.exists() {
        if path.is_dir() {
            *ok += 1;
            println!("[ok] {label}: {}", path.display());
        } else {
            *warn += 1;
            println!(
                "[warn] {label}: exists but is not a directory ({})",
                path.display()
            );
        }
    } else {
        match missing_status {
            MissingDirStatus::Warn => {
                *warn += 1;
                println!("[warn] {label}: missing ({})", path.display());
            }
            MissingDirStatus::Note => {
                *note += 1;
                println!("[note] {label}: missing ({})", path.display());
            }
        }
    }
}

fn print_advice(lines: &[&str]) {
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            println!("  next: {line}");
        } else {
            println!("  then: {line}");
        }
    }
}
