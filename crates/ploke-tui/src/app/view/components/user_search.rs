use ploke_core::ArcStr;
use ploke_core::rag_types::{AssembledContext, AssembledMeta, ContextPart};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize as _};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::app::input::keymap::{Action, to_action};
use crate::app::types::{Mode, RenderMsg};
use crate::app::utils::truncate_uuid;
use crate::app::view::components::conversation::ConversationView;
use crate::app::view::components::input_box::InputView;
use crate::llm::request::endpoint::Endpoint;
use crate::llm::router_only::openrouter::providers::ProviderName;
use crate::llm::{EndpointKey, ModelId, ModelKey, ModelName, ProviderKey};
use crate::user_config::OPENROUTER_URL;
use crate::utils::helper::truncate_string;
use color_eyre::Result;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{
    DisableBracketedPaste, DisableFocusChange, DisableMouseCapture, EnableBracketedPaste,
    EnableFocusChange, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use itertools::Itertools;
// use message_item::{measure_messages, render_messages}; // now handled by ConversationView
use ploke_db::search_similar;
use ratatui::text::{Line, Span};
use ratatui::widgets::Gauge;
// use textwrap::wrap; // moved into InputView
use std::fmt::Display;
use std::sync::Arc;
use tokio::sync::oneshot;
use toml::to_string;
use tracing::instrument;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct SearchItemId(Uuid);

impl SearchItemId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl AsRef<Uuid> for SearchItemId {
    fn as_ref(&self) -> &Uuid {
        &self.0
    }
}

impl std::fmt::Display for SearchItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug, Deserialize, Serialize)]
pub struct ItemName(ArcStr);

arcstr_wrapper!(ItemName);

use ploke_core::arcstr_wrapper;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    /// Wrapped Uuid of the NodeId for the given item.
    pub id: SearchItemId,
    /// The name (if there is one, e.g. impl blocks do not) for the code item returned in the
    /// search results
    pub name: Option<ItemName>,
    /// The search result (as for a context search using LLM-facing tools)
    pub context_part: ContextPart,
    /// Whether the user sent a command to expand the item for more info.
    pub expanded: bool,
    /// Whether the user sent a command to show the preview of the item.
    pub show_preview: ShowPreview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum ShowPreview {
    NoPreview,
    Small,
    Full,
}

impl ShowPreview {
    pub fn next_more_verbose(self) -> Self {
        match self {
            ShowPreview::NoPreview => ShowPreview::Small,
            ShowPreview::Small => ShowPreview::Full,
            ShowPreview::Full => ShowPreview::Full,
        }
    }

