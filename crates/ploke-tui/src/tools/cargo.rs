//! Cargo tool integration for running `cargo check` or `cargo test` with JSON diagnostics.
//!
//! This tool shells out to `cargo` with `--message-format=json` and parses the line-delimited
//! JSON stream to extract compiler diagnostics and artifact counts. Non-JSON output is kept in
//! bounded tails to aid debugging when build scripts or test binaries emit extra text.
//!
//! # Usage
//!
//! The tool accepts a strict schema with explicit flags. There are no free-form cargo args.
//! For test filters or runtime flags, use `test_args`, which are passed after `--`.
//!
//! ```rust
//! use ploke_tui::tools::{cargo::CargoTool, Tool};
//!
//! let params = CargoTool::deserialize_params(r#"{"command":"check"}"#).unwrap();
//! assert!(matches!(params.command, ploke_tui::tools::cargo::CargoCommand::Check));
//! ```
//!
//! ```rust
//! use ploke_tui::tools::{cargo::CargoTool, Tool};
//!
//! let params = CargoTool::deserialize_params(
//!     r#"{"command":"test","test_args":["my_test","--nocapture"]}"#,
//! ).unwrap();
//! assert!(params.test_args.is_some());
//! ```
use std::{borrow::Cow, collections::VecDeque, path::Path, time::Duration};

use cargo_metadata::Message;
use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::Instant,
};

use super::{
    Tool, ToolDescr, ToolError, ToolErrorCode, ToolInvocationError, ToolName, ToolResult,
    ToolUiPayload, ToolVerbosity, tool_io_error, tool_ui_error,
};
use crate::tracing_setup::TOOL_CALL_TARGET;
use ploke_test_utils::workspace_root;

const MAX_DIAGNOSTICS: usize = 50;
const MAX_SPANS_PER_DIAGNOSTIC: usize = 3;
const MAX_TAIL_LINES: usize = 200;
const MAX_JSON_PARSE_ERRORS: usize = 100;
const MAX_TOOL_RESPONSE_BYTES: usize = 200 * 1024;
const KILL_GRACE_SECS: u64 = 2;

const COMMAND_DESC: &str = "Which cargo command to run: test or check.";
const SCOPE_DESC: &str =
    "Run against the focused manifest (default) or the workspace root manifest.";
const PACKAGE_DESC: &str = "Optional workspace package name (workspace scope only).";
const FEATURES_DESC: &str = "Optional feature list passed to --features.";
const ALL_FEATURES_DESC: &str = "Enable all features (--all-features).";
const NO_DEFAULT_FEATURES_DESC: &str = "Disable default features (--no-default-features).";
const TARGET_DESC: &str = "Target triple for --target.";
const PROFILE_DESC: &str = "Cargo profile name for --profile.";
const RELEASE_DESC: &str = "Enable release profile (--release).";
const LIB_DESC: &str = "Check/test the library target only (--lib).";
const TESTS_DESC: &str = "Check/test all test targets (--tests).";
const BINS_DESC: &str = "Check/test all binary targets (--bins).";
const EXAMPLES_DESC: &str = "Check/test all example targets (--examples).";
const BENCHES_DESC: &str = "Check/test all bench targets (--benches).";
const TEST_ARGS_DESC: &str = "Arguments for the test binary (only for cargo test).";

lazy_static::lazy_static! {
    static ref CARGO_PARAMETERS: serde_json::Value = serde_json::json!({
        "type": "object",
        "properties": {
            "command": {
                "type": "string",
                "enum": ["test", "check"],
                "description": COMMAND_DESC
            },
            "scope": {
                "type": "string",
                "enum": ["focused", "workspace"],
                "description": SCOPE_DESC
            },
            "package": {
                "type": "string",
                "description": PACKAGE_DESC
            },
            "features": {
                "type": "array",
                "items": { "type": "string" },
                "description": FEATURES_DESC
            },
            "all_features": {
                "type": "boolean",
                "description": ALL_FEATURES_DESC
            },
            "no_default_features": {
                "type": "boolean",
                "description": NO_DEFAULT_FEATURES_DESC
            },
            "target": {
                "type": "string",
                "description": TARGET_DESC
            },
            "profile": {
                "type": "string",
                "description": PROFILE_DESC
            },
            "release": {
                "type": "boolean",
                "description": RELEASE_DESC
            },
            "lib": {
                "type": "boolean",
                "description": LIB_DESC
            },
            "tests": {
                "type": "boolean",
                "description": TESTS_DESC
            },
            "bins": {
                "type": "boolean",
                "description": BINS_DESC
            },
            "examples": {
                "type": "boolean",
                "description": EXAMPLES_DESC
            },
            "benches": {
                "type": "boolean",
                "description": BENCHES_DESC
            },
            "test_args": {
                "type": "array",
                "items": { "type": "string" },
                "description": TEST_ARGS_DESC
            }
        },
        "required": ["command"],
        "additionalProperties": false
    });
}

