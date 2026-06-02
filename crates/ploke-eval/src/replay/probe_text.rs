//! Text renderer for live replay probes.
//!
//! `ProbeRun` owns the replay facts and `cli.rs` owns command dispatch. This
//! module is deliberately just the table renderer between them, so the command
//! surface does not infer replay semantics or walk tool payload JSON directly.

use std::collections::BTreeMap;

use ploke_records::tool_contracts::{
    PersistedToolResultContent, ToolResultContent, decode_tool_result_content,
};

use crate::cli::prototype1_state::edit_surface::tui_adapter::{Event, Tool};

use super::probe::{ProbeRun, WorkspaceGit};

pub(crate) fn render_table(probe: &ProbeRun) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push("replay probe".to_owned());
    lines.push("-".repeat(40));
    lines.push(format!("run_dir: {}", probe.run_dir.display()));
    lines.push(format!("workspace: {}", probe.workspace.display()));
    lines.push(format!("artifact: {}", probe.anchor.cursor.artifact_path));
    lines.push(format!("event_index: {}", probe.anchor.cursor.event_index));
    lines.push(format!("event_kind: {:?}", probe.anchor.event_kind));
    lines.push(format!("task_id: {}", probe.anchor.task_id));
    lines.push(format!(
        "selected_model_recorded: {}",
        probe.anchor.selected_model
    ));
    lines.push(format!(
        "live_model: {}",
        probe.live_model.as_deref().unwrap_or("(tui default)")
    ));
    lines.push(format!(
        "live_provider: {}",
        probe.live_provider.as_deref().unwrap_or("(model default)")
    ));
    push_workspace_git(&mut lines, "workspace_before", &probe.workspace_before);
    lines.push(format!("tail: {:?}", probe.prefix.tail));
    lines.push(format!(
        "prefix: {}",
        format_prefix_selector(&probe.prefix.selector)
    ));
    lines.push(format!(
        "through_response_index: {}",
        probe
            .prefix
            .through_response_index
            .map(|index| index.to_string())
            .unwrap_or_else(|| "-".to_owned())
    ));
    lines.push(format!(
        "assistant_message_id: {}",
        probe.anchor.assistant_message_id()
    ));
    lines.push(format!("tape: {}", probe.tape_path.display()));
    lines.push(format!(
        "tape_records: {}/{}",
        probe.prefix.installed_records, probe.prefix.total_records
    ));
    lines.push(format!("loaded_tape_records: {}", probe.tape_records));
    lines.push(format!("branch_in_records: {}", probe.branch_in_records));
    lines.push(format!("branch_records: {}", probe.branch_records.len()));
    if let Some(path) = probe.branch_out.as_ref() {
        lines.push(format!("branch_out: {}", path.display()));
    }
    lines.push(format!(
        "captured_requests: {}",
        probe.captured_requests.len()
    ));
    lines.push(format!(
        "captured_responses: {}",
        probe.captured_responses.len()
    ));
    lines.push(format!("live_tail_reached: {}", probe.tail_reached()));
    lines.push(format!(
        "live_step_responses: {}",
        probe.live_step_response_count()
    ));
    push_live_step(&mut lines, probe);
    lines.push(format!(
        "live_step_boundary_reached: {}",
        probe.live_step_boundary_reached()
    ));
    lines.push(format!("attempts: {}", probe.attempt_count()));
    lines.push(format!("terminal: {}", probe.terminal_label()));
    lines.push(format!("elapsed_ms: {}", probe.elapsed_ms));
    if let Some(last) = probe.captured_requests.last() {
        lines.push(format!("last_request_messages: {}", last.message_count));
    }
    push_workspace_git(&mut lines, "workspace_after", &probe.workspace_after);
    push_events(&mut lines, probe);
    lines
}

