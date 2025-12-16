use std::collections::HashMap;

use itertools::Itertools;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style, Stylize as _},
    text::{Line, Span},
};
use serde_json::Value;

use super::context_browser::StepEnum;
use crate::ModelId;
use crate::llm::types::model_types::Architecture;
use crate::llm::types::newtypes::ModelName;
use ploke_core::ArcStr;
use ploke_llm::{SupportedParameters, request::ModelPricing, router_only::openrouter::TopProvider};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EmbeddingDetail {
    #[default]
    Collapsed,
    Summary,
    Full,
}

impl StepEnum<3> for EmbeddingDetail {
    const ORDER: [Self; 3] = [Self::Collapsed, Self::Summary, Self::Full];

    fn idx(self) -> usize {
        match self {
            Self::Collapsed => 0,
            Self::Summary => 1,
            Self::Full => 2,
        }
    }
}

#[derive(Debug)]
pub struct EmbeddingBrowserItem {
    pub id: ModelId,
    pub name: ModelName,
    pub created: i64,
    pub architecture: Architecture,
    pub top_provider: TopProvider,
    pub pricing: ModelPricing,
    pub canonical: Option<ModelId>,
    pub context_length: Option<u32>,
    pub hugging_face_id: Option<String>,
    pub per_request_limits: Option<HashMap<String, Value>>,
    pub supported_parameters: Option<Vec<SupportedParameters>>,
    pub description: ArcStr,
    pub detail: EmbeddingDetail,
}

#[derive(Debug)]
pub struct EmbeddingBrowserState {
    pub visible: bool,
    pub keyword: String,
    pub items: Vec<EmbeddingBrowserItem>,
    pub selected: usize,
    pub help_visible: bool,
    // support scrolling
    pub vscroll: u16,
    pub viewport_height: u16,
}

const EMBEDDING_BROWSER_HEADER_HEIGHT: usize = 1;

fn format_price_per_million(val: f64) -> String {
    format!("${:.4}/1M", val * 1_000_000.0)
}

fn format_optional_price(val: Option<f64>) -> String {
    val.map(format_price_per_million)
        .unwrap_or_else(|| "-".to_string())
}

fn truncate_with_ellipsis(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_len.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn context_length(it: &EmbeddingBrowserItem) -> Option<u32> {
    it.context_length.or(it.top_provider.context_length)
}

fn summary_detail_rows(it: &EmbeddingBrowserItem) -> Vec<String> {
    vec![
        format!(
            "    price (prompt): {}",
            format_price_per_million(it.pricing.prompt)
        ),
        format!(
            "    context_length: {}",
            context_length(it)
                .map(|v| v.to_string())
                .unwrap_or_else(|| "-".to_string())
        ),
        format!(
            "    desc: {}",
            truncate_with_ellipsis(it.description.as_ref(), 160)
        ),
    ]
}

fn full_detail_rows(it: &EmbeddingBrowserItem) -> Vec<String> {
    let mut rows = Vec::new();
    rows.push(format!("    created: {}", it.created));
    rows.push(format!(
        "    canonical: {}",
        it.canonical
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| "-".to_string())
    ));
    rows.push(format!(
        "    architecture: modality={:?}, tokenizer={:?}, inputs={}, outputs={}",
        it.architecture.modality,
        it.architecture.tokenizer,
        it.architecture
            .input_modalities
            .iter()
            .map(|m| format!("{m:?}"))
            .join(", "),
        it.architecture
            .output_modalities
            .iter()
            .map(|m| format!("{m:?}"))
            .join(", ")
    ));
    rows.push(format!(
        "    top_provider: ctx={} moderated={} max_completion_tokens={}",
        it.top_provider
            .context_length
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string()),
        it.top_provider.is_moderated,
        it.top_provider
            .max_completion_tokens
            .map(|v| v.to_string())
            .unwrap_or_else(|| "-".to_string())
    ));
    rows.push(format!(
        "    pricing: prompt={} completion={} request={} image_in={} image_out={} audio={} cache_read={} cache_write={} internal_reasoning={} web_search={}",
        format_price_per_million(it.pricing.prompt),
        format_price_per_million(it.pricing.completion),
        format_optional_price(it.pricing.request),
        format_optional_price(it.pricing.image),
        format_optional_price(it.pricing.image_output),
        format_optional_price(it.pricing.audio),
        format_optional_price(it.pricing.input_cache_read),
        format_optional_price(it.pricing.input_cache_write),
        format_optional_price(it.pricing.internal_reasoning),
        format_optional_price(it.pricing.web_search),
    ));
    rows.push(format!(
        "    hugging_face_id: {}",
        it.hugging_face_id.as_deref().unwrap_or("-")
    ));
    if let Some(params) = &it.supported_parameters {
        rows.push(format!(
            "    supported_parameters: {}",
            params.iter().map(|p| format!("{p:?}")).join(", ")
        ));
    }
    if let Some(prl) = &it.per_request_limits {
        let prl_str = serde_json::to_string(prl).unwrap_or_else(|_| "<invalid limits>".to_string());
        rows.push(format!("    per_request_limits: {}", prl_str));
    }
    rows.push(format!(
        "    description_full: {}",
        truncate_with_ellipsis(it.description.as_ref(), 320)
    ));
    rows
}

fn embedding_detail_rows(it: &EmbeddingBrowserItem) -> Vec<String> {
    match it.detail {
        EmbeddingDetail::Collapsed => Vec::new(),
        EmbeddingDetail::Summary => summary_detail_rows(it),
        EmbeddingDetail::Full => {
            let mut rows = summary_detail_rows(it);
            rows.extend(full_detail_rows(it));
            rows
        }
    }
}

