use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::spec::PrepareError;

use crate::cli::prototype1_state::cli_facing::HarnessRequestSlot;

#[cfg(test)]
pub(crate) fn broad_attempt_from_summary_fixture(
    slot: &HarnessRequestSlot,
) -> Option<Result<(), PrepareError>> {
    let path = std::env::var_os("PLOKE_EVAL_BROAD_TUI_SUMMARY_FIXTURE")?;
    let path = PathBuf::from(path);
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(source) => {
            return Some(Err(PrepareError::ReadManifest { path, source }));
        }
    };
    let summary = match serde_json::from_str::<super::super::evidence::Summary>(&text) {
        Ok(summary) => summary,
        Err(source) => {
            return Some(Err(PrepareError::ParseManifest { path, source }));
        }
    };
    let diagnostics_path = broad_headless_diagnostics_path(slot.published.submitted_result_path());
    if let Some(parent) = diagnostics_path.parent() {
        if let Err(source) = fs::create_dir_all(parent) {
            return Some(Err(PrepareError::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            }));
        }
    }
    if let Err(source) = fs::write(&diagnostics_path, text) {
        return Some(Err(PrepareError::WriteManifest {
            path: diagnostics_path,
            source,
        }));
    }

    Some(Err(match summary.terminal {
        Some(super::super::evidence::Terminal::ProviderUnavailable { reason }) => {
            PrepareError::ProviderUnavailable {
                phase: "broad_headless_tui_attempt",
                detail: format!("headless ploke-tui provider unavailable: {reason}"),
            }
        }
        Some(super::super::evidence::Terminal::SetupUnavailable { phase, reason }) => {
            PrepareError::DatabaseSetup {
                phase: if phase == "bm25_ready" {
                    "bm25_ready"
                } else {
                    "broad_headless_tui_attempt"
                },
                detail: reason,
            }
        }
        Some(terminal) => PrepareError::InvalidBatchSelection {
            detail: format!(
                "headless ploke-tui test fixture ended without an admissible edit: {}",
                terminal_reason(&terminal)
            ),
        },
        None => PrepareError::InvalidBatchSelection {
            detail: format!(
                "headless ploke-tui test fixture ended without terminal outcome after {} recorded attempt(s)",
                summary.attempts.len()
            ),
        },
    }))
}

#[cfg(test)]
fn broad_headless_diagnostics_path(submitted_result_path: &Path) -> PathBuf {
    submitted_result_path.with_extension("headless-tui.json")
}

#[cfg(test)]
fn terminal_reason(terminal: &super::super::evidence::Terminal) -> String {
    match terminal {
        super::super::evidence::Terminal::Applied { changed_paths, .. } => {
            format!(
                "reported applied terminal for {} path(s) but was not admitted",
                changed_paths.len()
            )
        }
        super::super::evidence::Terminal::Exhausted {
            attempts,
            last_feedback,
        } => {
            format!("exhausted {attempts} attempt(s) without an admissible edit: {last_feedback}")
        }
        super::super::evidence::Terminal::CompletedWithoutEdit { outcome, summary } => {
            format!("completed without edit: outcome={outcome}; {summary}")
        }
        super::super::evidence::Terminal::ToolFailed { error } => {
            format!("tool failed: {error}")
        }
        super::super::evidence::Terminal::NoEdit => "produced no edit".to_string(),
        super::super::evidence::Terminal::ContextUnavailable { reason } => {
            format!("prompt context unavailable: {reason}")
        }
        super::super::evidence::Terminal::ProviderUnavailable { reason } => {
            format!("provider unavailable: {reason}")
        }
        super::super::evidence::Terminal::SetupUnavailable { phase, reason } => {
            format!("setup unavailable during {phase}: {reason}")
        }
        super::super::evidence::Terminal::AppliedValidationFailed {
            changed_paths,
            feedback,
            ..
        } => format!(
            "applied validation failed for {} path(s): {feedback}",
            changed_paths.len()
        ),
        super::super::evidence::Terminal::AppliedValidationMissing {
            changed_paths,
            missing,
            ..
        } => format!(
            "applied validation missing for {} path(s): {}",
            changed_paths.len(),
            missing.join(", ")
        ),
        super::super::evidence::Terminal::AppliedTurnAborted {
            changed_paths,
            outcome,
            summary,
            ..
        } => format!(
            "applied turn aborted for {} path(s): outcome={outcome}; {summary}",
            changed_paths.len()
        ),
        super::super::evidence::Terminal::AppliedTimedOut {
            changed_paths,
            secs,
            ..
        } => format!(
            "applied timed out after {secs}s for {} path(s)",
            changed_paths.len()
        ),
        super::super::evidence::Terminal::TimedOut { secs } => {
            format!("timed out after {secs}s")
        }
    }
}
