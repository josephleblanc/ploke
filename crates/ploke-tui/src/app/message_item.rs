use super::*;
use crate::app::types::RenderMsg;
use crate::app::view::rendering::highlight::{StyledSpan, highlight_message_lines};
use crate::chat_history::{AnnotationKind, MessageKind};
use crate::tools::Audience;
use crate::tools::ToolVerbosity;
use crate::user_config::{MessageVerbosity, VerbosityLevel};
use ploke_core::tool_types::ToolName;
use std::collections::HashMap;
use uuid::Uuid;

fn base_style_for_kind(kind: MessageKind) -> Style {
    match kind {
        MessageKind::User => Style::new().blue(),
        MessageKind::Assistant => Style::new().green(),
        MessageKind::System => Style::new().cyan(),
        MessageKind::SysInfo => Style::new().magenta(),
        MessageKind::Tool => Style::new().green().dim(),
    }
}

#[instrument(skip(renderable_msg), level = "trace")]
pub fn measure_messages<'a, I, T: RenderMsg + 'a>(
    renderable_msg: I,
    conversation_width: u16,
    tool_verbosity: ToolVerbosity,
    message_verbosity_profile: &[MessageVerbosity],
    path_len: usize,
    _selected_index: Option<usize>,
) -> (u16, Vec<u16>)
where
    I: IntoIterator<Item = &'a T>,
{
    let policy = MessageRenderPolicy::from_profile(message_verbosity_profile);
    // Compute per-message heights and total height for the current frame.
    let mut heights: Vec<u16> = Vec::new();
    let mut total_height = 0u16;
    for (idx, msg) in renderable_msg.into_iter().enumerate() {
        if !should_render_message_with_policy(msg, &policy) {
            heights.push(0);
            continue;
        }
        // Always reserve a 1-column gutter for the selection bar to keep heights stable.
        let eff_w = conversation_width.saturating_sub(1).max(1);
        let content = render_message_content(msg, tool_verbosity, &policy, idx, path_len);
        let lines = highlight_message_lines(&content, base_style_for_kind(msg.kind()), eff_w);
        let annotation_lines = render_annotation_lines(msg, eff_w);
        let h = lines.len().saturating_add(annotation_lines.len()) as u16;
        let mut height = h.max(1);

        if let Some(payload) = msg.tool_payload() {
            if should_render_tool_buttons(payload) {
                height += 1;
            }
        }

        heights.push(height);
        total_height = total_height.saturating_add(height);
    }
    (total_height, heights)
}