pub fn render_embedding_browser<'a>(
    frame: &mut Frame<'_>,
    eb: &EmbeddingBrowserState,
) -> (Rect, Rect, Style, Vec<Line<'a>>) {
    let area = frame.area();
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = ratatui::layout::Rect::new(x, y, width.max(40), height.max(10));

    // Clear the underlying content in the overlay area to avoid bleed-through
    frame.render_widget(ratatui::widgets::Clear, rect);

    // Split overlay into body + footer (help)
    let footer_height = if eb.help_visible { 6 } else { 1 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    let body_area = layout[0];
    let footer_area = layout[1];

    let overlay_style = Style::new().fg(Color::LightBlue);
    let mut lines: Vec<Line> = Vec::new();

    if eb.items.is_empty() {
        lines.push(Line::from(Span::styled(
            "Loading embedding models…",
            overlay_style,
        )));
    }

    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);
    let detail_style = Style::new().fg(Color::Blue).dim();

    for (i, it) in eb.items.iter().enumerate() {
        let mut line = Line::from(vec![
            Span::styled(
                if i == eb.selected { ">" } else { " " },
                if i == eb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
            Span::raw(" "),
            Span::styled(
                format!("{} — {}", it.id, it.name.as_str()),
                if i == eb.selected {
                    selected_style
                } else {
                    overlay_style
                },
            ),
        ]);
        line.style = if i == eb.selected {
            selected_style
        } else {
            overlay_style
        };
        lines.push(line);

        for detail in embedding_detail_rows(it) {
            lines.push(Line::from(Span::styled(detail, detail_style)));
        }
    }

    (body_area, footer_area, overlay_style, lines)
}

pub fn embedding_browser_detail_lines(it: &EmbeddingBrowserItem) -> usize {
    embedding_detail_rows(it).len()
}

pub fn embedding_browser_total_lines(eb: &EmbeddingBrowserState) -> usize {
    let base = EMBEDDING_BROWSER_HEADER_HEIGHT
        + eb.items
            .iter()
            .map(embedding_browser_detail_lines)
            .map(|it| it + 1)
            .sum::<usize>();
    if eb.items.is_empty() { base + 1 } else { base }
}

pub fn embedding_browser_focus_line(eb: &EmbeddingBrowserState) -> usize {
    let header = EMBEDDING_BROWSER_HEADER_HEIGHT;
    if eb.items.is_empty() {
        return header;
    }

    let sel_idx = eb.selected.min(eb.items.len().saturating_sub(1));
    let mut line = header;
    for j in 0..sel_idx {
        let it = &eb.items[j];
        line += 1;
        line += embedding_browser_detail_lines(it);
    }
    line
}

pub(crate) fn compute_embedding_browser_scroll(body_area: Rect, eb: &mut EmbeddingBrowserState) {
    eb.viewport_height = body_area.height.saturating_sub(2);

    let total = embedding_browser_total_lines(eb);
    let focus = embedding_browser_focus_line(eb);
    let vh = eb.viewport_height as usize;
    let max_v = total.saturating_sub(vh);

    eb.vscroll = (eb.vscroll as usize).min(max_v) as u16;

    let v = eb.vscroll as usize;
    if focus < v {
        eb.vscroll = focus as u16;
    } else if focus >= v + vh {
        eb.vscroll = (focus + 1).saturating_sub(vh) as u16;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use ploke_llm::{
        InputModality, Modality, OutputModality, Tokenizer, router_only::openrouter::TopProvider,
    };

    fn sample_item(detail: EmbeddingDetail) -> EmbeddingBrowserItem {
        EmbeddingBrowserItem {
            id: ModelId::from_str("author/model").expect("model id"),
            name: ModelName::new("model"),
            created: 0,
            architecture: Architecture {
                input_modalities: vec![InputModality::Text],
                modality: Modality::TextToText,
                output_modalities: vec![OutputModality::Embeddings],
                tokenizer: Tokenizer::Llama3,
                instruct_type: None,
            },
            top_provider: TopProvider {
                context_length: Some(10),
                is_moderated: false,
                max_completion_tokens: Some(5),
            },
            pricing: ModelPricing {
                prompt: 0.0,
                completion: 0.0,
                audio: None,
                image: None,
                image_output: None,
                input_cache_read: None,
                input_cache_write: None,
                internal_reasoning: None,
                request: None,
                web_search: None,
                discount: None,
            },
            canonical: None,
            context_length: Some(10),
            hugging_face_id: None,
            per_request_limits: None,
            supported_parameters: None,
            description: ArcStr::from("desc"),
            detail,
        }
    }

    #[test]
    fn detail_lines_zero_when_not_expanded() {
        let item = sample_item(EmbeddingDetail::Collapsed);
        assert_eq!(embedding_browser_detail_lines(&item), 0);
    }

    #[test]
    fn detail_lines_present_when_summary() {
        let item = sample_item(EmbeddingDetail::Summary);
        assert_eq!(embedding_browser_detail_lines(&item), 3);
    }

    #[test]
    fn total_lines_accounts_for_loading() {
        let eb = EmbeddingBrowserState {
            visible: true,
            keyword: "kw".to_string(),
            items: Vec::new(),
            selected: 0,
            help_visible: false,
            vscroll: 0,
            viewport_height: 10,
        };
        assert_eq!(
            embedding_browser_total_lines(&eb),
            EMBEDDING_BROWSER_HEADER_HEIGHT + 1
        );
    }
}