fn push_workspace_git(lines: &mut Vec<String>, label: &str, git: &WorkspaceGit) {
    lines.push(format!("{label}_dirty: {}", git.is_dirty()));
    lines.push(format!("{label}_dirty_paths: {}", git.dirty_count()));
    for path in git.dirty_paths.iter().take(6) {
        lines.push(format!("  {}", path.display()));
    }
    if git.dirty_paths.len() > 6 {
        lines.push(format!("  ... {} more path(s)", git.dirty_paths.len() - 6));
    }
    if let Some(error) = git.error.as_ref() {
        lines.push(format!("{label}_git_error: {error}"));
    }
}

fn push_live_step(lines: &mut Vec<String>, probe: &ProbeRun) {
    let calls = probe.live_step_tool_calls();
    lines.push(format!("new_live_tool_calls: {}", calls.len()));
    for call in calls.iter().take(8) {
        lines.push(format!(
            "  response={} call_id={} tool={}",
            call.response_index, call.call_id, call.tool
        ));
    }
    if calls.len() > 8 {
        lines.push(format!("  ... {} more call(s)", calls.len() - 8));
    }

    let events = probe.live_step_tool_events();
    if events.is_empty() {
        return;
    }
    let tool_by_call = tool_map(probe.events.iter());
    lines.push("new_live_tool_events:".to_owned());
    for event in events.iter().take(12) {
        push_event(lines, "  ", event, &tool_by_call);
    }
    if events.len() > 12 {
        lines.push(format!("  ... {} more event(s)", events.len() - 12));
    }
}

fn push_events(lines: &mut Vec<String>, probe: &ProbeRun) {
    let event_count = probe.events.len();
    if event_count == 0 {
        return;
    }
    let tool_by_call = tool_map(probe.events.iter());
    let skip = event_count.saturating_sub(12);
    lines.push("events:".to_owned());
    if skip > 0 {
        lines.push(format!("  ... {skip} earlier events omitted"));
    }
    for (index, event) in probe.events.iter().enumerate().skip(skip) {
        push_event(lines, &format!("  {index}: "), event, &tool_by_call);
    }
}

fn tool_map<'a>(events: impl Iterator<Item = &'a Event>) -> BTreeMap<String, String> {
    events
        .filter_map(|event| match event {
            Event::ToolRequest { call_id, tool, .. } => Some((call_id.clone(), tool.clone())),
            _ => None,
        })
        .collect()
}

fn push_event(
    lines: &mut Vec<String>,
    prefix: &str,
    event: &Event,
    tool_by_call: &BTreeMap<String, String>,
) {
    let rendered = event_lines(event, tool_by_call);
    let continuation = " ".repeat(prefix.chars().count());
    for (index, line) in rendered.iter().enumerate() {
        if index == 0 {
            lines.push(format!("{prefix}{line}"));
        } else {
            lines.push(format!("{continuation}{line}"));
        }
    }
}

fn event_lines(event: &Event, tool_by_call: &BTreeMap<String, String>) -> Vec<String> {
    match event {
        Event::Proposal {
            id,
            edit_count,
            paths,
        } => vec![format!(
            "proposal id={id} edits={edit_count} paths={}",
            paths.len()
        )],
        Event::ToolRequest {
            call_id,
            tool,
            arguments,
            ..
        } => vec![format!(
            "tool_request call_id={call_id} tool={tool} args={}",
            truncate_middle(arguments, 360)
        )],
        Event::Tool { call_id, result } => match result {
            Tool::Completed { content } => {
                let tool = tool_by_call
                    .get(call_id)
                    .map(String::as_str)
                    .unwrap_or("(unknown)");
                let mut lines = vec![format!("tool_completed call_id={call_id} tool={tool}")];
                lines.extend(tool_result_lines(tool, content));
                lines
            }
            Tool::Failed { error } => vec![format!(
                "tool_failed call_id={call_id} error={}",
                truncate_middle(error, 500)
            )],
        },
        Event::AssistantMessage {
            id,
            status,
            content,
        } => vec![format!(
            "assistant_message id={id} status={status} content={}",
            truncate_middle(content, 360)
        )],
        Event::Turn {
            outcome,
            attempts,
            summary,
            ..
        } => vec![format!(
            "turn outcome={outcome} attempts={attempts} summary={}",
            truncate_middle(summary, 360)
        )],
        Event::Outcome(outcome) => vec![format!("outcome {outcome:?}")],
    }
}