/// Supported cargo subcommands.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CargoCommand {
    Test,
    Check,
}

impl CargoCommand {
    fn as_str(self) -> &'static str {
        match self {
            CargoCommand::Test => "test",
            CargoCommand::Check => "check",
        }
    }
}

/// Execution scope for the cargo invocation.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CargoScope {
    Focused,
    Workspace,
}

impl CargoScope {
    fn as_str(self) -> &'static str {
        match self {
            CargoScope::Focused => "focused",
            CargoScope::Workspace => "workspace",
        }
    }
}
impl Default for CargoScope {
    fn default() -> Self {
        CargoScope::Focused
    }
}

/// Parameters accepted by the cargo tool.
///
/// These map to a constrained set of cargo flags and are validated before execution.
///
/// ```rust
/// use ploke_tui::tools::{cargo::CargoTool, Tool};
///
/// let params = CargoTool::deserialize_params(
///     r#"{"command":"check","scope":"focused","all_features":true}"#,
/// ).unwrap();
/// assert!(params.all_features);
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct CargoToolParams<'a> {
    pub command: CargoCommand,
    #[serde(default)]
    pub scope: CargoScope,
    #[serde(default, borrow)]
    pub package: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub features: Option<Vec<Cow<'a, str>>>,
    #[serde(default)]
    pub all_features: bool,
    #[serde(default)]
    pub no_default_features: bool,
    #[serde(default, borrow)]
    pub target: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub profile: Option<Cow<'a, str>>,
    #[serde(default)]
    pub release: bool,
    #[serde(default)]
    pub lib: bool,
    #[serde(default)]
    pub tests: bool,
    #[serde(default)]
    pub bins: bool,
    #[serde(default)]
    pub examples: bool,
    #[serde(default)]
    pub benches: bool,
    #[serde(default, borrow)]
    pub test_args: Option<Vec<Cow<'a, str>>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CargoToolParamsOwned {
    pub command: CargoCommand,
    pub scope: CargoScope,
    pub package: Option<String>,
    pub features: Option<Vec<String>>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub target: Option<String>,
    pub profile: Option<String>,
    pub release: bool,
    pub lib: bool,
    pub tests: bool,
    pub bins: bool,
    pub examples: bool,
    pub benches: bool,
    pub test_args: Option<Vec<String>>,
}

/// Result payload emitted by the cargo tool.
///
/// Diagnostics are capped for size safety; tails contain the last observed non-JSON output.
#[derive(Debug, Clone, Serialize)]
pub struct CargoToolResult {
    pub ok: bool,
    pub status_reason: CargoStatusReason,
    pub command: CargoCommand,
    pub scope: CargoScope,
    pub manifest_path: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub summary: CargoSummary,
    pub diagnostics: Vec<CargoDiagnostic>,
    pub stderr_tail: Vec<String>,
    pub non_json_stdout_tail: Vec<String>,
    pub json_parse_errors_tail: Vec<String>,
    pub raw_messages_truncated: bool,
}

/// Summary counts derived from cargo JSON messages.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CargoSummary {
    pub errors: u32,
    pub warnings: u32,
    pub notes: u32,
    pub artifacts: u32,
    pub other_messages: u32,
}

/// Condensed diagnostic for LLM/UI consumption.
#[derive(Debug, Clone, Serialize)]
pub struct CargoDiagnostic {
    pub level: String,
    pub message: String,
    pub code: Option<String>,
    pub spans: Vec<CargoSpan>,
    pub rendered: Option<String>,
}

/// Source span attached to a diagnostic.
#[derive(Debug, Clone, Serialize)]
pub struct CargoSpan {
    pub file_name: String,
    pub line_start: u32,
    pub line_end: u32,
    pub column_start: u32,
    pub column_end: u32,
    pub is_primary: bool,
}

/// Final status category for a cargo invocation.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CargoStatusReason {
    Success,
    CompileFailed,
    TestsFailedOrRuntime,
    CargoFailedOrInvalidArgs,
    Timeout,
    Canceled,
    Killed,
}

