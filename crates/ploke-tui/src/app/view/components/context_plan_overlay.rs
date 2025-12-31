use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_core::rag_types::ContextPart;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize as _};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use uuid::Uuid;

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::utils::truncate_uuid;
use crate::app::view::rendering::highlight::{highlight_message_lines, styled_to_ratatui_lines};
use crate::app::view::widgets::expanding_list::{
    ExpandingItem, ExpandingList, ExpandingListState,
};
use crate::context_plan::{ContextPlanHistory, ContextPlanSnapshot};
use crate::llm::manager::events::{ContextExclusionReason, ContextPlanMessage, ContextPlanRagPart};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextPlanFilter {
    All,
    ExcludedAll,
    ExcludedBudget,
    ExcludedTtlExpired,
}

impl ContextPlanFilter {
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::ExcludedAll,
            Self::ExcludedAll => Self::ExcludedBudget,
            Self::ExcludedBudget => Self::ExcludedTtlExpired,
            Self::ExcludedTtlExpired => Self::All,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::ExcludedAll => "Excluded (all)",
            Self::ExcludedBudget => "Excluded (Budget)",
            Self::ExcludedTtlExpired => "Excluded (TTL)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContextPlanItemKey {
    IncludedMessage { message_id: Option<Uuid>, index: usize },
    ExcludedMessage { message_id: Uuid },
    RagPart { part_id: Uuid },
}

#[derive(Debug, Clone)]
enum ContextPlanRow {
    Header { title: String },
    IncludedMessage {
        key: ContextPlanItemKey,
        message: ContextPlanMessage,
        index: usize,
    },
    ExcludedMessage {
        key: ContextPlanItemKey,
        message_id: Uuid,
        kind: crate::chat_history::MessageKind,
        estimated_tokens: usize,
        reason: ContextExclusionReason,
    },
    RagPart {
        key: ContextPlanItemKey,
        part: ContextPlanRagPart,
    },
}

#[derive(Debug)]
pub struct ContextPlanOverlayState {
    pub list_state: ExpandingListState,
    pub filter: ContextPlanFilter,
    pub follow_latest: bool,
    pub history_index: usize,
    pub help_visible: bool,
    last_plan_id: Option<Uuid>,
    history: Arc<RwLock<ContextPlanHistory>>,
    expanded: HashSet<ContextPlanItemKey>,
    snippet_visible: HashSet<Uuid>,
    rows: Vec<ContextPlanRow>,
}

