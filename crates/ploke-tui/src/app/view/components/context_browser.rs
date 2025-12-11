use ploke_core::ArcStr;
use ploke_core::rag_types::{AssembledContext, AssembledMeta, ContextPart};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize as _};
use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;
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
use ratatui::widgets::Wrap;
use ratatui::widgets::{Block, Borders, Gauge, ListState, Paragraph};
use std::fmt::Display;
use std::sync::Arc;
use std::time::Instant;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextBrowserMode {
    Normal,
    Insert,
}

#[derive(Default, Debug, Clone)]
pub struct LineEdit {
    buf: String,
    cursor: usize, // byte index at char boundary
}

impl LineEdit {
    pub fn as_str(&self) -> &str {
        &self.buf
    }

    pub fn cursor_byte(&self) -> usize {
        self.cursor
    }

    pub fn set(&mut self, s: impl Into<String>) {
        self.buf = s.into();
        self.cursor = self.buf.len();
        self.snap_cursor();
    }

    pub fn clear(&mut self) {
        self.buf.clear();
        self.cursor = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        self.buf.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.snap_cursor();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev = prev_char_boundary(&self.buf, self.cursor);
        self.buf.drain(prev..self.cursor);
        self.cursor = prev;
        self.snap_cursor();
    }

    pub fn delete(&mut self) {
        if self.cursor >= self.buf.len() {
            return;
        }
        let next = next_char_boundary(&self.buf, self.cursor);
        self.buf.drain(self.cursor..next);
        self.snap_cursor();
    }

    pub fn move_left(&mut self) {
        self.cursor = prev_char_boundary(&self.buf, self.cursor);
        self.snap_cursor();
    }

    pub fn move_right(&mut self) {
        self.cursor = next_char_boundary(&self.buf, self.cursor);
        self.snap_cursor();
    }

    pub fn move_home(&mut self) {
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.buf.len();
        self.snap_cursor();
    }

    pub fn display_cursor_col(&self) -> u16 {
        let prefix = &self.buf[..self.cursor];
        UnicodeWidthStr::width(prefix) as u16
    }

    fn snap_cursor(&mut self) {
        while self.cursor > self.buf.len() {
            self.cursor = self.buf.len();
        }
        while !self.buf.is_char_boundary(self.cursor) {
            self.cursor = self.cursor.saturating_sub(1);
        }
    }
}

fn prev_char_boundary(s: &str, i: usize) -> usize {
    if i == 0 {
        return 0;
    }
    let mut j = i.saturating_sub(1);
    while j > 0 && !s.is_char_boundary(j) {
        j = j.saturating_sub(1);
    }
    j
}

fn next_char_boundary(s: &str, i: usize) -> usize {
    if i >= s.len() {
        return s.len();
    }
    let mut j = i + 1;
    while j < s.len() && !s.is_char_boundary(j) {
        j += 1;
    }
    j.min(s.len())
}

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
    /// Whether to show additional metadetail fields, default to short_fields, then show
    /// long_fields when expanding further.
    pub show_meta_details: ShowMetaDetails,
    /// Whether the user sent a command to show the preview of the item.
    pub show_preview: ShowPreview,
    /// Shortened summary of fields to include in the preview shown after initially expanding the
    /// target item. These should include only the most relevent details at a glance.
    pub short_fields: [SearchItemField; 3],
    pub long_fields: [SearchItemField; 7],
}

pub trait StepEnum<const N: usize>: Copy + Eq {
    /// Declaration of the logical order for stepping.
    const ORDER: [Self; N];

    /// Map variant -> index in ORDER.
    fn idx(self) -> usize;

    #[inline]
    fn next_clamped(self) -> Self {
        let i = self.idx();
        Self::ORDER[(i + 1).min(Self::ORDER.len() - 1)]
    }

    #[inline]
    fn prev_clamped(self) -> Self {
        let i = self.idx();
        Self::ORDER[i.saturating_sub(1)]
    }

    fn least(self) -> Self {
        Self::ORDER[0]
    }

    fn most(self) -> Self {
        Self::ORDER[N - 1]
    }

    /// Optional: generic “step by delta”, clamped.
    #[inline]
    fn step_clamped(self, delta: isize) -> Self {
        let i = self.idx() as isize;
        let max = (Self::ORDER.len() - 1) as isize;
        let j = (i + delta).clamp(0, max) as usize;
        Self::ORDER[j]
    }
}

