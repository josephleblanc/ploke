use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use crate::closure::{ClosureClass, ClosureInstanceRow, load_closure_state};
use crate::intervention::{
    Prototype1NodeRecord, Prototype1RunnerDisposition, Prototype1RunnerResult,
    load_runner_result_at,
};
use crate::projection::OperatorProjectionRead;
use crate::record::read_compressed_record;
use crate::run_registry::{RunExecutionStatus, RunSubmissionStatus};
use crate::runner::MultiSweBenchSubmissionRecord;
use crate::spec::{PrepareError, PreparedSingleRun, RunSource};

pub const CONFIG_FILE: &str = "mbe-evaluation-config.json";
pub const EVALUATION_WORKDIR: &str = "evals";
pub const FINAL_REPORT_FILE: &str = "final_report.json";
pub const FIX_PATCH_RUN_LOG_FILE: &str = "fix-patch-run.log";
pub const HARNESS_MODULE: &str = "multi_swe_bench.harness.run_evaluation";
pub const INSTANCE_REPORT_FILE: &str = "report.json";
pub const SUBMISSION_FILE: &str = "multi-swe-bench-submission.jsonl";
const FIX_PATCH_RUN_LOG_READ_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub prepared: PreparedSingleRun,
    pub submission_path: PathBuf,
    pub layout: Layout,
    pub options: Options,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub workdir: PathBuf,
    pub output_dir: PathBuf,
    pub repo_dir: PathBuf,
    pub log_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Options {
    pub force_build: bool,
    pub need_clone: bool,
    pub clear_env: bool,
    pub stop_on_error: bool,
    pub workers: Workers,
    pub global_env: Vec<String>,
    pub fix_patch_run_cmd: String,
    pub log_level: String,
    pub log_to_console: bool,
    pub human_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workers {
    pub general: u32,
    pub build_image: u32,
    pub run_instance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Evaluation,
    Instance,
    InstanceOnly,
    Image,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessConfig {
    pub mode: Mode,
    pub workdir: PathBuf,
    pub patch_files: Vec<PathBuf>,
    pub dataset_files: Vec<PathBuf>,
    pub force_build: bool,
    pub output_dir: PathBuf,
    pub specifics: Vec<String>,
    pub skips: Vec<String>,
    pub repo_dir: PathBuf,
    pub need_clone: bool,
    pub global_env: Vec<String>,
    pub clear_env: bool,
    pub stop_on_error: bool,
    pub max_workers: u32,
    pub max_workers_build_image: u32,
    pub max_workers_run_instance: u32,
    pub fix_patch_run_cmd: String,
    pub log_dir: PathBuf,
    pub log_level: String,
    pub log_to_console: bool,
    pub human_mode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenConfig {
    pub path: PathBuf,
    pub report_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessInvocation {
    pub program: String,
    pub args: Vec<String>,
    pub config_path: PathBuf,
    pub report_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessRun {
    pub written: WrittenConfig,
    pub invocation: HarnessInvocation,
    pub evidence: OracleEvidence,
    pub evaluation: OracleEvaluation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortMember {
    pub prepared: PreparedSingleRun,
    pub submission_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortRequest {
    pub members: Vec<CohortMember>,
    pub layout: Layout,
    pub options: Options,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CohortRun {
    pub written: WrittenConfig,
    pub invocation: HarnessInvocation,
    pub report: FinalReport,
    pub evaluations: Vec<OracleEvaluation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunCandidate {
    pub attempt: usize,
    pub latest: bool,
    pub run_id: String,
    pub instance_id: String,
    pub run_manifest: PathBuf,
    pub run_root: PathBuf,
    pub submission_path: Option<PathBuf>,
    pub execution_status: RunExecutionStatus,
    pub submission_status: RunSubmissionStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CampaignCandidate {
    pub node: Prototype1NodeRecord,
    pub runner_result: Prototype1RunnerResult,
    pub instances: Vec<CampaignInstance>,
}

#[derive(Debug, Clone)]
pub struct CampaignInstance {
    pub closure_row: ClosureInstanceRow,
    pub submission: MultiSweBenchSubmissionRecord,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalReport {
    pub total_instances: usize,
    pub submitted_instances: usize,
    pub completed_instances: usize,
    pub incomplete_instances: usize,
    pub resolved_instances: usize,
    pub unresolved_instances: usize,
    pub empty_patch_instances: usize,
    pub error_instances: usize,
    pub submitted_ids: Vec<String>,
    pub completed_ids: Vec<String>,
    pub incomplete_ids: Vec<String>,
    pub resolved_ids: Vec<String>,
    pub unresolved_ids: Vec<String>,
    pub empty_patch_ids: Vec<String>,
    pub error_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceReport {
    pub org: String,
    pub repo: String,
    pub number: u64,
    pub valid: Option<bool>,
    pub error_msg: Option<String>,
    pub fixed_tests: BTreeMap<String, TestTransition>,
    pub p2p_tests: BTreeMap<String, TestTransition>,
    pub f2p_tests: BTreeMap<String, TestTransition>,
    pub s2p_tests: BTreeMap<String, TestTransition>,
    pub n2p_tests: BTreeMap<String, TestTransition>,
    pub run_result: StageResult,
    pub test_patch_result: StageResult,
    pub fix_patch_result: StageResult,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageResult {
    pub passed_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub passed_tests: BTreeSet<String>,
    pub failed_tests: BTreeSet<String>,
    pub skipped_tests: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestTransition {
    pub run: TestStatus,
    pub test: TestStatus,
    pub fix: TestStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TestStatus {
    #[serde(rename = "PASS")]
    Pass,
    #[serde(rename = "FAIL")]
    Fail,
    #[serde(rename = "SKIP")]
    Skip,
    #[serde(rename = "NONE")]
    None,
    #[serde(rename = "FAILED")]
    Failed,
    #[serde(rename = "PASSED")]
    Passed,
    #[serde(rename = "SKIPPED")]
    Skipped,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "XFAIL")]
    Xfail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Resolved,
    Unresolved,
    EmptyPatch,
    Incomplete,
    Error,
    NotSubmitted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleEvidence {
    pub report_path: PathBuf,
    pub instance_id: String,
    pub report_id: String,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OracleEvaluation {
    pub evidence: OracleEvidence,
    pub instance_report_path: PathBuf,
    pub instance_report: Option<InstanceReport>,
    pub diagnostic: OracleDiagnostic,
    pub usable_for_selection: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleDiagnostic {
    Resolved,
    UnresolvedTestsRan,
    FixCompileFailed,
    MissingFixResults,
    InvalidInstanceReport,
    MissingInstanceReport,
    EmptyPatch,
    Incomplete,
    Error,
    NotSubmitted,
}

impl Request {
    pub fn from_instance(
        instance_id: &str,
        attempt: Option<usize>,
        submission_path: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
        options: Options,
    ) -> Result<Self, PrepareError> {
        let candidate = select_run_candidate(instance_id, attempt)?;
        Self::from_candidate(candidate, submission_path, output_dir, repo_dir, options)
    }

    pub fn from_candidate(
        candidate: RunCandidate,
        submission_path: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
        options: Options,
    ) -> Result<Self, PrepareError> {
        let submission_path = submission_path
            .or(candidate.submission_path)
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!(
                    "run {} has no registered MBE submission artifact",
                    candidate.run_id
                ),
            })?;
        let output_dir = output_dir.or_else(|| Some(candidate.run_root.join("mbe")));
        Self::from_manifest(
            candidate.run_manifest,
            Some(submission_path),
            output_dir,
            repo_dir,
            options,
        )
    }

    pub fn from_manifest(
        run_manifest: PathBuf,
        submission_path: Option<PathBuf>,
        output_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
        options: Options,
    ) -> Result<Self, PrepareError> {
        let prepared = PreparedSingleRun::load_manifest(run_manifest)?;
        let submission_path =
            submission_path.unwrap_or_else(|| prepared.output_dir.join(SUBMISSION_FILE));
        let output_dir = output_dir.unwrap_or_else(|| prepared.output_dir.join("mbe"));
        let repo_dir = match repo_dir {
            Some(path) => path,
            None => repo_cache_dir_for_prepared_run(&prepared)?,
        };
        Self::new(
            prepared,
            submission_path,
            Layout::under(output_dir, repo_dir),
            options,
        )
    }

    pub fn new(
        prepared: PreparedSingleRun,
        submission_path: PathBuf,
        layout: Layout,
        options: Options,
    ) -> Result<Self, PrepareError> {
        if !submission_path.is_file() {
            return Err(PrepareError::MissingMbeSubmission(submission_path));
        }

        let source = require_msb_source(&prepared)?;
        if !source.dataset_file.is_file() {
            return Err(PrepareError::MissingDatasetFile(
                source.dataset_file.clone(),
            ));
        }
        if !options.need_clone && !layout.repo_dir.is_dir() {
            return Err(PrepareError::InvalidMbeRequest {
                detail: format!(
                    "repo_dir '{}' must exist when need_clone is false",
                    layout.repo_dir.display()
                ),
            });
        }

        Ok(Self {
            prepared,
            submission_path,
            layout,
            options,
        })
    }

    pub fn harness_config(&self) -> Result<HarnessConfig, PrepareError> {
        let source = require_msb_source(&self.prepared)?;
        let report_id = report_id_for_source(source);

        Ok(HarnessConfig {
            mode: Mode::Evaluation,
            workdir: self.layout.workdir.clone(),
            patch_files: vec![self.submission_path.clone()],
            dataset_files: vec![source.dataset_file.clone()],
            force_build: self.options.force_build,
            output_dir: self.layout.output_dir.clone(),
            specifics: vec![report_id],
            skips: Vec::new(),
            repo_dir: self.layout.repo_dir.clone(),
            need_clone: self.options.need_clone,
            global_env: self.options.global_env.clone(),
            clear_env: self.options.clear_env,
            stop_on_error: self.options.stop_on_error,
            max_workers: self.options.workers.general,
            max_workers_build_image: self.options.workers.build_image,
            max_workers_run_instance: self.options.workers.run_instance,
            fix_patch_run_cmd: self.options.fix_patch_run_cmd.clone(),
            log_dir: self.layout.log_dir.clone(),
            log_level: self.options.log_level.clone(),
            log_to_console: self.options.log_to_console,
            human_mode: self.options.human_mode,
        })
    }

    pub fn write_config(&self) -> Result<WrittenConfig, PrepareError> {
        fs::create_dir_all(&self.layout.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.output_dir.clone(),
                source,
            }
        })?;
        fs::create_dir_all(&self.layout.workdir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.workdir.clone(),
                source,
            }
        })?;
        fs::create_dir_all(&self.layout.log_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.log_dir.clone(),
                source,
            }
        })?;

        let path = self.layout.output_dir.join(CONFIG_FILE);
        let json = serde_json::to_string_pretty(&self.harness_config()?)
            .map_err(PrepareError::Serialize)?;
        fs::write(&path, json).map_err(|source| PrepareError::WriteManifest {
            path: path.clone(),
            source,
        })?;

        Ok(WrittenConfig {
            path,
            report_path: self.layout.output_dir.join(FINAL_REPORT_FILE),
        })
    }

    pub fn write_harness_invocation(
        &self,
        python: impl Into<String>,
    ) -> Result<HarnessInvocation, PrepareError> {
        let written = self.write_config()?;
        Ok(written.harness_invocation(python))
    }

    pub fn run_harness(&self, python: impl Into<String>) -> Result<HarnessRun, PrepareError> {
        let written = self.write_config()?;
        let invocation = written.harness_invocation(python);
        let status = Command::new(&invocation.program)
            .args(&invocation.args)
            .status()
            .map_err(|source| PrepareError::MbeHarnessCommand {
                command: invocation.command_line(),
                source,
            })?;
        if !status.success() {
            return Err(PrepareError::MbeHarnessStatus {
                command: invocation.command_line(),
                status: status.code().unwrap_or(-1),
            });
        }

        let evidence = self.load_oracle_evidence_from(&written.report_path)?;
        let evaluation = self.load_oracle_evaluation_from(&written.report_path)?;
        Ok(HarnessRun {
            written,
            invocation,
            evidence,
            evaluation,
        })
    }

    pub fn load_oracle_evidence(&self) -> Result<OracleEvidence, PrepareError> {
        self.load_oracle_evidence_from(&self.layout.output_dir.join(FINAL_REPORT_FILE))
    }

    pub fn load_oracle_evidence_from(
        &self,
        report_path: &Path,
    ) -> Result<OracleEvidence, PrepareError> {
        let report = FinalReport::load(report_path)?;
        OracleEvidence::from_report(&self.prepared, report_path.to_path_buf(), &report)
    }

    pub fn load_oracle_evaluation(&self) -> Result<OracleEvaluation, PrepareError> {
        self.load_oracle_evaluation_from(&self.layout.output_dir.join(FINAL_REPORT_FILE))
    }

    pub fn load_oracle_evaluation_from(
        &self,
        report_path: &Path,
    ) -> Result<OracleEvaluation, PrepareError> {
        let final_report = FinalReport::load(report_path)?;
        let evidence =
            OracleEvidence::from_report(&self.prepared, report_path.to_path_buf(), &final_report)?;
        OracleEvaluation::from_evidence(&self.prepared, evidence, &self.layout)
    }
}

impl CohortRequest {
    pub fn from_campaign_candidate(
        candidate: &CampaignCandidate,
        output_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
        options: Options,
    ) -> Result<Self, PrepareError> {
        let output_dir = output_dir.unwrap_or_else(|| candidate.node.node_dir.join("mbe"));
        let members = cohort_members_from_campaign_instances(&candidate.instances)?;
        let repo_dir = match repo_dir {
            Some(path) => path,
            None => repo_cache_dir_for_members(&members)?,
        };
        Self::new(members, Layout::under(output_dir, repo_dir), options)
    }

    pub fn from_treatment_state(
        node: &Prototype1NodeRecord,
        treatment_state: &crate::closure::ClosureState,
        output_dir: Option<PathBuf>,
        repo_dir: Option<PathBuf>,
        options: Options,
    ) -> Result<Self, PrepareError> {
        let output_dir = output_dir.unwrap_or_else(|| node.node_dir.join("mbe"));
        let members = cohort_members_from_rows(treatment_state.instances.iter())?;
        let repo_dir = match repo_dir {
            Some(path) => path,
            None => repo_cache_dir_for_members(&members)?,
        };
        Self::new(members, Layout::under(output_dir, repo_dir), options)
    }

    pub fn new(
        members: Vec<CohortMember>,
        layout: Layout,
        options: Options,
    ) -> Result<Self, PrepareError> {
        if members.is_empty() {
            return Err(PrepareError::InvalidMbeRequest {
                detail: "MBE cohort request requires at least one prepared run".to_string(),
            });
        }
        let mut seen_report_ids = BTreeSet::new();
        for member in &members {
            if !member.submission_path.is_file() {
                return Err(PrepareError::MissingMbeSubmission(
                    member.submission_path.clone(),
                ));
            }
            let source = require_msb_source(&member.prepared)?;
            if !source.dataset_file.is_file() {
                return Err(PrepareError::MissingDatasetFile(
                    source.dataset_file.clone(),
                ));
            }
            let report_id = report_id_for_source(source);
            if !seen_report_ids.insert(report_id.clone()) {
                return Err(PrepareError::InvalidMbeRequest {
                    detail: format!(
                        "MBE cohort request contains duplicate report id '{report_id}'"
                    ),
                });
            }
        }
        if !options.need_clone && !layout.repo_dir.is_dir() {
            return Err(PrepareError::InvalidMbeRequest {
                detail: format!(
                    "repo_dir '{}' must exist when need_clone is false",
                    layout.repo_dir.display()
                ),
            });
        }
        Ok(Self {
            members,
            layout,
            options,
        })
    }

    pub fn harness_config(&self) -> Result<HarnessConfig, PrepareError> {
        Ok(HarnessConfig {
            mode: Mode::Evaluation,
            workdir: self.layout.workdir.clone(),
            patch_files: vec![self.aggregate_submission_path()],
            dataset_files: cohort_dataset_files(&self.members)?,
            force_build: self.options.force_build,
            output_dir: self.layout.output_dir.clone(),
            specifics: cohort_report_ids(&self.members)?,
            skips: Vec::new(),
            repo_dir: self.layout.repo_dir.clone(),
            need_clone: self.options.need_clone,
            global_env: self.options.global_env.clone(),
            clear_env: self.options.clear_env,
            stop_on_error: self.options.stop_on_error,
            max_workers: self.options.workers.general,
            max_workers_build_image: self.options.workers.build_image,
            max_workers_run_instance: self.options.workers.run_instance,
            fix_patch_run_cmd: self.options.fix_patch_run_cmd.clone(),
            log_dir: self.layout.log_dir.clone(),
            log_level: self.options.log_level.clone(),
            log_to_console: self.options.log_to_console,
            human_mode: self.options.human_mode,
        })
    }

    pub fn write_config(&self) -> Result<WrittenConfig, PrepareError> {
        fs::create_dir_all(&self.layout.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.output_dir.clone(),
                source,
            }
        })?;
        fs::create_dir_all(&self.layout.workdir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.workdir.clone(),
                source,
            }
        })?;
        fs::create_dir_all(&self.layout.log_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: self.layout.log_dir.clone(),
                source,
            }
        })?;

        let submission_path = self.aggregate_submission_path();
        fs::write(&submission_path, self.aggregate_submission_blob()?).map_err(|source| {
            PrepareError::WriteManifest {
                path: submission_path.clone(),
                source,
            }
        })?;

        let path = self.layout.output_dir.join(CONFIG_FILE);
        let json = serde_json::to_string_pretty(&self.harness_config()?)
            .map_err(PrepareError::Serialize)?;
        fs::write(&path, json).map_err(|source| PrepareError::WriteManifest {
            path: path.clone(),
            source,
        })?;

        Ok(WrittenConfig {
            path,
            report_path: self.layout.output_dir.join(FINAL_REPORT_FILE),
        })
    }

    pub fn run_harness(&self, python: impl Into<String>) -> Result<CohortRun, PrepareError> {
        let written = self.write_config()?;
        let invocation = written.harness_invocation(python);
        let status = Command::new(&invocation.program)
            .args(&invocation.args)
            .status()
            .map_err(|source| PrepareError::MbeHarnessCommand {
                command: invocation.command_line(),
                source,
            })?;
        if !status.success() {
            return Err(PrepareError::MbeHarnessStatus {
                command: invocation.command_line(),
                status: status.code().unwrap_or(-1),
            });
        }

        let report = FinalReport::load(&written.report_path)?;
        let evaluations = self.load_oracle_evaluations_from(&written.report_path)?;
        Ok(CohortRun {
            written,
            invocation,
            report,
            evaluations,
        })
    }

    pub fn load_oracle_evaluations(&self) -> Result<Vec<OracleEvaluation>, PrepareError> {
        self.load_oracle_evaluations_from(&self.layout.output_dir.join(FINAL_REPORT_FILE))
    }

    pub fn load_oracle_evaluations_from(
        &self,
        report_path: &Path,
    ) -> Result<Vec<OracleEvaluation>, PrepareError> {
        let final_report = FinalReport::load(report_path)?;
        self.validate_report_coverage(&final_report)?;
        self.members
            .iter()
            .map(|member| {
                let source = require_msb_source(&member.prepared)?;
                let evidence = OracleEvidence {
                    report_path: report_path.to_path_buf(),
                    instance_id: source.instance_id.clone(),
                    report_id: report_id_for_source(source),
                    verdict: final_report.verdict_for(&report_id_for_source(source)),
                };
                OracleEvaluation::from_evidence(&member.prepared, evidence, &self.layout)
            })
            .collect()
    }

    fn validate_report_coverage(&self, report: &FinalReport) -> Result<(), PrepareError> {
        let expected = cohort_report_ids(&self.members)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let mut submitted = BTreeSet::new();
        for id in &report.submitted_ids {
            if !submitted.insert(id.clone()) {
                return Err(PrepareError::InvalidMbeReport {
                    detail: format!("MBE final report contains duplicate submitted id '{id}'"),
                });
            }
        }
        if let Some(unknown) = submitted.iter().find(|id| !expected.contains(*id)) {
            return Err(PrepareError::InvalidMbeReport {
                detail: format!("MBE final report contains unknown report id '{unknown}'"),
            });
        }
        if let Some(missing) = expected.iter().find(|id| !submitted.contains(*id)) {
            return Err(PrepareError::InvalidMbeReport {
                detail: format!("MBE final report is missing configured report id '{missing}'"),
            });
        }
        Ok(())
    }

    fn aggregate_submission_path(&self) -> PathBuf {
        self.layout.output_dir.join(SUBMISSION_FILE)
    }

    fn aggregate_submission_blob(&self) -> Result<String, PrepareError> {
        let mut blob = String::new();
        for member in &self.members {
            let text = fs::read_to_string(&member.submission_path).map_err(|source| {
                PrepareError::ReadManifest {
                    path: member.submission_path.clone(),
                    source,
                }
            })?;
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Err(PrepareError::InvalidMbeRequest {
                    detail: format!(
                        "MBE submission '{}' is empty",
                        member.submission_path.display()
                    ),
                });
            }
            blob.push_str(trimmed);
            blob.push('\n');
        }
        Ok(blob)
    }
}

pub fn run_candidates(instance_id: &str) -> Result<Vec<RunCandidate>, PrepareError> {
    let instances_root = crate::layout::instances_dir()?;
    let mut registrations =
        crate::run_registry::list_registrations_for_instance(&instances_root, instance_id)?;
    registrations
        .sort_by(|left, right| run_candidate_sort_key(left).cmp(&run_candidate_sort_key(right)));
    let latest_index = registrations.len().saturating_sub(1);

    Ok(registrations
        .into_iter()
        .enumerate()
        .map(|(index, registration)| RunCandidate {
            attempt: index + 1,
            latest: index == latest_index,
            run_id: registration.run_id,
            instance_id: registration.frozen_spec.task_id,
            run_manifest: registration.artifacts.run_manifest,
            run_root: registration.artifacts.run_root,
            submission_path: registration.artifacts.msb_submission,
            execution_status: registration.lifecycle.execution_status,
            submission_status: registration.lifecycle.submission_status,
            started_at: registration.lifecycle.started_at,
            finished_at: registration.lifecycle.finished_at,
        })
        .collect())
}

pub fn select_run_candidate(
    instance_id: &str,
    attempt: Option<usize>,
) -> Result<RunCandidate, PrepareError> {
    let candidates = run_candidates(instance_id)?;
    if candidates.is_empty() {
        return Err(PrepareError::MissingRunManifest(
            crate::layout::instances_dir()?
                .join(instance_id)
                .join("runs")
                .join("run-*/run.json"),
        ));
    }

    if let Some(attempt) = attempt {
        return candidates
            .into_iter()
            .find(|candidate| candidate.attempt == attempt)
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!("instance '{instance_id}' has no MBE candidate attempt {attempt}"),
            });
    }

    candidates
        .into_iter()
        .rev()
        .find(|candidate| {
            candidate.execution_status == RunExecutionStatus::Completed
                && matches!(
                    candidate.submission_status,
                    RunSubmissionStatus::NonemptyPatch | RunSubmissionStatus::EmptyPatch
                )
                && candidate
                    .submission_path
                    .as_ref()
                    .is_some_and(|path| path.is_file())
        })
        .ok_or_else(|| PrepareError::InvalidMbeRequest {
            detail: format!(
                "instance '{instance_id}' has no completed run with an MBE submission artifact"
            ),
        })
}

