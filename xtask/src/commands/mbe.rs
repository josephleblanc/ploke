//! Multi-SWE-Bench evaluator setup helpers.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitStatus};

use serde::Serialize;

use super::{CommandContext, XtaskError};
use crate::executor::Command;

const DEFAULT_REMOTE: &str = "https://github.com/multi-swe-bench/multi-swe-bench.git";
const HARNESS_MODULE: &str = "multi_swe_bench.harness.run_evaluation";

/// Multi-SWE-Bench evaluator setup commands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Mbe {
    /// Clone/install the Multi-SWE-Bench harness and verify the Python import.
    Setup(Setup),
}

impl Mbe {
    /// Execute an MBE helper command.
    pub fn execute(&self, ctx: &CommandContext) -> Result<MbeOutput, XtaskError> {
        match self {
            Self::Setup(cmd) => cmd.execute(ctx).map(MbeOutput::Setup),
        }
    }
}

/// Output from an MBE helper command.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MbeOutput {
    /// Result of `cargo xtask mbe setup`.
    Setup(SetupReport),
}

/// Clone/install options for the Multi-SWE-Bench harness.
#[derive(Debug, Clone, clap::Args)]
pub struct Setup {
    /// Local source checkout. Defaults to `$PLOKE_EVAL_HOME/tools/multi-swe-bench` or `$HOME/.ploke-eval/tools/multi-swe-bench`.
    #[arg(long, value_name = "PATH")]
    pub source: Option<PathBuf>,

    /// Virtual environment directory. Defaults to `<source>/.venv`.
    #[arg(long, value_name = "PATH")]
    pub venv: Option<PathBuf>,

    /// Git remote used when the source checkout is absent.
    #[arg(long, default_value = DEFAULT_REMOTE, value_name = "URL")]
    pub remote: String,

    /// Optional git revision to checkout after clone/fetch.
    #[arg(long, value_name = "REV")]
    pub rev: Option<String>,

    /// Python interpreter used for the virtual environment.
    #[arg(long = "python", default_value = "python3.11", value_name = "PYTHON")]
    pub python_interpreter: String,
}

impl Command for Setup {
    type Output = SetupReport;
    type Error = XtaskError;

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let source_dir = self.source.clone().unwrap_or_else(default_source_dir);
        let venv_dir = self
            .venv
            .clone()
            .unwrap_or_else(|| source_dir.join(".venv"));
        let python = venv_dir.join("bin").join("python");

        let source_action = ensure_source(&source_dir, &self.remote, self.rev.as_deref())?;
        let source_head = git_head(&source_dir)?;
        ensure_venv(&venv_dir, &self.python_interpreter)?;
        let python_version = python_version(&python)?;
        validate_python_version(&python_version, &self.python_interpreter)?;
        install_source(&venv_dir, &source_dir)?;
        let probe = probe_import(&python)?;

        Ok(SetupReport {
            source_dir,
            venv_dir,
            python,
            remote: self.remote.clone(),
            source_head,
            source_action,
            python_interpreter: self.python_interpreter.clone(),
            python_version,
            harness_module: HARNESS_MODULE,
            import_ok: probe.import_ok,
            module_path: probe.module_path,
        })
    }
}

/// Report produced by `cargo xtask mbe setup`.
#[derive(Debug, Clone, Serialize)]
pub struct SetupReport {
    /// Source checkout path.
    pub source_dir: PathBuf,
    /// Virtual environment path.
    pub venv_dir: PathBuf,
    /// Python executable inside the virtual environment.
    pub python: PathBuf,
    /// Git remote used for the source checkout.
    pub remote: String,
    /// Current source checkout `HEAD` commit.
    pub source_head: String,
    /// Whether setup reused an existing checkout or cloned a new one.
    pub source_action: SourceAction,
    /// Requested interpreter passed to `uv venv --python`.
    pub python_interpreter: String,
    /// Version string reported by the virtual environment Python.
    pub python_version: String,
    /// Python harness module imported during the verification probe.
    pub harness_module: &'static str,
    /// Whether the harness import probe succeeded.
    pub import_ok: bool,
    /// Resolved module file path from the import probe.
    pub module_path: PathBuf,
}

/// Source checkout action taken by setup.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SourceAction {
    /// Existing source checkout was reused.
    Reused,
    /// Source checkout was cloned by this setup run.
    Cloned,
}

#[derive(Debug)]
struct Probe {
    import_ok: bool,
    module_path: PathBuf,
}

fn default_source_dir() -> PathBuf {
    default_eval_home().join("tools").join("multi-swe-bench")
}