#[instrument(skip(frame, renderable_msg, heights), level = "trace")]
pub fn render_messages<'a, I, T: RenderMsg + 'a>(
    frame: &mut Frame,
    renderable_msg: I,
    conversation_width: u16,
    conversation_area: Rect,
    offset_y: u16,
    heights: &[u16],
    selected_index: Option<usize>,
    tool_verbosity: ToolVerbosity,
    message_verbosity_profile: &[MessageVerbosity],
    confirmation_states: &HashMap<Uuid, bool>,
) where
    I: IntoIterator<Item = &'a T>,
{
    let policy = MessageRenderPolicy::from_profile(message_verbosity_profile);
    let path_len = heights.len();
    // 1) Clamp offset
    let viewport_height = conversation_area.height;
    let total_height: u16 = heights
        .iter()
        .copied()
        .fold(0u16, |acc, h| acc.saturating_add(h));
    let clamped_offset_y = offset_y.min(total_height.saturating_sub(viewport_height));

    // 2) Render visible slice
    let mut y_screen = 0u16;
    let mut y_virtual = 0u16;

    for (idx, msg) in renderable_msg.into_iter().enumerate() {
        let height = heights[idx];
        if height == 0 || !should_render_message_with_policy(msg, &policy) {
            continue;
        }
        let is_selected = selected_index == Some(idx);
        let base_style = base_style_for_kind(msg.kind());

        if y_virtual + height <= clamped_offset_y {
            y_virtual = y_virtual.saturating_add(height);
            continue;
        }

        // Use the same effective width as in height calc: always reserve 1-column gutter.
        let eff_w = conversation_width.saturating_sub(1).max(1);
        let content = render_message_content(msg, tool_verbosity, &policy, idx, path_len);
        let wrapped = highlight_message_lines(&content, base_style, eff_w);
        let annotation_lines = render_annotation_lines(msg, eff_w);
        let annotation_count = annotation_lines.len();
        let bar = Span::styled("│", base_style.fg(Color::White));

        // If offset lands inside this message, skip top lines so we don’t waste space
        let mut start_line = 0usize;
        if clamped_offset_y > y_virtual {
            start_line = (clamped_offset_y - y_virtual) as usize;
        }

        let mut remaining_skip = start_line;
        let content_lines = wrapped.len();
        for line in wrapped.into_iter() {
            if remaining_skip > 0 {
                remaining_skip -= 1;
                continue;
            }
            let mut spans = Vec::with_capacity(line.len() + 1);
            if is_selected {
                spans.push(bar.clone());
            }
            append_spans(&mut spans, line);
            let para = Paragraph::new(Line::from(spans)).style(base_style);

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_screen,
                conversation_width,
                1,
            );
            frame.render_widget(para, area);
            y_screen = y_screen.saturating_add(1);
            if y_screen >= viewport_height {
                return;
            }
        }

        for line in annotation_lines.into_iter() {
            if remaining_skip > 0 {
                remaining_skip -= 1;
                continue;
            }
            let mut spans = Vec::with_capacity(line.len() + 1);
            if is_selected {
                spans.push(bar.clone());
            }
            append_spans(&mut spans, line);
            let para = Paragraph::new(Line::from(spans));

            let area = Rect::new(
                conversation_area.x + 1,
                conversation_area.y + y_screen,
                conversation_width,
                1,
            );
            frame.render_widget(para, area);
            y_screen = y_screen.saturating_add(1);
            if y_screen >= viewport_height {
                return;
            }
        }

        // Render buttons if applicable
        if let Some(payload) = msg.tool_payload() {
            if should_render_tool_buttons(payload) {
                let button_y_rel = content_lines.saturating_add(annotation_count) as u16;
                let button_y_abs = y_virtual.saturating_add(button_y_rel);

                if remaining_skip == 0
                    && button_y_abs >= clamped_offset_y
                    && y_screen < viewport_height
                {
                    let is_yes = confirmation_states.get(&msg.id()).copied().unwrap_or(true);

                    let active_style = Style::new().bg(Color::Blue).fg(Color::White).bold();
                    let inactive_style = Style::new().dim();

                    let yes_span = if is_yes {
                        Span::styled("[ Yes ]", active_style)
                    } else {
                        Span::styled("[ Yes ]", inactive_style)
                    };
                    let no_span = if !is_yes {
                        Span::styled("[ No ]", active_style)
                    } else {
                        Span::styled("[ No ]", inactive_style)
                    };

                    let mut spans = Vec::new();
                    if is_selected {
                        spans.push(bar.clone());
                    }
                    spans.push(yes_span);
                    spans.push(Span::raw("  "));
                    spans.push(no_span);

                    let para = Paragraph::new(Line::from(spans));
                    let area = Rect::new(
                        conversation_area.x + 1,
                        conversation_area.y + y_screen,
                        conversation_width,
                        1,
                    );
                    frame.render_widget(para, area);
                    y_screen = y_screen.saturating_add(1);
                }
            }
        }

        y_virtual = y_virtual.saturating_add(height);

        if y_screen >= viewport_height {
            return;
        }
    }
}

fn append_spans(spans: &mut Vec<Span<'static>>, line: Vec<StyledSpan>) {
    for span in line {
        spans.push(Span::styled(span.content, span.style));
    }
}

pub(crate) fn should_render_message<T: RenderMsg>(
    msg: &T,
    message_verbosity_profile: &[MessageVerbosity],
) -> bool {
    let policy = MessageRenderPolicy::from_profile(message_verbosity_profile);
    should_render_message_with_policy(msg, &policy)
}

