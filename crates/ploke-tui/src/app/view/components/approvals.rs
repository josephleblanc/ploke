use std::sync::Arc;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};
use similar::{ChangeTag, TextDiff};

use crate::app::view::components::overlay_widgets;
use crate::app::view::rendering::highlight::{
    StyledLine, StyledSpan, highlight_diff_text, styled_to_ratatui_lines,
};
use crate::app_state::AppState as _;
use crate::app_state::core::{DiffPreview, EditProposalStatus};
use crate::{app::view::components::context_browser::StepEnum, app_state::core::BeforeAfter}; // trait bounds

#[derive(Debug, Clone, Default)]
pub struct ApprovalsState {
    pub selected: usize,
    pub help_visible: bool,
    /// Vertical scroll offset (in wrapped display lines) for the details pane.
    pub scroll_y: u16,
    pub view_lines: usize, // Number of lines to show in details view (0 = unlimited)
    pub filter: ApprovalsFilter,
    pub diff_view: DiffViewMode,
    diff_cache: DiffPreviewCache,
}

pub struct ApprovalsView<'a> {
    pub items: &'a [ApprovalListItem],
    pub selected: usize,
    pub help_visible: bool,
    pub view_lines: usize,
    pub filter: ApprovalsFilter,
    pub diff_view: DiffViewMode,
}

impl<'a> ApprovalsView<'a> {
    pub fn new(items: &'a [ApprovalListItem], ui: &ApprovalsState) -> Self {
        Self {
            items,
            selected: ui.selected,
            help_visible: ui.help_visible,
            view_lines: ui.view_lines,
            filter: ui.filter,
            diff_view: ui.diff_view,
        }
    }
}

impl ApprovalsState {
    pub fn select_next(&mut self) {
        self.selected = self.selected.saturating_add(1);
        self.scroll_y = 0;
    }
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
        self.scroll_y = 0;
    }

    pub fn scroll_up(&mut self, n: u16) {
        self.scroll_y = self.scroll_y.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: u16) {
        self.scroll_y = self.scroll_y.saturating_add(n);
    }

    pub fn increase_view_lines(&mut self) {
        if self.view_lines == 0 {
            self.view_lines = 10; // Start with 10 lines when first enabled
        } else {
            self.view_lines = (self.view_lines + 10).min(200); // Cap at 200 lines
        }
        self.scroll_y = 0;
        self.diff_cache.clear();
    }

    pub fn decrease_view_lines(&mut self) {
        if self.view_lines <= 10 {
            self.view_lines = 0; // 0 means no truncation
        } else {
            self.view_lines = self.view_lines.saturating_sub(10);
        }
        self.scroll_y = 0;
        self.diff_cache.clear();
    }

    pub fn toggle_unlimited(&mut self) {
        self.view_lines = if self.view_lines == 0 { 20 } else { 0 };
        self.scroll_y = 0;
        self.diff_cache.clear();
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next_wrap();
        self.selected = 0; // Reset selection to keep in-bounds on new list
        self.scroll_y = 0;
        self.diff_cache.clear();
    }

    pub fn toggle_diff_view(&mut self) {
        self.diff_view = self.diff_view.next();
        self.scroll_y = 0;
        self.diff_cache.clear();
    }

    fn diff_chunks<'a>(
        &'a mut self,
        selected_id: uuid::Uuid,
        preview: &DiffPreview,
    ) -> &'a [String] {
        let context_lines = match self.diff_view {
            DiffViewMode::Expanded => {
                if self.view_lines == 0 {
                    6
                } else {
                    // Heuristic: more allowed lines -> more surrounding context.
                    (self.view_lines / 6).clamp(3, 12)
                }
            }
            _ => 0,
        };

        if !self
            .diff_cache
            .matches(selected_id, self.diff_view, context_lines)
        {
            self.diff_cache
                .rebuild(selected_id, self.diff_view, context_lines, preview);
        }
        &self.diff_cache.chunks
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DiffViewMode {
    /// Only changed lines (add/remove), plus headers.
    #[default]
    Minimal,
    /// Changed lines plus surrounding context lines.
    Expanded,
    /// Full diff output.
    Full,
}

