use std::io::Write;
use std::path::{Path, PathBuf};

use crate::inner::registry::RunRegistration;
use crate::run_registry::{list_registrations_for_instance, load_registration_for_run_dir};
use crate::selection::{
    ActiveSelection, load_active_selection_at, render_selection_warnings,
};
use crate::spec::PrepareError;

#[derive(Debug, Clone)]
pub(super) struct RecordResolution {
    pub(super) record_path: PathBuf,
    pub(super) footer: Option<String>,
    pub(super) warnings: Vec<String>,
}

pub(super) fn has_legacy_instance_root_artifacts(instances_root: &Path, instance_id: &str) -> bool {
    let instance_root = instances_root.join(instance_id);
    [
        "record.json.gz",
        "execution-log.json",
        "repo-state.json",
        "indexing-status.json",
        "snapshot-status.json",
        "final-snapshot.db",
        "agent-turn-summary.json",
        "agent-turn-trace.json",
        "llm-full-responses.jsonl",
        "multi-swe-bench-submission.jsonl",
        "protocol-artifacts",
    ]
    .iter()
    .any(|name| instance_root.join(name).exists())
}

pub(super) fn legacy_instance_root_warning(instance_id: &str) -> String {
    format!(
        "instance {instance_id} still has legacy top-level run artifacts under ~/.ploke-eval/instances/{instance_id}; authoritative attempt data lives under runs/run-* and is selected from registrations"
    )
}

pub(super) fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
) -> Result<RecordResolution, PrepareError> {
    resolve_record_path_from_eval_home(record, instance, attempt, crate::layout::ploke_eval_home()?)
}

pub(super) fn resolve_record_path_from_eval_home(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
    eval_home: PathBuf,
) -> Result<RecordResolution, PrepareError> {
    let instances_root = eval_home.join("instances");
    let selection = load_active_selection_at(&eval_home)?;
    match (record, instance) {
        (Some(path), None) => Ok(RecordResolution {
            record_path: path,
            footer: None,
            warnings: Vec::new(),
        }),
        (None, explicit_instance) => {
            let resolved_instance = explicit_instance.or_else(|| selection.instance.clone());
            let (resolved_attempt, attempt_warning) =
                resolve_attempt_override(&selection, resolved_instance.as_deref(), attempt);
            if let Some(instance_id) = resolved_instance {
                let mut warnings = render_selection_warnings(&selection_with_resolution(
                    &selection,
                    Some(&instance_id),
                    resolved_attempt,
                ));
                if let Some(warning) = attempt_warning {
                    warnings.push(warning);
                }
                return resolve_instance_record_path(
                    &instances_root,
                    &instance_id,
                    resolved_attempt,
                    warnings,
                );
            }
            if resolved_attempt.is_some() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "an active attempt selection requires an active or explicit instance"
                        .to_string(),
                });
            }
            let last_run = crate::run_history::load_last_run_at(&eval_home)?;
            let record_path = last_run.run_dir.join("record.json.gz");
            let footer = match load_registration_for_run_dir(&last_run.run_dir)? {
                Some(registration) => {
                    let attempt = attempt_number_for_registration(
                        &instances_root,
                        &registration.frozen_spec.task_id,
                        &registration.run_id,
                    )?;
                    Some(format!(
                        "resolved run: {} attempt {} (latest)",
                        registration.frozen_spec.task_id, attempt
                    ))
                }
                None => Some("resolved run: most recent completed run".to_string()),
            };
            Ok(RecordResolution {
                record_path,
                footer,
                warnings: render_selection_warnings(&selection),
            })
        }
        (Some(_), Some(_)) => Err(PrepareError::MissingRunManifest(
            instances_root.join("<instance>/runs/run-*/record.json.gz"),
        )),
    }
}

fn resolve_attempt_override(
    selection: &ActiveSelection,
    resolved_instance: Option<&str>,
    explicit_attempt: Option<u32>,
) -> (Option<u32>, Option<String>) {
    if explicit_attempt.is_some() {
        return (explicit_attempt, None);
    }
    let Some(selected_attempt) = selection.attempt else {
        return (None, None);
    };
    let Some(selected_instance) = selection.instance.as_deref() else {
        return (
            None,
            Some("selected attempt was ignored because no selected instance is active".to_string()),
        );
    };
    match resolved_instance {
        Some(instance) if instance == selected_instance => (Some(selected_attempt), None),
        Some(instance) => (
            None,
            Some(format!(
                "selected attempt {} for instance {} was ignored because instance {} was requested",
                selected_attempt, selected_instance, instance
            )),
        ),
        None => (None, None),
    }
}