fn should_render_message_with_policy<T: RenderMsg>(msg: &T, policy: &MessageRenderPolicy) -> bool {
    match msg.kind() {
        MessageKind::System => {
            if is_initial_system_message(msg.content()) && !policy.system.display_init {
                return false;
            }
            is_level_visible(
                policy.system.verbosity,
                infer_message_level(msg.kind(), msg.content()),
            )
        }
        MessageKind::SysInfo => is_level_visible(
            policy.sysinfo.verbosity,
            infer_message_level(msg.kind(), msg.content()),
        ),
        _ => true,
    }
}

fn render_message_content<T: RenderMsg>(
    msg: &T,
    verbosity: ToolVerbosity,
    policy: &MessageRenderPolicy,
    idx: usize,
    path_len: usize,
) -> String {
    if let Some(payload) = msg.tool_payload() {
        return payload.render(verbosity);
    }
    let max_len = match msg.kind() {
        MessageKind::User => policy.user.max_len,
        MessageKind::Assistant => {
            if policy.assistant.truncate_prev_messages && idx + 1 < path_len {
                policy.assistant.truncated_len.or(policy.assistant.max_len)
            } else {
                policy.assistant.max_len
            }
        }
        MessageKind::SysInfo => policy.sysinfo.max_len,
        MessageKind::System => policy.system.max_len,
        MessageKind::Tool => None,
    };
    truncate_to_max_len(msg.content(), max_len)
}

fn annotation_style(kind: AnnotationKind) -> Style {
    match kind {
        AnnotationKind::Hint => Style::new().dim(),
        AnnotationKind::Info => Style::new().dim(),
        AnnotationKind::Warning => Style::new().fg(Color::Yellow),
    }
}

fn render_annotation_lines<T: RenderMsg>(msg: &T, eff_w: u16) -> Vec<Vec<StyledSpan>> {
    let mut out = Vec::new();
    let Some(annotations) = msg.annotations() else {
        return out;
    };
    for annotation in annotations {
        if annotation.audience != Audience::User {
            continue;
        }
        let label = match annotation.kind {
            AnnotationKind::Hint => "hint",
            AnnotationKind::Info => "info",
            AnnotationKind::Warning => "warning",
        };
        let line = format!("  {}: {}", label, annotation.text);
        let lines = highlight_message_lines(&line, annotation_style(annotation.kind), eff_w);
        out.extend(lines);
    }
    out
}

pub(crate) fn should_render_tool_buttons(payload: &crate::tools::ToolUiPayload) -> bool {
    let is_pending = tool_payload_status(payload)
        .map(|status| status.eq_ignore_ascii_case("pending"))
        .unwrap_or(true);
    matches!(
        payload.tool,
        ToolName::ApplyCodeEdit | ToolName::InsertRustItem | ToolName::NsPatch
    ) && payload.error.is_none()
        && payload.request_id.is_some()
        && is_pending
}

fn tool_payload_status(payload: &crate::tools::ToolUiPayload) -> Option<&str> {
    payload
        .fields
        .iter()
        .find(|field| field.name.as_ref() == "status")
        .map(|field| field.value.as_ref())
}