fn tool_result_lines(tool: &str, content: &str) -> Vec<String> {
    match decode_tool_result_content(tool, content) {
        PersistedToolResultContent::Decoded(ToolResultContent::RequestCodeContext(result)) => {
            let mut lines = vec![format!(
                "result: ok={} search_term={:?} top_k={} returned={} kind={:?}",
                result.ok,
                result.search_term,
                result.top_k,
                result.context.len(),
                result.kind
            )];
            if let Some(note) = result.note.as_deref() {
                lines.push(format!("note: {}", truncate_middle(note, 240)));
            }
            for step in &result.next_steps {
                lines.push(format!("next_step: {}", truncate_middle(step, 240)));
            }
            for (index, item) in result.context.iter().enumerate() {
                lines.push(format!(
                    "  {}. {} :: {}",
                    index + 1,
                    truncate_middle(item.file_path.as_ref(), 120),
                    truncate_middle(item.canon_path.as_ref(), 120)
                ));
                lines.push(format!("     {}", excerpt(&item.snippet, 220)));
            }
            lines
        }
        PersistedToolResultContent::Decoded(ToolResultContent::NsRead(result)) => {
            let mut lines = vec![format!(
                "result: ok={} file={} exists={} bytes={} lines={}..{} truncated={}",
                result.ok,
                truncate_middle(&result.file_path, 160),
                result.exists,
                result
                    .byte_len
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                result
                    .start_line
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                result
                    .end_line
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned()),
                result.truncated
            )];
            if let Some(content) = result.content.as_deref() {
                lines.push(format!("excerpt: {}", excerpt(content, 360)));
            }
            lines
        }
        PersistedToolResultContent::Decoded(ToolResultContent::CodeItemLookup(item)) => vec![
            format!(
                "result: {} :: {}",
                truncate_middle(item.file_path.as_ref(), 160),
                truncate_middle(item.canon_path.as_ref(), 160)
            ),
            format!("excerpt: {}", excerpt(&item.snippet, 360)),
        ],
        PersistedToolResultContent::Decoded(ToolResultContent::ApplyCodeEdit(result))
        | PersistedToolResultContent::Decoded(ToolResultContent::InsertRustItem(result)) => {
            let mut lines = vec![format!(
                "result: ok={} staged={} applied={} auto_confirmed={} files={}",
                result.ok,
                result.staged,
                result.applied,
                result.auto_confirmed,
                result.files.len()
            )];
            for file in &result.files {
                lines.push(format!("  file: {}", truncate_middle(file, 180)));
            }
            lines
        }
        PersistedToolResultContent::Decoded(ToolResultContent::CreateFile(result)) => {
            let mut lines = vec![format!(
                "result: ok={} staged={} applied={} files={}",
                result.ok,
                result.staged,
                result.applied,
                result.files.len()
            )];
            for file in &result.files {
                lines.push(format!("  file: {}", truncate_middle(file, 180)));
            }
            lines
        }
        PersistedToolResultContent::Decoded(ToolResultContent::ListDir(result)) => {
            let mut lines = vec![format!(
                "result: ok={} dir={} exists={} entries={} truncated={}",
                result.ok,
                truncate_middle(&result.dir, 160),
                result.exists,
                result.entries.len(),
                result.truncated
            )];
            for (index, entry) in result.entries.iter().enumerate() {
                let size = entry
                    .size_bytes
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "-".to_owned());
                lines.push(format!(
                    "  {}. {} {} bytes={} path={}",
                    index + 1,
                    entry.kind,
                    entry.name,
                    size,
                    truncate_middle(&entry.path, 180)
                ));
            }
            lines
        }
        PersistedToolResultContent::Decoded(ToolResultContent::Cargo(result)) => vec![format!(
            "result: ok={} command={:?} scope={:?} exit={:?} duration_ms={} errors={} warnings={} diagnostics={} stderr_tail={}",
            result.ok,
            result.command,
            result.scope,
            result.exit_code,
            result.duration_ms,
            result.summary.errors,
            result.summary.warnings,
            result.diagnostics.len(),
            result.stderr_tail.len()
        )],
        PersistedToolResultContent::Decoded(ToolResultContent::NsPatch(result)) => vec![format!(
            "result: ok={} staged={} applied={} files={}",
            result.ok,
            result.staged,
            result.applied,
            result.files.len()
        )],
        PersistedToolResultContent::ParseFailure(failure) => vec![
            format!("decode_error: {:?}", failure.error),
            format!("content: {}", truncate_middle(content, 700)),
        ],
    }
}

