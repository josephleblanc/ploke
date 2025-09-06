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

#[derive(Debug, Clone)]
pub struct ApprovalsState {
    pub selected: usize,
    pub help_visible: bool,
    pub view_lines: usize, // Number of lines to show in details view (None = unlimited)
}

impl Default for ApprovalsState {
    fn default() -> Self {
        Self {
            selected: 0,
            help_visible: false,
            view_lines: 0, // 0 means no truncation (show all)
        }
    }
}

impl ApprovalsState {
    pub fn select_next(&mut self) { self.selected = self.selected.saturating_add(1); }
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
            // Determine line limit for display
            let line_limit = if ui.view_lines == 0 { 
                usize::MAX // No limit
            } else { 
                ui.view_lines 
            };

            match &p.preview {
                DiffPreview::UnifiedDiff { text } => {
                    let header = Line::from(vec![Span::styled(
                        "Unified Diff:",
                        Style::new().fg(Color::Green),
                    )]);
                    detail_lines.push(header);
                    
                    let mut lines_added = 0;
                    for ln in text.lines() {
                        if lines_added >= line_limit {
                            detail_lines.push(Line::from(format!(
                                "... [truncated at {} lines, use +/- to adjust]", 
                                line_limit
                            )));
                            break;
                        }
                        detail_lines.push(Line::from(ln.to_string()));
                        lines_added += 1;
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
            "Keys: Enter=approve  n=deny  o=open in editor  ↑/↓,j/k=navigate  q/Esc=close\n\
             View: +=more lines  -=fewer lines  u=toggle unlimited (current: {})\n\
             Commands:\n\
             - Enter: Approve selected proposal\n\
             - n: Deny selected proposal\n\
             - o: Open files in configured editor\n\
             - +: Show more lines in preview (current: {})\n\
             - -: Show fewer lines in preview\n\
             - u: Toggle unlimited view\n\
             - q/Esc: Close approvals overlay",
            truncation_status, truncation_status
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
        
        let hint = Paragraph::new(format!(" ? Help | View: {} ", truncation_info))
            .style(overlay_style)
            .alignment(ratatui::layout::Alignment::Right);
        frame.render_widget(hint, footer_area);
    }

    selected_id
}