impl ContextPlanOverlayState {
    pub fn new(history: Arc<RwLock<ContextPlanHistory>>) -> Self {
        Self {
            list_state: ExpandingListState::default(),
            filter: ContextPlanFilter::All,
            follow_latest: true,
            history_index: 0,
            help_visible: false,
            last_plan_id: None,
            history,
            expanded: HashSet::new(),
            snippet_visible: HashSet::new(),
            rows: Vec::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> Vec<OverlayAction> {
        let mut actions = Vec::new();
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                actions.push(OverlayAction::CloseOverlay(OverlayKind::ContextPlan));
            }
            KeyCode::Char('?') => {
                self.help_visible = !self.help_visible;
            }
            KeyCode::Up | KeyCode::Char('k') => self.select_prev(),
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Enter | KeyCode::Char(' ') => self.toggle_expanded_selected(),
            KeyCode::Char('H')
            | KeyCode::Char('h')
                if shift =>
            {
                self.step_history_prev();
            }
            KeyCode::Char('L')
            | KeyCode::Char('l')
                if shift =>
            {
                self.step_history_next();
            }
            KeyCode::Left if shift => {
                self.step_history_prev();
            }
            KeyCode::Right if shift => {
                self.step_history_next();
            }
            KeyCode::Char('h') | KeyCode::Left => self.collapse_selected(),
            KeyCode::Char('l') | KeyCode::Right => self.expand_selected(),
            KeyCode::Char('f') => {
                self.filter = self.filter.next();
                self.reset_view_state();
            }
            KeyCode::Char('s') => {
                self.toggle_snippet_selected();
            }
            _ => {}
        }
        actions
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, state: &Arc<crate::app_state::AppState>) {
        let _ = state;
        let area = centered_overlay_area(frame.area(), 8, 10);
        let footer_height = if self.help_visible { 5 } else { 3 };
        let body_height = area.height.saturating_sub(footer_height);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(body_height), Constraint::Length(footer_height)])
            .split(area);
        let body_area = layout[0];
        let footer_area = layout[1];
        frame.render_widget(Clear, area);

        let (snapshot, total_entries) = self.current_snapshot();
        if let Some(snapshot) = snapshot {
            if self.last_plan_id != Some(snapshot.plan.plan_id) {
                self.last_plan_id = Some(snapshot.plan.plan_id);
                self.reset_view_state();
            }
            let title = format!(
                " Context Plan — {}/{} — filter: {} ",
                self.history_index + 1,
                total_entries,
                self.filter.label()
            );
            let block = Block::bordered().title(title);
            let inner = block.inner(body_area);
            let detail_width = inner.width.saturating_sub(2);
            self.rows = build_rows(&snapshot, self.filter);
            let focus_root = state
                .system
                .try_read()
                .ok()
                .and_then(|guard| guard.focused_crate_root());
            let items = build_display_items(
                &snapshot,
                &self.rows,
                &self.expanded,
                &self.snippet_visible,
                focus_root.as_deref(),
                inner.width,
                detail_width,
            );
            if self.list_state.selected >= items.len() && !items.is_empty() {
                self.list_state.selected = items.len().saturating_sub(1);
            }
            frame.render_widget(block, body_area);
            let widget = ExpandingList {
                items: &items,
                normal_style: Style::default(),
                detail_style: Style::default(),
                selected_style: Style::default().bg(Color::DarkGray),
            };
            frame.render_stateful_widget(widget, inner, &mut self.list_state);
        } else {
            let block = Block::bordered().title(" Context Plan ");
            let inner = block.inner(body_area);
            frame.render_widget(block, body_area);
            let empty = Paragraph::new(Line::from(Span::raw("No context plans recorded yet.")))
                .wrap(Wrap { trim: true });
            frame.render_widget(empty, inner);
        }

        render_context_plan_footer(frame, footer_area, self.help_visible);
    }

    fn current_snapshot(&mut self) -> (Option<ContextPlanSnapshot>, usize) {
        let guard = match self.history.read() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        let total = guard.len();
        if total == 0 {
            self.history_index = 0;
            return (None, 0);
        }
        if self.follow_latest || self.history_index >= total {
            self.history_index = total.saturating_sub(1);
        }
        let snapshot = guard.get(self.history_index).cloned();
        (snapshot, total)
    }

    fn reset_view_state(&mut self) {
        self.list_state.selected = 0;
        self.list_state.vscroll = 0;
        self.expanded.clear();
        self.snippet_visible.clear();
    }

    fn select_prev(&mut self) {
        if self.list_state.selected > 0 {
            self.list_state.selected = self.list_state.selected.saturating_sub(1);
        }
    }

    fn select_next(&mut self) {
        self.list_state.selected = self.list_state.selected.saturating_add(1);
    }

    fn toggle_expanded_selected(&mut self) {
        if let Some(key) = self.selected_item_key() {
            if self.expanded.contains(&key) {
                self.expanded.remove(&key);
            } else {
                self.expanded.insert(key);
            }
        }
    }

    fn expand_selected(&mut self) {
        if let Some(key) = self.selected_item_key() {
            self.expanded.insert(key);
        }
    }

    fn collapse_selected(&mut self) {
        if let Some(key) = self.selected_item_key() {
            self.expanded.remove(&key);
        }
    }

    fn toggle_snippet_selected(&mut self) {
        if let Some(ContextPlanItemKey::RagPart { part_id }) = self.selected_item_key() {
            if self.snippet_visible.contains(&part_id) {
                self.snippet_visible.remove(&part_id);
            } else {
                self.snippet_visible.insert(part_id);
                self.expanded.insert(ContextPlanItemKey::RagPart { part_id });
            }
        }
    }

    fn selected_item_key(&mut self) -> Option<ContextPlanItemKey> {
        if self.rows.is_empty() {
            return None;
        }
        let idx = self.list_state.selected.min(self.rows.len().saturating_sub(1));
        match self.rows.get(idx) {
            Some(ContextPlanRow::IncludedMessage { key, .. }) => Some(*key),
            Some(ContextPlanRow::ExcludedMessage { key, .. }) => Some(*key),
            Some(ContextPlanRow::RagPart { key, .. }) => Some(*key),
            _ => None,
        }
    }

    fn step_history_prev(&mut self) {
        if self.history_index > 0 {
            self.history_index = self.history_index.saturating_sub(1);
            self.follow_latest = false;
            self.reset_view_state();
        }
    }

    fn step_history_next(&mut self) {
        let len = match self.history.read() {
            Ok(guard) => guard.len(),
            Err(poisoned) => poisoned.into_inner().len(),
        };
        if self.history_index + 1 < len {
            self.history_index += 1;
            self.follow_latest = false;
            self.reset_view_state();
        } else {
            self.follow_latest = true;
        }
    }
}