impl CargoStatusReason {
    fn as_str(self) -> &'static str {
        match self {
            CargoStatusReason::Success => "success",
            CargoStatusReason::CompileFailed => "compile_failed",
            CargoStatusReason::TestsFailedOrRuntime => "tests_failed_or_runtime",
            CargoStatusReason::CargoFailedOrInvalidArgs => "cargo_failed_or_invalid_args",
            CargoStatusReason::Timeout => "timeout",
            CargoStatusReason::Canceled => "canceled",
            CargoStatusReason::Killed => "killed",
        }
    }
}

#[derive(Default)]
struct StdoutState {
    summary: CargoSummary,
    diagnostics: Vec<CargoDiagnostic>,
    non_json_stdout_tail: VecDeque<String>,
    json_parse_errors_tail: VecDeque<String>,
    raw_messages_truncated: bool,
}

#[derive(Default)]
struct StderrState {
    stderr_tail: VecDeque<String>,
    raw_messages_truncated: bool,
}

/// Tool entry point for `cargo check` / `cargo test`.
///
/// ```rust
/// use ploke_tui::tools::{cargo::CargoTool, Tool};
///
/// let params = CargoTool::deserialize_params(r#"{"command":"check"}"#).unwrap();
/// assert!(params.features.is_none());
/// ```
pub struct CargoTool;

impl Tool for CargoTool {
    type Output = CargoToolResult;
    type OwnedParams = CargoToolParamsOwned;
    type Params<'de> = CargoToolParams<'de>;

    fn name() -> ToolName {
        ToolName::Cargo
    }

    fn description() -> ToolDescr {
        ToolDescr::Cargo
    }