impl DiffViewMode {
    fn next(self) -> Self {
        match self {
            DiffViewMode::Minimal => DiffViewMode::Expanded,
            DiffViewMode::Expanded => DiffViewMode::Full,
            DiffViewMode::Full => DiffViewMode::Minimal,
        }
    }

    fn label(self) -> &'static str {
        match self {
            DiffViewMode::Minimal => "minimal",
            DiffViewMode::Expanded => "expanded",
            DiffViewMode::Full => "full",
        }
    }
}

#[derive(Debug, Default, Clone)]
struct DiffPreviewCache {
    selected_id: Option<uuid::Uuid>,
    mode: DiffViewMode,
    context_lines: usize,
    chunks: Vec<String>,
}

impl DiffPreviewCache {
    fn matches(&self, selected_id: uuid::Uuid, mode: DiffViewMode, context_lines: usize) -> bool {
        self.selected_id == Some(selected_id)
            && self.mode == mode
            && self.context_lines == context_lines
    }

    fn clear(&mut self) {
        self.selected_id = None;
        self.context_lines = 0;
        self.chunks.clear();
    }

    fn rebuild(
        &mut self,
        selected_id: uuid::Uuid,
        mode: DiffViewMode,
        context_lines: usize,
        preview: &DiffPreview,
    ) {
        self.selected_id = Some(selected_id);
        self.mode = mode;
        self.context_lines = context_lines;
        self.chunks = diff_preview_chunks(preview, mode, context_lines);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalKind {
    Edit,
    Create,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalsFilter {
    /// Default: show Pending + Failed + Stale (things needing attention).
    #[default]
    PendingOrErrored,
    PendingOnly,
    ApprovedApplied,
    FailedOnly,
    StaleOnly,
    All,
}

impl ApprovalsFilter {
    fn matches_status(self, status: &EditProposalStatus) -> bool {
        use ApprovalsFilter::*;
        match self {
            PendingOrErrored => matches!(
                status,
                EditProposalStatus::Pending
                    | EditProposalStatus::Failed(_)
                    | EditProposalStatus::Stale(_)
            ),
            PendingOnly => matches!(status, EditProposalStatus::Pending),
            ApprovedApplied => matches!(
                status,
                EditProposalStatus::Approved | EditProposalStatus::Applied
            ),
            FailedOnly => matches!(status, EditProposalStatus::Failed(_)),
            StaleOnly => matches!(status, EditProposalStatus::Stale(_)),
            All => true,
        }
    }

    pub fn next_wrap(self) -> Self {
        let i = self.idx();
        Self::ORDER[(i + 1) % Self::ORDER.len()]
    }

    fn label(self) -> &'static str {
        match self {
            ApprovalsFilter::PendingOrErrored => "pending+errored",
            ApprovalsFilter::PendingOnly => "pending",
            ApprovalsFilter::ApprovedApplied => "approved/applied",
            ApprovalsFilter::FailedOnly => "errored",
            ApprovalsFilter::StaleOnly => "stale",
            ApprovalsFilter::All => "all",
        }
    }
}

impl StepEnum<6> for ApprovalsFilter {
    const ORDER: [Self; 6] = [
        ApprovalsFilter::PendingOrErrored,
        ApprovalsFilter::PendingOnly,
        ApprovalsFilter::ApprovedApplied,
        ApprovalsFilter::FailedOnly,
        ApprovalsFilter::StaleOnly,
        ApprovalsFilter::All,
    ];

    fn idx(self) -> usize {
        Self::ORDER.iter().position(|v| v == &self).unwrap_or(0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalListItem {
    pub kind: ProposalKind,
    pub id: uuid::Uuid,
    pub status: EditProposalStatus,
    pub label: String,
    pub proposed_at_ms: i64,
}

fn status_rank(status: &EditProposalStatus) -> usize {
    match status {
        EditProposalStatus::Pending => 0,
        EditProposalStatus::Failed(_) => 1,
        EditProposalStatus::Stale(_) => 2,
        EditProposalStatus::Approved | EditProposalStatus::Applied => 3,
        EditProposalStatus::Denied => 4,
    }
}

fn status_style(status: &EditProposalStatus) -> Style {
    match status {
        EditProposalStatus::Pending => Style::new().fg(Color::Cyan),
        EditProposalStatus::Failed(_) => Style::new().fg(Color::Red),
        EditProposalStatus::Stale(_) => Style::new().fg(Color::Yellow),
        EditProposalStatus::Approved | EditProposalStatus::Applied => {
            Style::new().fg(Color::DarkGray)
        }
        EditProposalStatus::Denied => Style::new().fg(Color::Gray),
    }
}

/// Build a unified list of proposal items (edits + creates) with display strings.
/// Items are sorted by status priority then recency.
pub fn filtered_items(
    state: &Arc<crate::app_state::AppState>,
    filter: ApprovalsFilter,
) -> Vec<ApprovalListItem> {
    // Read both registries within a single block_in_place scope (non-blocking for async executors)
    let (proposals_guard, create_guard) = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            let p = state.proposals.read().await;
            let c = state.create_proposals.read().await;
            (p, c)
        })
    });

    let mut items: Vec<ApprovalListItem> = Vec::new();

    for (id, p) in proposals_guard.iter() {
        if filter.matches_status(&p.status) {
            items.push(ApprovalListItem {
                kind: ProposalKind::Edit,
                id: *id,
                status: p.status.clone(),
                label: format!(
                    "[E] {}  {:<7}  files:{}",
                    crate::app::utils::truncate_uuid(*id),
                    &p.status.as_str_outer(),
                    p.files.len()
                ),
                proposed_at_ms: p.proposed_at_ms,
            });
        }
    }
    for (id, p) in create_guard.iter() {
        if filter.matches_status(&p.status) {
            items.push(ApprovalListItem {
                kind: ProposalKind::Create,
                id: *id,
                status: p.status.clone(),
                label: format!(
                    "[C] {}  {:<7}  files:{}",
                    crate::app::utils::truncate_uuid(*id),
                    &p.status.as_str_outer(),
                    p.files.len()
                ),
                proposed_at_ms: p.proposed_at_ms,
            });
        }
    }

    items.sort_by(|a, b| {
        let (rank_a, rank_b) = match filter {
            // Default and specific filters: primarily recency
            ApprovalsFilter::PendingOrErrored
            | ApprovalsFilter::PendingOnly
            | ApprovalsFilter::ApprovedApplied
            | ApprovalsFilter::FailedOnly
            | ApprovalsFilter::StaleOnly => (0, 0),
            // When showing everything, group by status priority then recency.
            ApprovalsFilter::All => (status_rank(&a.status), status_rank(&b.status)),
        };
        rank_a
            .cmp(&rank_b)
            .then(b.proposed_at_ms.cmp(&a.proposed_at_ms))
            .then(a.id.cmp(&b.id))
    });
    items
}

pub fn render_approvals_overlay(
    frame: &mut Frame,
    area: Rect,
    state: &Arc<crate::app_state::AppState>,
    ui: &mut ApprovalsState,
) -> Option<uuid::Uuid> {
    // Clear the underlying content in the overlay area to avoid "bleed-through"
    frame.render_widget(ratatui::widgets::Clear, area);

    let outer = Block::bordered().title(" Approvals ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Build unified list across edits and creates with filtering
    let items: Vec<ApprovalListItem> = filtered_items(state, ui.filter);
    let view = ApprovalsView::new(&items, ui);
    let selected_idx = view.selected.min(view.items.len().saturating_sub(1));

    // Split overlay into body + footer (help)
    let footer_height = if view.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(inner);
    let body_area = layout[0];
    let footer_area = layout[1];

    // Split body into list and details
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(body_area);

    let list_items: Vec<ListItem> = view
        .items
        .iter()
        .map(|item| ListItem::new(item.label.clone()).style(status_style(&item.status)))
        .collect();
    let list = List::new(list_items)
        .block(Block::bordered().title(" Pending Proposals "))
        .highlight_style(Style::new().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("▶ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    let mut list_state = ListState::default();
    if !view.items.is_empty() {
        list_state.select(Some(selected_idx));
    }
    frame.render_stateful_widget(list, cols[0], &mut list_state);

    // Details
    let selected = view
        .items
        .get(selected_idx)
        .map(|item| (item.kind, item.id));
    let mut detail_lines: Vec<Line<'static>> = Vec::new();
    let detail_width = cols[1].width.saturating_sub(2).max(1);
    if let Some((sel_kind, sel_id)) = selected {
        // Use the established pattern for accessing async data from sync context
        let (proposals_guard, create_guard) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let p = state.proposals.write().await;
                let c = state.create_proposals.read().await;
                (p, c)
            })
        });
        let context_lines = match view.diff_view {
            DiffViewMode::Expanded => {
                if view.view_lines == 0 {
                    6
                } else {
                    // Heuristic: more allowed lines -> more surrounding context.
                    (view.view_lines / 6).clamp(3, 12)
                }
            }
            _ => 0,
        };

        let mut render_preview =
            |status: &EditProposalStatus, files_len: usize, preview: &DiffPreview| {
                detail_lines.push(Line::from(vec![Span::styled(
                    format!("request_id: {}", sel_id),
                    Style::new().fg(Color::Yellow),
                )]));
                detail_lines.push(Line::from(format!(
                    "status: {:?}  files:{}",
                    status, files_len
                )));

                // Determine line limit for display
                let line_limit = if view.view_lines == 0 {
                    usize::MAX
                } else {
                    view.view_lines
                };
                let mut rendered_preview_lines = 0usize;

                let chunks = diff_preview_chunks(preview, view.diff_view, context_lines);

                match preview {
                    DiffPreview::UnifiedDiff { text: _text } => {
                        detail_lines.push(Line::from(vec![Span::styled(
                            "Unified Diff:",
                            Style::new().fg(Color::Green),
                        )]));

                        for chunk in chunks.iter().take(1) {
                            if push_highlighted_with_limit(
                                &mut detail_lines,
                                highlight_diff_text(chunk.trim_end_matches('\n'), detail_width),
                                &mut rendered_preview_lines,
                                line_limit,
                            ) {
                                detail_lines.push(truncation_line(line_limit));
                                break;
                            }
                        }
                    }
                    DiffPreview::CodeBlocks { per_file: _ } => {
                        detail_lines.push(Line::from(vec![Span::styled(
                            "Before/After:",
                            Style::new().fg(Color::Green),
                        )]));

                        for chunk in chunks.iter().take(2) {
                            if rendered_preview_lines >= line_limit {
                                detail_lines.push(truncation_line(line_limit));
                                break;
                            }
                            if push_highlighted_with_limit(
                                &mut detail_lines,
                                highlight_diff_text(chunk.trim_end_matches('\n'), detail_width),
                                &mut rendered_preview_lines,
                                line_limit,
                            ) {
                                detail_lines.push(truncation_line(line_limit));
                                break;
                            }
                        }
                    }
                }
            };

        match sel_kind {
            ProposalKind::Edit => {
                if let Some(p) = proposals_guard.get(&sel_id) {
                    render_preview(&p.status, p.files.len(), &p.preview);
                }
            }
            ProposalKind::Create => {
                if let Some(p) = create_guard.get(&sel_id) {
                    render_preview(&p.status, p.files.len(), &p.preview);
                }
            }
        }
    }
    let viewport_height = cols[1].height.saturating_sub(2) as usize; // bordered block inner height
    let max_scroll = detail_lines.len().saturating_sub(viewport_height);
    let max_scroll_u16 = u16::try_from(max_scroll).unwrap_or(u16::MAX);
    let scroll_y = ui.scroll_y.min(max_scroll_u16);
    ui.scroll_y = scroll_y;

