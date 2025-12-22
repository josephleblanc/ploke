use ploke_embed::local::DevicePreference;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app_state::RuntimeConfig;
use crate::tools::ToolVerbosity;
use crate::user_config::CommandStyle;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOverlayPane {
    Categories,
    Items,
    Values,
}

#[derive(Debug, Clone)]
pub struct ConfigOverlayItem {
    pub label: String,
    pub description: String,
    pub values: Vec<String>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct ConfigOverlayCategory {
    pub name: String,
    pub items: Vec<ConfigOverlayItem>,
}

#[derive(Debug, Clone)]
pub struct ConfigOverlayState {
    pub visible: bool,
    pub categories: Vec<ConfigOverlayCategory>,
    pub category_idx: usize,
    pub item_idx: usize,
    pub value_idx: usize,
    pub pane: ConfigOverlayPane,
    pub help_visible: bool,
    pub dirty: bool,
}

impl ConfigOverlayState {
    pub fn from_runtime_config(cfg: &RuntimeConfig) -> Self {
        let ui_items = vec![
            enum_item(
                "Command Style",
                "How the command palette is invoked.",
                &["Slash", "NeoVim"],
                match cfg.command_style {
                    CommandStyle::Slash => 0,
                    CommandStyle::NeoVim => 1,
                },
            ),
            enum_item(
                "Tool Verbosity",
                "Controls how much detail tool responses include.",
                &["Minimal", "Normal", "Verbose"],
                match cfg.tool_verbosity {
                    ToolVerbosity::Minimal => 0,
                    ToolVerbosity::Normal => 1,
                    ToolVerbosity::Verbose => 2,
                },
            ),
        ];

        let chat_items = vec![bool_item(
            "Retry Without Tools",
            "Retry if tool-capable routing fails.",
            cfg.chat_policy.retry_without_tools_on_404,
        )];

        let rag_items = vec![bool_item(
            "Strict BM25",
            "Use strict BM25 by default when RAG is enabled.",
            cfg.rag.strict_bm25_by_default,
        )];

        let embedding_items = vec![
            enum_item(
                "Device Preference",
                "Preferred device for local embeddings.",
                &["Auto", "ForceCpu", "ForceGpu"],
                match cfg.embedding_local.device_preference {
                    DevicePreference::Auto => 0,
                    DevicePreference::ForceCpu => 1,
                    DevicePreference::ForceGpu => 2,
                },
            ),
            bool_item(
                "Allow Fallback",
                "Permit fallback if the preferred device is unavailable.",
                cfg.embedding_local.allow_fallback,
            ),
            bool_item(
                "Approximate GELU",
                "Use approximate GELU for faster local embeddings.",
                cfg.embedding_local.approximate_gelu,
            ),
            bool_item(
                "Use .pth Weights",
                "Prefer PyTorch .pth weights for local embeddings.",
                cfg.embedding_local.use_pth,
            ),
        ];

        let categories = vec![
            ConfigOverlayCategory {
                name: "UI".to_string(),
                items: ui_items,
            },
            ConfigOverlayCategory {
                name: "Chat".to_string(),
                items: chat_items,
            },
            ConfigOverlayCategory {
                name: "RAG".to_string(),
                items: rag_items,
            },
            ConfigOverlayCategory {
                name: "Embedding".to_string(),
                items: embedding_items,
            },
        ];

        let mut state = ConfigOverlayState {
            visible: true,
            categories,
            category_idx: 0,
            item_idx: 0,
            value_idx: 0,
            pane: ConfigOverlayPane::Categories,
            help_visible: false,
            dirty: false,
        };
        state.sync_value_idx();
        state
    }

    pub fn current_category(&self) -> Option<&ConfigOverlayCategory> {
        self.categories.get(self.category_idx)
    }

    pub fn current_item(&self) -> Option<&ConfigOverlayItem> {
        self.current_category()
            .and_then(|cat| cat.items.get(self.item_idx))
    }

    pub fn current_item_mut(&mut self) -> Option<&mut ConfigOverlayItem> {
        self.categories
            .get_mut(self.category_idx)
            .and_then(|cat| cat.items.get_mut(self.item_idx))
    }

    pub fn sync_value_idx(&mut self) {
        if let Some(item) = self.current_item() {
            self.value_idx = item.selected.min(item.values.len().saturating_sub(1));
        } else {
            self.value_idx = 0;
        }
    }

    pub fn next_pane(&mut self) {
        self.pane = match self.pane {
            ConfigOverlayPane::Categories => ConfigOverlayPane::Items,
            ConfigOverlayPane::Items => ConfigOverlayPane::Values,
            ConfigOverlayPane::Values => ConfigOverlayPane::Categories,
        };
        self.normalize_indices();
    }