    fn schema() -> &'static serde_json::Value {
        &CARGO_PARAMETERS
    }

    fn build(_ctx: &super::Ctx) -> Self {
        Self
    }

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams {
        CargoToolParamsOwned {
            command: params.command,
            scope: params.scope,
            package: params.package.as_ref().map(|s| s.to_string()),
            features: params
                .features
                .as_ref()
                .map(|v| v.iter().map(|s| s.to_string()).collect()),
            all_features: params.all_features,
            no_default_features: params.no_default_features,
            target: params.target.as_ref().map(|s| s.to_string()),
            profile: params.profile.as_ref().map(|s| s.to_string()),
            release: params.release,
            lib: params.lib,
            tests: params.tests,
            bins: params.bins,
            examples: params.examples,
            benches: params.benches,
            test_args: params
                .test_args
                .as_ref()
                .map(|v| v.iter().map(|s| s.to_string()).collect()),
        }
    }

    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        let params: CargoToolParams<'a> =
            serde_json::from_str(json).map_err(|e| ToolInvocationError::Deserialize {
                source: e,
                raw: Some(json.to_string()),
            })?;
        validate_params(&params)?;
        Ok(params)
    }

    #[tracing::instrument(
        target = TOOL_CALL_TARGET,
        skip_all,
        fields(
            tool = "cargo",
            request_id = %ctx.request_id,
            parent_id = %ctx.parent_id,
            call_id = %ctx.call_id,
            command = %params.command.as_str(),
            scope = %params.scope.as_str(),
            package = ?params.package.as_ref().map(|s| s.as_ref()),
            features = ?params.features.as_ref().map(|v| v.iter().map(|s| s.as_ref()).collect::<Vec<_>>()),
            all_features = params.all_features,
            no_default_features = params.no_default_features,
            target = ?params.target.as_ref().map(|s| s.as_ref()),
            profile = ?params.profile.as_ref().map(|s| s.as_ref()),
            release = params.release,
            lib = params.lib,
            tests = params.tests,
            bins = params.bins,
            examples = params.examples,
            benches = params.benches,
            test_args = ?params.test_args.as_ref().map(|v| v.iter().map(|s| s.as_ref()).collect::<Vec<_>>()),
        )
    )]
    async fn execute<'de>(
        params: Self::Params<'de>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        let started = Instant::now();

        let crate_root = ctx
            .state
            .system
            .read()
            .await
            .focused_crate_root()
            .ok_or_else(|| {
                tool_ui_error("No crate is currently focused; load a workspace first.")
            })?;
        let crate_root = tokio::fs::canonicalize(crate_root)
            .await
            .map_err(|err| tool_io_error(format!("Failed to resolve crate root: {err}")))?;
        let current_workspace = workspace_root();
        if !crate_root.starts_with(&current_workspace) {
            return Err(tool_ui_error(format!(
                "Focused crate path is outside the current workspace; database may be from another clone. focused={}, workspace={}",
                crate_root.display(),
                current_workspace.display()
            )));
        }

        let focused_manifest = crate_root.join("Cargo.toml");
        if tokio::fs::metadata(&focused_manifest).await.is_err() {
            return Err(tool_ui_error(format!(
                "Missing Cargo.toml at focused path: {}",
                focused_manifest.display()
            )));
        }

        let metadata = load_metadata(&focused_manifest).await?;
        let workspace_root = tokio::fs::canonicalize(metadata.workspace_root.as_std_path())
            .await
            .map_err(|err| tool_io_error(format!("Failed to resolve workspace root: {err}")))?;
        let workspace_manifest = workspace_root.join("Cargo.toml");
        if tokio::fs::metadata(&workspace_manifest).await.is_err() {
            return Err(tool_ui_error(format!(
                "Missing Cargo.toml at workspace root: {}",
                workspace_manifest.display()
            )));
        }

        if let Some(package) = params.package.as_deref() {
            let found = metadata.packages.iter().any(|pkg| pkg.name == package);
            if !found {
                return Err(tool_ui_error(format!(
                    "Package '{package}' not found in workspace."
                )));
            }
        }

        let manifest_path = match params.scope {
            CargoScope::Focused => focused_manifest,
            CargoScope::Workspace => workspace_manifest,
        };
        let manifest_path = tokio::fs::canonicalize(manifest_path)
            .await
            .map_err(|err| tool_io_error(format!("Failed to resolve manifest path: {err}")))?;
        if !manifest_path.starts_with(&workspace_root) {
            return Err(tool_ui_error(format!(
                "Manifest path must be within workspace root: {}",
                workspace_root.display()
            )));
        }

        let mut cmd = Command::new("cargo");
        cmd.arg(match params.command {
            CargoCommand::Test => "test",
            CargoCommand::Check => "check",
        });
        cmd.arg("--message-format=json");
        cmd.arg("--manifest-path");
        cmd.arg(&manifest_path);

        if let Some(package) = params.package.as_ref() {
            cmd.arg("--package").arg(package.as_ref());
        }
        if params.release {
            cmd.arg("--release");
        }
        if let Some(profile) = params.profile.as_ref() {
            cmd.arg("--profile").arg(profile.as_ref());
        }
        if let Some(target) = params.target.as_ref() {
            cmd.arg("--target").arg(target.as_ref());
        }
        if params.all_features {
            cmd.arg("--all-features");
        }
        if params.no_default_features {
            cmd.arg("--no-default-features");
        }
        if let Some(features) = params.features.as_ref() {
            let joined = features
                .iter()
                .map(|f| f.as_ref())
                .collect::<Vec<_>>()
                .join(",");
            if !joined.is_empty() {
                cmd.arg("--features").arg(joined);
            }
        }
        if params.lib {
            cmd.arg("--lib");
        }
        if params.tests {
            cmd.arg("--tests");
        }
        if params.bins {
            cmd.arg("--bins");
        }
        if params.examples {
            cmd.arg("--examples");
        }
        if params.benches {
            cmd.arg("--benches");
        }
        if let Some(test_args) = params.test_args.as_ref() {
            if !test_args.is_empty() {
                cmd.arg("--");
                cmd.args(test_args.iter().map(|s| s.as_ref()));
            }
        }

        if let Some(parent) = manifest_path.parent() {
            cmd.current_dir(parent);
        }
        tracing::info!(target: TOOL_CALL_TARGET,
            manifest_path = %manifest_path.display(),
            "cargo_command_start"
        );

        let mut child = cmd
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| tool_io_error(format!("Failed to spawn cargo: {err}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| tool_io_error("Failed to capture cargo stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| tool_io_error("Failed to capture cargo stderr".to_string()))?;

        let stdout_task = tokio::spawn(read_stdout(stdout));
        let stderr_task = tokio::spawn(read_stderr(stderr));

        let (tool_verbosity, tooling) = {
            let cfg = ctx.state.config.read().await;
            (cfg.tool_verbosity, cfg.tooling.clone())
        };
        let timeout_secs = match params.command {
            CargoCommand::Check => tooling.cargo_check_timeout_secs,
            CargoCommand::Test => tooling.cargo_test_timeout_secs,
        };
        let mut timed_out = false;
        let status = if timeout_secs == 0 {
            child
                .wait()
                .await
                .map_err(|err| tool_io_error(format!("cargo wait failed: {err}")))?
        } else {
            let timeout_dur = Duration::from_secs(timeout_secs);
            match tokio::time::timeout(timeout_dur, child.wait()).await {
                Ok(res) => res.map_err(|err| tool_io_error(format!("cargo wait failed: {err}")))?,
                Err(_) => {
                    timed_out = true;
                    let _ = child.kill().await;
                    tokio::time::timeout(Duration::from_secs(KILL_GRACE_SECS), child.wait())
                        .await
                        .map_err(|_| tool_io_error("cargo did not exit after kill".to_string()))?
                        .map_err(|err| {
                            tool_io_error(format!("cargo wait failed after kill: {err}"))
                        })?
                }
            }
        };

        let stdout_state = stdout_task
            .await
            .map_err(|err| tool_io_error(format!("stdout task join failed: {err}")))??;
        let stderr_state = stderr_task
            .await
            .map_err(|err| tool_io_error(format!("stderr task join failed: {err}")))??;

        let mut raw_messages_truncated =
            stdout_state.raw_messages_truncated || stderr_state.raw_messages_truncated;
        let exit_code = status.code();
        let status_reason = determine_status_reason(
            params.command,
            exit_code,
            timed_out,
            status.code().is_none(),
            stdout_state.summary.errors > 0,
        );

        let mut result = CargoToolResult {
            ok: status_reason == CargoStatusReason::Success,
            status_reason,
            command: params.command,
            scope: params.scope,
            manifest_path: manifest_path.display().to_string(),
            exit_code,
            duration_ms: started.elapsed().as_millis() as u64,
            summary: stdout_state.summary,
            diagnostics: stdout_state.diagnostics,
            stderr_tail: stderr_state.stderr_tail.into_iter().collect(),
            non_json_stdout_tail: stdout_state.non_json_stdout_tail.into_iter().collect(),
            json_parse_errors_tail: stdout_state.json_parse_errors_tail.into_iter().collect(),
            raw_messages_truncated,
        };
        tracing::info!(target: TOOL_CALL_TARGET,
            manifest_path = %result.manifest_path,
            exit_code = ?result.exit_code,
            status_reason = %result.status_reason.as_str(),
            errors = result.summary.errors,
            warnings = result.summary.warnings,
            notes = result.summary.notes,
            artifacts = result.summary.artifacts,
            duration_ms = result.duration_ms,
            raw_messages_truncated = result.raw_messages_truncated,
            "cargo_command_finished"
        );

        if enforce_response_cap(&mut result, MAX_TOOL_RESPONSE_BYTES) {
            raw_messages_truncated = true;
        }
        result.raw_messages_truncated |= raw_messages_truncated;

        let summary = format!(
            "cargo {} {} (errors: {}, warnings: {}, notes: {})",
            params.command.as_str(),
            if result.ok { "succeeded" } else { "failed" },
            result.summary.errors,
            result.summary.warnings,
            result.summary.notes
        );
        let ui_payload = build_ui_payload(&result, ctx.call_id.clone(), summary, tool_verbosity);
        let serialized = serde_json::to_string(&result).expect("serialize cargo tool result");

        Ok(ToolResult {
            content: serialized,
            ui_payload: Some(ui_payload),
        })
    }
}