impl StepEnum<3> for ShowPreview {
    const ORDER: [ShowPreview; 3] = [Self::NoPreview, Self::Small, Self::Full];

    fn idx(self) -> usize {
        match self {
            Self::NoPreview => 0,
            Self::Small => 1,
            Self::Full => 2,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
pub enum ShowMetaDetails {
    #[default]
    NoDetails,
    Short,
    Long,
}

impl ShowMetaDetails {
    fn iter_details(self) -> impl IntoIterator<Item = SearchItemField> {
        match self {
            ShowMetaDetails::NoDetails => [].iter().copied(),
            ShowMetaDetails::Short => [
                SearchItemField::Name,
                SearchItemField::FilePath,
                SearchItemField::CanonPath,
            ]
            .iter()
            .copied(),
            ShowMetaDetails::Long => [
                SearchItemField::Id,
                SearchItemField::Name,
                SearchItemField::FilePath,
                SearchItemField::CanonPath,
                SearchItemField::Kind,
                SearchItemField::Score,
                SearchItemField::Modality,
            ]
            .iter()
            .copied(),
        }
    }
}

impl StepEnum<3> for ShowMetaDetails {
    const ORDER: [Self; 3] = [Self::NoDetails, Self::Short, Self::Long];

    fn idx(self) -> usize {
        match self {
            Self::NoDetails => 0,
            Self::Short => 1,
            Self::Long => 2,
        }
    }
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord, Default,
)]
pub enum ShowPreview {
    #[default]
    NoPreview,
    Small,
    Full,
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

const DETAIL_INDENT: &str = "    ";
const SMALL_PREVIEW_MAX_LINES: usize = 12;

impl SearchItem {
    fn format_line_field_val(&self, field: SearchItemField) -> Line<'_> {
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
            format!("{DETAIL_INDENT}{field_str}: {val}"),
            OVERLAY_STYLE.detail,
        ))
    }

    fn format_line_field_val_to_width(&self, field: SearchItemField, width: usize) -> Line<'_> {
        let field_str: &'static str = field.into();

        let truncated_len = width
            .saturating_sub(DETAIL_INDENT.len())
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
            format!("{DETAIL_INDENT}{field_str}: {val}"),
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
            show_meta_details: ShowMetaDetails::NoDetails,
            short_fields: [
                SearchItemField::Name,
                SearchItemField::FilePath,
                SearchItemField::CanonPath,
            ],
            long_fields: [
                SearchItemField::Id,
                SearchItemField::Name,
                SearchItemField::FilePath,
                SearchItemField::CanonPath,
                SearchItemField::Kind,
                SearchItemField::Score,
                SearchItemField::Modality,
            ],
        }
    }
}

#[derive(Debug)]
pub struct ContextSearchState {
    pub visible: bool,
    pub mode: ContextBrowserMode,
    pub input: LineEdit,
    pub last_sent_query: String,
    pub query_id: u64,
    pub pending_dispatch: bool,
    pub debounce_ms: u64,
    pub last_edit_at: Instant,
    /// The items returned in the search
    pub items: Vec<SearchItem>,
    pub list_state: ListState,
    // Toggle for bottom-right help panel within the Model Browser overlay
    pub help_visible: bool,
    // Provider selection mode for the currently selected item
    pub preview_select_active: bool,
    // support scrolling
    pub vscroll: u16,
    pub viewport_height: u16,

    /// Whether or not the search is loading (will be false when search complete)
    pub loading_search: bool,
}

impl ContextSearchState {
    pub fn new(search_input: String) -> Self {
        let mut input = LineEdit::default();
        input.set(search_input);
        let mut list_state = ListState::default();
        list_state.select(None);
        Self {
            visible: true,
            mode: ContextBrowserMode::Insert,
            input,
            last_sent_query: String::new(),
            query_id: 0,
            pending_dispatch: true,
            debounce_ms: 100,
            last_edit_at: Instant::now(),
            items: Vec::new(),
            list_state,
            help_visible: false,
            preview_select_active: false,
            vscroll: 0,
            viewport_height: 0,
            loading_search: true,
        }
    }

    pub fn with_items(search_input: String, items: Vec<SearchItem>) -> Self {
        let mut this = Self::new(search_input);
        this.items = items;
        this.ensure_selection();
        this
    }