    pub fn next_less_verbose(self) -> Self {
        match self {
            ShowPreview::NoPreview => ShowPreview::NoPreview,
            ShowPreview::Small => ShowPreview::NoPreview,
            ShowPreview::Full => ShowPreview::Small,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
enum SearchItemField {
    Id,
    Name,
    FilePath,
    CanonPath,
    Kind,
    Text,
    Score,
    Modality,
}

const DISPLAYED_FIELDS: [SearchItemField; 8] = [
    SearchItemField::Id,
    SearchItemField::Name,
    SearchItemField::FilePath,
    SearchItemField::CanonPath,
    SearchItemField::Kind,
    SearchItemField::Text,
    SearchItemField::Score,
    SearchItemField::Modality,
];

impl From<SearchItemField> for &'static str {
    fn from(value: SearchItemField) -> Self {
        use SearchItemField::*;
        match value {
            Id => "Id",
            Name => "Name",
            FilePath => "FilePath",
            CanonPath => "CanonPath",
            Kind => "Kind",
            Text => "Text",
            Score => "Score",
            Modality => "Modality",
        }
    }
}

impl SearchItem {
    fn format_line_field_val(&self, field: SearchItemField) -> Line<'_> {
        let indent = "    ";
        let field_str: &'static str = field.into();

        use SearchItemField::*;
        let val = match field {
            Id => self.id.to_string(),
            Name => {
                if let Some(name) = self.name.as_ref() {
                    name.as_ref().to_string()
                } else {
                    "unnamed".to_string()
                }
            }
            FilePath => self.context_part.file_path.0.clone(),
            CanonPath => self.context_part.canon_path.0.clone(),
            Kind => self.context_part.kind.to_static_str().to_string(),
            Text => self.context_part.text.clone(),
            Score => {
                format!("{:2}", self.context_part.score)
            }
            Modality => self.context_part.modality.to_static_str().to_string(),
        };

        Line::from(Span::styled(
            format!("{indent}{field_str}: {val}"),
            OVERLAY_STYLE.detail,
        ))
    }

    fn format_line_field_val_to_width(&self, field: SearchItemField, width: usize) -> Line<'_> {
        let indent = "    ";
        let field_str: &'static str = field.into();

        let truncated_len = width
            .saturating_sub(indent.len())
            .saturating_sub(field_str.len())
            // additional 4 subtracted for some additional padding so it isn't right up
            // against the right border
            // - also includes the two needed for `: ` in the final format! statement at the end of
            // this function
            .saturating_sub(4);

        use SearchItemField::*;
        let mut val = match field {
            Id => truncate_uuid(self.id.0),
            Name => {
                if let Some(name) = self.name.as_ref() {
                    name.as_ref().to_string()
                } else {
                    "unnamed".to_string()
                }
            }
            FilePath => {
                let file_path_len = self.context_part.file_path.0.len();
                let trunc_start_index = file_path_len
                    .saturating_sub(truncated_len)
                    // subtract additional 3 for leading ellipses
                    .saturating_sub(3);
                format!(
                    "...{}",
                    &self.context_part.file_path.0.as_str()[trunc_start_index..]
                )
            }
            CanonPath => self.context_part.canon_path.0.clone(),
            Kind => self.context_part.kind.to_static_str().to_string(),
            Text => self.context_part.text.clone(),
            Score => format!("{:.3}", self.context_part.score),
            Modality => self.context_part.modality.to_static_str().to_string(),
        };
        val.truncate(width);

        Line::from(Span::styled(
            format!("{indent}{field_str}: {val}"),
            OVERLAY_STYLE.detail,
        ))
    }
}

impl From<ContextPart> for SearchItem {
    fn from(value: ContextPart) -> Self {
        let name = if !value.canon_path.as_ref().is_empty() {
            value
                .canon_path
                .as_ref()
                .rsplit_once("::")
                .map(|(first, last)| ItemName::new_from_str(last))
        } else {
            None
        };
        Self {
            id: SearchItemId::new(value.id),
            name,
            context_part: value,
            expanded: false,
            show_preview: ShowPreview::NoPreview,
        }
    }
}

#[derive(Debug)]
pub struct ContextSearchState {
    pub visible: bool,
    pub search_input: String,
    /// The items returned in the search
    pub items: Vec<SearchItem>,
    /// The index of the currently selected item
    pub selected: usize,
    // Toggle for bottom-right help panel within the Model Browser overlay
    pub help_visible: bool,
    // Provider selection mode for the currently selected item
    pub preview_select_active: bool,
    pub item_selected: usize,
    // support scrolling
    pub vscroll: u16,
    pub viewport_height: u16,

    /// Whether or not the search is loading (will be false when search complete)
    pub loading_search: bool,
}

lazy_static::lazy_static! {
    static ref OVERLAY_STYLE: OverlayStyle = OverlayStyle::default();
}

pub struct OverlayStyle {
    background: Style,
    selected: Style,
    detail: Style,
}

impl Default for OverlayStyle {
    fn default() -> Self {
        Self {
            background: Style::new().fg(Color::LightBlue),
            selected: Style::new().fg(Color::Black).bg(Color::LightCyan),
            detail: Style::new().fg(Color::Blue).dim(),
        }
    }
}