impl WrittenConfig {
    pub fn harness_invocation(&self, python: impl Into<String>) -> HarnessInvocation {
        HarnessInvocation {
            program: python.into(),
            args: vec![
                "-m".to_string(),
                HARNESS_MODULE.to_string(),
                "--config".to_string(),
                self.path.display().to_string(),
            ],
            config_path: self.path.clone(),
            report_path: self.report_path.clone(),
        }
    }
}

impl HarnessInvocation {
    pub fn command_line(&self) -> String {
        std::iter::once(self.program.as_str())
            .chain(self.args.iter().map(String::as_str))
            .map(shell_quote)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl FinalReport {
    pub fn load(path: &Path) -> Result<Self, PrepareError> {
        if !path.is_file() {
            return Err(PrepareError::MissingMbeReport(path.to_path_buf()));
        }
        let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        let report: Self =
            serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        report.validate()?;
        Ok(report)
    }

    pub fn validate(&self) -> Result<(), PrepareError> {
        validate_count("submitted", self.submitted_instances, &self.submitted_ids)?;
        validate_count("completed", self.completed_instances, &self.completed_ids)?;
        validate_count(
            "incomplete",
            self.incomplete_instances,
            &self.incomplete_ids,
        )?;
        validate_count("resolved", self.resolved_instances, &self.resolved_ids)?;
        validate_count(
            "unresolved",
            self.unresolved_instances,
            &self.unresolved_ids,
        )?;
        validate_count(
            "empty_patch",
            self.empty_patch_instances,
            &self.empty_patch_ids,
        )?;
        validate_count("error", self.error_instances, &self.error_ids)?;

        let total_from_categories = self.resolved_instances
            + self.unresolved_instances
            + self.empty_patch_instances
            + self.error_instances;
        if self.total_instances != total_from_categories {
            return Err(PrepareError::InvalidMbeReport {
                detail: format!(
                    "total_instances={} but resolved+unresolved+empty_patch+error={}",
                    self.total_instances, total_from_categories
                ),
            });
        }
        if self.submitted_instances != self.total_instances {
            return Err(PrepareError::InvalidMbeReport {
                detail: format!(
                    "submitted_instances={} but total_instances={}",
                    self.submitted_instances, self.total_instances
                ),
            });
        }
        if self.completed_instances + self.incomplete_instances != self.total_instances {
            return Err(PrepareError::InvalidMbeReport {
                detail: format!(
                    "completed+incomplete={} but total_instances={}",
                    self.completed_instances + self.incomplete_instances,
                    self.total_instances
                ),
            });
        }

        validate_terminal_membership(self)?;
        Ok(())
    }

    pub fn verdict_for(&self, report_id: &str) -> Verdict {
        if self.resolved_ids.iter().any(|id| id == report_id) {
            Verdict::Resolved
        } else if self.unresolved_ids.iter().any(|id| id == report_id) {
            Verdict::Unresolved
        } else if self.empty_patch_ids.iter().any(|id| id == report_id) {
            Verdict::EmptyPatch
        } else if self.error_ids.iter().any(|id| id == report_id) {
            Verdict::Error
        } else if self.incomplete_ids.iter().any(|id| id == report_id) {
            Verdict::Incomplete
        } else {
            Verdict::NotSubmitted
        }
    }
}

impl InstanceReport {
    pub fn load(path: &Path) -> Result<Self, PrepareError> {
        if !path.is_file() {
            return Err(PrepareError::MissingMbeReport(path.to_path_buf()));
        }
        let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
        let report: Self =
            serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        report.validate()?;
        Ok(report)
    }

    pub fn report_id(&self) -> String {
        format!("{}/{}:pr-{}", self.org, self.repo, self.number)
    }

    pub fn validate(&self) -> Result<(), PrepareError> {
        self.run_result.validate("run_result")?;
        self.test_patch_result.validate("test_patch_result")?;
        self.fix_patch_result.validate("fix_patch_result")?;
        Ok(())
    }

    pub fn fix_result_count(&self) -> usize {
        self.fix_patch_result.total_count()
    }
}

impl StageResult {
    pub fn total_count(&self) -> usize {
        self.passed_count + self.failed_count + self.skipped_count
    }

    pub fn validate(&self, label: &str) -> Result<(), PrepareError> {
        validate_len(
            &format!("{label}.passed"),
            self.passed_count,
            self.passed_tests.len(),
        )?;
        validate_len(
            &format!("{label}.failed"),
            self.failed_count,
            self.failed_tests.len(),
        )?;
        validate_len(
            &format!("{label}.skipped"),
            self.skipped_count,
            self.skipped_tests.len(),
        )?;
        validate_disjoint_stage_sets(label, self)?;
        Ok(())
    }
}

impl OracleEvidence {
    pub fn from_report(
        prepared: &PreparedSingleRun,
        report_path: PathBuf,
        report: &FinalReport,
    ) -> Result<Self, PrepareError> {
        let source = require_msb_source(prepared)?;
        let report_id = report_id_for_source(source);
        Ok(Self {
            report_path,
            instance_id: source.instance_id.clone(),
            verdict: report.verdict_for(&report_id),
            report_id,
        })
    }
}

impl OracleEvaluation {
    pub fn from_evidence(
        prepared: &PreparedSingleRun,
        evidence: OracleEvidence,
        layout: &Layout,
    ) -> Result<Self, PrepareError> {
        let source = require_msb_source(prepared)?;
        let instance_report_path = instance_report_path(&layout.workdir, source);
        let instance_report = if instance_report_path.is_file() {
            Some(InstanceReport::load(&instance_report_path)?)
        } else {
            None
        };
        if let Some(report) = &instance_report {
            let expected = report_id_for_source(source);
            let actual = report.report_id();
            if actual != expected {
                return Err(PrepareError::InvalidMbeReport {
                    detail: format!(
                        "instance report id '{actual}' does not match expected '{expected}'"
                    ),
                });
            }
        }

        let diagnostic = classify_oracle_diagnostic(
            evidence.verdict,
            instance_report.as_ref(),
            &fix_patch_run_log_path(&layout.workdir, source),
        )?;
        Ok(Self {
            evidence,
            instance_report_path,
            instance_report,
            diagnostic,
            usable_for_selection: diagnostic.usable_for_selection(),
        })
    }
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Resolved => "resolved",
            Verdict::Unresolved => "unresolved",
            Verdict::EmptyPatch => "empty_patch",
            Verdict::Incomplete => "incomplete",
            Verdict::Error => "error",
            Verdict::NotSubmitted => "not_submitted",
        }
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl OracleDiagnostic {
    pub fn as_str(self) -> &'static str {
        match self {
            OracleDiagnostic::Resolved => "resolved",
            OracleDiagnostic::UnresolvedTestsRan => "unresolved_tests_ran",
            OracleDiagnostic::FixCompileFailed => "fix_compile_failed",
            OracleDiagnostic::MissingFixResults => "missing_fix_results",
            OracleDiagnostic::InvalidInstanceReport => "invalid_instance_report",
            OracleDiagnostic::MissingInstanceReport => "missing_instance_report",
            OracleDiagnostic::EmptyPatch => "empty_patch",
            OracleDiagnostic::Incomplete => "incomplete",
            OracleDiagnostic::Error => "error",
            OracleDiagnostic::NotSubmitted => "not_submitted",
        }
    }

    pub fn usable_for_selection(self) -> bool {
        matches!(
            self,
            OracleDiagnostic::Resolved | OracleDiagnostic::UnresolvedTestsRan
        )
    }
}

impl fmt::Display for OracleDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Layout {
    pub fn under(output_dir: PathBuf, repo_dir: PathBuf) -> Self {
        Self {
            workdir: output_dir.join("workdir"),
            log_dir: output_dir.join("logs"),
            output_dir,
            repo_dir,
        }
    }
}

impl Default for Options {
    fn default() -> Self {
        Self {
            force_build: false,
            need_clone: false,
            clear_env: true,
            stop_on_error: true,
            workers: Workers::default(),
            global_env: Vec::new(),
            fix_patch_run_cmd: String::new(),
            log_level: "INFO".to_string(),
            log_to_console: true,
            human_mode: true,
        }
    }
}

impl Default for Workers {
    fn default() -> Self {
        Self {
            general: 1,
            build_image: 1,
            run_instance: 1,
        }
    }
}

pub fn config_path(output_dir: &Path) -> PathBuf {
    output_dir.join(CONFIG_FILE)
}

pub fn report_path(output_dir: &Path) -> PathBuf {
    output_dir.join(FINAL_REPORT_FILE)
}

impl CampaignCandidate {
    pub fn node_id(&self) -> &str {
        &self.node.node_id
    }