fn default_eval_home() -> PathBuf {
    env::var_os("PLOKE_EVAL_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".ploke-eval")))
        .unwrap_or_else(|| PathBuf::from(".ploke-eval"))
}

fn ensure_source(
    source_dir: &Path,
    remote: &str,
    rev: Option<&str>,
) -> Result<SourceAction, XtaskError> {
    let action = if source_dir.exists() {
        if !source_dir.join("setup.py").exists() {
            return Err(XtaskError::validation(format!(
                "Multi-SWE-Bench checkout `{}` exists but has no setup.py",
                source_dir.display()
            ))
            .with_recovery("Pass --source PATH pointing at the multi-swe-bench repository root, or remove the invalid directory and rerun setup."));
        }
        SourceAction::Reused
    } else {
        let parent = source_dir.parent().ok_or_else(|| {
            XtaskError::validation(format!(
                "Multi-SWE-Bench source path `{}` has no parent directory",
                source_dir.display()
            ))
        })?;
        fs::create_dir_all(parent)?;
        run(
            "git",
            &["clone", remote, &source_dir.display().to_string()],
            None,
        )?;
        SourceAction::Cloned
    };

    if let Some(rev) = rev {
        run("git", &["fetch", "origin", rev], Some(source_dir))?;
        run("git", &["checkout", rev], Some(source_dir))?;
    }

    Ok(action)
}

fn git_head(source_dir: &Path) -> Result<String, XtaskError> {
    let output = ProcessCommand::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(source_dir)
        .output()
        .map_err(|err| XtaskError::Io(format!("failed to run git rev-parse: {err}")))?;
    ensure_success(
        "git rev-parse HEAD",
        output.status,
        &output.stdout,
        &output.stderr,
    )?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn ensure_venv(venv_dir: &Path, python_interpreter: &str) -> Result<(), XtaskError> {
    run(
        "uv",
        &[
            "venv",
            "--clear",
            "--python",
            python_interpreter,
            &venv_dir.display().to_string(),
        ],
        None,
    )
}

fn install_source(venv_dir: &Path, source_dir: &Path) -> Result<(), XtaskError> {
    let python = venv_dir.join("bin").join("python");
    run(
        "uv",
        &[
            "pip",
            "install",
            "--python",
            &python.display().to_string(),
            "-e",
            &source_dir.display().to_string(),
        ],
        None,
    )
}

fn python_version(python: &Path) -> Result<String, XtaskError> {
    let output = ProcessCommand::new(python)
        .arg("--version")
        .output()
        .map_err(|err| XtaskError::Io(format!("failed to run python --version: {err}")))?;
    ensure_success(
        "python --version",
        output.status,
        &output.stdout,
        &output.stderr,
    )?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stdout.is_empty() {
        Ok(stderr)
    } else {
        Ok(stdout)
    }
}

fn validate_python_version(version: &str, interpreter: &str) -> Result<(), XtaskError> {
    if version.starts_with("Python 3.11.") || version == "Python 3.11" {
        return Ok(());
    }

    Err(XtaskError::validation(format!(
        "Multi-SWE-Bench setup resolved `{interpreter}` to unsupported {version}; Python 3.11 is required on this VM"
    ))
    .with_recovery("Re-run `cargo xtask mbe setup --python python3.11`; uv will recreate the venv with Python 3.11."))
}

fn probe_import(python: &Path) -> Result<Probe, XtaskError> {
    let script = format!(
        "import importlib; module = importlib.import_module({HARNESS_MODULE:?}); print(module.__file__)"
    );
    let output = ProcessCommand::new(python)
        .arg("-c")
        .arg(script)
        .output()
        .map_err(|err| XtaskError::Io(format!("failed to run import probe: {err}")))?;
    ensure_success(
        "python import probe for multi_swe_bench.harness.run_evaluation",
        output.status,
        &output.stdout,
        &output.stderr,
    )?;

    let module_path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if module_path.is_empty() {
        return Err(XtaskError::validation(
            "Multi-SWE-Bench import probe returned an empty module path",
        )
        .with_recovery(
            "Re-run `cargo xtask mbe setup`; if it repeats, inspect the virtualenv site-packages.",
        ));
    }

    Ok(Probe {
        import_ok: true,
        module_path: PathBuf::from(module_path),
    })
}

fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<(), XtaskError> {
    let mut command = ProcessCommand::new(program);
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let output = command
        .output()
        .map_err(|err| XtaskError::Io(format!("failed to run {program}: {err}")))?;
    let rendered = format_command(program, args);
    ensure_success(&rendered, output.status, &output.stdout, &output.stderr)
}

fn ensure_success(
    command: &str,
    status: ExitStatus,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<(), XtaskError> {
    if status.success() {
        return Ok(());
    }

    Err(XtaskError::validation(format!(
        "command `{command}` failed with status {status}\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(stdout).trim(),
        String::from_utf8_lossy(stderr).trim()
    ))
    .with_recovery("Install the missing host tool/dependency or rerun with --source/--venv pointing at a known-good checkout/environment."))
}

fn format_command(program: &str, args: &[&str]) -> String {
    let mut rendered = String::from(program);
    for arg in args {
        rendered.push(' ');
        rendered.push_str(arg);
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_eval_home_prefers_ploke_eval_home() {
        let path = eval_home_from(Some("/tmp/eval".into()), Some("/tmp/home".into()));
        assert_eq!(path, PathBuf::from("/tmp/eval"));
    }

    #[test]
    fn default_eval_home_falls_back_to_home() {
        let path = eval_home_from(None, Some("/tmp/home".into()));
        assert_eq!(path, PathBuf::from("/tmp/home/.ploke-eval"));
    }

    #[test]
    fn python_version_validation_rejects_python_3_14() {
        let err = validate_python_version("Python 3.14.5", "python3")
            .expect_err("Python 3.14 must fail for Multi-SWE-Bench setup");
        assert!(err.to_string().contains("Python 3.11 is required"));
    }

    #[test]
    fn python_version_validation_accepts_python_3_11() {
        validate_python_version("Python 3.11.15", "python3.11")
            .expect("Python 3.11 is the supported MBE setup interpreter");
    }

    fn eval_home_from(ploke_eval_home: Option<PathBuf>, home: Option<PathBuf>) -> PathBuf {
        ploke_eval_home
            .or_else(|| home.map(|home| home.join(".ploke-eval")))
            .unwrap_or_else(|| PathBuf::from(".ploke-eval"))
    }
}