    pub fn prev_pane(&mut self) {
        self.pane = match self.pane {
            ConfigOverlayPane::Categories => ConfigOverlayPane::Values,
            ConfigOverlayPane::Items => ConfigOverlayPane::Categories,
            ConfigOverlayPane::Values => ConfigOverlayPane::Items,
        };
        self.normalize_indices();
    }

    pub fn move_up(&mut self) {
        match self.pane {
            ConfigOverlayPane::Categories => {
                let len = self.categories.len();
                if len == 0 {
                    return;
                }
                if self.category_idx > 0 {
                    self.category_idx -= 1;
                } else {
                    self.category_idx = len - 1;
                }
                self.item_idx = 0;
                self.sync_value_idx();
            }
            ConfigOverlayPane::Items => {
                if let Some(cat) = self.current_category() {
                    let len = cat.items.len();
                    if len == 0 {
                        return;
                    }
                    if self.item_idx > 0 {
                        self.item_idx -= 1;
                    } else {
                        self.item_idx = len - 1;
                    }
                    self.sync_value_idx();
                }
            }
            ConfigOverlayPane::Values => {
                if let Some(item) = self.current_item() {
                    let len = item.values.len();
                    if len == 0 {
                        return;
                    }
                    if self.value_idx > 0 {
                        self.value_idx -= 1;
                    } else {
                        self.value_idx = len - 1;
                    }
                }
            }
        }
    }

    pub fn move_down(&mut self) {
        match self.pane {
            ConfigOverlayPane::Categories => {
                let len = self.categories.len();
                if len == 0 {
                    return;
                }
                if self.category_idx + 1 < len {
                    self.category_idx += 1;
                } else {
                    self.category_idx = 0;
                }
                self.item_idx = 0;
                self.sync_value_idx();
            }
            ConfigOverlayPane::Items => {
                if let Some(cat) = self.current_category() {
                    let len = cat.items.len();
                    if len == 0 {
                        return;
                    }
                    if self.item_idx + 1 < len {
                        self.item_idx += 1;
                    } else {
                        self.item_idx = 0;
                    }
                    self.sync_value_idx();
                }
            }
            ConfigOverlayPane::Values => {
                if let Some(item) = self.current_item() {
                    let len = item.values.len();
                    if len == 0 {
                        return;
                    }
                    if self.value_idx + 1 < len {
                        self.value_idx += 1;
                    } else {
                        self.value_idx = 0;
                    }
                }
            }
        }
    }

    pub fn activate(&mut self) {
        match self.pane {
            ConfigOverlayPane::Categories => {
                self.pane = ConfigOverlayPane::Items;
                self.item_idx = 0;
                self.sync_value_idx();
            }
            ConfigOverlayPane::Items => {
                self.pane = ConfigOverlayPane::Values;
                self.sync_value_idx();
            }
            ConfigOverlayPane::Values => {
                let value_idx = self.value_idx;
                if let Some(item) = self.current_item_mut()
                    && item.selected != value_idx
                {
                    item.selected = value_idx;
                    self.dirty = true;
                }
            }
        }
    }

    pub fn normalize_indices(&mut self) {
        let cat_len = self.categories.len();
        if cat_len == 0 {
            self.category_idx = 0;
            self.item_idx = 0;
            self.value_idx = 0;
            return;
        }
        self.category_idx = self.category_idx.min(cat_len - 1);
        let item_len = self
            .categories
            .get(self.category_idx)
            .map(|c| c.items.len())
            .unwrap_or(0);
        if item_len == 0 {
            self.item_idx = 0;
            self.value_idx = 0;
            return;
        }
        self.item_idx = self.item_idx.min(item_len - 1);
        self.sync_value_idx();
    }
}

pub fn render_config_overlay(frame: &mut Frame<'_>, cfg: &ConfigOverlayState) {
    let area = frame.area();
    let width = area.width.saturating_mul(8) / 10;
    let height = area.height.saturating_mul(8) / 10;
    let x = area.x.saturating_add(area.width.saturating_sub(width) / 2);
    let y = area
        .y
        .saturating_add(area.height.saturating_sub(height) / 2);
    let rect = Rect::new(x, y, width.max(50), height.max(12));

    frame.render_widget(ratatui::widgets::Clear, rect);

    let footer_height = if cfg.help_visible { 5 } else { 4 };
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(footer_height)])
        .split(rect);
    let body_area = layout[0];
    let footer_area = layout[1];

    let overlay_style = Style::new().fg(Color::LightBlue);
    let selected_style = Style::new().fg(Color::Black).bg(Color::LightCyan);

    let pane_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(40),
            Constraint::Percentage(35),
        ])
        .split(body_area);

    let category_lines = build_category_lines(cfg, overlay_style, selected_style);
    let item_lines = build_item_lines(cfg, overlay_style, selected_style);
    let value_lines = build_value_lines(cfg, overlay_style, selected_style);

    let cat_title = pane_title(" Categories ", cfg.pane == ConfigOverlayPane::Categories);
    let item_title = pane_title(" Settings ", cfg.pane == ConfigOverlayPane::Items);
    let value_title = pane_title(" Values ", cfg.pane == ConfigOverlayPane::Values);

    let cat_widget = Paragraph::new(category_lines)
        .style(overlay_style)
        .block(Block::bordered().title(cat_title).style(overlay_style))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(cat_widget, pane_layout[0]);

    let item_widget = Paragraph::new(item_lines)
        .style(overlay_style)
        .block(Block::bordered().title(item_title).style(overlay_style))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(item_widget, pane_layout[1]);

    let value_widget = Paragraph::new(value_lines)
        .style(overlay_style)
        .block(Block::bordered().title(value_title).style(overlay_style))
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(value_widget, pane_layout[2]);

    let desc = cfg
        .current_item()
        .map(|it| format!("{} — {}", it.label, it.description))
        .unwrap_or_else(|| "No setting selected.".to_string());

    let footer_text = if cfg.help_visible {
        format!(
            "Keys: tab/shift+tab=change pane  ↑/↓=navigate  Enter=select value  q/Esc=close\n\
             Note: changes are staged only (apply/persist not wired yet).\n\
             {desc}"
        )
    } else {
        format!(
            "{} \n ? Help  | Enter select  | Tab pane  | q/Esc close",
            desc
        )
    };

    let footer = Paragraph::new(footer_text)
        .style(overlay_style)
        .block(Block::bordered().title(" Help ").style(overlay_style))
        .wrap(ratatui::widgets::Wrap { trim: true });
    frame.render_widget(footer, footer_area);
}