    pub fn ensure_selection(&mut self) {
        if self.items.is_empty() {
            self.list_state.select(None);
            return;
        }
        if let Some(sel) = self.list_state.selected() {
            let capped = sel.min(self.items.len().saturating_sub(1));
            self.list_state.select(Some(capped));
        } else {
            self.list_state.select(Some(0));
        }
    }

    pub fn selected_index(&self) -> usize {
        self.list_state
            .selected()
            .unwrap_or(0)
            .min(self.items.len().saturating_sub(1))
    }

    pub fn select_next(&mut self) {
        if self.items.is_empty() {
            self.list_state.select(None);
            return;
        }
        let next = self
            .selected_index()
            .saturating_add(1)
            .min(self.items.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    pub fn select_prev(&mut self) {
        if self.items.is_empty() {
            self.list_state.select(None);
            return;
        }
        let prev = self.selected_index().saturating_sub(1);
        self.list_state.select(Some(prev));
    }

    pub fn set_results(&mut self, items: Vec<SearchItem>) {
        self.items = items;
        self.loading_search = false;
        self.ensure_selection();
    }

    pub fn mark_dirty(&mut self) {
        self.pending_dispatch = true;
        self.last_edit_at = Instant::now();
        self.loading_search = true;
    }
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

fn wrap_preview_lines(
    preview: &str,
    detail_style: Style,
    details_width: usize,
    max_lines: Option<usize>,
) -> Vec<Line<'static>> {
    let available_width = details_width.saturating_sub(DETAIL_INDENT.len()).max(1);

    if preview.is_empty() {
        return vec![Line::from(Span::styled(
            format!("{DETAIL_INDENT}<no preview>"),
            detail_style,
        ))];
    }

    let mut lines = Vec::new();
    for raw in preview.lines() {
        let wrapped = textwrap::wrap(raw, available_width);
        if wrapped.is_empty() {
            lines.push(Line::from(Span::styled(
                DETAIL_INDENT.to_string(),
                detail_style,
            )));
            continue;
        }

        for segment in wrapped {
            if let Some(limit) = max_lines
                && lines.len() >= limit
            {
                lines.push(Line::from(Span::styled(
                    format!("{DETAIL_INDENT}…"),
                    detail_style,
                )));
                return lines;
            }
            lines.push(Line::from(Span::styled(
                format!("{DETAIL_INDENT}{segment}"),
                detail_style,
            )));
        }
    }

    lines
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

    // Split overlay into input + body + footer (help)
    let footer_height = if cb.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(3),
            Constraint::Length(footer_height),
        ])
        .split(rect);
    let input_area = layout[0];
    let body_area = layout[1];
    let footer_area = layout[2];

    // Consistent overlay style (foreground/background)
    // Choose a high-contrast, uniform scheme that doesn't depend on background UI
    let overlay_style = Style::new().fg(Color::LightBlue);

    // Input bar (telescope-like prompt with mode indicator)
    let mode_label = match cb.mode {
        ContextBrowserMode::Insert => "INSERT",
        ContextBrowserMode::Normal => "NORMAL",
    };
    let prompt_prefix = format!("[{mode_label}] ");
    let input_line = Line::from(vec![
        Span::styled(prompt_prefix.as_str(), overlay_style),
        Span::styled(cb.input.as_str(), overlay_style),
    ]);
    let input_widget = Paragraph::new(input_line)
        .style(overlay_style)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Query ")
                .style(overlay_style),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(input_widget, input_area);
    if matches!(cb.mode, ContextBrowserMode::Insert) {
        let cursor_x = input_area.x
            + 1 // left border padding
            + UnicodeWidthStr::width(prompt_prefix.as_str()) as u16
            + cb.input.display_cursor_col();
        let cursor_y = input_area.y + 1;
        frame.set_cursor_position((cursor_x, cursor_y));
    }

    // Build list content (styled). Header moved to Block title; keep only list lines here.
    let mut lines: Vec<Line> = Vec::new();
    // let search_lines: SearchItemLines = SearchItemLines {
    //     lines: lines.clone(),
    // };

    // Loading indicator when opened before results arrive
    if cb.items.is_empty() {
        let empty_msg = if cb.loading_search {
            "Loading search results…"
        } else {
            "No results found"
        };
        lines.push(Line::from(Span::styled(empty_msg, overlay_style)));
    }

