use ploke_core::ArcStr;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style, Stylize as _};

use crate::app::input::keymap::{Action, to_action};
use crate::app::types::{Mode, RenderMsg};
use crate::app::utils::truncate_uuid;
use crate::app::view::components::conversation::ConversationView;
use crate::app::view::components::input_box::InputView;
use crate::llm::request::endpoint::Endpoint;
use crate::llm::router_only::openrouter::providers::ProviderName;
use crate::llm::{EndpointKey, ModelId, ModelKey, ModelName, ProviderKey};
use crate::user_config::OPENROUTER_URL;
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
use tokio::sync::oneshot;
use toml::to_string;
use tracing::instrument;

#[derive(Debug)]
pub struct ModelBrowserItem {
    pub id: ModelId,
    pub name: Option<ModelName>,
    pub context_length: Option<u32>,
    /// Input cost in USD per token (displayed as USD per 1M tokens).
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub supports_tools: bool,
    pub providers: Vec<ModelProviderRow>,
    pub direct_route: bool,
    pub expanded: bool,
    // Runtime flags for async provider loading and deferred selection
    pub loading_providers: bool,
    pub pending_select: bool,
}

#[derive(Debug)]
pub struct ModelProviderRow {
    pub provider_name: ProviderName,
    pub provider_key: ProviderKey,
    pub model_id: ModelId,
    pub context_length: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub supports_tools: bool,
}

impl ModelProviderRow {
    pub(crate) fn from_id_endpoint(m: ModelId, k: &ProviderKey, v: Endpoint) -> Self {
        let supports_tools = v.supports_tools();
        ModelProviderRow {
            provider_key: k.clone(),
            provider_name: v.provider_name,
            context_length: v.context_length as u32,
            input_cost: v.pricing.prompt * 1_000_000.0,
            output_cost: v.pricing.completion * 1_000_000.0,
            supports_tools,
            model_id: m,
        }
    }
}

pub trait ProviderSelectionCandidate {
    fn provider_key(&self) -> ProviderKey;
    fn supports_tools(&self) -> bool;
}

impl ProviderSelectionCandidate for ModelProviderRow {
    fn provider_key(&self) -> ProviderKey {
        self.provider_key.clone()
    }

    fn supports_tools(&self) -> bool {
        self.supports_tools
    }
}

impl ProviderSelectionCandidate for Endpoint {
    fn provider_key(&self) -> ProviderKey {
        ProviderKey {
            slug: self.tag.provider_name.clone(),
        }
    }

    fn supports_tools(&self) -> bool {
        Endpoint::supports_tools(self)
    }
}

pub fn preferred_provider_key<T: ProviderSelectionCandidate>(
    providers: &[T],
) -> Option<ProviderKey> {
    providers
        .iter()
        .find(|p| p.supports_tools())
        .or_else(|| providers.first())
        .map(|p| p.provider_key())
}

pub fn tool_capable_provider_key<T: ProviderSelectionCandidate>(
    providers: &[T],
) -> Option<ProviderKey> {
    providers
        .iter()
        .find(|p| p.supports_tools())
        .map(|p| p.provider_key())
}

#[derive(Debug)]
pub struct ModelBrowserState {
    pub visible: bool,
    pub keyword: String,
    pub items: Vec<ModelBrowserItem>,
    pub selected: usize,
    // Toggle for bottom-right help panel within the Model Browser overlay
    pub help_visible: bool,
    // Provider selection mode for the currently selected item
    pub provider_select_active: bool,
    pub provider_selected: usize,
    // support scrolling
    pub vscroll: u16,
    pub viewport_height: u16,
}

