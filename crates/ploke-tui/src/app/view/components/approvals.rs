use std::sync::Arc;

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
};

use crate::app::view::components::context_browser::StepEnum;
use crate::app_state::AppState as _;
use crate::app_state::core::{DiffPreview, EditProposalStatus}; // trait bounds

#[derive(Debug, Clone, Default)]
pub struct ApprovalsState {
    pub selected: usize,
    pub help_visible: bool,
    pub view_lines: usize, // Number of lines to show in details view (None = unlimited)
    pub filter: ApprovalsFilter,
}

impl ApprovalsState {
    pub fn select_next(&mut self) {
        self.selected = self.selected.saturating_add(1);
    }
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn increase_view_lines(&mut self) {
        if self.view_lines == 0 {
            self.view_lines = 10; // Start with 10 lines when first enabled
        } else {
            self.view_lines = (self.view_lines + 10).min(200); // Cap at 200 lines
        }
    }

    pub fn decrease_view_lines(&mut self) {
        if self.view_lines <= 10 {
            self.view_lines = 0; // 0 means no truncation
        } else {
            self.view_lines = self.view_lines.saturating_sub(10);
        }
    }

    pub fn toggle_unlimited(&mut self) {
        self.view_lines = if self.view_lines == 0 { 20 } else { 0 };
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next_wrap();
        self.selected = 0; // Reset selection to keep in-bounds on new list
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
    ui: &ApprovalsState,
) -> Option<uuid::Uuid> {
    // Clear the underlying content in the overlay area to avoid "bleed-through"
    frame.render_widget(ratatui::widgets::Clear, area);

    let outer = Block::bordered().title(" Approvals ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split overlay into body + footer (help)
    let footer_height = if ui.help_visible { 6 } else { 1 };
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

    // Build unified list across edits and creates with filtering
    let items: Vec<ApprovalListItem> = filtered_items(state, ui.filter);
    let selected_idx = ui.selected.min(items.len().saturating_sub(1));

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|item| ListItem::new(item.label.clone()).style(status_style(&item.status)))
        .collect();
    let list = List::new(list_items)
        .block(Block::bordered().title(" Pending Proposals "))
        .highlight_style(Style::new().fg(Color::Black).bg(Color::Cyan))
        .highlight_symbol("▶ ")
        .highlight_spacing(ratatui::widgets::HighlightSpacing::Always);
    let mut list_state = ListState::default();
    if !items.is_empty() {
        list_state.select(Some(selected_idx));
    }
    frame.render_stateful_widget(list, cols[0], &mut list_state);

    // Details
    let selected = items.get(selected_idx).map(|item| (item.kind, item.id));
    let mut detail_lines: Vec<Line> = Vec::new();
    if let Some((sel_kind, sel_id)) = selected {
        // Use the established pattern for accessing async data from sync context
        let (proposals_guard, create_guard) = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let p = state.proposals.read().await;
                let c = state.create_proposals.read().await;
                (p, c)
            })
        });
        let mut render_preview = |status: &EditProposalStatus,
                                  files_len: usize,
                                  preview: &DiffPreview| {
            detail_lines.push(Line::from(vec![Span::styled(
                format!("request_id: {}", sel_id),
                Style::new().fg(Color::Yellow),
            )]));
            detail_lines.push(Line::from(format!(
                "status: {:?}  files:{}",
                status, files_len
            )));

            // Determine line limit for display
            let line_limit = if ui.view_lines == 0 {
                usize::MAX
            } else {
                ui.view_lines
            };

            match preview {
                DiffPreview::UnifiedDiff { text } => {
                    let header = Line::from(vec![Span::styled(
                        "Unified Diff:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);

                    for (lines_added, ln) in text.lines().enumerate() {
                        if lines_added >= line_limit {
                            detail_lines.push(Line::from(format!(
                                "... [truncated at {} lines, use +/- to adjust]",
                                line_limit
                            )));
                            break;
                        }
                        detail_lines.push(Line::from(ln.to_string()));
                    }
                }
                DiffPreview::CodeBlocks { per_file } => {
                    let header = Line::from(vec![Span::styled(
                        "Before/After:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);

                    let mut total_lines_added = 0;
                    for ba in per_file.iter().take(2) {
                        if total_lines_added >= line_limit {
                            detail_lines.push(Line::from(format!(
                                "... [more files truncated at {} lines]",
                                line_limit
                            )));
                            break;
                        }

                        detail_lines.push(Line::from(format!("--- {}", ba.file_path.display())));
                        total_lines_added += 1;

                        // Before section
                        for ln in ba.before.lines() {
                            if total_lines_added >= line_limit {
                                detail_lines.push(Line::from(format!(
                                    "... [truncated at {} lines, use +/- to adjust]",
                                    line_limit
                                )));
                                break;
                            }
                            detail_lines.push(Line::from(format!("- {}", ln)));
                            total_lines_added += 1;
                        }

                        // After section
                        for ln in ba.after.lines() {
                            if total_lines_added >= line_limit {
                                break;
                            }
                            detail_lines.push(Line::from(format!("+ {}", ln)));
                            total_lines_added += 1;
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
    let detail = Paragraph::new(detail_lines)
        .block(Block::bordered().title(" Details "))
        .alignment(Alignment::Left);
    frame.render_widget(detail, cols[1]);

    // Render help footer with truncation status
    let overlay_style = Style::new().fg(Color::LightBlue);
    if ui.help_visible {
        let truncation_status = if ui.view_lines == 0 {
            "unlimited".to_string()
        } else {
            format!("{} lines", ui.view_lines)
        };

        let help_text = format!(
            "Keys: Enter=approve  n=deny  o=open in editor  ↑/↓,j/k=navigate  f=cycle filter  q/Esc=close\n\
             View: +=more lines  -=fewer lines  u=toggle unlimited (current: {})\n\
             Filter: current={} (f to cycle)\n\
             Commands:\n\
             - Enter: Approve selected proposal\n\
             - n: Deny selected proposal\n\
             - o: Open files in configured editor\n\
             - +: Show more lines in preview (current: {})\n\
             - -: Show fewer lines in preview\n\
             - u: Toggle unlimited view\n\
             - f: Cycle filter\n\
             - q/Esc: Close approvals overlay",
            truncation_status,
            ui.filter.label(),
            truncation_status
        );

        let help = Paragraph::new(help_text)
            .style(overlay_style)
            .block(Block::bordered().title(" Help ").style(overlay_style));
        frame.render_widget(help, footer_area);
    } else {
        let truncation_info = if ui.view_lines == 0 {
            "unlimited".to_string()
        } else {
            format!("{} lines", ui.view_lines)
        };

        let hint = Paragraph::new(format!(
            " ? Help | View: {} | Filter: {} ",
            truncation_info,
            ui.filter.label()
        ))
        .style(overlay_style)
        .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(hint, footer_area);
    }

    selected.map(|(_, id)| id)
}