    // Selected row highlighting
    // TODO: create a unified style for overlays that both this context search and the model picker
    // overlay can use
    // - add as an associated const/static or something to a trait for this overlay?
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    if cb.loading_search && !cb.items.is_empty() {
        lines.push(Line::from(Span::styled(
            format!("{DETAIL_INDENT}Searching…"),
            detail_style,
        )));
    }

    let selected_idx = cb.selected_index();

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
                if i == selected_idx { ">" } else { " " },
                if i == selected_idx {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                title,
                if i == selected_idx {
                    selected_style
                } else {
                    overlay_style
                },
            ),
        ]);
        // Ensure entire line style is applied (for background fill)
        line.style = if i == selected_idx {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        if it.expanded {
            let indent = DETAIL_INDENT;
            // Indented details for readability while navigating (preserve spaces; do not trim)

            let details_width = body_area
                .width
                .saturating_sub(indent.len() as u16)
                // subtract a few cols for the borders + margin on the left and right.
                .saturating_sub(4);
            for field in it.show_meta_details.iter_details() {
                lines.push(it.format_line_field_val_to_width(field, details_width as usize));
            }

            lines.push(Line::from(Span::styled(
                format!("{indent}Preview:"),
                detail_style,
            )));

            match it.show_preview {
                ShowPreview::NoPreview => {
                    lines.push(Line::from(Span::styled(
                        format!("{indent}Press `l` or RightArrow to show preview"),
                        detail_style,
                    )));
                }
                ShowPreview::Small | ShowPreview::Full => {
                    let max_lines = match it.show_preview {
                        ShowPreview::Small => Some(SMALL_PREVIEW_MAX_LINES),
                        ShowPreview::Full => None,
                        ShowPreview::NoPreview => unreachable!(),
                    };

                    let preview_lines = wrap_preview_lines(
                        &it.context_part.text,
                        detail_style,
                        details_width as usize,
                        max_lines,
                    );
                    lines.extend(preview_lines);
                }
            }
        }
    }
    (body_area, footer_area, overlay_style, lines)
}

/// Provider item height:
///     context_length + supports_tools + pricing
const PROVIDER_DETAILS_HEIGHT: usize = 3;

pub fn context_browser_detail_lines(it: &SearchItem) -> usize {
    if !it.expanded {
        return 0;
    }
    let expanded_rows = if it.expanded {
        DISPLAYED_FIELDS.len()
    } else {
        0
    };
    // add 1 for provider header, "providers:"
    PROVIDER_DETAILS_HEIGHT + 1 + expanded_rows
}

// Header is not part of scrollable content (it's displayed in the Block title).
const CONTEXT_BROWSER_HEADER_HEIGHT: usize = 0;
pub fn context_browser_total_lines(mb: &ContextSearchState) -> usize {
    let searching_line = if mb.loading_search && !mb.items.is_empty() {
        1
    } else {
        0
    };
    let base = CONTEXT_BROWSER_HEADER_HEIGHT
        + searching_line
        + mb.items
            .iter()
            .map(context_browser_detail_lines)
            .map(|it| it + 1)
            .sum::<usize>();
    if mb.items.is_empty() {
        base + 1 // account for "Loading search results…" line
    } else {
        base
    }
}

pub fn context_browser_focus_line(mb: &ContextSearchState) -> usize {
    let header = CONTEXT_BROWSER_HEADER_HEIGHT;
    if mb.items.is_empty() {
        return header;
    }

    let sel_idx = mb.selected_index();

    let mut line = header;
    if mb.loading_search {
        line += 1;
    }
    for j in 0..sel_idx {
        let it = &mb.items[j];
        line += 1; // title
        line += context_browser_detail_lines(it);
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

    let total = context_browser_total_lines(mb);
    let focus = context_browser_focus_line(mb);
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
        let sel_idx = mb.selected_index();
        let sel = &mb.items[sel_idx];
        if sel.expanded {
            // Compute the top line (0-based in content space) of the selected item's title
            let mut block_top = CONTEXT_BROWSER_HEADER_HEIGHT;
            if mb.loading_search {
                block_top += 1;
            }
            for j in 0..sel_idx {
                block_top += 1; // title line
                block_top += context_browser_detail_lines(&mb.items[j]);
            }
            // Height of the expanded block: title + details
            let block_height = 1 + context_browser_detail_lines(sel);
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