pub fn render_model_browser<'a>(
    frame: &mut Frame<'_>,
    mb: &ModelBrowserState,
) -> (Rect, Rect, Style, Vec<Line<'a>>) {
    let area = frame.area();
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

    // Clear the underlying content in the overlay area to avoid "bleed-through"
    frame.render_widget(ratatui::widgets::Clear, rect);

    // Split overlay into body + footer (help)
    let footer_height = if mb.help_visible { 8 } else { 1 };
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

    // Loading indicator when opened before results arrive
    if mb.items.is_empty() {
        lines.push(Line::from(Span::styled("Loading models…", overlay_style)));
    }

    // Selected row highlighting
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();
    let openrouter_source_style = Style::new().fg(Color::LightMagenta).bold();
    let google_source_style = Style::new().fg(Color::LightGreen).bold();
    let selected_openrouter_source_style = Style::new().fg(Color::Black).bg(Color::LightMagenta);
    let selected_google_source_style = Style::new().fg(Color::Black).bg(Color::LightGreen);

    for (i, it) in mb.items.iter().enumerate() {
        let title = if let Some(name) = &it.name {
            if name.as_str().is_empty() {
                it.id.to_string()
            } else {
                format!("{} — {}", it.id, name.as_str())
            }
        } else {
            it.id.to_string()
        };
        let is_selected = i == mb.selected;
        let source_label = if it.direct_route {
            "[via Google API]"
        } else {
            "[via OpenRouter]"
        };
        let source_style = match (it.direct_route, is_selected) {
            (true, true) => selected_google_source_style,
            (true, false) => google_source_style,
            (false, true) => selected_openrouter_source_style,
            (false, false) => openrouter_source_style,
        };

        let mut line = Line::from(vec![
            Span::styled(
                if is_selected { ">" } else { " " },
                if is_selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                title,
                if is_selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(source_label, source_style),
        ]);
        // Ensure entire line style is applied (for background fill)
        line.style = if is_selected {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        if it.expanded {
            // Indented details for readability while navigating (preserve spaces; do not trim)
            lines.push(Line::from(Span::styled(
                format!(
                    "    source: {}",
                    if it.direct_route {
                        "Google API direct catalog"
                    } else {
                        "OpenRouter catalog"
                    }
                ),
                if it.direct_route {
                    google_source_style
                } else {
                    openrouter_source_style
                },
            )));
            lines.push(Line::from(Span::styled(
                format!(
                    "    context_length: {}",
                    it.context_length
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".to_string())
                ),
                detail_style,
            )));
            lines.push(Line::from(Span::styled(
                format!("    supports_tools: {}", it.supports_tools),
                detail_style,
            )));
            lines.push(Line::from(Span::styled(
                format!(
                    "    pricing: in={} out={}",
                    it.input_cost
                        .map(|v| format!("${:.3}", v))
                        .unwrap_or_else(|| "-".to_string()),
                    it.output_cost
                        .map(|v| format!("${:.3}", v))
                        .unwrap_or_else(|| "-".to_string())
                ),
                detail_style,
            )));

            if it.direct_route {
                lines.push(Line::from(Span::styled(
                    "    route: direct; provider endpoints are not used",
                    detail_style,
                )));
            } else {
                // Provider breakdown (with loading/empty states)
                lines.push(Line::from(Span::styled("    providers:", detail_style)));
            }
            if it.direct_route {
                // route line rendered above
            } else if it.loading_providers {
                lines.push(Line::from(Span::styled("      (loading…)", detail_style)));
            } else if it.providers.is_empty() {
                lines.push(Line::from(Span::styled("      (none)", detail_style)));
            } else {
                for (row_idx, p) in it.providers.iter().enumerate() {
                    let indent = "      ";
                    let is_sel = mb.provider_select_active
                        && i == mb.selected
                        && row_idx == mb.provider_selected;
                    let row_style = if is_sel { selected_style } else { detail_style };
                    let pointer = if is_sel { ">" } else { "-" };
                    lines.push(Line::from(Span::styled(
                        format!(
                            "{indent}{} {} in=${} out=${} ctx={} tools={}",
                            pointer,
                            p.provider_name,
                            format_args!("{:.3}", p.input_cost),
                            format_args!("{:.3}", p.output_cost),
                            format_args!("{:.0}", p.context_length),
                            p.supports_tools,
                        ),
                        row_style,
                    )));
                }
            }
        }
    }
    (body_area, footer_area, overlay_style, lines)
}

/// Provider item height:
///     source + context_length + supports_tools + pricing
const PROVIDER_DETAILS_HEIGHT: usize = 4;

pub fn model_browser_detail_lines(it: &ModelBrowserItem) -> usize {
    if !it.expanded {
        return 0;
    }
    if it.direct_route {
        return PROVIDER_DETAILS_HEIGHT + 1;
    }
    let providers_rows = if it.loading_providers || it.providers.is_empty() {
        1
    } else {
        it.providers.len()
    };
    // add 1 for provider header, "providers:"
    PROVIDER_DETAILS_HEIGHT + 1 + providers_rows
}

// Header is not part of scrollable content (it's displayed in the Block title).
const MODEL_BROWSER_HEADER_HEIGHT: usize = 0;
pub fn model_browser_total_lines(mb: &ModelBrowserState) -> usize {
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

pub fn model_browser_focus_line(mb: &ModelBrowserState) -> usize {
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
    if mb.provider_select_active && sel.expanded && !sel.providers.is_empty() {
        let prov_idx = mb
            .provider_selected
            .min(sel.providers.len().saturating_sub(1));
        line + 1 + PROVIDER_DETAILS_HEIGHT + 1 + prov_idx
    } else {
        line
    }
}

pub(crate) fn compute_browser_scroll(body_area: Rect, mb: &mut ModelBrowserState) {
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
    if !mb.items.is_empty() && !mb.provider_select_active {
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

#[cfg(feature = "test_harness")]
pub fn snapshot_text_for_test(
    models: Vec<TestModelItem>,
    keyword: &str,
    selected: usize,
    provider_select_active: bool,
    provider_selected: usize,
) -> String {
    use ratatui::{Terminal, backend::TestBackend};
    let items: Vec<ModelBrowserItem> = models.into_iter().map(|m| m.into_item()).collect();
    let mb = ModelBrowserState {
        visible: true,
        keyword: keyword.to_string(),
        items,
        selected,
        help_visible: false,
        provider_select_active,
        provider_selected,
        vscroll: 0,
        viewport_height: 25,
    };

    let backend = TestBackend::new(100, 25);
    let mut terminal = Terminal::new(backend).expect("terminal");
    let mut out = String::new();
    terminal
        .draw(|frame| {
            let (_a, _b, _s, lines) = render_model_browser(frame, &mb);
            out = lines_to_text(&lines);
        })
        .expect("draw");
    out
}

#[cfg(feature = "test_harness")]
fn lines_to_text(lines: &[Line<'_>]) -> String {
    lines
        .iter()
        .map(|line| {
            let mut s = String::new();
            for span in &line.spans {
                s.push_str(span.content.as_ref());
            }
            s
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(feature = "test_harness")]
pub struct TestProviderRow {
    pub provider_slug: String,
    pub context_length: u32,
    pub input_cost: f64,
    pub output_cost: f64,
    pub supports_tools: bool,
}

#[cfg(feature = "test_harness")]
pub struct TestModelItem {
    pub id: String,
    pub name: Option<String>,
    pub context_length: Option<u32>,
    pub input_cost: Option<f64>,
    pub output_cost: Option<f64>,
    pub supports_tools: bool,
    pub providers: Vec<TestProviderRow>,
    pub expanded: bool,
    pub loading_providers: bool,
}

#[cfg(feature = "test_harness")]
impl TestModelItem {
    fn into_item(self) -> ModelBrowserItem {
        use std::str::FromStr;
        let id = crate::llm::ModelId::from_str(&self.id).expect("valid model id");
        let name = self.name.map(crate::llm::ModelName::new);
        let providers = self
            .providers
            .into_iter()
            .map(|p| {
                let slug = crate::llm::router_only::openrouter::providers::ProviderSlug::from_str(
                    &p.provider_slug,
                )
                .expect("provider slug");
                let pname = slug.to_provider_name().unwrap_or_else(|| {
                    crate::llm::router_only::openrouter::providers::ProviderName::new(
                        &p.provider_slug,
                    )
                });
                let provider_key =
                    crate::llm::ProviderKey::new(&slug.to_string()).expect("provider key");
                ModelProviderRow {
                    provider_name: pname,
                    provider_key,
                    model_id: id.clone(),
                    context_length: p.context_length,
                    input_cost: p.input_cost,
                    output_cost: p.output_cost,
                    supports_tools: p.supports_tools,
                }
            })
            .collect();
        ModelBrowserItem {
            id,
            name,
            context_length: self.context_length,
            input_cost: self.input_cost,
            output_cost: self.output_cost,
            supports_tools: self.supports_tools,
            providers,
            direct_route: false,
            expanded: self.expanded,
            loading_providers: self.loading_providers,
            pending_select: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    fn model_item(id: &str, direct_route: bool, expanded: bool) -> ModelBrowserItem {
        ModelBrowserItem {
            id: ModelId::from_str(id).expect("model id"),
            name: None,
            context_length: Some(1_048_576),
            input_cost: Some(0.0),
            output_cost: Some(0.0),
            supports_tools: true,
            providers: Vec::new(),
            direct_route,
            expanded,
            loading_providers: false,
            pending_select: false,
        }
    }

    fn render_lines(mb: &ModelBrowserState) -> Vec<Line<'static>> {
        let backend = TestBackend::new(100, 25);
        let mut terminal = Terminal::new(backend).expect("terminal");
        let mut lines = Vec::new();
        terminal
            .draw(|frame| {
                let (_, _, _, rendered) = render_model_browser(frame, mb);
                lines = rendered;
            })
            .expect("draw");
        lines
    }

    fn lines_to_text(lines: &[Line<'_>]) -> String {
        lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn source_badges_and_details_distinguish_openrouter_from_google_rows() {
        let mb = ModelBrowserState {
            visible: true,
            keyword: "gemini".to_string(),
            items: vec![
                model_item("google/gemini-2.5-flash", false, true),
                model_item("google/gemini-2.5-pro", true, true),
            ],
            selected: 0,
            help_visible: false,
            provider_select_active: false,
            provider_selected: 0,
            vscroll: 0,
            viewport_height: 25,
        };

        let lines = render_lines(&mb);
        let text = lines_to_text(&lines);

        assert!(text.contains("google/gemini-2.5-flash [via OpenRouter]"));
        assert!(text.contains("google/gemini-2.5-pro [via Google API]"));
        assert!(text.contains("source: OpenRouter catalog"));
        assert!(text.contains("source: Google API direct catalog"));
        assert!(text.contains("route: direct; provider endpoints are not used"));

        let openrouter_style = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "[via OpenRouter]")
            .expect("openrouter badge")
            .style;
        let google_style = lines
            .iter()
            .flat_map(|line| line.spans.iter())
            .find(|span| span.content.as_ref() == "[via Google API]")
            .expect("google badge")
            .style;
        assert_ne!(openrouter_style, google_style);
    }

    #[test]
    fn google_author_model_from_openrouter_is_labeled_as_openrouter_supplied() {
        let mb = ModelBrowserState {
            visible: true,
            keyword: "gemini".to_string(),
            items: vec![model_item("google/gemini-flash-latest", false, false)],
            selected: 0,
            help_visible: false,
            provider_select_active: false,
            provider_selected: 0,
            vscroll: 0,
            viewport_height: 25,
        };

        let text = lines_to_text(&render_lines(&mb));

        assert!(text.contains("google/gemini-flash-latest [via OpenRouter]"));
        assert!(!text.contains("[via Google API]"));
    }
}
