//! Provider authentication preflight commands.

use std::env;

use ploke_llm::{ModelId, Router, router_only::google::Google};
use serde::Serialize;

use super::{CommandContext, XtaskError};
use crate::executor::Command;

const DEFAULT_GOOGLE_CHAT_MODEL: &str = "google/gemini-2.5-flash";

/// Provider authentication checks.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Auth {
    /// Verify direct Google/Vertex live-test prerequisites without printing secrets.
    #[command(name = "google")]
    Google(GoogleAuth),
}

impl Auth {
    /// Execute an authentication preflight command.
    pub fn execute(&self, ctx: &CommandContext) -> Result<GoogleAuthPreflight, XtaskError> {
        match self {
            Self::Google(cmd) => cmd.execute(ctx),
        }
    }
}

/// Arguments for the direct Google authentication preflight.
#[derive(Debug, Clone, clap::Args)]
pub struct GoogleAuth {
    /// Model id to validate for live Google chat tests.
    #[arg(long, value_name = "MODEL_ID")]
    pub model: Option<String>,

    /// Also require PLOKE_RUN_LIVE_TESTS=1/true/yes.
    #[arg(long)]
    pub strict_live: bool,

    /// Return a report with passed=false instead of exiting with a validation error.
    #[arg(long)]
    pub report_only: bool,
}

impl Command for GoogleAuth {
    type Output = GoogleAuthPreflight;
    type Error = XtaskError;

    fn execute(&self, _ctx: &CommandContext) -> Result<Self::Output, Self::Error> {
        let route = check_google_route();
        let auth = check_google_auth()?;
        let model = check_google_model(self.model.as_deref());
        let live_gate = live_gate_enabled();

        let mut warnings = Vec::new();
        if env::var_os("GOOGLE_API_KEY").is_some() {
            warnings.push(
                "GOOGLE_API_KEY is set, but direct Google live tests use ADC bearer-token resolution; this env var is not required for the supported Google route."
                    .to_string(),
            );
        }
        if !live_gate {
            warnings.push(
                "PLOKE_RUN_LIVE_TESTS is not enabled; set it when running strict live tests."
                    .to_string(),
            );
        }

        let strict_live_gate_ready = !self.strict_live || live_gate;
        let passed = route.available
            && auth.adc_config_available
            && auth.bearer_token_available
            && model.parses
            && strict_live_gate_ready;

        let report = GoogleAuthPreflight {
            kind: "google_auth_preflight",
            passed,
            strict_live: self.strict_live,
            route,
            auth,
            model,
            live_gate,
            warnings,
            next_commands: google_next_commands(),
        };

        if !report.passed && !self.report_only {
            return Err(XtaskError::validation(report.failure_message())
                .with_recovery(report.recovery_suggestion()));
        }

        Ok(report)
    }
}

/// Direct Google live-test preflight report.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleAuthPreflight {
    /// Stable output discriminator for renderers and scripts.
    pub kind: &'static str,
    /// Whether the requested preflight checks passed.
    pub passed: bool,
    /// Whether PLOKE_RUN_LIVE_TESTS was required for this invocation.
    pub strict_live: bool,
    /// Route environment status.
    pub route: GoogleRouteStatus,
    /// ADC and bearer-token status.
    pub auth: GoogleAdcStatus,
    /// Selected model status.
    pub model: GoogleModelStatus,
    /// Whether PLOKE_RUN_LIVE_TESTS is enabled.
    pub live_gate: bool,
    /// Non-fatal setup notes.
    pub warnings: Vec<String>,
    /// Focused canary commands that exercise the configured route.
    pub next_commands: Vec<String>,
}

impl GoogleAuthPreflight {
    fn failure_message(&self) -> String {
        let mut missing = Vec::new();
        if !self.route.available {
            missing.push("GOOGLE_PROJECT_ID/GOOGLE_REGION route config");
        }
        if !self.auth.adc_config_available {
            missing.push("Google ADC config");
        }
        if !self.auth.bearer_token_available {
            missing.push("Google ADC bearer-token resolution");
        }
        if !self.model.parses {
            missing.push("valid Google model id");
        }
        if self.strict_live && !self.live_gate {
            missing.push("PLOKE_RUN_LIVE_TESTS=1");
        }

        if missing.is_empty() {
            "Google auth preflight failed".to_string()
        } else {
            format!(
                "Google auth preflight failed: missing {}",
                missing.join(", ")
            )
        }
    }

    fn recovery_suggestion(&self) -> String {
        let project = self
            .route
            .project_id
            .as_deref()
            .unwrap_or("<your-google-project-id>");
        let region = self.route.region.as_deref().unwrap_or("global");
        let model = self
            .model
            .normalized
            .as_deref()
            .unwrap_or(DEFAULT_GOOGLE_CHAT_MODEL);

        format!(
            "Set GOOGLE_PROJECT_ID={project}, GOOGLE_REGION={region}, ensure ADC can mint a cloud-platform token, set PLOKE_LIVE_GOOGLE_CHAT_MODEL={model}, and run `cargo xtask auth google --strict-live` from the same shell before live tests."
        )
    }
}

/// Route env status for direct Google.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleRouteStatus {
    /// Whether both GOOGLE_PROJECT_ID and GOOGLE_REGION are present and accepted by the router.
    pub available: bool,
    /// Current project id, if set.
    pub project_id: Option<String>,
    /// Current Vertex location, if set.
    pub region: Option<String>,
    /// Resolved OpenAI-compatible chat completions URL when route config is available.
    pub completion_url: Option<String>,
    /// Sanitized error message, if route config failed.
    pub error: Option<String>,
}

