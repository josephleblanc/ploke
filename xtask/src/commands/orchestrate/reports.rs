use std::fs;
use std::path::Path;

use crate::commands::{CommandContext, XtaskError};

use super::{Board, display, now};

impl Board {
    pub(super) fn write_inline_report(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
        task_id: &str,
        kind: &str,
        summary: &str,
    ) -> Result<String, XtaskError> {
        let report_dir = board_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("reports");
        fs::create_dir_all(&report_dir)?;
        let timestamp = now();
        let file_name = format!(
            "{}-{}-{}.md",
            file_fragment(task_id),
            kind,
            file_fragment(&timestamp)
        );
        let path = report_dir.join(file_name);
        let body = format!(
            "# Orchestrator {kind} Summary\n\n- task: {task_id}\n- recorded_at: {timestamp}\n\n{summary}\n"
        );
        fs::write(&path, body)?;
        display(ctx, &path)
    }
}

fn file_fragment(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}