fn validate_params(params: &CargoToolParams<'_>) -> Result<(), ToolInvocationError> {
    if matches!(params.scope, CargoScope::Focused) && params.package.is_some() {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::Cargo,
                ToolErrorCode::InvalidFormat,
                "package is only allowed when scope=workspace",
            )
            .field("package")
            .expected("omit package when scope=focused")
            .received("package provided"),
        ));
    }
    if params.all_features && params.features.as_ref().map_or(false, |v| !v.is_empty()) {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::Cargo,
                ToolErrorCode::InvalidFormat,
                "all_features cannot be combined with features",
            )
            .field("features")
            .expected("features omitted when all_features=true")
            .received("features provided"),
        ));
    }
    if matches!(params.command, CargoCommand::Check)
        && params.test_args.as_ref().map_or(false, |v| !v.is_empty())
    {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::Cargo,
                ToolErrorCode::InvalidFormat,
                "test_args are only allowed for command=test",
            )
            .field("test_args")
            .expected("omit test_args when command=check")
            .received("test_args provided"),
        ));
    }

    let mut target_flags = 0;
    if params.lib {
        target_flags += 1;
    }
    if params.tests {
        target_flags += 1;
    }
    if params.bins {
        target_flags += 1;
    }
    if params.examples {
        target_flags += 1;
    }
    if params.benches {
        target_flags += 1;
    }
    if target_flags > 1 {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::Cargo,
                ToolErrorCode::InvalidFormat,
                "only one of lib/tests/bins/examples/benches may be set",
            )
            .field("lib")
            .expected("at most one target selector true")
            .received(format!("{target_flags} selectors enabled")),
        ));
    }

    validate_name("package", params.package.as_deref())?;
    validate_name("target", params.target.as_deref())?;
    validate_name("profile", params.profile.as_deref())?;
    if let Some(features) = params.features.as_ref() {
        for feature in features {
            validate_name("features", Some(feature.as_ref()))?;
        }
    }

    Ok(())
}