    overlay_widgets::render_diff_preview_scrolled(
        frame,
        cols[1],
        " Details ",
        detail_lines,
        scroll_y,
    );

    // Render help footer with truncation status
    let overlay_style = Style::new().fg(Color::LightBlue);
    if view.help_visible {
        let truncation_status = if view.view_lines == 0 {
            "unlimited".to_string()
        } else {
            format!("{} lines", view.view_lines)
        };

        let help_text = format!(
            "Keys: Enter=approve  n=deny  o=open in editor  ↑/↓,j/k=navigate  f=cycle filter  q/Esc=close\n\
             View: +=more lines  -=fewer lines  u=toggle unlimited (current: {})  v=diff view ({})\n\
             Filter: current={} (f to cycle)  Diff: {} (v to toggle)\n\
             Commands:\n\
             - Enter: Approve selected proposal\n\
             - n: Deny selected proposal\n\
             - o: Open files in configured editor\n\
             - +: Show more lines in preview (current: {})\n\
             - -: Show fewer lines in preview\n\
             - u: Toggle unlimited view\n\
             - f: Cycle filter\n\
             - v: Toggle diff view (minimal/expanded/full)\n\
             - q/Esc: Close approvals overlay",
            truncation_status,
            view.diff_view.label(),
            view.filter.label(),
            view.diff_view.label(),
            truncation_status
        );

        let help = Paragraph::new(help_text)
            .style(overlay_style)
            .block(Block::bordered().title(" Help ").style(overlay_style));
        frame.render_widget(help, footer_area);
    } else {
        let truncation_info = if view.view_lines == 0 {
            "unlimited".to_string()
        } else {
            format!("{} lines", view.view_lines)
        };

        let hint = Paragraph::new(format!(
            " ? Help | View: {} | Filter: {} | Diff: {} ",
            truncation_info,
            view.filter.label(),
            view.diff_view.label()
        ))
        .style(overlay_style)
        .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(hint, footer_area);
    }