#[derive(Debug, Clone, Copy)]
struct UserRenderPolicy {
    max_len: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct AssistantRenderPolicy {
    max_len: Option<u32>,
    truncate_prev_messages: bool,
    truncated_len: Option<u32>,
}

#[derive(Debug, Clone, Copy)]
struct SysInfoRenderPolicy {
    max_len: Option<u32>,
    verbosity: VerbosityLevel,
}

#[derive(Debug, Clone, Copy)]
struct SystemRenderPolicy {
    max_len: Option<u32>,
    verbosity: VerbosityLevel,
    display_init: bool,
}

#[derive(Debug, Clone, Copy)]
struct MessageRenderPolicy {
    user: UserRenderPolicy,
    assistant: AssistantRenderPolicy,
    sysinfo: SysInfoRenderPolicy,
    system: SystemRenderPolicy,
}

impl MessageRenderPolicy {
    fn from_profile(profile: &[MessageVerbosity]) -> Self {
        let mut policy = Self {
            user: UserRenderPolicy { max_len: None },
            assistant: AssistantRenderPolicy {
                max_len: None,
                truncate_prev_messages: false,
                truncated_len: None,
            },
            sysinfo: SysInfoRenderPolicy {
                max_len: None,
                verbosity: VerbosityLevel::Info,
            },
            system: SystemRenderPolicy {
                max_len: None,
                verbosity: VerbosityLevel::Info,
                display_init: false,
            },
        };

        for setting in profile {
            match setting {
                MessageVerbosity::User { max_len, .. } => {
                    policy.user.max_len = *max_len;
                }
                MessageVerbosity::Assistant {
                    max_len,
                    truncate_prev_messages,
                    truncated_len,
                    ..
                } => {
                    policy.assistant.max_len = *max_len;
                    policy.assistant.truncate_prev_messages = *truncate_prev_messages;
                    policy.assistant.truncated_len = *truncated_len;
                }
                MessageVerbosity::SysInfo { max_len, verbosity } => {
                    policy.sysinfo.max_len = *max_len;
                    policy.sysinfo.verbosity = *verbosity;
                }
                MessageVerbosity::System {
                    max_len,
                    verbosity,
                    display_init,
                } => {
                    policy.system.max_len = *max_len;
                    policy.system.verbosity = *verbosity;
                    policy.system.display_init = *display_init;
                }
            }
        }

        policy
    }
}

fn truncate_to_max_len(content: &str, max_len: Option<u32>) -> String {
    let Some(max_len) = max_len else {
        return content.to_string();
    };
    let max_len = max_len as usize;
    if content.chars().count() <= max_len {
        return content.to_string();
    }
    let mut out: String = content.chars().take(max_len).collect();
    out.push_str("... [truncated]");
    out
}

fn infer_message_level(kind: MessageKind, content: &str) -> VerbosityLevel {
    if kind == MessageKind::System && is_initial_system_message(content) {
        return VerbosityLevel::Debug;
    }

    let lower = content.to_ascii_lowercase();

    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("failure")
        || lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("panic")
        || lower.contains("invalid")
    {
        return VerbosityLevel::Error;
    }

    if lower.contains("warning")
        || lower.contains("warn:")
        || lower.contains("missing")
        || lower.contains("not found")
        || lower.contains("stale")
        || lower.contains("unavailable")
    {
        return VerbosityLevel::Warn;
    }

    if lower.contains("debug")
        || lower.contains("trace")
        || lower.contains("req_id")
        || lower.contains("diagnostic")
        || lower.contains(" bm25")
        || lower.contains(" dense")
        || lower.contains(" hybrid")
    {
        return VerbosityLevel::Debug;
    }

    VerbosityLevel::Info
}

fn is_level_visible(threshold: VerbosityLevel, level: VerbosityLevel) -> bool {
    match threshold {
        VerbosityLevel::Debug => true,
        VerbosityLevel::Info => !matches!(level, VerbosityLevel::Debug),
        VerbosityLevel::Warn => matches!(level, VerbosityLevel::Warn | VerbosityLevel::Error),
        VerbosityLevel::Error => matches!(level, VerbosityLevel::Error),
    }
}

fn is_initial_system_message(content: &str) -> bool {
    content.contains("BEGIN SYSTEM PROMPT")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_history::{ContextStatus, Message, MessageKind, MessageStatus};
    use ratatui::{Terminal, backend::TestBackend};

    fn tool_message() -> Message {
        let payload = crate::tools::ToolUiPayload::new(
            ToolName::ApplyCodeEdit,
            "call-1".into(),
            "edit staged",
        )
        .with_request_id(Uuid::new_v4())
        .with_field("status", "pending");

        Message {
            id: Uuid::new_v4(),
            branch_id: Uuid::nil(),
            status: MessageStatus::Completed,
            metadata: None,
            parent: None,
            children: Vec::new(),
            selected_child: None,
            content: String::new(),
            kind: MessageKind::Tool,
            tool_call_id: None,
            tool_payload: Some(payload),
            context_status: ContextStatus::default(),
            last_included_turn: None,
            include_count: 0,
        }
    }

    fn render_single_line(msgs: &[Message], offset_y: u16) -> String {
        let area = Rect::new(0, 0, 60, 1);
        let profile = vec![
            MessageVerbosity::SysInfo {
                max_len: None,
                verbosity: VerbosityLevel::Debug,
            },
            MessageVerbosity::System {
                max_len: None,
                verbosity: VerbosityLevel::Debug,
                display_init: true,
            },
        ];
        let (_, heights) = measure_messages(
            msgs.iter(),
            area.width,
            ToolVerbosity::Minimal,
            &profile,
            msgs.len(),
            None,
        );
        let mut terminal =
            Terminal::new(TestBackend::new(area.width, area.height)).expect("terminal");
        let confirmation_states = HashMap::new();
        terminal
            .draw(|frame| {
                render_messages(
                    frame,
                    msgs.iter(),
                    area.width,
                    area,
                    offset_y,
                    &heights,
                    None,
                    ToolVerbosity::Minimal,
                    &profile,
                    &confirmation_states,
                );
            })
            .expect("draw");

        let buf = terminal.backend().buffer();
        let mut line = String::new();
        for x in 0..area.width {
            line.push_str(buf.cell((x, 0)).expect("cell").symbol());
        }
        line
    }

    #[test]
    fn tool_buttons_render_on_scrolled_row() {
        let msgs = vec![tool_message()];
        let top_row = render_single_line(&msgs, 0);
        let button_row = render_single_line(&msgs, 1);

        assert!(
            top_row.contains("Tool: apply_code_edit"),
            "expected tool content on first row, got: {top_row}"
        );
        assert!(
            !top_row.contains("[ Yes ]"),
            "did not expect buttons on first row, got: {top_row}"
        );
        assert!(
            button_row.contains("[ Yes ]") && button_row.contains("[ No ]"),
            "expected approval buttons when scrolled, got: {button_row}"
        );
    }

    #[test]
    fn minimal_profile_hides_initial_system_prompt() {
        let msg = Message {
            id: Uuid::new_v4(),
            branch_id: Uuid::nil(),
            status: MessageStatus::Completed,
            metadata: None,
            parent: None,
            children: Vec::new(),
            selected_child: None,
            content: "<-- BEGIN SYSTEM PROMPT --> You are a highly skilled...".to_string(),
            kind: MessageKind::System,
            tool_call_id: None,
            tool_payload: None,
            context_status: ContextStatus::default(),
            last_included_turn: None,
            include_count: 0,
        };
        let profile = vec![MessageVerbosity::System {
            max_len: None,
            verbosity: VerbosityLevel::Warn,
            display_init: false,
        }];
        assert!(!super::should_render_message(&msg, &profile));
    }

    #[test]
    fn assistant_previous_messages_use_truncated_len() {
        let msgs = vec![
            Message {
                id: Uuid::new_v4(),
                branch_id: Uuid::nil(),
                status: MessageStatus::Completed,
                metadata: None,
                parent: None,
                children: Vec::new(),
                selected_child: None,
                content: "abcdefghijklmnopqrstuvwxyz".to_string(),
                kind: MessageKind::Assistant,
                tool_call_id: None,
                tool_payload: None,
                context_status: ContextStatus::default(),
                last_included_turn: None,
                include_count: 0,
            },
            Message {
                id: Uuid::new_v4(),
                branch_id: Uuid::nil(),
                status: MessageStatus::Completed,
                metadata: None,
                parent: None,
                children: Vec::new(),
                selected_child: None,
                content: "latest".to_string(),
                kind: MessageKind::Assistant,
                tool_call_id: None,
                tool_payload: None,
                context_status: ContextStatus::default(),
                last_included_turn: None,
                include_count: 0,
            },
        ];
        let policy = MessageRenderPolicy::from_profile(&[MessageVerbosity::Assistant {
            max_len: None,
            syntax_highlighting: false,
            truncate_prev_messages: true,
            truncated_len: Some(8),
        }]);
        let first = super::render_message_content(&msgs[0], ToolVerbosity::Normal, &policy, 0, 2);
        let second = super::render_message_content(&msgs[1], ToolVerbosity::Normal, &policy, 1, 2);
        assert!(first.starts_with("abcdefgh"));
        assert!(first.contains("[truncated]"));
        assert_eq!(second, "latest");
    }
}
