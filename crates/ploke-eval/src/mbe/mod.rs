use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::spec::{PrepareError, PreparedSingleRun, RunSource};

pub const CONFIG_FILE: &str = "mbe-evaluation-config.json";
pub const FINAL_REPORT_FILE: &str = "final_report.json";

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

impl Request {
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

        Ok(HarnessConfig {
            mode: Mode::Evaluation,
            workdir: self.layout.workdir.clone(),
            patch_files: vec![self.submission_path.clone()],
            dataset_files: vec![source.dataset_file.clone()],
            force_build: self.options.force_build,
            output_dir: self.layout.output_dir.clone(),
            specifics: vec![source.instance_id.clone()],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{EvalBudget, IssueInput, MultiSweBenchSource, PreparedSingleRun, RunSource};

    fn prepared_run(tmp: &Path) -> PreparedSingleRun {
        PreparedSingleRun {
            task_id: "BurntSushi__ripgrep-2209".to_string(),
            repo_root: tmp.join("repos").join("BurntSushi").join("ripgrep"),
            output_dir: tmp.join("instances").join("BurntSushi__ripgrep-2209"),
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
                instance_id: "BurntSushi__ripgrep-2209".to_string(),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                language: Some("rust".to_string()),
                expected_patch_files: Vec::new(),
            })),
            campaign: None,
        }
    }

    #[test]
    fn request_projects_prepared_run_into_harness_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let submission_path = tmp.path().join("multi-swe-bench-submission.jsonl");
        fs::create_dir_all(tmp.path().join("repos")).expect("create repos");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");
        fs::write(&submission_path, "{}\n").expect("write submission");

        let request = Request::new(
            prepared_run(tmp.path()),
            submission_path.clone(),
            Layout::under(tmp.path().join("mbe"), tmp.path().join("repos")),
            Options::default(),
        )
        .expect("valid request");

        let config = request.harness_config().expect("harness config");

        assert_eq!(config.mode, Mode::Evaluation);
        assert_eq!(config.patch_files, vec![submission_path]);
        assert_eq!(config.dataset_files, vec![tmp.path().join("dataset.jsonl")]);
        assert_eq!(
            config.specifics,
            vec!["BurntSushi__ripgrep-2209".to_string()]
        );
        assert!(!config.need_clone);
        assert_eq!(config.max_workers, 1);
    }

    #[test]
    fn write_config_persists_typed_harness_config() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let submission_path = tmp.path().join("multi-swe-bench-submission.jsonl");
        fs::create_dir_all(tmp.path().join("repos")).expect("create repos");
        fs::write(tmp.path().join("dataset.jsonl"), "{}\n").expect("write dataset");
        fs::write(&submission_path, "{}\n").expect("write submission");
        let output_dir = tmp.path().join("mbe");

        let request = Request::new(
            prepared_run(tmp.path()),
            submission_path,
            Layout::under(output_dir.clone(), tmp.path().join("repos")),
            Options::default(),
        )
        .expect("valid request");

        let written = request.write_config().expect("write config");
        let text = fs::read_to_string(&written.path).expect("read config");
        let config: HarnessConfig = serde_json::from_str(&text).expect("typed config");

        assert_eq!(written.path, config_path(&output_dir));
        assert_eq!(written.report_path, report_path(&output_dir));
        assert_eq!(config.output_dir, output_dir);
        assert_eq!(config.log_dir, tmp.path().join("mbe").join("logs"));
    }
}