/// ADC status for direct Google.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleAdcStatus {
    /// Whether GOOGLE_APPLICATION_CREDENTIALS is present. ADC may still work without it.
    pub google_application_credentials_set: bool,
    /// Whether the Google auth builder found usable ADC config.
    pub adc_config_available: bool,
    /// Whether the router resolved a non-empty bearer token.
    pub bearer_token_available: bool,
    /// Sanitized ADC config error, if present.
    pub adc_config_error: Option<String>,
    /// Sanitized bearer-token error, if present.
    pub bearer_token_error: Option<String>,
}

/// Selected Google model status.
#[derive(Debug, Clone, Serialize)]
pub struct GoogleModelStatus {
    /// Raw selected model value.
    pub raw: String,
    /// Selected model source: cli, env, or default.
    pub source: &'static str,
    /// Normalized model id with a provider prefix.
    pub normalized: Option<String>,
    /// Whether the model id parsed as a ploke model id.
    pub parses: bool,
    /// Sanitized parse error, if present.
    pub parse_error: Option<String>,
}

fn check_google_route() -> GoogleRouteStatus {
    let project_id = non_empty_env("GOOGLE_PROJECT_ID");
    let region = non_empty_env("GOOGLE_REGION");
    match Google::route_config_available() {
        Ok(()) => GoogleRouteStatus {
            available: true,
            project_id,
            region,
            completion_url: Google::completion_url().ok().map(ToString::to_string),
            error: None,
        },
        Err(error) => GoogleRouteStatus {
            available: false,
            project_id,
            region,
            completion_url: None,
            error: Some(sanitize_error(error)),
        },
    }
}

fn check_google_auth() -> Result<GoogleAdcStatus, XtaskError> {
    let google_application_credentials_set =
        env::var_os("GOOGLE_APPLICATION_CREDENTIALS").is_some();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| XtaskError::Internal(format!("failed to create auth runtime: {error}")))?;
    let (adc_config, bearer) = runtime.block_on(async {
        let adc_config = Google::auth_config_available();
        let bearer = Google::resolve_bearer_token().await;
        (adc_config, bearer)
    });
    let adc_config_available = adc_config.is_ok();
    let adc_config_error = adc_config.err().map(sanitize_error);
    let (bearer_token_available, bearer_token_error) = match bearer {
        Ok(token) if !token.trim().is_empty() => (true, None),
        Ok(_) => (
            false,
            Some("Google ADC bearer-token resolver returned an empty token".to_string()),
        ),
        Err(error) => (false, Some(sanitize_error(error))),
    };

    Ok(GoogleAdcStatus {
        google_application_credentials_set,
        adc_config_available,
        bearer_token_available,
        adc_config_error,
        bearer_token_error,
    })
}

fn check_google_model(cli_model: Option<&str>) -> GoogleModelStatus {
    let (raw, source) = match cli_model {
        Some(model) => (model.to_string(), "cli"),
        None => match non_empty_env("PLOKE_LIVE_GOOGLE_CHAT_MODEL") {
            Some(model) => (model, "env:PLOKE_LIVE_GOOGLE_CHAT_MODEL"),
            None => match non_empty_env("PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID") {
                Some(model) => (model, "env:PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID"),
                None => match non_empty_env("PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID") {
                    Some(model) => (model, "env:PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID"),
                    None => (DEFAULT_GOOGLE_CHAT_MODEL.to_string(), "default"),
                },
            },
        },
    };

    let normalized = if raw.contains('/') {
        raw.clone()
    } else {
        format!("google/{raw}")
    };
    match normalized.parse::<ModelId>() {
        Ok(_) => GoogleModelStatus {
            raw,
            source,
            normalized: Some(normalized),
            parses: true,
            parse_error: None,
        },
        Err(error) => GoogleModelStatus {
            raw,
            source,
            normalized: Some(normalized),
            parses: false,
            parse_error: Some(sanitize_error(error)),
        },
    }
}

fn non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn live_gate_enabled() -> bool {
    non_empty_env("PLOKE_RUN_LIVE_TESTS")
        .is_some_and(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
}

fn google_next_commands() -> Vec<String> {
    vec![
        "cargo test -p ploke-llm --features live_api_tests live_google_chat_step_forced_tool_call_success_or_quota -- --ignored --nocapture".to_string(),
        "cargo test -p ploke-tui --features live_api_tests live_google_chat_session_executes_list_dir_tool_call_success_or_quota -- --ignored --nocapture".to_string(),
        "cargo test -p ploke-tui --features live_api_tests live_google_harness_router_command_runs_list_dir_through_llm_manager -- --ignored --nocapture".to_string(),
    ]
}

fn sanitize_error(error: impl std::fmt::Display) -> String {
    let mut text = error.to_string();
    for marker in ["ya29.", "sk-", "sess-"] {
        while let Some(start) = text.find(marker) {
            let tail = &text[start..];
            let end_rel = tail
                .find(|ch: char| ch.is_whitespace() || ch == '"' || ch == '\'' || ch == ',')
                .unwrap_or(tail.len());
            text.replace_range(start..start + end_rel, "[redacted]");
        }
    }
    text
}