    selected.map(|(_, id)| id)
}

fn diff_preview_chunks(
    preview: &DiffPreview,
    mode: DiffViewMode,
    context_lines: usize,
) -> Vec<String> {
    match preview {
        DiffPreview::UnifiedDiff { text } => match mode {
            DiffViewMode::Full => vec![text.clone()],
            DiffViewMode::Minimal => vec![filter_unified_diff(text)],
            DiffViewMode::Expanded => vec![filter_unified_diff_with_context(text, context_lines)],
        },
        DiffPreview::CodeBlocks { per_file } => per_file
            .iter()
            .map(|ba| textdiff_chunk_for_before_after(ba, mode, context_lines))
            .collect(),
    }
}

fn filter_unified_diff(text: &str) -> String {
    let mut out = String::new();
    for line in text.lines() {
        let is_add = line.starts_with('+') && !line.starts_with("+++");
        let is_del = line.starts_with('-') && !line.starts_with("---");
        let is_hunk = line.starts_with("@@");
        let is_header = line.starts_with("---") || line.starts_with("+++");
        let is_file = line.starts_with("diff --git");
        if is_add || is_del || is_hunk || is_header || is_file {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn filter_unified_diff_with_context(text: &str, context_lines: usize) -> String {
    if context_lines == 0 {
        return filter_unified_diff(text);
    }

    let lines: Vec<&str> = text.lines().collect();
    let mut keep = vec![false; lines.len()];

    for (i, line) in lines.iter().enumerate() {
        // Always keep file/hunk headers.
        if line.starts_with("diff --git")
            || line.starts_with("---")
            || line.starts_with("+++")
            || line.starts_with("@@")
        {
            keep[i] = true;
            continue;
        }

        let is_add = line.starts_with('+') && !line.starts_with("+++");
        let is_del = line.starts_with('-') && !line.starts_with("---");
        if is_add || is_del {
            let start = i.saturating_sub(context_lines);
            let end = (i + context_lines).min(lines.len().saturating_sub(1));
            for j in start..=end {
                keep[j] = true;
            }
        }
    }

    let mut out = String::new();
    for (i, line) in lines.iter().enumerate() {
        if keep[i] {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

fn textdiff_chunk_for_before_after(
    ba: &BeforeAfter,
    mode: DiffViewMode,
    context_lines: usize,
) -> String {
    // Note: `BeforeAfter` content is already truncated at proposal time.
    let diff = TextDiff::from_lines(&ba.before, &ba.after);

    let mut out = String::new();
    out.push_str(&format!("--- {}\n", ba.file_path.display()));

    let mut changes: Vec<(ChangeTag, &str)> = Vec::new();
    for change in diff.iter_all_changes() {
        changes.push((change.tag(), change.value()));
    }

    let mut keep = vec![false; changes.len()];
    match mode {
        DiffViewMode::Full => {
            keep.fill(true);
        }
        DiffViewMode::Minimal => {
            for (i, (tag, _)) in changes.iter().enumerate() {
                keep[i] = *tag != ChangeTag::Equal;
            }
        }
        DiffViewMode::Expanded => {
            // Keep changes plus N equal lines of context around them.
            for (i, (tag, _)) in changes.iter().enumerate() {
                if *tag == ChangeTag::Equal {
                    continue;
                }
                let start = i.saturating_sub(context_lines);
                let end = (i + context_lines).min(changes.len().saturating_sub(1));
                for j in start..=end {
                    keep[j] = true;
                }
            }
        }
    }

    for (i, (tag, value)) in changes.iter().enumerate() {
        if !keep[i] {
            continue;
        }

        let prefix = match tag {
            ChangeTag::Delete => '-',
            ChangeTag::Insert => '+',
            ChangeTag::Equal => ' ',
        };

        out.push(prefix);
        out.push(' ');

        let v = value.strip_suffix('\n').unwrap_or(value);
        out.push_str(v);
        out.push('\n');
    }

    out
}

fn push_highlighted_with_limit(
    target: &mut Vec<Line<'static>>,
    highlighted: Vec<StyledLine>,
    rendered: &mut usize,
    limit: usize,
) -> bool {
    if limit == usize::MAX {
        let lines = styled_to_ratatui_lines(highlighted);
        *rendered = (*rendered).saturating_add(lines.len());
        target.extend(lines);
        return false;
    }

    for line in styled_to_ratatui_lines(highlighted) {
        if *rendered >= limit {
            return true;
        }
        target.push(line);
        *rendered += 1;
    }
    false
}

fn truncation_line(limit: usize) -> Line<'static> {
    Line::from(format!(
        "... [truncated at {} lines, use +/- to adjust]",
        limit
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(txt: &str) -> StyledLine {
        vec![StyledSpan {
            content: txt.to_string(),
            style: Style::default(),
        }]
    }

    #[test]
    fn push_highlighted_obeys_limit() {
        let mut rendered = 0usize;
        let mut out = Vec::new();
        let highlighted = vec![make_line("one"), make_line("two"), make_line("three")];
        let truncated = push_highlighted_with_limit(&mut out, highlighted, &mut rendered, 2);
        assert!(truncated);
        assert_eq!(out.len(), 2);
        assert_eq!(rendered, 2);
    }

    #[test]
    fn push_highlighted_unbounded_adds_all() {
        let mut rendered = 0usize;
        let mut out = Vec::new();
        let highlighted = vec![make_line("a"), make_line("b")];
        let truncated =
            push_highlighted_with_limit(&mut out, highlighted.clone(), &mut rendered, usize::MAX);
        assert!(!truncated);
        assert_eq!(rendered, highlighted.len());
        assert_eq!(out.len(), highlighted.len());
    }
}