#[derive(Clone)]
struct ContextPlanDisplayItem {
    title: Line<'static>,
    details: Vec<Line<'static>>,
    expanded: bool,
}

impl ExpandingItem for ContextPlanDisplayItem {
    fn title_line(&self) -> Line<'_> {
        self.title.clone()
    }

    fn detail_lines(&self) -> Vec<Line<'_>> {
        self.details.clone()
    }

    fn is_expanded(&self) -> bool {
        self.expanded
    }
}

fn build_rows(snapshot: &ContextPlanSnapshot, filter: ContextPlanFilter) -> Vec<ContextPlanRow> {
    let plan = &snapshot.plan;
    let included_msg_tokens: usize = plan
        .included_messages
        .iter()
        .map(|m| m.estimated_tokens)
        .sum();
    let included_rag_tokens: usize = plan
        .included_rag_parts
        .iter()
        .map(|p| p.estimated_tokens)
        .sum();
    let excluded_budget_tokens: usize = plan
        .excluded_messages
        .iter()
        .filter(|m| m.reason == ContextExclusionReason::Budget)
        .map(|m| m.estimated_tokens)
        .sum();
    let excluded_ttl_tokens: usize = plan
        .excluded_messages
        .iter()
        .filter(|m| m.reason == ContextExclusionReason::TtlExpired)
        .map(|m| m.estimated_tokens)
        .sum();
    let candidate_total =
        included_msg_tokens + included_rag_tokens + excluded_budget_tokens + excluded_ttl_tokens;
    let denom = candidate_total.max(1) as f32;

    let mut rows = Vec::new();
    rows.push(ContextPlanRow::Header {
        title: format!(
            "Plan {} (parent {}) — est {} tokens",
            truncate_uuid(plan.plan_id),
            truncate_uuid(plan.parent_id),
            plan.estimated_total_tokens
        ),
    });

    let include_included = matches!(filter, ContextPlanFilter::All);
    let include_excluded_all = matches!(
        filter,
        ContextPlanFilter::All | ContextPlanFilter::ExcludedAll
    );
    let include_excluded_budget =
        matches!(filter, ContextPlanFilter::ExcludedBudget | ContextPlanFilter::ExcludedAll);
    let include_excluded_ttl =
        matches!(filter, ContextPlanFilter::ExcludedTtlExpired | ContextPlanFilter::ExcludedAll);
    let include_rag = matches!(filter, ContextPlanFilter::All);

    if include_included && !plan.included_messages.is_empty() {
        let percent = included_msg_tokens as f32 / denom * 100.0;
        rows.push(ContextPlanRow::Header {
            title: format!(
                "Included messages — {} items, {} tok ({:.1}%)",
                plan.included_messages.len(),
                included_msg_tokens,
                percent
            ),
        });
        for (idx, message) in plan.included_messages.iter().cloned().enumerate() {
            let key = ContextPlanItemKey::IncludedMessage {
                message_id: message.message_id,
                index: idx,
            };
            rows.push(ContextPlanRow::IncludedMessage {
                key,
                message,
                index: idx,
            });
        }
    }

    if include_rag && !plan.included_rag_parts.is_empty() {
        let percent = included_rag_tokens as f32 / denom * 100.0;
        rows.push(ContextPlanRow::Header {
            title: format!(
                "Included RAG parts — {} items, {} tok ({:.1}%)",
                plan.included_rag_parts.len(),
                included_rag_tokens,
                percent
            ),
        });
        for part in &plan.included_rag_parts {
            let key = ContextPlanItemKey::RagPart { part_id: part.part_id };
            rows.push(ContextPlanRow::RagPart {
                key,
                part: part.clone(),
            });
        }
    }

    if include_excluded_all {
        let excluded_budget = plan
            .excluded_messages
            .iter()
            .filter(|m| m.reason == ContextExclusionReason::Budget)
            .cloned()
            .collect::<Vec<_>>();
        if include_excluded_budget && !excluded_budget.is_empty() {
            let percent = excluded_budget_tokens as f32 / denom * 100.0;
            rows.push(ContextPlanRow::Header {
                title: format!(
                    "Excluded (Budget) — {} items, {} tok ({:.1}%)",
                    excluded_budget.len(),
                    excluded_budget_tokens,
                    percent
                ),
            });
            for message in excluded_budget {
                let key = ContextPlanItemKey::ExcludedMessage {
                    message_id: message.message_id,
                };
                rows.push(ContextPlanRow::ExcludedMessage {
                    key,
                    message_id: message.message_id,
                    kind: message.kind,
                    estimated_tokens: message.estimated_tokens,
                    reason: message.reason,
                });
            }
        }

        let excluded_ttl = plan
            .excluded_messages
            .iter()
            .filter(|m| m.reason == ContextExclusionReason::TtlExpired)
            .cloned()
            .collect::<Vec<_>>();
        if include_excluded_ttl && !excluded_ttl.is_empty() {
            let percent = excluded_ttl_tokens as f32 / denom * 100.0;
            rows.push(ContextPlanRow::Header {
                title: format!(
                    "Excluded (TTL) — {} items, {} tok ({:.1}%)",
                    excluded_ttl.len(),
                    excluded_ttl_tokens,
                    percent
                ),
            });
            for message in excluded_ttl {
                let key = ContextPlanItemKey::ExcludedMessage {
                    message_id: message.message_id,
                };
                rows.push(ContextPlanRow::ExcludedMessage {
                    key,
                    message_id: message.message_id,
                    kind: message.kind,
                    estimated_tokens: message.estimated_tokens,
                    reason: message.reason,
                });
            }
        }
    }

    rows
}