    pub fn generation(&self) -> u32 {
        self.node.generation
    }

    pub fn parent_node_id(&self) -> Option<&str> {
        self.node.parent_node_id.as_deref()
    }

    pub fn branch_id(&self) -> &str {
        &self.node.branch_id
    }

    pub fn primary_instance_id(&self) -> &str {
        &self.node.instance_id
    }

    pub fn treatment_campaign_id(&self) -> Result<&str, PrepareError> {
        self.runner_result
            .treatment_campaign_id
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!("node '{}' has no treatment campaign id", self.node.node_id),
            })
    }

    pub fn instances(&self) -> &[CampaignInstance] {
        &self.instances
    }

    pub fn cohort_size(&self) -> usize {
        self.instances.len()
    }

    pub fn instance_ids(&self) -> Vec<&str> {
        self.instances
            .iter()
            .map(CampaignInstance::instance_id)
            .collect()
    }

    pub fn nonempty_instance_count(&self) -> usize {
        self.instances
            .iter()
            .filter(|instance| !instance.empty_patch())
            .count()
    }

    pub fn oracle_eligible_instance_count(&self) -> Result<usize, PrepareError> {
        self.instances.iter().try_fold(0usize, |count, instance| {
            Ok(count + usize::from(instance.oracle_eligible()?))
        })
    }

    pub fn total_fix_patch_bytes(&self) -> usize {
        self.instances
            .iter()
            .map(CampaignInstance::fix_patch_bytes)
            .sum()
    }

    pub fn total_fix_patch_lines(&self) -> usize {
        self.instances
            .iter()
            .map(CampaignInstance::fix_patch_lines)
            .sum()
    }
}