fn pane_title(base: &str, focused: bool) -> String {
    if focused {
        format!("{base}*")
    } else {
        base.to_string()
    }
}

fn bool_item(label: &str, description: &str, value: bool) -> ConfigOverlayItem {
    ConfigOverlayItem {
        label: label.to_string(),
        description: description.to_string(),
        values: vec!["false".to_string(), "true".to_string()],
        selected: if value { 1 } else { 0 },
    }
}

fn enum_item(
    label: &str,
    description: &str,
    values: &[&str],
    selected: usize,
) -> ConfigOverlayItem {
    ConfigOverlayItem {
        label: label.to_string(),
        description: description.to_string(),
        values: values.iter().map(|v| v.to_string()).collect(),
        selected: selected.min(values.len().saturating_sub(1)),
    }
}

fn build_category_lines(
    cfg: &ConfigOverlayState,
    overlay_style: Style,
    selected_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (idx, cat) in cfg.categories.iter().enumerate() {
        let is_selected = idx == cfg.category_idx;
        let focused = cfg.pane == ConfigOverlayPane::Categories;
        let style = if is_selected && focused {
            selected_style
        } else {
            overlay_style
        };
        let prefix = if is_selected { ">" } else { " " };
        let mut line = Line::from(vec![
            Span::styled(prefix, style),
            Span::raw(" "),
            Span::styled(cat.name.clone(), style),
        ]);
        line.style = style;
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("(no categories)", overlay_style)));
    }
    lines
}

fn build_item_lines(
    cfg: &ConfigOverlayState,
    overlay_style: Style,
    selected_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let Some(cat) = cfg.current_category() else {
        lines.push(Line::from(Span::styled("(no settings)", overlay_style)));
        return lines;
    };
    for (idx, item) in cat.items.iter().enumerate() {
        let is_selected = idx == cfg.item_idx;
        let focused = cfg.pane == ConfigOverlayPane::Items;
        let style = if is_selected && focused {
            selected_style
        } else {
            overlay_style
        };
        let value = item
            .values
            .get(item.selected)
            .cloned()
            .unwrap_or_else(|| "-".to_string());
        let label = format!("{}: {}", item.label, value);
        let prefix = if is_selected { ">" } else { " " };
        let mut line = Line::from(vec![
            Span::styled(prefix, style),
            Span::raw(" "),
            Span::styled(label, style),
        ]);
        line.style = style;
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("(no settings)", overlay_style)));
    }
    lines
}

fn build_value_lines(
    cfg: &ConfigOverlayState,
    overlay_style: Style,
    selected_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let Some(item) = cfg.current_item() else {
        lines.push(Line::from(Span::styled("(no values)", overlay_style)));
        return lines;
    };
    for (idx, value) in item.values.iter().enumerate() {
        let is_selected = idx == cfg.value_idx;
        let focused = cfg.pane == ConfigOverlayPane::Values;
        let style = if is_selected && focused {
            selected_style
        } else {
            overlay_style
        };
        let prefix = if is_selected { ">" } else { " " };
        let mut line = Line::from(vec![
            Span::styled(prefix, style),
            Span::raw(" "),
            Span::styled(value.clone(), style),
        ]);
        line.style = style;
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(Line::from(Span::styled("(no values)", overlay_style)));
    }
    lines
}
