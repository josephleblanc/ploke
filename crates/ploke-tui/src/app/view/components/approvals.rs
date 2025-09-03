use std::sync::Arc;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame,
};

use crate::app_state::core::{DiffPreview, EditProposalStatus};
use crate::app_state::AppState as _; // trait bounds

#[derive(Debug, Default, Clone)]
pub struct ApprovalsState {
    pub selected: usize,
    pub help_visible: bool,
}

impl ApprovalsState {
    pub fn select_next(&mut self) { self.selected = self.selected.saturating_add(1); }
    pub fn select_prev(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }
}

pub fn render_approvals_overlay(
    frame: &mut Frame,
    area: Rect,
    state: &Arc<crate::app_state::AppState>,
    ui: &ApprovalsState,
) -> Option<uuid::Uuid> {
    let outer = Block::bordered().title(" Approvals ");
    let inner = outer.inner(area);
    frame.render_widget(outer, area);

    // Split into list and details
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    // Use the established pattern for accessing async data from sync context
    let proposals_guard = tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            state.proposals.read().await
        })
    });
    
    let mut items: Vec<(uuid::Uuid, String)> = proposals_guard
        .iter()
        .map(|(id, p)| {
            let status = match &p.status {
                EditProposalStatus::Pending => "Pending",
                EditProposalStatus::Approved => "Approved",
                EditProposalStatus::Denied => "Denied",
                EditProposalStatus::Applied => "Applied",
                EditProposalStatus::Failed(_) => "Failed",
            };
            (
                *id,
                format!(
                    "{}  {:<7}  files:{}",
                    crate::app::utils::truncate_uuid(*id),
                    status,
                    p.files.len()
                ),
            )
        })
        .collect();
    items.sort_by_key(|(id, _)| *id);

    let list_items: Vec<ListItem> = items
        .iter()
        .map(|(_, s)| ListItem::new(s.clone()))
        .collect();
    let list = List::new(list_items)
        .block(Block::bordered().title(" Pending Proposals "))
        .highlight_style(Style::new().fg(Color::Cyan));
    frame.render_widget(list, cols[0]);

    // Details
    let selected_id = items.get(ui.selected).map(|(id, _)| *id);
    let mut detail_lines: Vec<Line> = Vec::new();
    if let Some(sel) = selected_id {
        if let Some(p) = proposals_guard.get(&sel) {
            detail_lines.push(Line::from(vec![Span::styled(
                format!("request_id: {}", sel),
                Style::new().fg(Color::Yellow),
            )]));
            detail_lines.push(Line::from(format!(
                "status: {:?}  files:{}",
                p.status,
                p.files.len()
            )));
            match &p.preview {
                DiffPreview::UnifiedDiff { text } => {
                    let header = Line::from(vec![Span::styled(
                        "Unified Diff:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);
                    for ln in text.lines().take(40) {
                        detail_lines.push(Line::from(ln.to_string()));
                    }
                }
                DiffPreview::CodeBlocks { per_file } => {
                    let header = Line::from(vec![Span::styled(
                        "Before/After:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);
                    for ba in per_file.iter().take(2) {
                        detail_lines.push(Line::from(format!("--- {}", ba.file_path.display())));
                        for ln in ba.before.lines().take(10) {
                            detail_lines.push(Line::from(format!("- {}", ln)));
                        }
                        for ln in ba.after.lines().take(10) {
                            detail_lines.push(Line::from(format!("+ {}", ln)));
                        }
                    }
                }
            }
        }
    }
    let detail = Paragraph::new(detail_lines)
        .block(Block::bordered().title(" Details "))
        .alignment(Alignment::Left);
    frame.render_widget(detail, cols[1]);

    selected_id
}