impl CampaignInstance {
    pub fn instance_id(&self) -> &str {
        &self.closure_row.instance_id
    }

    pub fn run_manifest(&self) -> Result<&Path, PrepareError> {
        self.closure_row
            .artifacts
            .run_manifest
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!(
                    "campaign instance '{}' has no run manifest",
                    self.closure_row.instance_id
                ),
            })
    }

    pub fn run_root(&self) -> Result<&Path, PrepareError> {
        self.closure_row
            .artifacts
            .run_root
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!(
                    "campaign instance '{}' has no run root",
                    self.closure_row.instance_id
                ),
            })
    }

    pub fn submission_path(&self) -> Result<&Path, PrepareError> {
        self.closure_row
            .artifacts
            .msb_submission
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidMbeRequest {
                detail: format!(
                    "campaign instance '{}' has no MBE submission artifact",
                    self.closure_row.instance_id
                ),
            })
    }

    pub fn empty_patch(&self) -> bool {
        self.submission.fix_patch.trim().is_empty()
    }

    pub fn fix_patch_bytes(&self) -> usize {
        self.submission.fix_patch.len()
    }

    pub fn fix_patch_lines(&self) -> usize {
        self.submission.fix_patch.lines().count()
    }

    pub fn oracle_eligible(&self) -> Result<bool, PrepareError> {
        row_has_oracle_eligible_projection(&self.closure_row)
    }
}

pub fn campaign_candidates(
    campaign_id: &str,
    nonempty_only: bool,
) -> Result<Vec<CampaignCandidate>, PrepareError> {
    let prototype_root = crate::layout::campaigns_dir()?
        .join(campaign_id)
        .join("prototype1");
    let nodes_dir = prototype_root.join("nodes");
    let entries = fs::read_dir(&nodes_dir).map_err(|source| PrepareError::ReadManifest {
        path: nodes_dir.clone(),
        source,
    })?;

    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: nodes_dir.clone(),
            source,
        })?;
        let node_dir = entry.path();
        if !node_dir.is_dir() {
            continue;
        }
        let node_path = node_dir.join("node.json");
        let result_path = node_dir.join("runner-result.json");
        if !node_path.is_file() || !result_path.is_file() {
            continue;
        }

        let node = load_node_record_at(&node_path)?;
        let result =
            load_runner_result_at(&result_path, OperatorProjectionRead::projection_module())?;
        candidates.extend(candidates_for_result(&node, &result, nonempty_only)?);
    }

    candidates.sort_by(|left, right| {
        left.generation()
            .cmp(&right.generation())
            .then_with(|| {
                left.runner_result
                    .recorded_at
                    .cmp(&right.runner_result.recorded_at)
            })
            .then_with(|| left.node_id().cmp(right.node_id()))
    });
    Ok(candidates)
}

pub fn campaign_candidate_by_node(
    campaign_id: &str,
    node_id: &str,
) -> Result<CampaignCandidate, PrepareError> {
    let candidates = campaign_candidates(campaign_id, false)?;
    candidates
        .into_iter()
        .find(|candidate| candidate.node_id() == node_id)
        .ok_or_else(|| PrepareError::InvalidMbeRequest {
            detail: format!("campaign '{campaign_id}' has no MBE candidate for node '{node_id}'"),
        })
}

