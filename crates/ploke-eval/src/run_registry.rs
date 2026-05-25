use std::fs;
use std::path::{Path, PathBuf};

use crate::inner::core::{RegisteredRunRole, RunIntent, RunStorageRoots};
use crate::inner::registry::{
    RunLifecyclePhase, RunPhaseStatus, RunRegistration, RunRegistrationError,
};
use crate::layout::registries_dir;
use crate::spec::PrepareError;

pub use crate::inner::registry::{RunArtifactRefs, RunLifecycle, RunPhaseLifecycle};
pub use crate::inner::registry::{RunExecutionStatus, RunSubmissionStatus};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedProtocolRunIdentity {
    pub record_path: PathBuf,
    pub run_dir: PathBuf,
    pub run_id: String,
    pub subject_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunSelectionPreference {
    LatestAny,
    PreferTreatment,
    PreferTreatmentWithSubmission,
}

pub fn storage_roots_for_instance(instance_dir: &Path) -> Result<RunStorageRoots, PrepareError> {
    Ok(RunStorageRoots::new(
        registries_dir()?,
        artifact_runs_dir(instance_dir),
    ))
}

pub fn register_live_run(
    intent: RunIntent,
    run_id: impl Into<String>,
) -> Result<RunRegistration, PrepareError> {
    let registration = RunRegistration::register_with_run_id(intent, run_id.into())
        .map_err(map_registration_error)?;
    registration.persist().map_err(map_registration_error)?;
    Ok(registration)
}

pub fn persist_registration(registration: &RunRegistration) -> Result<(), PrepareError> {
    registration.persist().map_err(map_registration_error)
}

pub fn load_registration_for_run_dir(
    run_dir: &Path,
) -> Result<Option<RunRegistration>, PrepareError> {
    let Some(run_id) = run_id_from_run_dir(run_dir) else {
        return Ok(None);
    };
    let path = registries_dir()?
        .join("runs")
        .join(format!("{run_id}.json"));
    match RunRegistration::load(&path) {
        Ok(registration) => Ok(Some(registration)),
        Err(RunRegistrationError::Read { .. }) => Ok(None),
        Err(err) => Err(map_registration_error(err)),
    }
}

pub fn load_registration_for_record_path(
    record_path: &Path,
) -> Result<Option<RunRegistration>, PrepareError> {
    let run_dir = record_path
        .parent()
        .ok_or_else(|| PrepareError::MissingRunManifest(record_path.to_path_buf()))?;
    load_registration_for_run_dir(run_dir)
}

pub fn resolve_protocol_run_identity(
    record_path: &Path,
) -> Result<ResolvedProtocolRunIdentity, PrepareError> {
    let run_dir = record_path
        .parent()
        .ok_or_else(|| PrepareError::MissingRunManifest(record_path.to_path_buf()))?;

    if let Some(registration) = load_registration_for_record_path(record_path)? {
        return Ok(ResolvedProtocolRunIdentity {
            record_path: registration.artifacts.record_path.clone(),
            run_dir: registration.artifacts.run_root.clone(),
            run_id: registration.run_id,
            subject_id: registration.frozen_spec.task_id,
        });
    }

    let run_id = run_id_from_run_dir(run_dir).unwrap_or_else(|| "run".to_string());
    let record = crate::record::read_compressed_record(record_path).map_err(|source| {
        PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        }
    })?;

    Ok(ResolvedProtocolRunIdentity {
        record_path: record_path.to_path_buf(),
        run_dir: run_dir.to_path_buf(),
        run_id,
        subject_id: record.metadata.benchmark.instance_id,
    })
}

pub fn list_registrations_for_instance(
    instances_root: &Path,
    instance_id: &str,
) -> Result<Vec<RunRegistration>, PrepareError> {
    let mut registrations = Vec::new();
    let expected_runs_dir = instances_root.join(instance_id).join("runs");
    for registry_root in registry_roots_for_instances_root(instances_root)? {
        let entries = match fs::read_dir(&registry_root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(PrepareError::ReadManifest {
                    path: registry_root,
                    source,
                });
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let registration = match RunRegistration::load(&path) {
                Ok(registration) => registration,
                Err(_) => continue,
            };
            if registration.frozen_spec.task_id != instance_id {
                continue;
            }
            if registration.frozen_spec.storage_roots.runs_dir != expected_runs_dir {
                continue;
            }
            registrations.push(registration);
        }
    }

    registrations
        .sort_by(|left, right| registration_sort_key(right).cmp(&registration_sort_key(left)));
    Ok(registrations)
}