fn validate_name(field: &'static str, value: Option<&str>) -> Result<(), ToolInvocationError> {
    let Some(value) = value else {
        return Ok(());
    };
    if value.is_empty()
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ToolInvocationError::Validation(
            ToolError::new(
                ToolName::Cargo,
                ToolErrorCode::InvalidFormat,
                format!("{field} must be ASCII alphanumeric plus -/_"),
            )
            .field(field)
            .expected("[A-Za-z0-9_-]+")
            .received(value.to_string()),
        ));
    }
    Ok(())
}

async fn load_metadata(
    manifest_path: &Path,
) -> Result<cargo_metadata::Metadata, ploke_error::Error> {
    let manifest_path = manifest_path.to_path_buf();
    let metadata = tokio::task::spawn_blocking(move || {
        cargo_metadata::MetadataCommand::new()
            .no_deps()
            .manifest_path(&manifest_path)
            .exec()
    })
    .await
    .map_err(|err| {
        ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
            "cargo metadata task failed: {err}"
        )))
    })?
    .map_err(|err| tool_ui_error(format!("cargo metadata failed: {err}")))?;
    Ok(metadata)
}

async fn read_stdout(
    stdout: tokio::process::ChildStdout,
) -> Result<StdoutState, ploke_error::Error> {
    let mut state = StdoutState::default();
    let mut lines = BufReader::new(stdout).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| tool_io_error(format!("stdout read failed: {err}")))?
    {
        parse_stdout_line(&line, &mut state);
    }
    Ok(state)
}

fn parse_stdout_line(line: &str, state: &mut StdoutState) {
    if line.starts_with('{') {
        match serde_json::from_str::<Message>(line) {
            Ok(msg) => handle_message(msg, state),
            Err(err) => {
                if push_tail(
                    &mut state.json_parse_errors_tail,
                    format!("{err}: {line}"),
                    MAX_JSON_PARSE_ERRORS,
                ) {
                    state.raw_messages_truncated = true;
                }
            }
        }
    } else if push_tail(
        &mut state.non_json_stdout_tail,
        line.to_string(),
        MAX_TAIL_LINES,
    ) {
        state.raw_messages_truncated = true;
    }
}

fn handle_message(msg: Message, state: &mut StdoutState) {
    match msg {
        Message::CompilerMessage(msg) => {
            let diag = msg.message;
            match &diag.level {
                cargo_metadata::diagnostic::DiagnosticLevel::Error => {
                    state.summary.errors += 1;
                }
                cargo_metadata::diagnostic::DiagnosticLevel::Warning => {
                    state.summary.warnings += 1;
                }
                cargo_metadata::diagnostic::DiagnosticLevel::Note
                | cargo_metadata::diagnostic::DiagnosticLevel::Help
                | cargo_metadata::diagnostic::DiagnosticLevel::FailureNote => {
                    state.summary.notes += 1;
                }
                _ => {}
            }
            if state.diagnostics.len() < MAX_DIAGNOSTICS {
                state.diagnostics.push(convert_diagnostic(diag));
            } else {
                state.raw_messages_truncated = true;
            }
        }
        Message::CompilerArtifact(_) => {
            state.summary.artifacts += 1;
        }
        _ => {
            state.summary.other_messages += 1;
        }
    }
}