fn build_display_items(
    snapshot: &ContextPlanSnapshot,
    rows: &[ContextPlanRow],
    expanded: &HashSet<ContextPlanItemKey>,
    snippet_visible: &HashSet<Uuid>,
    focus_root: Option<&Path>,
    title_width: u16,
    detail_width: u16,
) -> Vec<ContextPlanDisplayItem> {
    let mut items = Vec::new();
    let mut part_cache: HashMap<Uuid, &ContextPart> = HashMap::new();
    if let Some(rag_ctx) = &snapshot.rag_context {
        for part in &rag_ctx.parts {
            part_cache.insert(part.id, part);
        }
    }

    for row in rows {
        match row {
            ContextPlanRow::Header { title } => {
                items.push(ContextPlanDisplayItem {
                    title: Line::from(Span::styled(title.clone(), Style::default().bold())),
                    details: Vec::new(),
                    expanded: false,
                });
            }
            ContextPlanRow::IncludedMessage { key, message, .. } => {
                let expanded = expanded.contains(key);
                let title = format!(
                    "  [msg] {} — ~{} tok{}",
                    message.kind,
                    message.estimated_tokens,
                    message
                        .message_id
                        .map(|id| format!(" ({})", truncate_uuid(id)))
                        .unwrap_or_default()
                );
                let mut details = Vec::new();
                if expanded {
                    details.push(Line::from(format!(
                        "    message_id: {}",
                        message
                            .message_id
                            .map(truncate_uuid)
                            .unwrap_or_else(|| "none".to_string())
                    )));
                    details.push(Line::from(format!("    kind: {}", message.kind)));
                    details.push(Line::from(format!(
                        "    estimated_tokens: {}",
                        message.estimated_tokens
                    )));
                }
                items.push(ContextPlanDisplayItem {
                    title: Line::from(title),
                    details,
                    expanded,
                });
            }
            ContextPlanRow::ExcludedMessage {
                key,
                message_id,
                kind,
                estimated_tokens,
                reason,
            } => {
                let expanded = expanded.contains(key);
                let title = format!(
                    "  [msg] {} — ~{} tok (excluded: {:?})",
                    kind, estimated_tokens, reason
                );
                let mut details = Vec::new();
                if expanded {
                    details.push(Line::from(format!("    message_id: {}", truncate_uuid(*message_id))));
                    details.push(Line::from(format!("    kind: {}", kind)));
                    details.push(Line::from(format!("    estimated_tokens: {}", estimated_tokens)));
                    details.push(Line::from(format!("    reason: {:?}", reason)));
                }
                items.push(ContextPlanDisplayItem {
                    title: Line::from(title),
                    details,
                    expanded,
                });
            }
            ContextPlanRow::RagPart { key, part } => {
                let expanded = expanded.contains(key) || snippet_visible.contains(&part.part_id);
                let display_path = display_relative_path(&part.file_path, focus_root);
                let suffix = format!(
                    " ({}, score {:.3}) — ~{} tok",
                    part.kind.to_static_str(),
                    part.score,
                    part.estimated_tokens
                );
                let title_path = truncate_path_start(
                    &display_path,
                    title_width as usize,
                    "  [rag] ",
                    &suffix,
                );
                let title = format!(
                    "  [rag] {}{}",
                    title_path,
                    suffix
                );
                let mut details = Vec::new();
                if expanded {
                    details.push(Line::from(format!("    part_id: {}", truncate_uuid(part.part_id))));
                    details.push(Line::from(format!("    file_path: {}", display_path)));
                    details.push(Line::from(format!(
                        "    kind: {}",
                        part.kind.to_static_str()
                    )));
                    details.push(Line::from(format!("    score: {:.3}", part.score)));
                    details.push(Line::from(format!(
                        "    estimated_tokens: {}",
                        part.estimated_tokens
                    )));
                }
                if snippet_visible.contains(&part.part_id) {
                    match part_cache.get(&part.part_id) {
                        Some(ctx_part) => {
                            let mut snippet_lines =
                                highlight_snippet_lines(ctx_part, detail_width);
                            details.append(&mut snippet_lines);
                        }
                        None => {
                            details.push(Line::from(
                                "    snippet: unavailable (context not cached)".to_string(),
                            ));
                        }
                    }
                }
                items.push(ContextPlanDisplayItem {
                    title: Line::from(title),
                    details,
                    expanded,
                });
            }
        }
    }

    items
}