pub(super) fn selection_with_resolution(
    selection: &ActiveSelection,
    instance: Option<&str>,
    attempt: Option<u32>,
) -> ActiveSelection {
    let mut resolved = selection.clone();
    if let Some(instance) = instance {
        resolved.instance = Some(instance.to_string());
        resolved.attempt = attempt;
    }
    resolved
}

fn resolve_instance_record_path(
    instances_root: &Path,
    instance_id: &str,
    attempt: Option<u32>,
    mut warnings: Vec<String>,
) -> Result<RecordResolution, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    if !registrations.is_empty() {
        if has_legacy_instance_root_artifacts(instances_root, instance_id) {
            warnings.push(legacy_instance_root_warning(instance_id));
        }
        let selected_index = match attempt {
            Some(number) if number > 0 => {
                let index = (number - 1) as usize;
                if index >= registrations.len() {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "resolve_record_path",
                        detail: format!(
                            "instance {instance_id} has {} attempt(s); attempt {} is out of range",
                            registrations.len(),
                            number
                        ),
                    });
                }
                index
            }
            Some(_) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "attempt numbers are 1-based".to_string(),
                });
            }
            None => registrations.len() - 1,
        };
        let selected = &registrations[selected_index];
        return Ok(RecordResolution {
            record_path: selected.artifacts.record_path.clone(),
            footer: attempt.is_none().then(|| {
                format!(
                    "resolved run: {} attempt {} (latest)",
                    instance_id,
                    selected_index + 1
                )
            }),
            warnings,
        });
    }

    if attempt.is_some() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!(
                "instance {instance_id} has no registered attempts; cannot resolve a numbered attempt"
            ),
        });
    }

    Err(PrepareError::DatabaseSetup {
        phase: "resolve_record_path",
        detail: format!(
            "instance {instance_id} has no registered attempts; legacy instance-root artifacts are no longer used"
        ),
    })
}

pub(super) fn list_attempt_registrations(
    instances_root: &Path,
    instance_id: &str,
) -> Result<Vec<RunRegistration>, PrepareError> {
    let mut registrations = list_registrations_for_instance(instances_root, instance_id)?;
    registrations.sort_by(|left, right| {
        run_registration_sort_key(left).cmp(&run_registration_sort_key(right))
    });
    Ok(registrations)
}

fn attempt_number_for_registration(
    instances_root: &Path,
    instance_id: &str,
    run_id: &str,
) -> Result<usize, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    registrations
        .iter()
        .position(|registration| registration.run_id == run_id)
        .map(|index| index + 1)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!("run {run_id} is not registered under instance {instance_id}"),
        })
}

fn run_registration_sort_key(registration: &RunRegistration) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