fn convert_diagnostic(diag: cargo_metadata::diagnostic::Diagnostic) -> CargoDiagnostic {
    let level = match diag.level {
        cargo_metadata::diagnostic::DiagnosticLevel::Error => "error",
        cargo_metadata::diagnostic::DiagnosticLevel::Warning => "warning",
        cargo_metadata::diagnostic::DiagnosticLevel::Note => "note",
        cargo_metadata::diagnostic::DiagnosticLevel::Help => "help",
        cargo_metadata::diagnostic::DiagnosticLevel::FailureNote => "failure_note",
        cargo_metadata::diagnostic::DiagnosticLevel::Ice => "ice",
        _ => "other",
    }
    .to_string();
    let to_u32 = |value: usize| u32::try_from(value).unwrap_or(u32::MAX);
    let mut spans: Vec<CargoSpan> = diag
        .spans
        .iter()
        .filter(|span| span.is_primary)
        .take(MAX_SPANS_PER_DIAGNOSTIC)
        .map(|span| CargoSpan {
            file_name: span.file_name.clone(),
            line_start: to_u32(span.line_start),
            line_end: to_u32(span.line_end),
            column_start: to_u32(span.column_start),
            column_end: to_u32(span.column_end),
            is_primary: span.is_primary,
        })
        .collect();
    if spans.is_empty() {
        spans = diag
            .spans
            .iter()
            .take(MAX_SPANS_PER_DIAGNOSTIC)
            .map(|span| CargoSpan {
                file_name: span.file_name.clone(),
                line_start: to_u32(span.line_start),
                line_end: to_u32(span.line_end),
                column_start: to_u32(span.column_start),
                column_end: to_u32(span.column_end),
                is_primary: span.is_primary,
            })
            .collect();
    }

    CargoDiagnostic {
        level,
        message: diag.message,
        code: diag.code.map(|code| code.code),
        spans,
        rendered: diag.rendered,
    }
}

async fn read_stderr(
    stderr: tokio::process::ChildStderr,
) -> Result<StderrState, ploke_error::Error> {
    let mut state = StderrState::default();
    let mut lines = BufReader::new(stderr).lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|err| tool_io_error(format!("stderr read failed: {err}")))?
    {
        if push_tail(&mut state.stderr_tail, line, MAX_TAIL_LINES) {
            state.raw_messages_truncated = true;
        }
    }
    Ok(state)
}

fn push_tail(buf: &mut VecDeque<String>, line: String, limit: usize) -> bool {
    buf.push_back(line);
    if buf.len() > limit {
        buf.pop_front();
        true
    } else {
        false
    }
}

fn determine_status_reason(
    command: CargoCommand,
    exit_code: Option<i32>,
    timed_out: bool,
    killed: bool,
    had_compile_errors: bool,
) -> CargoStatusReason {
    if timed_out {
        return CargoStatusReason::Timeout;
    }
    if killed {
        return CargoStatusReason::Killed;
    }
    if exit_code == Some(0) {
        return CargoStatusReason::Success;
    }
    if had_compile_errors {
        return CargoStatusReason::CompileFailed;
    }
    if matches!(command, CargoCommand::Test) {
        return CargoStatusReason::TestsFailedOrRuntime;
    }
    CargoStatusReason::CargoFailedOrInvalidArgs
}

fn build_ui_payload(
    result: &CargoToolResult,
    call_id: ploke_core::ArcStr,
    summary: String,
    verbosity: ToolVerbosity,
) -> ToolUiPayload {
    let mut payload = ToolUiPayload::new(ToolName::Cargo, call_id, summary)
        .with_field("command", result.command.as_str())
        .with_field("scope", result.scope.as_str())
        .with_field("status", result.status_reason.as_str())
        .with_field("exit_code", display_exit_code(result.exit_code))
        .with_field("errors", result.summary.errors.to_string())
        .with_field("warnings", result.summary.warnings.to_string())
        .with_field("notes", result.summary.notes.to_string())
        .with_field("artifacts", result.summary.artifacts.to_string())
        .with_field("duration_ms", result.duration_ms.to_string())
        .with_field("manifest", result.manifest_path.as_str())
        .with_verbosity(verbosity);

    let details = format_details(result);
    if !details.is_empty() {
        payload = payload.with_details(details);
    }
    payload
}

