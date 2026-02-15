use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, RwLock};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_core::rag_types::ContextPart;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Style, Stylize as _};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Wrap};
use uuid::Uuid;

use crate::app::overlay::{OverlayAction, OverlayKind};
use crate::app::utils::truncate_uuid;
use crate::app::view::rendering::highlight::{
    StyledLine, StyledSpan, highlight_message_lines, styled_to_ratatui_lines,
};
use crate::app::view::widgets::expanding_list::{ExpandingItem, ExpandingList, ExpandingListState};
use crate::chat_history::{ContextTokens, Message, MessageKind, TokenKind};
use crate::context_plan::{ContextPlanHistory, ContextPlanSnapshot};
use crate::llm::manager::events::{ContextExclusionReason, ContextPlanMessage, ContextPlanRagPart};
use crate::ui_theme::UiTheme;
use unicode_width::UnicodeWidthChar;

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
    IncludedMessage {
        message_id: Option<Uuid>,
        index: usize,
    },
    ExcludedMessage {
        message_id: Uuid,
    },
    RagPart {
        part_id: Uuid,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContextPlanSectionKind {
    IncludedMessages,
    IncludedRag,
    ExcludedBudget,
    ExcludedTtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContextPlanHeaderKind {
    Plan,
    Section(ContextPlanSectionKind),
}

#[derive(Debug, Clone)]
enum ContextPlanRow {
    Header {
        title: String,
        kind: ContextPlanHeaderKind,
    },
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

#[derive(Debug, Clone)]
struct ContextPlanSection {
    kind: ContextPlanSectionKind,
    header_index: usize,
    first_item_index: usize,
    last_item_index: usize,
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
    sections: Vec<ContextPlanSection>,
    section_last_selection: HashMap<ContextPlanSectionKind, usize>,
    theme: UiTheme,
}

impl ContextPlanOverlayState {
    pub fn new(history: Arc<RwLock<ContextPlanHistory>>, theme: UiTheme) -> Self {
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
            sections: Vec::new(),
            section_last_selection: HashMap::new(),
            theme,
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
            KeyCode::Char('H') | KeyCode::Char('h') if shift => {
                self.step_history_prev();
            }
            KeyCode::Char('L') | KeyCode::Char('l') if shift => {
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
            KeyCode::Tab => {
                self.switch_section(true);
            }
            KeyCode::BackTab => {
                self.switch_section(false);
            }
            _ => {}
        }
        actions
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, state: &Arc<crate::app_state::AppState>) {
        let _ = state;
        let area = centered_overlay_area(frame.area(), 8, 8, 2);
        let footer_height = if self.help_visible { 5 } else { 3 };
        let body_height = area.height.saturating_sub(footer_height);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(body_height),
                Constraint::Length(footer_height),
            ])
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
            let current_tokens = state
                .chat
                .try_read()
                .ok()
                .and_then(|guard| guard.current_context_tokens);
            let summary_lines =
                build_token_summary_lines(&snapshot.plan, current_tokens, &self.theme);
            let mut summary_height = summary_lines.len() as u16;
            if summary_height >= inner.height {
                summary_height = inner.height.saturating_sub(1);
            }
            let (summary_area, list_area) = if summary_height > 0 {
                let layout = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([Constraint::Length(summary_height), Constraint::Min(1)])
                    .split(inner);
                (Some(layout[0]), layout[1])
            } else {
                (None, inner)
            };
            if let Some(area) = summary_area {
                let summary = Paragraph::new(summary_lines).wrap(Wrap { trim: true });
                frame.render_widget(summary, area);
            }
            let detail_width = list_area.width.saturating_sub(2);
            let (rows, sections) = build_rows(&snapshot, self.filter);
            self.rows = rows;
            self.sections = sections;
            self.clamp_selection();
            let focus_root = state
                .system
                .try_read()
                .ok()
                .and_then(|guard| guard.focused_crate_root());
            let message_previews = self.build_message_previews(state, detail_width);
            let items = build_display_items(
                &snapshot,
                &self.rows,
                &self.expanded,
                &self.snippet_visible,
                focus_root.as_deref(),
                list_area.width,
                detail_width,
                &message_previews,
                &self.theme,
            );
            frame.render_widget(block, body_area);
            let widget = ExpandingList {
                items: &items,
                normal_style: Style::default(),
                detail_style: Style::default(),
                selected_style: Style::default()
                    .bg(self.theme.context_plan_selected_bg)
                    .fg(self.theme.context_plan_selected_fg),
                selected_detail_style: Style::default(),
            };
            frame.render_stateful_widget(widget, list_area, &mut self.list_state);
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
        self.section_last_selection.clear();
    }

    fn select_prev(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let current = self
            .list_state
            .selected
            .min(self.rows.len().saturating_sub(1));
        if let Some(section_idx) = self.section_for_index(current) {
            let section = &self.sections[section_idx];
            if current <= section.first_item_index
                && let Some(prev_idx) = self.prev_section_index(section_idx)
            {
                let prev_section = &self.sections[prev_idx];
                let target = self
                    .section_last_selection
                    .get(&prev_section.kind)
                    .copied()
                    .unwrap_or(prev_section.last_item_index);
                self.list_state.selected = target;
                self.remember_section_selection(target);
                return;
            }
        }
        for idx in (0..current).rev() {
            if Self::is_selectable_row(&self.rows[idx]) {
                self.list_state.selected = idx;
                self.remember_section_selection(idx);
                return;
            }
        }
    }

    fn select_next(&mut self) {
        if self.rows.is_empty() {
            return;
        }
        let current = self
            .list_state
            .selected
            .min(self.rows.len().saturating_sub(1));
        if let Some(section_idx) = self.section_for_index(current) {
            let section = &self.sections[section_idx];
            if current >= section.last_item_index
                && let Some(next_idx) = self.next_section_index(section_idx)
            {
                let next_section = &self.sections[next_idx];
                let target = next_section.first_item_index;
                self.list_state.selected = target;
                self.remember_section_selection(target);
                return;
            }
        }
        let mut idx = current.saturating_add(1);
        while idx < self.rows.len() {
            if Self::is_selectable_row(&self.rows[idx]) {
                self.list_state.selected = idx;
                self.remember_section_selection(idx);
                return;
            }
            idx = idx.saturating_add(1);
        }
    }

    fn switch_section(&mut self, forward: bool) {
        if self.sections.is_empty() {
            return;
        }
        let current = self
            .list_state
            .selected
            .min(self.rows.len().saturating_sub(1));
        let section_idx = match self.section_for_index(current) {
            Some(idx) => idx,
            None => return,
        };
        let target_idx = if forward {
            self.next_section_index(section_idx)
        } else {
            self.prev_section_index(section_idx)
        };
        let Some(target_idx) = target_idx else {
            return;
        };
        let target_section = &self.sections[target_idx];
        let target = if forward {
            self.section_last_selection
                .get(&target_section.kind)
                .copied()
                .unwrap_or(target_section.first_item_index)
        } else {
            self.section_last_selection
                .get(&target_section.kind)
                .copied()
                .unwrap_or(target_section.last_item_index)
        };
        self.list_state.selected = target;
        self.remember_section_selection(target);
    }

    fn is_selectable_row(row: &ContextPlanRow) -> bool {
        matches!(
            row,
            ContextPlanRow::IncludedMessage { .. }
                | ContextPlanRow::ExcludedMessage { .. }
                | ContextPlanRow::RagPart { .. }
        )
    }

    fn clamp_selection(&mut self) {
        if self.rows.is_empty() {
            self.list_state.selected = 0;
            return;
        }
        if self.list_state.selected >= self.rows.len() {
            self.list_state.selected = self.rows.len().saturating_sub(1);
        }
        if !Self::is_selectable_row(&self.rows[self.list_state.selected]) {
            if let Some(first) = self.first_selectable_index() {
                self.list_state.selected = first;
            } else {
                self.list_state.selected = 0;
            }
        }
        self.remember_section_selection(self.list_state.selected);
    }

    fn first_selectable_index(&self) -> Option<usize> {
        self.rows
            .iter()
            .position(|row| Self::is_selectable_row(row))
    }

    fn section_for_index(&self, idx: usize) -> Option<usize> {
        self.sections
            .iter()
            .position(|section| idx >= section.first_item_index && idx <= section.last_item_index)
    }

    fn next_section_index(&self, idx: usize) -> Option<usize> {
        let next = idx.saturating_add(1);
        if next < self.sections.len() {
            Some(next)
        } else {
            None
        }
    }

    fn prev_section_index(&self, idx: usize) -> Option<usize> {
        if idx > 0 {
            Some(idx.saturating_sub(1))
        } else {
            None
        }
    }

    fn remember_section_selection(&mut self, idx: usize) {
        if let Some(section_idx) = self.section_for_index(idx) {
            let kind = self.sections[section_idx].kind;
            self.section_last_selection.insert(kind, idx);
        }
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
                self.expanded
                    .insert(ContextPlanItemKey::RagPart { part_id });
            }
        }
    }

    fn selected_item_key(&mut self) -> Option<ContextPlanItemKey> {
        if self.rows.is_empty() {
            return None;
        }
        let idx = self
            .list_state
            .selected
            .min(self.rows.len().saturating_sub(1));
        match self.rows.get(idx) {
            Some(ContextPlanRow::IncludedMessage { key, .. }) => Some(*key),
            Some(ContextPlanRow::ExcludedMessage { key, .. }) => Some(*key),
            Some(ContextPlanRow::RagPart { key, .. }) => Some(*key),
            _ => None,
        }
    }

    fn build_message_previews(
        &self,
        state: &Arc<crate::app_state::AppState>,
        detail_width: u16,
    ) -> HashMap<Uuid, String> {
        let available = detail_width
            .saturating_sub("    preview: ".len() as u16)
            .max(1) as usize;
        let mut ids = HashSet::new();
        for msg in &self.rows {
            match msg {
                ContextPlanRow::IncludedMessage { message, .. } => {
                    if let Some(id) = message.message_id {
                        ids.insert(id);
                    }
                }
                ContextPlanRow::ExcludedMessage { message_id, .. } => {
                    ids.insert(*message_id);
                }
                _ => {}
            }
        }
        let Ok(guard) = state.chat.try_read() else {
            return HashMap::new();
        };
        let mut previews = HashMap::new();
        for id in ids {
            if let Some(message) = guard.messages.get(&id) {
                previews.insert(id, format_preview_line(message, available));
            }
        }
        previews
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

fn build_rows(
    snapshot: &ContextPlanSnapshot,
    filter: ContextPlanFilter,
) -> (Vec<ContextPlanRow>, Vec<ContextPlanSection>) {
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
    let mut sections = Vec::new();
    rows.push(ContextPlanRow::Header {
        title: format!(
            "Plan {} (parent {}) — est {} tokens",
            truncate_uuid(plan.plan_id),
            truncate_uuid(plan.parent_id),
            plan.estimated_total_tokens
        ),
        kind: ContextPlanHeaderKind::Plan,
    });

    let include_included = matches!(filter, ContextPlanFilter::All);
    let include_excluded_all = matches!(
        filter,
        ContextPlanFilter::All | ContextPlanFilter::ExcludedAll
    );
    let include_excluded_budget = matches!(
        filter,
        ContextPlanFilter::ExcludedBudget | ContextPlanFilter::ExcludedAll
    );
    let include_excluded_ttl = matches!(
        filter,
        ContextPlanFilter::ExcludedTtlExpired | ContextPlanFilter::ExcludedAll
    );
    let include_rag = matches!(filter, ContextPlanFilter::All);

    if include_included && !plan.included_messages.is_empty() {
        let percent = included_msg_tokens as f32 / denom * 100.0;
        let header_index = rows.len();
        rows.push(ContextPlanRow::Header {
            title: format!(
                "Included messages — {} items, {} tok ({:.1}%)",
                plan.included_messages.len(),
                included_msg_tokens,
                percent
            ),
            kind: ContextPlanHeaderKind::Section(ContextPlanSectionKind::IncludedMessages),
        });
        let first_item_index = rows.len();
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
        if rows.len() > first_item_index {
            sections.push(ContextPlanSection {
                kind: ContextPlanSectionKind::IncludedMessages,
                header_index,
                first_item_index,
                last_item_index: rows.len().saturating_sub(1),
            });
        }
    }

    if include_rag && !plan.included_rag_parts.is_empty() {
        let percent = included_rag_tokens as f32 / denom * 100.0;
        let header_index = rows.len();
        rows.push(ContextPlanRow::Header {
            title: format!(
                "Included RAG parts — {} items, {} tok ({:.1}%)",
                plan.included_rag_parts.len(),
                included_rag_tokens,
                percent
            ),
            kind: ContextPlanHeaderKind::Section(ContextPlanSectionKind::IncludedRag),
        });
        let first_item_index = rows.len();
        for part in &plan.included_rag_parts {
            let key = ContextPlanItemKey::RagPart {
                part_id: part.part_id,
            };
            rows.push(ContextPlanRow::RagPart {
                key,
                part: part.clone(),
            });
        }
        if rows.len() > first_item_index {
            sections.push(ContextPlanSection {
                kind: ContextPlanSectionKind::IncludedRag,
                header_index,
                first_item_index,
                last_item_index: rows.len().saturating_sub(1),
            });
        }
    }

    if include_excluded_all {
        let excluded_budget_count = plan
            .excluded_messages
            .iter()
            .filter(|m| m.reason == ContextExclusionReason::Budget)
            .count();
        if include_excluded_budget && excluded_budget_count > 0 {
            let percent = excluded_budget_tokens as f32 / denom * 100.0;
            let header_index = rows.len();
            rows.push(ContextPlanRow::Header {
                title: format!(
                    "Excluded (Budget) — {} items, {} tok ({:.1}%)",
                    excluded_budget_count, excluded_budget_tokens, percent
                ),
                kind: ContextPlanHeaderKind::Section(ContextPlanSectionKind::ExcludedBudget),
            });
            let first_item_index = rows.len();
            for message in plan
                .excluded_messages
                .iter()
                .filter(|m| m.reason == ContextExclusionReason::Budget)
            {
                let key = ContextPlanItemKey::ExcludedMessage {
                    message_id: message.message_id,
                };
                rows.push(ContextPlanRow::ExcludedMessage {
                    key,
                    message_id: message.message_id,
                    kind: message.kind,
                    estimated_tokens: message.estimated_tokens,
                    reason: message.reason.clone(),
                });
            }
            if rows.len() > first_item_index {
                sections.push(ContextPlanSection {
                    kind: ContextPlanSectionKind::ExcludedBudget,
                    header_index,
                    first_item_index,
                    last_item_index: rows.len().saturating_sub(1),
                });
            }
        }

        let excluded_ttl_count = plan
            .excluded_messages
            .iter()
            .filter(|m| m.reason == ContextExclusionReason::TtlExpired)
            .count();
        if include_excluded_ttl && excluded_ttl_count > 0 {
            let percent = excluded_ttl_tokens as f32 / denom * 100.0;
            let header_index = rows.len();
            rows.push(ContextPlanRow::Header {
                title: format!(
                    "Excluded (TTL) — {} items, {} tok ({:.1}%)",
                    excluded_ttl_count, excluded_ttl_tokens, percent
                ),
                kind: ContextPlanHeaderKind::Section(ContextPlanSectionKind::ExcludedTtl),
            });
            let first_item_index = rows.len();
            for message in plan
                .excluded_messages
                .iter()
                .filter(|m| m.reason == ContextExclusionReason::TtlExpired)
            {
                let key = ContextPlanItemKey::ExcludedMessage {
                    message_id: message.message_id,
                };
                rows.push(ContextPlanRow::ExcludedMessage {
                    key,
                    message_id: message.message_id,
                    kind: message.kind,
                    estimated_tokens: message.estimated_tokens,
                    reason: message.reason.clone(),
                });
            }
            if rows.len() > first_item_index {
                sections.push(ContextPlanSection {
                    kind: ContextPlanSectionKind::ExcludedTtl,
                    header_index,
                    first_item_index,
                    last_item_index: rows.len().saturating_sub(1),
                });
            }
        }
    }

    (rows, sections)
}

fn build_display_items(
    snapshot: &ContextPlanSnapshot,
    rows: &[ContextPlanRow],
    expanded: &HashSet<ContextPlanItemKey>,
    snippet_visible: &HashSet<Uuid>,
    focus_root: Option<&Path>,
    title_width: u16,
    detail_width: u16,
    message_previews: &HashMap<Uuid, String>,
    theme: &UiTheme,
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
            ContextPlanRow::Header { title, kind } => {
                let header_style = match kind {
                    ContextPlanHeaderKind::Plan => {
                        Style::default().fg(theme.context_plan_header_fg)
                    }
                    ContextPlanHeaderKind::Section(_) => {
                        Style::default().fg(theme.context_plan_section_fg)
                    }
                }
                .bold();
                items.push(ContextPlanDisplayItem {
                    title: Line::from(Span::styled(title.clone(), header_style)),
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
                    if let Some(message_id) = message.message_id {
                        if let Some(preview) = message_previews.get(&message_id) {
                            details.push(Line::from(format!("    preview: {}", preview)));
                        }
                    }
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
                    details.push(Line::from(format!(
                        "    message_id: {}",
                        truncate_uuid(*message_id)
                    )));
                    details.push(Line::from(format!("    kind: {}", kind)));
                    details.push(Line::from(format!(
                        "    estimated_tokens: {}",
                        estimated_tokens
                    )));
                    details.push(Line::from(format!("    reason: {:?}", reason)));
                    if let Some(preview) = message_previews.get(message_id) {
                        details.push(Line::from(format!("    preview: {}", preview)));
                    }
                }
                items.push(ContextPlanDisplayItem {
                    title: Line::from(title),
                    details,
                    expanded,
                });
            }
            ContextPlanRow::RagPart { key, part } => {
                let expanded = expanded.contains(key) || snippet_visible.contains(&part.part_id);
                let show_snippet_gutter = snippet_visible.contains(&part.part_id);
                let display_path = display_relative_path(&part.file_path, focus_root);
                let suffix = format!(
                    " ({}, score {:.3}) — ~{} tok",
                    part.kind.to_static_str(),
                    part.score,
                    part.estimated_tokens
                );
                let title_path =
                    truncate_path_start(&display_path, title_width as usize, "  [rag] ", &suffix);
                let title = format!("  [rag] {}{}", title_path, suffix);
                let mut details = Vec::new();
                if expanded {
                    details.push(Line::from(format!(
                        "    part_id: {}",
                        truncate_uuid(part.part_id)
                    )));
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
                            let mut snippet_lines = highlight_snippet_lines(
                                ctx_part,
                                detail_width,
                                show_snippet_gutter,
                                theme,
                            );
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

fn build_token_summary_lines(
    plan: &crate::llm::manager::events::ContextPlan,
    current_tokens: Option<ContextTokens>,
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let est_style = Style::default().fg(theme.context_plan_token_est_fg);
    let actual_style = Style::default().fg(theme.context_plan_token_actual_fg);

    if let Some(tokens) = current_tokens {
        let (label, style) = match tokens.kind {
            TokenKind::Estimated => ("est", est_style),
            TokenKind::Actual => ("actual", actual_style),
        };
        lines.push(Line::from(vec![
            Span::raw("Context tokens: "),
            Span::styled(format!("{label} {}", tokens.count), style.bold()),
        ]));
    }

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

    lines.push(Line::from(vec![
        Span::raw("Plan est: total "),
        Span::styled(format!("{}", plan.estimated_total_tokens), est_style),
        Span::raw(" (messages "),
        Span::styled(format!("{}", included_msg_tokens), est_style),
        Span::raw(", RAG "),
        Span::styled(format!("{}", included_rag_tokens), est_style),
        Span::raw(")"),
    ]));

    if excluded_budget_tokens > 0 || excluded_ttl_tokens > 0 {
        lines.push(Line::from(vec![
            Span::raw("Excluded est: budget "),
            Span::styled(format!("{}", excluded_budget_tokens), est_style),
            Span::raw(", ttl "),
            Span::styled(format!("{}", excluded_ttl_tokens), est_style),
        ]));
    }

    let mut user_tokens = 0usize;
    let mut assistant_tokens = 0usize;
    let mut tool_tokens = 0usize;
    let mut system_tokens = 0usize;
    let mut sysinfo_tokens = 0usize;
    let mut tool_call_tokens = 0usize;
    for msg in &plan.included_messages {
        match msg.kind {
            MessageKind::User => user_tokens = user_tokens.saturating_add(msg.estimated_tokens),
            MessageKind::Assistant => {
                if msg.message_id.is_none() {
                    tool_call_tokens = tool_call_tokens.saturating_add(msg.estimated_tokens);
                } else {
                    assistant_tokens = assistant_tokens.saturating_add(msg.estimated_tokens);
                }
            }
            MessageKind::Tool => tool_tokens = tool_tokens.saturating_add(msg.estimated_tokens),
            MessageKind::System => {
                system_tokens = system_tokens.saturating_add(msg.estimated_tokens)
            }
            MessageKind::SysInfo => {
                sysinfo_tokens = sysinfo_tokens.saturating_add(msg.estimated_tokens)
            }
        }
    }
    lines.push(Line::from(vec![
        Span::raw("Messages est: user "),
        Span::styled(format!("{}", user_tokens), est_style),
        Span::raw(", assistant "),
        Span::styled(format!("{}", assistant_tokens), est_style),
        Span::raw(", tool-call "),
        Span::styled(format!("{}", tool_call_tokens), est_style),
        Span::raw(", tool "),
        Span::styled(format!("{}", tool_tokens), est_style),
        Span::raw(", system "),
        Span::styled(format!("{}", system_tokens), est_style),
        Span::raw(", sysinfo "),
        Span::styled(format!("{}", sysinfo_tokens), est_style),
    ]));

    lines
}

fn highlight_snippet_lines(
    part: &ContextPart,
    detail_width: u16,
    show_gutter: bool,
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    const SNIPPET_MAX_LINES: usize = 16;
    let indent = "    ";
    let gutter_width = 2usize;
    let lang = Path::new(part.file_path.as_ref())
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("text");
    let (snippet_text, _) = truncate_snippet_text(part.text.as_str(), SNIPPET_MAX_LINES);
    let mut fenced = String::new();
    fenced.push_str("```");
    fenced.push_str(lang);
    fenced.push('\n');
    fenced.push_str(&snippet_text);
    if !snippet_text.ends_with('\n') {
        fenced.push('\n');
    }
    fenced.push_str("```");

    let width = detail_width
        .saturating_sub((indent.len() + gutter_width) as u16)
        .max(1) as usize;
    let highlighted = highlight_message_lines(&fenced, Style::default(), u16::MAX);
    let wrapped = wrap_styled_lines_on_words(highlighted, width);
    let mut lines = styled_to_ratatui_lines(wrapped);
    trim_trailing_empty_lines(&mut lines);
    prefix_snippet_lines(lines, indent, show_gutter, theme)
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
    if available < 3 {
        return String::new();
    }
    if path.len() <= available {
        return path.to_string();
    }
    let keep = available.saturating_sub(3);
    let start = path.len().saturating_sub(keep);
    format!("...{}", &path[start..])
}

fn prefix_snippet_lines(
    lines: Vec<Line<'static>>,
    indent: &str,
    show_gutter: bool,
    theme: &UiTheme,
) -> Vec<Line<'static>> {
    let gutter_style = Style::default().fg(theme.context_plan_snippet_gutter_fg);
    lines
        .into_iter()
        .map(|line| {
            let mut spans = Vec::with_capacity(line.spans.len() + 3);
            spans.push(Span::raw(indent.to_string()));
            if show_gutter {
                spans.push(Span::styled("│", gutter_style));
                spans.push(Span::raw(" "));
            } else {
                spans.push(Span::raw("  "));
            }
            spans.extend(line.spans.into_iter());
            let mut out = Line::from(spans);
            out.style = line.style;
            out
        })
        .collect()
}

fn truncate_snippet_text(text: &str, max_lines: usize) -> (String, bool) {
    let mut out = String::new();
    let mut truncated = false;
    for (idx, line) in text.lines().enumerate() {
        if idx >= max_lines {
            truncated = true;
            break;
        }
        if idx > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    if truncated {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str("... [truncated]");
    }
    (out, truncated)
}

fn trim_trailing_empty_lines(lines: &mut Vec<Line<'static>>) {
    while let Some(last) = lines.last() {
        let is_empty = last.spans.iter().all(|span| span.content.is_empty());
        if is_empty {
            lines.pop();
        } else {
            break;
        }
    }
}

fn wrap_styled_lines_on_words(lines: Vec<StyledLine>, width: usize) -> Vec<StyledLine> {
    let width = width.max(1);
    let mut wrapped = Vec::new();
    for line in lines {
        wrapped.extend(wrap_styled_line_on_words(&line, width));
    }
    if wrapped.is_empty() {
        wrapped.push(Vec::new());
    }
    wrapped
}

fn wrap_styled_line_on_words(line: &StyledLine, width: usize) -> Vec<StyledLine> {
    let mut out = Vec::new();
    let mut current: StyledLine = Vec::new();
    let mut current_width = 0usize;
    let mut tokens = Vec::new();
    for span in line {
        split_span_tokens(span, &mut tokens);
    }

    for token in tokens {
        let token_width = string_width(&token.content);
        if current_width > 0 && current_width + token_width > width {
            out.push(std::mem::take(&mut current));
            current_width = 0;
        }
        if token_width > width && current_width == 0 {
            let mut remaining = token.content.as_str();
            while !remaining.is_empty() {
                let (take_bytes, take_width) = take_prefix_by_width(remaining, width);
                if take_bytes == 0 {
                    break;
                }
                let chunk = &remaining[..take_bytes];
                current.push(StyledSpan {
                    content: chunk.to_string(),
                    style: token.style,
                });
                remaining = &remaining[take_bytes..];
                out.push(std::mem::take(&mut current));
            }
            continue;
        }
        if token.content.chars().all(char::is_whitespace) && current_width == 0 {
            current.push(StyledSpan {
                content: token.content,
                style: token.style,
            });
            current_width += token_width;
            continue;
        }
        current.push(StyledSpan {
            content: token.content,
            style: token.style,
        });
        current_width += token_width;
    }

    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(Vec::new());
    }
    out
}

fn split_span_tokens(span: &StyledSpan, out: &mut Vec<StyledSpan>) {
    let mut buf = String::new();
    let mut in_ws: Option<bool> = None;
    for ch in span.content.chars() {
        let is_ws = ch.is_whitespace();
        if in_ws == Some(is_ws) {
            buf.push(ch);
        } else {
            if !buf.is_empty() {
                out.push(StyledSpan {
                    content: std::mem::take(&mut buf),
                    style: span.style,
                });
            }
            buf.push(ch);
            in_ws = Some(is_ws);
        }
    }
    if !buf.is_empty() {
        out.push(StyledSpan {
            content: buf,
            style: span.style,
        });
    }
}

fn string_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

fn take_prefix_by_width(s: &str, max_width: usize) -> (usize, usize) {
    if max_width == 0 {
        return (0, 0);
    }
    let mut accum_width = 0usize;
    let mut byte_idx = 0usize;
    for (idx, ch) in s.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_width == 0 && accum_width == 0 {
            let len = ch.len_utf8();
            return (len, 0);
        }
        if accum_width + ch_width > max_width {
            if accum_width == 0 {
                let len = ch.len_utf8();
                return (len, ch_width);
            }
            break;
        }
        accum_width += ch_width;
        byte_idx = idx + ch.len_utf8();
    }
    if byte_idx == 0 {
        byte_idx = s.len();
    }
    (byte_idx, accum_width)
}

fn format_preview_line(message: &Message, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    let prefix = match message.kind {
        MessageKind::User => "User:",
        MessageKind::Assistant => "Assistant:",
        MessageKind::System => "System:",
        MessageKind::SysInfo => "SysInfo:",
        MessageKind::Tool => {
            if let Some(payload) = message.tool_payload.as_ref() {
                return truncate_preview(
                    &format!(
                        "Tool:{:?}: {}",
                        payload.tool,
                        sanitize_preview_text(&message.content)
                    ),
                    max_width,
                );
            }
            "Tool:"
        }
    };
    let combined = format!("{} {}", prefix, sanitize_preview_text(&message.content));
    truncate_preview(&combined, max_width)
}

fn sanitize_preview_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_preview(input: &str, max_width: usize) -> String {
    if max_width == 0 {
        return String::new();
    }
    input.chars().take(max_width).collect()
}

fn centered_overlay_area(area: Rect, width_ratio: u16, height_ratio: u16, v_margin: u16) -> Rect {
    let w = area.width.saturating_mul(width_ratio) / 10;
    let mut h = area.height.saturating_mul(height_ratio) / 10;
    let max_h = area
        .height
        .saturating_sub(v_margin.saturating_mul(2))
        .max(1);
    h = h.min(max_h);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 2;
    Rect::new(x, y, w, h)
}

fn render_context_plan_footer(frame: &mut Frame<'_>, area: Rect, help_visible: bool) {
    let text = if help_visible {
        "Keys: j/k or ↑/↓=navigate  Enter/Space=toggle  h/l=collapse/expand  s=toggle snippet\n\
         Sections: Tab/Shift+Tab=jump  History: Shift+H/L or Shift+←/→  Filter: f\n\
         Token colors: est=estimated, actual=provider usage  ?=help  q/Esc=close"
            .to_string()
    } else {
        " ? Help ".to_string()
    };
    let widget = Paragraph::new(text)
        .block(Block::bordered().title(" Help "))
        .wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}