fn load_node_record_at(path: &Path) -> Result<Prototype1NodeRecord, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn load_submission_record(path: &Path) -> Result<MultiSweBenchSubmissionRecord, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(line) = lines.next() else {
        return Err(PrepareError::InvalidMbeRequest {
            detail: format!("MBE submission '{}' is empty", path.display()),
        });
    };
    if lines.next().is_some() {
        return Err(PrepareError::InvalidMbeRequest {
            detail: format!(
                "MBE campaign candidate submission '{}' contains more than one record",
                path.display()
            ),
        });
    }
    serde_json::from_str(line).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn candidates_for_result(
    node: &Prototype1NodeRecord,
    result: &Prototype1RunnerResult,
    nonempty_only: bool,
) -> Result<Vec<CampaignCandidate>, PrepareError> {
    if result.disposition != Prototype1RunnerDisposition::Succeeded {
        return Ok(Vec::new());
    }
    let Some(treatment_campaign_id) = result.treatment_campaign_id.as_deref() else {
        return Ok(Vec::new());
    };

    let state = load_closure_state(treatment_campaign_id)?;
    let instances = campaign_instances_from_rows(state.instances.iter())?;
    let keep = if nonempty_only {
        instances
            .iter()
            .try_fold(false, |keep, instance| -> Result<bool, PrepareError> {
                Ok(keep || (!instance.empty_patch() && instance.oracle_eligible()?))
            })?
    } else {
        !instances.is_empty()
    };
    if keep {
        Ok(vec![CampaignCandidate {
            node: node.clone(),
            runner_result: result.clone(),
            instances,
        }])
    } else {
        Ok(Vec::new())
    }
}

fn row_has_oracle_eligible_projection(row: &ClosureInstanceRow) -> Result<bool, PrepareError> {
    let Some(record_path) = row.artifacts.record_path.as_ref() else {
        return Ok(false);
    };
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.clone(),
            source,
        })?;
    Ok(record.operational_metrics().oracle_eligible)
}

fn campaign_instances_from_rows<'a>(
    rows: impl IntoIterator<Item = &'a ClosureInstanceRow>,
) -> Result<Vec<CampaignInstance>, PrepareError> {
    let mut instances = Vec::new();
    for row in rows {
        if row.eval_status != ClosureClass::Complete || row.artifacts.msb_submission.is_none() {
            continue;
        }
        let submission_path = row
            .artifacts
            .msb_submission
            .as_deref()
            .expect("checked submission artifact");
        instances.push(CampaignInstance {
            closure_row: row.clone(),
            submission: load_submission_record(submission_path)?,
        });
    }
    Ok(instances)
}

fn cohort_members_from_campaign_instances(
    instances: &[CampaignInstance],
) -> Result<Vec<CohortMember>, PrepareError> {
    instances
        .iter()
        .map(|instance| {
            let run_manifest = instance.run_manifest()?.to_path_buf();
            let submission_path = instance.submission_path()?.to_path_buf();
            Ok(CohortMember {
                prepared: PreparedSingleRun::load_manifest(run_manifest)?,
                submission_path,
            })
        })
        .collect()
}

fn cohort_members_from_rows<'a>(
    rows: impl IntoIterator<Item = &'a ClosureInstanceRow>,
) -> Result<Vec<CohortMember>, PrepareError> {
    let instances = campaign_instances_from_rows(rows)?;
    cohort_members_from_campaign_instances(&instances)
}

fn require_msb_source(
    prepared: &PreparedSingleRun,
) -> Result<&crate::spec::MultiSweBenchSource, PrepareError> {
    match prepared.source.as_ref() {
        Some(RunSource::MultiSweBench(source)) => Ok(source),
        None => Err(PrepareError::InvalidMbeRequest {
            detail: format!(
                "prepared run '{}' is not a Multi-SWE-bench run",
                prepared.task_id
            ),
        }),
    }
}

fn repo_cache_dir_for_prepared_run(prepared: &PreparedSingleRun) -> Result<PathBuf, PrepareError> {
    let source = require_msb_source(prepared)?;
    let repo_dir = prepared
        .repo_root
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| PrepareError::InvalidMbeRequest {
            detail: format!(
                "could not derive repo_dir from repo_root '{}'",
                prepared.repo_root.display()
            ),
        })?;

    let expected_repo_root = repo_dir.join(&source.org).join(&source.repo);
    if expected_repo_root != prepared.repo_root {
        return Err(PrepareError::InvalidMbeRequest {
            detail: format!(
                "repo_root '{}' does not match expected MBE repo layout '{}'",
                prepared.repo_root.display(),
                expected_repo_root.display()
            ),
        });
    }

    Ok(repo_dir.to_path_buf())
}

fn repo_cache_dir_for_members(members: &[CohortMember]) -> Result<PathBuf, PrepareError> {
    let mut repo_dirs = members
        .iter()
        .map(|member| repo_cache_dir_for_prepared_run(&member.prepared))
        .collect::<Result<BTreeSet<_>, PrepareError>>()?;
    match repo_dirs.len() {
        0 => Err(PrepareError::InvalidMbeRequest {
            detail: "MBE cohort request requires at least one prepared run".to_string(),
        }),
        1 => Ok(repo_dirs.pop_first().expect("single repo dir present")),
        _ => Err(PrepareError::InvalidMbeRequest {
            detail:
                "MBE cohort request spans multiple repo cache roots; pass --repo-dir explicitly"
                    .to_string(),
        }),
    }
}

fn cohort_dataset_files(members: &[CohortMember]) -> Result<Vec<PathBuf>, PrepareError> {
    members
        .iter()
        .map(|member| {
            require_msb_source(&member.prepared).map(|source| source.dataset_file.clone())
        })
        .collect::<Result<BTreeSet<_>, PrepareError>>()
        .map(|paths| paths.into_iter().collect())
}

fn cohort_report_ids(members: &[CohortMember]) -> Result<Vec<String>, PrepareError> {
    members
        .iter()
        .map(|member| require_msb_source(&member.prepared).map(report_id_for_source))
        .collect()
}

fn report_id_for_source(source: &crate::spec::MultiSweBenchSource) -> String {
    format!("{}/{}:pr-{}", source.org, source.repo, source.number)
}

fn instance_report_path(workdir: &Path, source: &crate::spec::MultiSweBenchSource) -> PathBuf {
    workdir
        .join(&source.org)
        .join(&source.repo)
        .join(EVALUATION_WORKDIR)
        .join(format!("pr-{}", source.number))
        .join(INSTANCE_REPORT_FILE)
}

fn fix_patch_run_log_path(workdir: &Path, source: &crate::spec::MultiSweBenchSource) -> PathBuf {
    workdir
        .join(&source.org)
        .join(&source.repo)
        .join(EVALUATION_WORKDIR)
        .join(format!("pr-{}", source.number))
        .join(FIX_PATCH_RUN_LOG_FILE)
}

fn classify_oracle_diagnostic(
    verdict: Verdict,
    report: Option<&InstanceReport>,
    fix_log_path: &Path,
) -> Result<OracleDiagnostic, PrepareError> {
    match verdict {
        Verdict::Resolved => Ok(OracleDiagnostic::Resolved),
        Verdict::EmptyPatch => Ok(OracleDiagnostic::EmptyPatch),
        Verdict::Incomplete => Ok(OracleDiagnostic::Incomplete),
        Verdict::Error => Ok(OracleDiagnostic::Error),
        Verdict::NotSubmitted => Ok(OracleDiagnostic::NotSubmitted),
        Verdict::Unresolved => match report {
            Some(report) if report.fix_result_count() == 0 => {
                if fix_log_indicates_compile_failure(fix_log_path)? {
                    Ok(OracleDiagnostic::FixCompileFailed)
                } else {
                    Ok(OracleDiagnostic::MissingFixResults)
                }
            }
            Some(_) => Ok(OracleDiagnostic::UnresolvedTestsRan),
            None => Ok(OracleDiagnostic::MissingInstanceReport),
        },
    }
}