fn format_details(result: &CargoToolResult) -> String {
    let mut out = String::new();
    if !result.diagnostics.is_empty() {
        out.push_str("Diagnostics:\n");
        for diag in result.diagnostics.iter().take(10) {
            out.push_str(&format!("{}: {}", diag.level, diag.message));
            if let Some(span) = diag.spans.first() {
                out.push_str(&format!(
                    " ({}:{}:{})",
                    span.file_name, span.line_start, span.column_start
                ));
            }
            out.push('\n');
        }
    }
    if !result.stderr_tail.is_empty() {
        out.push_str("Stderr tail:\n");
        for line in result.stderr_tail.iter().take(20) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !result.non_json_stdout_tail.is_empty() {
        out.push_str("Stdout tail:\n");
        for line in result.non_json_stdout_tail.iter().take(20) {
            out.push_str(line);
            out.push('\n');
        }
    }
    if !result.json_parse_errors_tail.is_empty() {
        out.push_str("JSON parse errors:\n");
        for line in result.json_parse_errors_tail.iter().take(10) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.trim_end().to_string()
}

fn display_exit_code(exit_code: Option<i32>) -> String {
    exit_code
        .map(|code| code.to_string())
        .unwrap_or_else(|| "signal".to_string())
}

fn enforce_response_cap(result: &mut CargoToolResult, max_bytes: usize) -> bool {
    let mut truncated = false;
    let mut serialized = serde_json::to_vec(result).unwrap_or_default();
    if serialized.len() <= max_bytes {
        return false;
    }

    if result.diagnostics.iter().any(|d| d.rendered.is_some()) {
        for diag in &mut result.diagnostics {
            diag.rendered = None;
        }
        truncated = true;
        serialized = serde_json::to_vec(result).unwrap_or_default();
        if serialized.len() <= max_bytes {
            return truncated;
        }
    }

    if !result.stderr_tail.is_empty()
        || !result.non_json_stdout_tail.is_empty()
        || !result.json_parse_errors_tail.is_empty()
    {
        result.stderr_tail.clear();
        result.non_json_stdout_tail.clear();
        result.json_parse_errors_tail.clear();
        truncated = true;
        serialized = serde_json::to_vec(result).unwrap_or_default();
        if serialized.len() <= max_bytes {
            return truncated;
        }
    }

    if result.diagnostics.len() > 10 {
        result.diagnostics.truncate(10);
        truncated = true;
        serialized = serde_json::to_vec(result).unwrap_or_default();
        if serialized.len() <= max_bytes {
            return truncated;
        }
    }

    if !result.diagnostics.is_empty() {
        result.diagnostics.clear();
        truncated = true;
    }

    truncated
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    #[test]
    fn validate_params_rejects_package_with_focused_scope() {
        let params = CargoToolParams {
            command: CargoCommand::Check,
            scope: CargoScope::Focused,
            package: Some(Cow::Borrowed("foo")),
            features: None,
            all_features: false,
            no_default_features: false,
            target: None,
            profile: None,
            release: false,
            lib: false,
            tests: false,
            bins: false,
            examples: false,
            benches: false,
            test_args: None,
        };
        assert!(validate_params(&params).is_err());
    }

    #[test]
    fn parse_stdout_line_handles_non_json_and_invalid_json() {
        let mut state = StdoutState::default();
        parse_stdout_line("not-json", &mut state);
        parse_stdout_line("{oops", &mut state);
        assert_eq!(state.non_json_stdout_tail.len(), 1);
        assert_eq!(state.json_parse_errors_tail.len(), 1);
    }

    #[test]
    fn parse_stdout_line_handles_build_finished() {
        let mut state = StdoutState::default();
        parse_stdout_line(r#"{"reason":"build-finished","success":true}"#, &mut state);
        assert_eq!(state.summary.other_messages, 1);
    }

    #[test]
    fn status_reason_respects_compile_errors() {
        let reason = determine_status_reason(CargoCommand::Check, Some(1), false, false, true);
        assert_eq!(reason, CargoStatusReason::CompileFailed);
    }

    #[test]
    fn enforce_response_cap_truncates() {
        let mut result = CargoToolResult {
            ok: false,
            status_reason: CargoStatusReason::CargoFailedOrInvalidArgs,
            command: CargoCommand::Check,
            scope: CargoScope::Focused,
            manifest_path: "/tmp/Cargo.toml".to_string(),
            exit_code: Some(1),
            duration_ms: 10,
            summary: CargoSummary::default(),
            diagnostics: vec![CargoDiagnostic {
                level: "error".to_string(),
                message: "m".repeat(1024),
                code: None,
                spans: Vec::new(),
                rendered: Some("r".repeat(1024)),
            }],
            stderr_tail: vec!["e".repeat(1024)],
            non_json_stdout_tail: vec!["o".repeat(1024)],
            json_parse_errors_tail: vec!["j".repeat(1024)],
            raw_messages_truncated: false,
        };
        let truncated = enforce_response_cap(&mut result, 512);
        assert!(truncated);
    }
}