pub(super) fn print_record_resolution_footer(resolution: &RecordResolution) {
    let _ = std::io::stdout().flush();
    for warning in &resolution.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(footer) = &resolution.footer {
        eprintln!("{footer}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
    use crate::inner::registry::RunRegistration;
    use crate::run_registry::RunExecutionStatus;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn hold_env_lock() -> std::sync::MutexGuard<'static, ()> {
        env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    struct EnvVarGuard {
        key: &'static str,
        prev: Option<OsString>,
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.prev.as_ref() {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    fn set_env_var_scoped(key: &'static str, value: impl Into<OsString>) -> EnvVarGuard {
        let prev = std::env::var_os(key);
        unsafe {
            std::env::set_var(key, value.into());
        }
        EnvVarGuard { key, prev }
    }

    fn write_test_run_record(path: &Path, run_arm: crate::runner::RunArm) {
        let prepared = crate::spec::PreparedSingleRun {
            task_id: "org__repo-1".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            output_dir: PathBuf::from("/tmp/output"),
            issue: crate::spec::IssueInput {
                title: None,
                body: None,
                body_path: None,
            },
            base_sha: None,
            head_sha: None,
            budget: crate::spec::EvalBudget::default(),
            source: None,
            campaign: None,
        };
        let record = crate::record::RunRecord::new(&prepared, run_arm);
        crate::record::write_compressed_record(path, &record).expect("write record");
    }

    fn sample_run_intent(base: &Path, instances_root: &Path) -> RunIntent {
        RunIntent {
            task_id: "org__repo-1".to_string(),
            repo_root: base.join("repo"),
            storage_roots: RunStorageRoots::new(
                base.join("registries"),
                instances_root.join("org__repo-1").join("runs"),
            ),
            base_sha: Some("deadbeef".to_string()),
            budget: crate::spec::EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: None,
            batch_id: None,
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        }
    }

    fn register_attempt(
        eval_home: &Path,
        run_id: &str,
        run_arm: crate::runner::RunArm,
        finished_at: &str,
    ) -> RunRegistration {
        let instances_root = eval_home.join("instances");
        let mut intent = sample_run_intent(eval_home, &instances_root);
        intent.run_arm_id = run_arm.id.clone();
        intent.run_role = match run_arm.role {
            crate::runner::RunArmRole::Control => RegisteredRunRole::Control,
            crate::runner::RunArmRole::Treatment => RegisteredRunRole::Treatment,
        };
        let mut registration =
            RunRegistration::register_with_run_id(intent, run_id).expect("registration");
        registration.lifecycle.execution_status = RunExecutionStatus::Completed;
        registration.lifecycle.finished_at = Some(finished_at.to_string());
        std::fs::create_dir_all(&registration.artifacts.run_root).expect("run root");
        write_test_run_record(&registration.artifacts.record_path, run_arm);
        registration.persist().expect("persist registration");
        registration
    }

    #[test]
    fn resolve_record_path_prefers_latest_registered_attempt_for_instance() {
        let _env_lock = hold_env_lock();
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let env_guard = set_env_var_scoped("PLOKE_EVAL_HOME", &eval_home);
        let older = register_attempt(
            &eval_home,
            "run-older",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:00:00Z",
        );
        let newer = register_attempt(
            &eval_home,
            "run-newer",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:05:00Z",
        );

        let resolution = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect("instance record path should resolve");

        assert_eq!(resolution.record_path, newer.artifacts.record_path);
        assert_ne!(resolution.record_path, older.artifacts.record_path);
        drop(env_guard);
    }

    #[test]
    fn resolve_record_path_defaults_to_latest_registered_attempt_even_if_control_is_newer() {
        let _env_lock = hold_env_lock();
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let env_guard = set_env_var_scoped("PLOKE_EVAL_HOME", &eval_home);
        let _treatment = register_attempt(
            &eval_home,
            "run-treatment",
            crate::runner::RunArm::structured_current_policy_treatment(),
            "2026-04-23T10:00:00Z",
        );
        let control = register_attempt(
            &eval_home,
            "run-control",
            crate::runner::RunArm::shell_only_control(),
            "2026-04-23T10:05:00Z",
        );

        let resolution = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect("instance record path should resolve");

        assert_eq!(resolution.record_path, control.artifacts.record_path);
        drop(env_guard);
    }

    #[test]
    fn resolve_record_path_rejects_legacy_instance_root_without_registration() {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let instance_root = eval_home.join("instances").join("org__repo-1");
        std::fs::create_dir_all(&instance_root).expect("instance root");
        write_test_run_record(
            &instance_root.join("record.json.gz"),
            crate::runner::RunArm::structured_current_policy_treatment(),
        );

        let err = resolve_record_path_from_eval_home(
            None,
            Some("org__repo-1".to_string()),
            None,
            eval_home,
        )
        .expect_err("legacy instance-root record should not resolve");

        let detail = err.to_string();
        assert!(detail.contains("has no registered attempts"));
        assert!(detail.contains("legacy instance-root artifacts are no longer used"));
    }

    #[test]
    fn resolve_record_path_defaults_to_last_run_record() {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let run_dir = eval_home.join("instances").join("demo-run");
        std::fs::create_dir_all(&run_dir).expect("run dir");
        crate::run_history::record_last_run_at(&eval_home, &run_dir).expect("record last run");

        let resolution = resolve_record_path_from_eval_home(None, None, None, eval_home)
            .expect("default record path should resolve");

        assert_eq!(resolution.record_path, run_dir.join("record.json.gz"));
    }
}