fn fix_log_indicates_compile_failure(path: &Path) -> Result<bool, PrepareError> {
    if !path.is_file() {
        return Ok(false);
    }
    let file = fs::File::open(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut text = String::new();
    file.take(FIX_PATCH_RUN_LOG_READ_LIMIT)
        .read_to_string(&mut text)
        .map_err(|source| PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(text.contains("error[E") || text.contains("could not compile"))
}

fn run_candidate_sort_key(
    registration: &crate::inner::registry::RunRegistration,
) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

fn shell_quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'/' | b':'))
    {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

fn validate_count(label: &str, expected: usize, ids: &[String]) -> Result<(), PrepareError> {
    validate_len(label, expected, ids.len())
}

fn validate_len(label: &str, expected: usize, actual: usize) -> Result<(), PrepareError> {
    if expected != actual {
        return Err(PrepareError::InvalidMbeReport {
            detail: format!(
                "{label}_count={} but {label}_items has {} entries",
                expected, actual
            ),
        });
    }
    Ok(())
}

fn validate_disjoint_stage_sets(label: &str, result: &StageResult) -> Result<(), PrepareError> {
    if let Some(test) = result
        .passed_tests
        .intersection(&result.failed_tests)
        .next()
    {
        return Err(PrepareError::InvalidMbeReport {
            detail: format!("{label} test '{test}' appears in both passed_tests and failed_tests"),
        });
    }
    if let Some(test) = result
        .passed_tests
        .intersection(&result.skipped_tests)
        .next()
    {
        return Err(PrepareError::InvalidMbeReport {
            detail: format!("{label} test '{test}' appears in both passed_tests and skipped_tests"),
        });
    }
    if let Some(test) = result
        .failed_tests
        .intersection(&result.skipped_tests)
        .next()
    {
        return Err(PrepareError::InvalidMbeReport {
            detail: format!("{label} test '{test}' appears in both failed_tests and skipped_tests"),
        });
    }
    Ok(())
}

fn validate_terminal_membership(report: &FinalReport) -> Result<(), PrepareError> {
    let mut seen = BTreeMap::<&str, &str>::new();
    for (label, ids) in [
        ("resolved", report.resolved_ids.as_slice()),
        ("unresolved", report.unresolved_ids.as_slice()),
        ("empty_patch", report.empty_patch_ids.as_slice()),
        ("error", report.error_ids.as_slice()),
    ] {
        for id in ids {
            if let Some(previous) = seen.insert(id.as_str(), label) {
                return Err(PrepareError::InvalidMbeReport {
                    detail: format!(
                        "report id '{id}' appears in both {previous}_ids and {label}_ids"
                    ),
                });
            }
        }
    }

    let submitted: BTreeSet<&str> = report.submitted_ids.iter().map(String::as_str).collect();
    for (label, ids) in [
        ("completed", report.completed_ids.as_slice()),
        ("incomplete", report.incomplete_ids.as_slice()),
        ("resolved", report.resolved_ids.as_slice()),
        ("unresolved", report.unresolved_ids.as_slice()),
        ("empty_patch", report.empty_patch_ids.as_slice()),
        ("error", report.error_ids.as_slice()),
    ] {
        for id in ids {
            if !submitted.contains(id.as_str()) {
                return Err(PrepareError::InvalidMbeReport {
                    detail: format!(
                        "report id '{id}' appears in {label}_ids but not submitted_ids"
                    ),
                });
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::closure::{ClosureArtifactRefs, ClosureClass, RegistryInstanceStatus};
    use crate::spec::{EvalBudget, IssueInput, MultiSweBenchSource, PreparedSingleRun, RunSource};

    fn prepared_run_for(tmp: &Path, instance_id: &str, number: u64) -> PreparedSingleRun {
        PreparedSingleRun {
            task_id: instance_id.to_string(),
            repo_root: tmp.join("repos").join("BurntSushi").join("ripgrep"),
            output_dir: tmp.join("instances").join(instance_id),
            issue: IssueInput {
                title: Some("issue".to_string()),
                body: None,
                body_path: None,
            },
            base_sha: Some("abc123".to_string()),
            head_sha: None,
            budget: EvalBudget::default(),
            source: Some(RunSource::MultiSweBench(MultiSweBenchSource {
                dataset_file: tmp.join("dataset.jsonl"),
                dataset_url: None,
                instance_id: instance_id.to_string(),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number,
                language: Some("rust".to_string()),
                expected_patch_files: Vec::new(),
            })),
            campaign: None,
        }
    }

    fn request(tmp: &Path) -> Request {
        let submission_path = tmp.join("multi-swe-bench-submission.jsonl");
        fs::create_dir_all(tmp.join("repos")).expect("create repos");
        fs::write(tmp.join("dataset.jsonl"), "{}\n").expect("write dataset");
        fs::write(&submission_path, "{}\n").expect("write submission");

        Request::new(
            prepared_run_for(tmp, "BurntSushi__ripgrep-2209", 2209),
            submission_path,
            Layout::under(tmp.join("mbe"), tmp.join("repos")),
            Options::default(),
        )
        .expect("valid request")
    }

    fn write_submission_record(path: &Path, number: u64) {
        fs::write(
            path,
            serde_json::to_string(&MultiSweBenchSubmissionRecord {
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number,
                fix_patch: format!("diff --git a/file-{number} b/file-{number}\n"),
            })
            .expect("serialize submission")
                + "\n",
        )
        .expect("write submission");
    }

    fn cohort_request_for_2209_and_454(tmp: &Path) -> CohortRequest {
        fs::create_dir_all(tmp.join("repos").join("BurntSushi").join("ripgrep"))
            .expect("create repo");
        fs::write(tmp.join("dataset.jsonl"), "{}\n").expect("write dataset");

        let first_submission = tmp.join("submission-2209.jsonl");
        let second_submission = tmp.join("submission-454.jsonl");
        write_submission_record(&first_submission, 2209);
        write_submission_record(&second_submission, 454);

        CohortRequest::new(
            vec![
                CohortMember {
                    prepared: prepared_run_for(tmp, "BurntSushi__ripgrep-2209", 2209),
                    submission_path: first_submission,
                },
                CohortMember {
                    prepared: prepared_run_for(tmp, "BurntSushi__ripgrep-454", 454),
                    submission_path: second_submission,
                },
            ],
            Layout::under(tmp.join("mbe"), tmp.join("repos")),
            Options::default(),
        )
        .expect("valid cohort request")
    }

    fn final_report_with_resolved_id(report_id: &str) -> FinalReport {
        FinalReport {
            total_instances: 1,
            submitted_instances: 1,
            completed_instances: 1,
            incomplete_instances: 0,
            resolved_instances: 1,
            unresolved_instances: 0,
            empty_patch_instances: 0,
            error_instances: 0,
            submitted_ids: vec![report_id.to_string()],
            completed_ids: vec![report_id.to_string()],
            incomplete_ids: Vec::new(),
            resolved_ids: vec![report_id.to_string()],
            unresolved_ids: Vec::new(),
            empty_patch_ids: Vec::new(),
            error_ids: Vec::new(),
        }
    }

    fn final_report_with_resolved_ids(report_ids: &[&str]) -> FinalReport {
        let ids = report_ids
            .iter()
            .map(|report_id| (*report_id).to_string())
            .collect::<Vec<_>>();
        FinalReport {
            total_instances: ids.len(),
            submitted_instances: ids.len(),
            completed_instances: ids.len(),
            incomplete_instances: 0,
            resolved_instances: ids.len(),
            unresolved_instances: 0,
            empty_patch_instances: 0,
            error_instances: 0,
            submitted_ids: ids.clone(),
            completed_ids: ids.clone(),
            incomplete_ids: Vec::new(),
            resolved_ids: ids,
            unresolved_ids: Vec::new(),
            empty_patch_ids: Vec::new(),
            error_ids: Vec::new(),
        }
    }

    fn fake_python_that_writes_report(tmp: &Path, report: &FinalReport) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;

        let argv_path = tmp.join("fake-python-argv.txt");
        let report_json = serde_json::to_string_pretty(report).expect("serialize final report");
        let fake_python = tmp.join("fake-python");
        fs::write(
            &fake_python,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat > \"$(dirname \"$4\")/{}\" <<'JSON'\n{}\nJSON\n",
                argv_path.display(),
                FINAL_REPORT_FILE,
                report_json
            ),
        )
        .expect("write fake python");
        let mut permissions = fs::metadata(&fake_python)
            .expect("fake python metadata")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&fake_python, permissions).expect("chmod fake python");
        (fake_python, argv_path)
    }

    fn stage_result(passed: usize, failed: usize, skipped: usize) -> StageResult {
        let passed_tests = (0..passed)
            .map(|index| format!("passed-{index}"))
            .collect::<BTreeSet<_>>();
        let failed_tests = (0..failed)
            .map(|index| format!("failed-{index}"))
            .collect::<BTreeSet<_>>();
        let skipped_tests = (0..skipped)
            .map(|index| format!("skipped-{index}"))
            .collect::<BTreeSet<_>>();
        StageResult {
            passed_count: passed,
            failed_count: failed,
            skipped_count: skipped,
            passed_tests,
            failed_tests,
            skipped_tests,
        }
    }

    fn missing_fix_results_instance_report() -> InstanceReport {
        InstanceReport {
            org: "BurntSushi".to_string(),
            repo: "ripgrep".to_string(),
            number: 2209,
            valid: Some(false),
            error_msg: Some("After applying the fix patch, no test results were captured".into()),
            fixed_tests: BTreeMap::new(),
            p2p_tests: BTreeMap::new(),
            f2p_tests: BTreeMap::new(),
            s2p_tests: BTreeMap::new(),
            n2p_tests: BTreeMap::new(),
            run_result: stage_result(274, 0, 0),
            test_patch_result: stage_result(274, 2, 0),
            fix_patch_result: stage_result(0, 0, 0),
        }
    }

    #[test]
    fn request_projects_prepared_run_into_harness_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let submission_path = tmp.path().join("multi-swe-bench-submission.jsonl");
        let request = request(tmp.path());

        let config = request.harness_config().expect("harness config");

        assert_eq!(config.mode, Mode::Evaluation);
        assert_eq!(config.patch_files, vec![submission_path]);
        assert_eq!(config.dataset_files, vec![tmp.path().join("dataset.jsonl")]);
        assert_eq!(
            config.specifics,
            vec!["BurntSushi/ripgrep:pr-2209".to_string()]
        );
        assert!(!config.need_clone);
        assert_eq!(config.max_workers, 1);
    }

    #[test]
    fn write_config_persists_typed_harness_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let output_dir = tmp.path().join("mbe");
        let request = request(tmp.path());

        let written = request.write_config().expect("write config");
        let text = fs::read_to_string(&written.path).expect("read config");
        let config: HarnessConfig = serde_json::from_str(&text).expect("typed config");

        assert_eq!(written.path, config_path(&output_dir));
        assert_eq!(written.report_path, report_path(&output_dir));
        assert_eq!(config.output_dir, output_dir);
        assert_eq!(config.log_dir, tmp.path().join("mbe").join("logs"));
    }

    #[test]
    fn written_config_projects_harness_invocation() {
        let written = WrittenConfig {
            path: PathBuf::from("/tmp/ploke eval/mbe-evaluation-config.json"),
            report_path: PathBuf::from("/tmp/ploke eval/final_report.json"),
        };

        let invocation = written.harness_invocation("python3");

        assert_eq!(invocation.program, "python3");
        assert_eq!(
            invocation.args,
            vec![
                "-m".to_string(),
                HARNESS_MODULE.to_string(),
                "--config".to_string(),
                "/tmp/ploke eval/mbe-evaluation-config.json".to_string()
            ]
        );
        assert_eq!(
            invocation.command_line(),
            "python3 -m multi_swe_bench.harness.run_evaluation --config '/tmp/ploke eval/mbe-evaluation-config.json'"
        );
    }

    #[test]
    fn run_harness_executes_configured_python_command_and_loads_report() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = request(tmp.path());
        let (fake_python, argv_path) = fake_python_that_writes_report(
            tmp.path(),
            &final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209"),
        );

        let run = request
            .run_harness(fake_python.display().to_string())
            .expect("fake harness run succeeds");

        let argv = fs::read_to_string(argv_path).expect("read fake python argv");
        assert_eq!(
            argv,
            format!(
                "-m\n{}\n--config\n{}\n",
                HARNESS_MODULE,
                run.written.path.display()
            )
        );
        assert_eq!(run.invocation.program, fake_python.display().to_string());
        assert_eq!(run.evidence.instance_id, "BurntSushi__ripgrep-2209");
        assert_eq!(run.evidence.verdict, Verdict::Resolved);
        assert_eq!(run.evaluation.evidence.verdict, Verdict::Resolved);
    }

    #[test]
    fn cohort_run_harness_executes_configured_python_command_and_loads_report() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = cohort_request_for_2209_and_454(tmp.path());
        let (fake_python, argv_path) = fake_python_that_writes_report(
            tmp.path(),
            &final_report_with_resolved_ids(&[
                "BurntSushi/ripgrep:pr-2209",
                "BurntSushi/ripgrep:pr-454",
            ]),
        );

        let run = request
            .run_harness(fake_python.display().to_string())
            .expect("fake cohort harness run succeeds");

        let argv = fs::read_to_string(argv_path).expect("read fake python argv");
        assert_eq!(
            argv,
            format!(
                "-m\n{}\n--config\n{}\n",
                HARNESS_MODULE,
                run.written.path.display()
            )
        );
        assert_eq!(run.invocation.program, fake_python.display().to_string());
        assert_eq!(run.report.resolved_instances, 2);
        assert_eq!(run.evaluations.len(), 2);
        assert!(
            run.evaluations
                .iter()
                .all(|evaluation| evaluation.evidence.verdict == Verdict::Resolved)
        );
    }

    #[test]
    fn request_from_manifest_defaults_mbe_layout_from_prepared_run() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prepared = prepared_run_for(tmp.path(), "BurntSushi__ripgrep-2209", 2209);
        fs::create_dir_all(tmp.path().join("repos").join("BurntSushi").join("ripgrep"))
            .expect("create repo");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");
        let run_manifest = tmp.path().join("run.json");
        fs::write(
            &run_manifest,
            serde_json::to_string_pretty(&prepared).expect("serialize run"),
        )
        .expect("write run");
        fs::create_dir_all(&prepared.output_dir).expect("create prepared output dir");
        let submission_path = prepared.output_dir.join(SUBMISSION_FILE);
        fs::write(&submission_path, "{}\n").expect("write submission");

        let request = Request::from_manifest(run_manifest, None, None, None, Options::default())
            .expect("request from manifest");

        assert_eq!(
            request.layout.output_dir,
            tmp.path()
                .join("instances")
                .join("BurntSushi__ripgrep-2209")
                .join("mbe")
        );
        assert_eq!(request.layout.repo_dir, tmp.path().join("repos"));
        assert_eq!(request.submission_path, submission_path);
    }

    #[test]
    fn request_from_candidate_uses_attempt_scoped_submission_and_output_dir() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let prepared = prepared_run_for(tmp.path(), "BurntSushi__ripgrep-2209", 2209);
        fs::create_dir_all(tmp.path().join("repos").join("BurntSushi").join("ripgrep"))
            .expect("create repo");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");
        fs::create_dir_all(&prepared.output_dir).expect("create prepared output dir");
        let run_manifest = prepared.output_dir.join("run.json");
        fs::write(
            &run_manifest,
            serde_json::to_string_pretty(&prepared).expect("serialize run"),
        )
        .expect("write run");
        let run_root = prepared.output_dir.join("runs").join("run-1");
        fs::create_dir_all(&run_root).expect("create run root");
        let submission_path = run_root.join(SUBMISSION_FILE);
        fs::write(&submission_path, "{}\n").expect("write attempt submission");

        let request = Request::from_candidate(
            RunCandidate {
                attempt: 1,
                latest: true,
                run_id: "run-1".to_string(),
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                run_manifest,
                run_root: run_root.clone(),
                submission_path: Some(submission_path.clone()),
                execution_status: RunExecutionStatus::Completed,
                submission_status: RunSubmissionStatus::EmptyPatch,
                started_at: None,
                finished_at: None,
            },
            None,
            None,
            None,
            Options::default(),
        )
        .expect("request from candidate");

        assert_eq!(request.submission_path, submission_path);
        assert_eq!(request.layout.output_dir, run_root.join("mbe"));
        assert_eq!(request.layout.repo_dir, tmp.path().join("repos"));
    }

    #[test]
    fn final_report_projects_oracle_evidence_for_prepared_run() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = request(tmp.path());
        let report_path = tmp.path().join("final_report.json");
        let report = final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209");
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&report).expect("serialize report"),
        )
        .expect("write report");

        let evidence = request
            .load_oracle_evidence_from(&report_path)
            .expect("oracle evidence");

        assert_eq!(evidence.instance_id, "BurntSushi__ripgrep-2209");
        assert_eq!(evidence.report_id, "BurntSushi/ripgrep:pr-2209");
        assert_eq!(evidence.verdict, Verdict::Resolved);
        assert_eq!(evidence.report_path, report_path);
    }

    #[test]
    fn final_report_rejects_count_mismatch() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let report_path = tmp.path().join("final_report.json");
        let mut report = final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209");
        report.resolved_instances = 2;
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&report).expect("serialize report"),
        )
        .expect("write report");

        let err = FinalReport::load(&report_path).expect_err("invalid report");
        assert!(matches!(err, PrepareError::InvalidMbeReport { .. }));
    }

    #[test]
    fn instance_report_loads_structured_stage_counts() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let report_path = tmp.path().join("report.json");
        let report = missing_fix_results_instance_report();
        fs::write(
            &report_path,
            serde_json::to_string_pretty(&report).expect("serialize report"),
        )
        .expect("write report");

        let loaded = InstanceReport::load(&report_path).expect("instance report");

        assert_eq!(loaded.report_id(), "BurntSushi/ripgrep:pr-2209");
        assert_eq!(loaded.run_result.total_count(), 274);
        assert_eq!(loaded.test_patch_result.total_count(), 276);
        assert_eq!(loaded.fix_result_count(), 0);
    }

    #[test]
    fn oracle_evaluation_derives_missing_fix_results_from_instance_report() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = request(tmp.path());
        let final_report_path = request.layout.output_dir.join(FINAL_REPORT_FILE);
        fs::create_dir_all(&request.layout.output_dir).expect("create output dir");
        let mut final_report = final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209");
        final_report.resolved_instances = 0;
        final_report.resolved_ids.clear();
        final_report.unresolved_instances = 1;
        final_report
            .unresolved_ids
            .push("BurntSushi/ripgrep:pr-2209".to_string());
        fs::write(
            &final_report_path,
            serde_json::to_string_pretty(&final_report).expect("serialize final report"),
        )
        .expect("write final report");
        let instance_report_path = request
            .layout
            .workdir
            .join("BurntSushi")
            .join("ripgrep")
            .join(EVALUATION_WORKDIR)
            .join("pr-2209")
            .join(INSTANCE_REPORT_FILE);
        fs::create_dir_all(
            instance_report_path
                .parent()
                .expect("instance report parent"),
        )
        .expect("create instance report dir");
        fs::write(
            &instance_report_path,
            serde_json::to_string_pretty(&missing_fix_results_instance_report())
                .expect("serialize instance report"),
        )
        .expect("write instance report");

        let evaluation = request
            .load_oracle_evaluation_from(&final_report_path)
            .expect("oracle evaluation");

        assert_eq!(evaluation.evidence.verdict, Verdict::Unresolved);
        assert_eq!(evaluation.diagnostic, OracleDiagnostic::MissingFixResults);
        assert!(!evaluation.usable_for_selection);
        assert_eq!(evaluation.instance_report_path, instance_report_path);
    }

    #[test]
    fn oracle_evaluation_derives_fix_compile_failed_from_fix_log() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = request(tmp.path());
        let final_report_path = request.layout.output_dir.join(FINAL_REPORT_FILE);
        fs::create_dir_all(&request.layout.output_dir).expect("create output dir");
        let mut final_report = final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209");
        final_report.resolved_instances = 0;
        final_report.resolved_ids.clear();
        final_report.unresolved_instances = 1;
        final_report
            .unresolved_ids
            .push("BurntSushi/ripgrep:pr-2209".to_string());
        fs::write(
            &final_report_path,
            serde_json::to_string_pretty(&final_report).expect("serialize final report"),
        )
        .expect("write final report");
        let source = match request.prepared.source.as_ref().expect("source") {
            RunSource::MultiSweBench(source) => source,
        };
        let instance_report_path = instance_report_path(&request.layout.workdir, source);
        fs::create_dir_all(
            instance_report_path
                .parent()
                .expect("instance report parent"),
        )
        .expect("create instance report dir");
        fs::write(
            &instance_report_path,
            serde_json::to_string_pretty(&missing_fix_results_instance_report())
                .expect("serialize instance report"),
        )
        .expect("write instance report");
        fs::write(
            fix_patch_run_log_path(&request.layout.workdir, source),
            "error[E0425]: cannot find function `replace_all_clipped`\nerror: could not compile `grep-printer`\n",
        )
        .expect("write fix log");

        let evaluation = request
            .load_oracle_evaluation_from(&final_report_path)
            .expect("oracle evaluation");

        assert_eq!(evaluation.diagnostic, OracleDiagnostic::FixCompileFailed);
        assert!(!evaluation.usable_for_selection);
    }

    #[test]
    fn cohort_request_projects_multiple_runs_into_harness_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("repos").join("BurntSushi").join("ripgrep"))
            .expect("create repo");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");

        let first_submission = tmp.path().join("submission-2209.jsonl");
        let second_submission = tmp.path().join("submission-454.jsonl");
        write_submission_record(&first_submission, 2209);
        write_submission_record(&second_submission, 454);

        let request = CohortRequest::new(
            vec![
                CohortMember {
                    prepared: prepared_run_for(tmp.path(), "BurntSushi__ripgrep-2209", 2209),
                    submission_path: first_submission,
                },
                CohortMember {
                    prepared: prepared_run_for(tmp.path(), "BurntSushi__ripgrep-454", 454),
                    submission_path: second_submission,
                },
            ],
            Layout::under(tmp.path().join("mbe"), tmp.path().join("repos")),
            Options::default(),
        )
        .expect("valid cohort request");

        let config = request.harness_config().expect("harness config");

        assert_eq!(
            config.patch_files,
            vec![tmp.path().join("mbe").join(SUBMISSION_FILE)]
        );
        assert_eq!(config.dataset_files, vec![tmp.path().join("dataset.jsonl")]);
        assert_eq!(
            config.specifics,
            vec![
                "BurntSushi/ripgrep:pr-2209".to_string(),
                "BurntSushi/ripgrep:pr-454".to_string()
            ]
        );
    }

    #[test]
    fn cohort_request_writes_aggregate_submission_and_loads_per_instance_evaluations() {
        let tmp = tempfile::tempdir().expect("tempdir");
        fs::create_dir_all(tmp.path().join("repos").join("BurntSushi").join("ripgrep"))
            .expect("create repo");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");

        let first_submission = tmp.path().join("submission-2209.jsonl");
        let second_submission = tmp.path().join("submission-454.jsonl");
        write_submission_record(&first_submission, 2209);
        write_submission_record(&second_submission, 454);

        let request = CohortRequest::new(
            vec![
                CohortMember {
                    prepared: prepared_run_for(tmp.path(), "BurntSushi__ripgrep-2209", 2209),
                    submission_path: first_submission,
                },
                CohortMember {
                    prepared: prepared_run_for(tmp.path(), "BurntSushi__ripgrep-454", 454),
                    submission_path: second_submission,
                },
            ],
            Layout::under(tmp.path().join("mbe"), tmp.path().join("repos")),
            Options::default(),
        )
        .expect("valid cohort request");

        let written = request.write_config().expect("write config");
        let aggregate = fs::read_to_string(request.layout.output_dir.join(SUBMISSION_FILE))
            .expect("read aggregate submission");
        assert_eq!(aggregate.lines().count(), 2);

        let report = FinalReport {
            total_instances: 2,
            submitted_instances: 2,
            completed_instances: 2,
            incomplete_instances: 0,
            resolved_instances: 1,
            unresolved_instances: 1,
            empty_patch_instances: 0,
            error_instances: 0,
            submitted_ids: vec![
                "BurntSushi/ripgrep:pr-2209".to_string(),
                "BurntSushi/ripgrep:pr-454".to_string(),
            ],
            completed_ids: vec![
                "BurntSushi/ripgrep:pr-2209".to_string(),
                "BurntSushi/ripgrep:pr-454".to_string(),
            ],
            incomplete_ids: Vec::new(),
            resolved_ids: vec!["BurntSushi/ripgrep:pr-2209".to_string()],
            unresolved_ids: vec!["BurntSushi/ripgrep:pr-454".to_string()],
            empty_patch_ids: Vec::new(),
            error_ids: Vec::new(),
        };
        fs::write(
            &written.report_path,
            serde_json::to_string_pretty(&report).expect("serialize report"),
        )
        .expect("write report");

        let evaluations = request
            .load_oracle_evaluations_from(&written.report_path)
            .expect("load evaluations");

        assert_eq!(evaluations.len(), 2);
        assert_eq!(
            evaluations[0].evidence.instance_id,
            "BurntSushi__ripgrep-2209"
        );
        assert_eq!(evaluations[0].diagnostic, OracleDiagnostic::Resolved);
        assert_eq!(
            evaluations[1].evidence.instance_id,
            "BurntSushi__ripgrep-454"
        );
        assert_eq!(
            evaluations[1].diagnostic,
            OracleDiagnostic::MissingInstanceReport
        );
    }

    #[test]
    fn oracle_cohort_report_must_cover_configured_members_exactly() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let request = cohort_request_for_2209_and_454(tmp.path());
        let written = request.write_config().expect("write config");

        let missing = final_report_with_resolved_id("BurntSushi/ripgrep:pr-2209");
        fs::write(
            &written.report_path,
            serde_json::to_string_pretty(&missing).expect("serialize report"),
        )
        .expect("write report");
        let err = request
            .load_oracle_evaluations_from(&written.report_path)
            .expect_err("missing configured report id is rejected");
        assert!(err.to_string().contains("missing configured report id"));

        let unknown = final_report_with_resolved_id("BurntSushi/ripgrep:pr-999");
        fs::write(
            &written.report_path,
            serde_json::to_string_pretty(&unknown).expect("serialize report"),
        )
        .expect("write report");
        let err = request
            .load_oracle_evaluations_from(&written.report_path)
            .expect_err("unknown report id is rejected");
        assert!(err.to_string().contains("unknown report id"));
    }

    #[test]
    fn campaign_instances_from_rows_preserves_full_cohort() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let first_submission = tmp.path().join("submission-2209.jsonl");
        let second_submission = tmp.path().join("submission-454.jsonl");
        let first_manifest = tmp.path().join("run-2209.json");
        let second_manifest = tmp.path().join("run-454.json");
        write_submission_record(&first_submission, 2209);
        write_submission_record(&second_submission, 454);
        fs::write(&first_manifest, "{}").expect("write first manifest placeholder");
        fs::write(&second_manifest, "{}").expect("write second manifest placeholder");

        let rows = vec![
            ClosureInstanceRow {
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                dataset_label: "ripgrep".to_string(),
                repo_family: "ripgrep".to_string(),
                registry_status: RegistryInstanceStatus::Mapped,
                eval_status: ClosureClass::Complete,
                protocol_status: ClosureClass::Complete,
                eval_failure: None,
                protocol_failure: None,
                artifacts: ClosureArtifactRefs {
                    run_manifest: Some(first_manifest),
                    msb_submission: Some(first_submission),
                    ..ClosureArtifactRefs::default()
                },
                protocol_procedures: std::collections::BTreeMap::new(),
                protocol_counts: None,
                last_event_at: None,
            },
            ClosureInstanceRow {
                instance_id: "BurntSushi__ripgrep-454".to_string(),
                dataset_label: "ripgrep".to_string(),
                repo_family: "ripgrep".to_string(),
                registry_status: RegistryInstanceStatus::Mapped,
                eval_status: ClosureClass::Complete,
                protocol_status: ClosureClass::Complete,
                eval_failure: None,
                protocol_failure: None,
                artifacts: ClosureArtifactRefs {
                    run_manifest: Some(second_manifest),
                    msb_submission: Some(second_submission),
                    ..ClosureArtifactRefs::default()
                },
                protocol_procedures: std::collections::BTreeMap::new(),
                protocol_counts: None,
                last_event_at: None,
            },
        ];

        let instances = campaign_instances_from_rows(rows.iter()).expect("campaign instances");

        assert_eq!(instances.len(), 2);
        assert_eq!(instances[0].instance_id(), "BurntSushi__ripgrep-2209");
        assert_eq!(instances[1].instance_id(), "BurntSushi__ripgrep-454");
    }
}