fn excerpt(text: &str, max_chars: usize) -> String {
    let normalized = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(4)
        .collect::<Vec<_>>()
        .join(" / ");
    if normalized.is_empty() {
        return "(empty)".to_owned();
    }
    truncate_middle(&normalized, max_chars)
}

fn format_prefix_selector(selector: &super::turn::ReplayPrefixSelector) -> String {
    match selector {
        super::turn::ReplayPrefixSelector::FullTape => "full_tape".to_owned(),
        super::turn::ReplayPrefixSelector::ThroughResponseIndex { response_index } => {
            format!("through_response:{response_index}")
        }
        super::turn::ReplayPrefixSelector::ThroughEvent { cursor } => {
            format!("through_event:{}", format_cursor(cursor))
        }
    }
}

fn format_cursor(cursor: &ploke_tree::TurnCursor) -> String {
    format!(
        "{:?}:{}:{}",
        cursor.artifact_kind, cursor.artifact_path, cursor.event_index
    )
}

fn truncate_middle(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let front = (max_len - 3) / 2;
    let back = max_len - 3 - front;
    format!(
        "{}...{}",
        chars[..front].iter().collect::<String>(),
        chars[chars.len() - back..].iter().collect::<String>()
    )
}

#[cfg(test)]
mod tests {
    use super::tool_result_lines;

    #[test]
    fn request_code_context_renderer_shows_each_result() {
        let raw = serde_json::json!({
            "ok": true,
            "search_term": "MAX_LOOK_AHEAD matcher replace",
            "top_k": 3,
            "kind": "code",
            "note": null,
            "next_steps": [],
            "context": [
                {
                    "id": "00000000-0000-0000-0000-000000000001",
                    "file_path": "crates/printer/src/lib.rs",
                    "canon_path": "crate::MAX_LOOK_AHEAD",
                    "snippet": "const MAX_LOOK_AHEAD: usize = 128;"
                },
                {
                    "id": "00000000-0000-0000-0000-000000000002",
                    "file_path": "crates/matcher/src/lib.rs",
                    "canon_path": "crate::replace",
                    "snippet": "fn replace<F>(haystack: &[u8], dst: &mut Vec<u8>, append: F) {}"
                },
                {
                    "id": "00000000-0000-0000-0000-000000000003",
                    "file_path": "crates/regex/src/matcher.rs",
                    "canon_path": "crate::matcher::replace",
                    "snippet": "match self.matcher { Standard(ref m) => m.replace(haystack, dst, append) }"
                }
            ]
        })
        .to_string();

        let rendered = tool_result_lines("request_code_context", &raw).join("\n");

        assert!(rendered.contains("returned=3"));
        assert!(rendered.contains("1. crates/printer/src/lib.rs :: crate::MAX_LOOK_AHEAD"));
        assert!(rendered.contains("2. crates/matcher/src/lib.rs :: crate::replace"));
        assert!(rendered.contains("3. crates/regex/src/matcher.rs :: crate::matcher::replace"));
        assert!(rendered.contains("const MAX_LOOK_AHEAD"));
    }
}