fn highlight_snippet_lines(part: &ContextPart, detail_width: u16) -> Vec<Line<'static>> {
    let indent = "    ";
    let lang = Path::new(part.file_path.as_ref())
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("text");
    let mut fenced = String::new();
    fenced.push_str("```");
    fenced.push_str(lang);
    fenced.push('\n');
    fenced.push_str(part.text.as_str());
    if !part.text.ends_with('\n') {
        fenced.push('\n');
    }
    fenced.push_str("```");

    let width = detail_width
        .saturating_sub(indent.len() as u16)
        .max(1);
    let highlighted = highlight_message_lines(&fenced, Style::default(), width);
    let lines = styled_to_ratatui_lines(highlighted);
    indent_lines(lines, indent)
}

fn display_relative_path(path: &str, focus_root: Option<&Path>) -> String {
    if let Some(root) = focus_root {
        let path = Path::new(path);
        if let Ok(relative) = path.strip_prefix(root) {
            return relative.display().to_string();
        }
    }
    path.to_string()
}

fn truncate_path_start(path: &str, max_width: usize, prefix: &str, suffix: &str) -> String {
    let fixed = prefix.len() + suffix.len();
    let available = max_width.saturating_sub(fixed);
    if available == 0 {
        return String::new();
    }
    if path.len() <= available {
        return path.to_string();
    }
    let keep = available.saturating_sub(3);
    let start = path.len().saturating_sub(keep);
    format!("...{}", &path[start..])
}

fn indent_lines(lines: Vec<Line<'static>>, indent: &str) -> Vec<Line<'static>> {
    lines
        .into_iter()
        .map(|line| {
            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::raw(indent.to_string()));
            spans.extend(line.spans.into_iter());
            let mut out = Line::from(spans);
            out.style = line.style;
            out
        })
        .collect()
}

fn centered_overlay_area(area: Rect, width_ratio: u16, height_ratio: u16) -> Rect {
    let w = area.width.saturating_mul(width_ratio) / 10;
    let h = area.height.saturating_mul(height_ratio) / 10;
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

fn render_context_plan_footer(frame: &mut Frame<'_>, area: Rect, help_visible: bool) {
    let text = if help_visible {
        "Keys: j/k or ↑/↓=navigate  Enter/Space=toggle  h/l=collapse/expand  s=toggle snippet\n\
         History: Shift+H/L or Shift+←/→  Filter: f  ?=help  q/Esc=close"
            .to_string()
    } else {
        " ? Help ".to_string()
    };
    let widget = Paragraph::new(text)
        .block(Block::bordered().title(" Help "))
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