fn registry_roots_for_instances_root(instances_root: &Path) -> Result<Vec<PathBuf>, PrepareError> {
    let mut roots = Vec::new();
    if let Ok(root) = registries_dir() {
        roots.push(root.join("runs"));
    }
    if let Some(eval_home) = instances_root.parent() {
        roots.push(eval_home.join("registries").join("runs"));
    }
    roots.sort();
    roots.dedup();
    Ok(roots)
}

pub fn preferred_registration_for_instance(
    instances_root: &Path,
    instance_id: &str,
    preference: RunSelectionPreference,
) -> Result<Option<RunRegistration>, PrepareError> {
    let registrations = list_registrations_for_instance(instances_root, instance_id)?;
    let mut first_any = None;

    for registration in registrations {
        if first_any.is_none() {
            first_any = Some(registration.clone());
        }
        if registration_matches_preference(&registration, preference) {
            return Ok(Some(registration));
        }
    }

    Ok(first_any)
}

pub fn completed_record_paths_for_instances_root(
    instances_root: &Path,
) -> Result<Vec<PathBuf>, PrepareError> {
    let mut paths = Vec::new();
    for registry_root in registry_roots_for_instances_root(instances_root)? {
        let entries = match fs::read_dir(&registry_root) {
            Ok(entries) => entries,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => continue,
            Err(source) => {
                return Err(PrepareError::ReadManifest {
                    path: registry_root,
                    source,
                });
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            let registration = match RunRegistration::load(&path) {
                Ok(registration) => registration,
                Err(_) => continue,
            };
            if registration.lifecycle.execution_status != RunExecutionStatus::Completed {
                continue;
            }
            if !registration.artifacts.run_root.starts_with(instances_root) {
                continue;
            }
            paths.push(registration.artifacts.record_path.clone());
        }
    }

    paths.sort();
    Ok(paths)
}

pub fn sync_protocol_registration_status(record_path: &Path) -> Result<(), PrepareError> {
    let Some(mut registration) = load_registration_for_record_path(record_path)? else {
        return Ok(());
    };

    registration.artifacts.protocol_artifacts_dir =
        crate::layout::protocol_artifacts_dir_for_run(&registration.run_root());
    let artifacts = crate::protocol_artifacts::list_protocol_artifacts(record_path)?;
    if artifacts.is_empty() {
        registration.update_phase(
            RunLifecyclePhase::Protocol,
            RunPhaseStatus::NotStarted,
            Option::<String>::None,
        );
        registration.update_protocol_anchor(None);
        return persist_registration(&registration);
    }

    let latest = artifacts
        .iter()
        .max_by_key(|artifact| artifact.stored.created_at_ms)
        .map(|artifact| artifact.path.clone());
    registration.update_protocol_anchor(latest);

    match crate::protocol::protocol_aggregate::load_protocol_aggregate(record_path) {
        Ok(_) => registration.update_phase(
            RunLifecyclePhase::Protocol,
            RunPhaseStatus::Completed,
            Some("protocol aggregate available".to_string()),
        ),
        Err(crate::protocol::protocol_aggregate::ProtocolAggregateError::MissingAnchor {
            ..
        }) => {
            registration.update_phase(
                RunLifecyclePhase::Protocol,
                RunPhaseStatus::InProgress,
                Some("protocol artifacts exist but aggregate is not yet complete".to_string()),
            );
        }
        Err(err) => {
            registration.update_phase(
                RunLifecyclePhase::Protocol,
                RunPhaseStatus::Failed,
                Some(err.to_string()),
            );
        }
    }

    persist_registration(&registration)
}

fn artifact_runs_dir(instance_dir: &Path) -> PathBuf {
    instance_dir.join("runs")
}

fn run_id_from_run_dir(run_dir: &Path) -> Option<String> {
    run_dir
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn registration_matches_preference(
    registration: &RunRegistration,
    preference: RunSelectionPreference,
) -> bool {
    match preference {
        RunSelectionPreference::LatestAny => true,
        RunSelectionPreference::PreferTreatment => {
            registration.frozen_spec.run_role == RegisteredRunRole::Treatment
        }
        RunSelectionPreference::PreferTreatmentWithSubmission => {
            registration.frozen_spec.run_role == RegisteredRunRole::Treatment
                && matches!(
                    registration.lifecycle.submission_status,
                    RunSubmissionStatus::NonemptyPatch | RunSubmissionStatus::EmptyPatch
                )
        }
    }
}

fn registration_sort_key(registration: &RunRegistration) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

fn map_registration_error(error: RunRegistrationError) -> PrepareError {
    match error {
        RunRegistrationError::Fingerprint { source }
        | RunRegistrationError::Serialize { source } => PrepareError::Serialize(source),
        RunRegistrationError::CreateDir { path, source } => {
            PrepareError::CreateOutputDir { path, source }
        }
        RunRegistrationError::Write { path, source } => {
            PrepareError::WriteManifest { path, source }
        }
        RunRegistrationError::Read { path, source } => PrepareError::ReadManifest { path, source },
        RunRegistrationError::Parse { path, source } => {
            PrepareError::ParseManifest { path, source }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner::core::RegisteredRunRole;
    use crate::spec::EvalBudget;
    use std::ffi::OsString;
    use std::path::Path;
    use tempfile::tempdir;

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

    fn sample_intent(base: &Path, instances_root: &Path) -> RunIntent {
        RunIntent {
            task_id: "org__repo-1".to_string(),
            repo_root: base.join("repo"),
            storage_roots: RunStorageRoots::new(
                base.join("registries"),
                instances_root.join("org__repo-1").join("runs"),
            ),
            base_sha: Some("deadbeef".to_string()),
            budget: EvalBudget::default(),
            model_id: Some("model".to_string()),
            provider_slug: Some("provider".to_string()),
            campaign_id: None,
            batch_id: None,
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        }
    }

    fn write_test_run_record(path: &Path) {
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
        let record =
            crate::record::RunRecord::new(&prepared, crate::runner::RunArm::shell_only_control());
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("record dir");
        }
        crate::record::write_compressed_record(path, &record).expect("write record");
    }

    #[test]
    fn preferred_registration_uses_registration_store_before_dir_guessing() {
        let _env_lock = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = tempdir().expect("tmp");
        let env_guard = set_env_var_scoped("PLOKE_EVAL_HOME", tmp.path());
        let instances_root = tmp.path().join("instances");
        let intent = sample_intent(tmp.path(), &instances_root);
        let mut registration =
            RunRegistration::register_with_run_id(intent, "run-123").expect("registration");
        registration.lifecycle.execution_status = RunExecutionStatus::Completed;
        registration.update_submission_status(Some(""));
        registration.persist().expect("persist");

        let selected = preferred_registration_for_instance(
            &instances_root,
            "org__repo-1",
            RunSelectionPreference::PreferTreatment,
        )
        .expect("preferred")
        .expect("registration");
        assert_eq!(selected.run_id, "run-123");
        drop(env_guard);
    }

    #[test]
    fn resolve_protocol_run_identity_prefers_registration_authority() {
        let _env_lock = crate::test_support::env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let tmp = tempdir().expect("tmp");
        let env_guard = set_env_var_scoped("PLOKE_EVAL_HOME", tmp.path());
        let instances_root = tmp.path().join("instances");
        let intent = sample_intent(tmp.path(), &instances_root);
        let registration =
            RunRegistration::register_with_run_id(intent, "run-123").expect("registration");
        let record_path = registration.artifacts.record_path.clone();
        write_test_run_record(&record_path);
        registration.persist().expect("persist");

        let resolved = resolve_protocol_run_identity(&record_path).expect("resolved identity");
        assert_eq!(resolved.run_id, "run-123");
        assert_eq!(resolved.subject_id, "org__repo-1");
        assert_eq!(resolved.record_path, record_path);
        assert_eq!(resolved.run_dir, registration.artifacts.run_root);
        drop(env_guard);
    }
}