pub fn render_context_search<'a>(
    frame: &mut Frame<'_>,
    cb: &'a ContextSearchState,
) -> (Rect, Rect, Style, Vec<Line<'a>>) {
    let area = frame.area();
    // calculates about 80% of width
    let width = area.width.saturating_mul(8) / 10;
    // calculates about 80% of height
    let height = area.height.saturating_mul(8) / 10;
    // NOTE: I don't really get what this is doing. Play around with it once this renders to get an
    // idea of if we need it.
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

    // Clear the underlying content in the overlay area to avoid "bleed-through"
    // TODO: Experiment with this some more.
    // - this seems to clear the background every time the frame is drawn?
    // - could we add a state field like `initialized` or something so we only need to clear the
    // background once when first opening the search overlay?
    //  - might be more trouble than worth to mess with this.
    frame.render_widget(ratatui::widgets::Clear, rect);

    // Split overlay into body + footer (help)
    // TODO: Add a search bar area at the top
    let footer_height = if cb.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    let body_area = layout[0];
    let footer_area = layout[1];

    // Consistent overlay style (foreground/background)
    // Choose a high-contrast, uniform scheme that doesn't depend on background UI
    let overlay_style = Style::new().fg(Color::LightBlue);

    // Build list content (styled). Header moved to Block title; keep only list lines here.
    let mut lines: Vec<Line> = Vec::new();
    // let search_lines: SearchItemLines = SearchItemLines {
    //     lines: lines.clone(),
    // };

    // Loading indicator when opened before results arrive
    if cb.items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Loading search results…",
            overlay_style,
        )));
    }

    // Selected row highlighting
    // TODO: create a unified style for overlays that both this context search and the model picker
    // overlay can use
    // - add as an associated const/static or something to a trait for this overlay?
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    for (i, it) in cb.items.iter().enumerate() {
        let title = if let Some(name) = &it.name
            && !name.as_ref().is_empty()
        {
            name.as_ref().to_string()
        } else {
            "unnamed".to_string()
        };

        let mut line = Line::from(vec![
            Span::styled(
                if i == cb.selected { ">" } else { " " },
                if i == cb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                title,
                if i == cb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
        ]);
        // Ensure entire line style is applied (for background fill)
        line.style = if i == cb.selected {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        if it.expanded {
            let indent = "    ";
            // Indented details for readability while navigating (preserve spaces; do not trim)

            let displayed_fields = [
                SearchItemField::Id,
                SearchItemField::Name,
                SearchItemField::FilePath,
                SearchItemField::CanonPath,
                SearchItemField::Kind,
                SearchItemField::Score,
                SearchItemField::Modality,
            ];
            let details_width = body_area
                .width
                .saturating_sub(indent.len() as u16)
                // subtract a few cols for the borders + margin on the left and right.
                .saturating_sub(4);
            for field in displayed_fields {
                lines.push(it.format_line_field_val_to_width(field, details_width as usize));
            }

            // Provider breakdown (with loading/empty states)
            let preview = match it.show_preview {
                ShowPreview::NoPreview => "Press `l` or RightArrow to show preview".to_string(),
                ShowPreview::Small => it
                    .context_part
                    .text
                    .chars()
                    .take(body_area.width as usize)
                    .collect::<String>(),
                ShowPreview::Full => it.context_part.text.clone(),
            };
            lines.push(Line::from(Span::styled(preview, detail_style)))
        }
    }
    (body_area, footer_area, overlay_style, lines)
}

// pub struct SearchItemLines<'a> {
//     lines: Vec<Line<'a>>,
// }
//
// impl<'a> AsRef<Vec<Line<'a>>> for SearchItemLines<'a> {
//     fn as_ref(&self) -> &Vec<Line<'a>> {
//         &self.lines
//     }
// }
//
// impl<'a> AsMut<Vec<Line<'a>>> for SearchItemLines<'a> {
//     fn as_mut(&mut self) -> &mut Vec<Line<'a>> {
//         &mut self.lines
//     }
// }

// impl<'a> SearchItemLines<'a> {
//     pub fn new(lines: Vec<Line<'a>>) -> Self {
//         Self { lines }
//     }
//
//     fn append_field_val(&mut self, indent: &'static str, field: &'static str, val: String) {
//         let SearchItemLines { lines } = self;
//         lines.push(Line::from(Span::styled(
//             format!("{indent}{field}: {val}"),
//             OVERLAY_STYLE.detail,
//         )));
//     }
// }

/// Provider item height:
///     context_length + supports_tools + pricing
const PROVIDER_DETAILS_HEIGHT: usize = 3;

pub fn model_browser_detail_lines(it: &SearchItem) -> usize {
    if !it.expanded {
        return 0;
    }
    let expanded_rows = if it.expanded {
        DISPLAYED_FIELDS.len()
        // might need to add more rows dynamically here depending on how we display the text for
        // previewed items
        // } else {
        //     DISPLAYED_FIELDS.len()
    } else {
        0
    };
    // add 1 for provider header, "providers:"
    PROVIDER_DETAILS_HEIGHT + 1 + expanded_rows
}

// Header is not part of scrollable content (it's displayed in the Block title).
const MODEL_BROWSER_HEADER_HEIGHT: usize = 0;
pub fn model_browser_total_lines(mb: &ContextSearchState) -> usize {
    let base = MODEL_BROWSER_HEADER_HEIGHT
        + mb.items
            .iter()
            .map(model_browser_detail_lines)
            .map(|it| it + 1)
            .sum::<usize>();
    if mb.items.is_empty() {
        base + 1 // account for "Loading models…" line
    } else {
        base
    }
}

pub fn model_browser_focus_line(mb: &ContextSearchState) -> usize {
    let header = MODEL_BROWSER_HEADER_HEIGHT;
    if mb.items.is_empty() {
        return header;
    }

    let sel_idx = mb.selected.min(mb.items.len().saturating_sub(1));

    let mut line = header;
    for j in 0..sel_idx {
        let it = &mb.items[j];
        line += 1; // title
        line += model_browser_detail_lines(it);
    }

    let sel = &mb.items[sel_idx];
    if mb.preview_select_active && sel.expanded && !sel.context_part.text.is_empty() {
        // let prov_idx = mb
        //     .provider_selected
        //     .min(sel.providers.len().saturating_sub(1));
        // line + 1 + PROVIDER_DETAILS_HEIGHT + 1 + prov_idx

        line + 1 + PROVIDER_DETAILS_HEIGHT + 1 + 1 // last item was prov_idx
    } else {
        line
    }
}

pub(crate) fn compute_browser_scroll(body_area: Rect, mb: &mut ContextSearchState) {
    // Track viewport height for scrolling (inner area excludes the block borders)
    mb.viewport_height = body_area.height.saturating_sub(2);

    let total = model_browser_total_lines(mb);
    let focus = model_browser_focus_line(mb);
    let vh = mb.viewport_height as usize;
    let max_v = total.saturating_sub(vh);

    // Clamp current vscroll to content bounds
    mb.vscroll = (mb.vscroll as usize).min(max_v) as u16;

    // Auto-reveal focus line if outside view
    let v = mb.vscroll as usize;
    if focus < v {
        mb.vscroll = focus as u16;
    } else if focus >= v + vh {
        mb.vscroll = (focus + 1).saturating_sub(vh) as u16;
    }

    // If the currently selected item is expanded (and we're not in provider-row focus),
    // minimally reveal the entire expanded block within the viewport when possible.
    // This avoids the jarring effect where expanding adds lines that end up hidden below
    // the viewport, making it look like the scroll offset ignored the expanded height.
    if !mb.items.is_empty() && !mb.preview_select_active {
        let sel_idx = mb.selected.min(mb.items.len().saturating_sub(1));
        let sel = &mb.items[sel_idx];
        if sel.expanded {
            // Compute the top line (0-based in content space) of the selected item's title
            let mut block_top = MODEL_BROWSER_HEADER_HEIGHT;
            for j in 0..sel_idx {
                block_top += 1; // title line
                block_top += model_browser_detail_lines(&mb.items[j]);
            }
            // Height of the expanded block: title + details
            let block_height = 1 + model_browser_detail_lines(sel);
            let block_bottom_excl = block_top + block_height; // exclusive end

            // Only try to fully reveal when it can fit; otherwise prefer keeping the top visible.
            if block_height <= vh {
                let v = mb.vscroll as usize;
                if block_bottom_excl > v + vh {
                    mb.vscroll = block_bottom_excl.saturating_sub(vh) as u16;
                }
            }
        }
    }
}
