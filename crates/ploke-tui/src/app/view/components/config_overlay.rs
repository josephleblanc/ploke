use ploke_embed::local::DevicePreference;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app_state::RuntimeConfig;
use crate::tools::ToolVerbosity;
use crate::user_config::{
    CommandStyle, CtxMode, MessageVerbosity, MessageVerbosityProfile, VerbosityLevel,
};

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
            enum_item(
                "Default Message Verbosity",
                "Default conversation verbosity profile for UI message rendering.",
                &["Minimal", "Normal", "Verbose", "Custom"],
                match cfg.default_verbosity {
                    MessageVerbosityProfile::Minimal => 0,
                    MessageVerbosityProfile::Normal => 1,
                    MessageVerbosityProfile::Verbose => 2,
                    MessageVerbosityProfile::Custom => 3,
                },
            ),
        ];
        let len_values = ["None", "120", "220", "512", "1024", "2048", "4096", "8192"];
        let message_verbosity_items = vec![
            enum_item(
                "Minimal User Max Len",
                "Maximum rendered length for user messages in Minimal profile.",
                &len_values,
                max_len_to_idx(user_max_len_for_profile(&cfg.message_verbosity_profiles.minimal)),
            ),
            bool_item(
                "Minimal User Syntax Highlight",
                "Whether Minimal profile enables user syntax highlighting.",
                user_syntax_for_profile(&cfg.message_verbosity_profiles.minimal),
            ),
            enum_item(
                "Minimal Assistant Max Len",
                "Maximum rendered length for assistant messages in Minimal profile.",
                &len_values,
                max_len_to_idx(assistant_max_len_for_profile(&cfg.message_verbosity_profiles.minimal)),
            ),
            bool_item(
                "Minimal Assistant Syntax Highlight",
                "Whether Minimal profile enables assistant syntax highlighting.",
                assistant_syntax_for_profile(&cfg.message_verbosity_profiles.minimal),
            ),
            bool_item(
                "Minimal Assistant Truncate Prev",
                "Whether Minimal profile truncates earlier assistant messages.",
                assistant_truncate_prev_for_profile(&cfg.message_verbosity_profiles.minimal),
            ),
            enum_item(
                "Minimal Assistant Truncated Len",
                "Rendered length for earlier assistant messages in Minimal profile.",
                &len_values,
                max_len_to_idx(assistant_truncated_len_for_profile(
                    &cfg.message_verbosity_profiles.minimal,
                )),
            ),
            enum_item(
                "Minimal SysInfo Level",
                "Severity threshold shown for SysInfo in Minimal profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(sysinfo_level_for_profile(
                    &cfg.message_verbosity_profiles.minimal,
                )),
            ),
            enum_item(
                "Minimal System Level",
                "Severity threshold shown for System messages in Minimal profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(system_level_for_profile(
                    &cfg.message_verbosity_profiles.minimal,
                )),
            ),
            bool_item(
                "Minimal Show Init System",
                "Whether Minimal profile renders the initial base system message.",
                display_init_for_profile(&cfg.message_verbosity_profiles.minimal),
            ),
            enum_item(
                "Normal User Max Len",
                "Maximum rendered length for user messages in Normal profile.",
                &len_values,
                max_len_to_idx(user_max_len_for_profile(&cfg.message_verbosity_profiles.normal)),
            ),
            bool_item(
                "Normal User Syntax Highlight",
                "Whether Normal profile enables user syntax highlighting.",
                user_syntax_for_profile(&cfg.message_verbosity_profiles.normal),
            ),
            enum_item(
                "Normal Assistant Max Len",
                "Maximum rendered length for assistant messages in Normal profile.",
                &len_values,
                max_len_to_idx(assistant_max_len_for_profile(&cfg.message_verbosity_profiles.normal)),
            ),
            bool_item(
                "Normal Assistant Syntax Highlight",
                "Whether Normal profile enables assistant syntax highlighting.",
                assistant_syntax_for_profile(&cfg.message_verbosity_profiles.normal),
            ),
            bool_item(
                "Normal Assistant Truncate Prev",
                "Whether Normal profile truncates earlier assistant messages.",
                assistant_truncate_prev_for_profile(&cfg.message_verbosity_profiles.normal),
            ),
            enum_item(
                "Normal Assistant Truncated Len",
                "Rendered length for earlier assistant messages in Normal profile.",
                &len_values,
                max_len_to_idx(assistant_truncated_len_for_profile(
                    &cfg.message_verbosity_profiles.normal,
                )),
            ),
            enum_item(
                "Normal SysInfo Level",
                "Severity threshold shown for SysInfo in Normal profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(sysinfo_level_for_profile(
                    &cfg.message_verbosity_profiles.normal,
                )),
            ),
            enum_item(
                "Normal System Level",
                "Severity threshold shown for System messages in Normal profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(system_level_for_profile(
                    &cfg.message_verbosity_profiles.normal,
                )),
            ),
            bool_item(
                "Normal Show Init System",
                "Whether Normal profile renders the initial base system message.",
                display_init_for_profile(&cfg.message_verbosity_profiles.normal),
            ),
            enum_item(
                "Verbose User Max Len",
                "Maximum rendered length for user messages in Verbose profile.",
                &len_values,
                max_len_to_idx(user_max_len_for_profile(&cfg.message_verbosity_profiles.verbose)),
            ),
            bool_item(
                "Verbose User Syntax Highlight",
                "Whether Verbose profile enables user syntax highlighting.",
                user_syntax_for_profile(&cfg.message_verbosity_profiles.verbose),
            ),
            enum_item(
                "Verbose Assistant Max Len",
                "Maximum rendered length for assistant messages in Verbose profile.",
                &len_values,
                max_len_to_idx(assistant_max_len_for_profile(&cfg.message_verbosity_profiles.verbose)),
            ),
            bool_item(
                "Verbose Assistant Syntax Highlight",
                "Whether Verbose profile enables assistant syntax highlighting.",
                assistant_syntax_for_profile(&cfg.message_verbosity_profiles.verbose),
            ),
            bool_item(
                "Verbose Assistant Truncate Prev",
                "Whether Verbose profile truncates earlier assistant messages.",
                assistant_truncate_prev_for_profile(&cfg.message_verbosity_profiles.verbose),
            ),
            enum_item(
                "Verbose Assistant Truncated Len",
                "Rendered length for earlier assistant messages in Verbose profile.",
                &len_values,
                max_len_to_idx(assistant_truncated_len_for_profile(
                    &cfg.message_verbosity_profiles.verbose,
                )),
            ),
            enum_item(
                "Verbose SysInfo Level",
                "Severity threshold shown for SysInfo in Verbose profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(sysinfo_level_for_profile(
                    &cfg.message_verbosity_profiles.verbose,
                )),
            ),
            enum_item(
                "Verbose System Level",
                "Severity threshold shown for System messages in Verbose profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(system_level_for_profile(
                    &cfg.message_verbosity_profiles.verbose,
                )),
            ),
            bool_item(
                "Verbose Show Init System",
                "Whether Verbose profile renders the initial base system message.",
                display_init_for_profile(&cfg.message_verbosity_profiles.verbose),
            ),
            enum_item(
                "Custom User Max Len",
                "Maximum rendered length for user messages in Custom profile.",
                &len_values,
                max_len_to_idx(user_max_len_for_profile(&cfg.message_verbosity_profiles.custom)),
            ),
            bool_item(
                "Custom User Syntax Highlight",
                "Whether Custom profile enables user syntax highlighting.",
                user_syntax_for_profile(&cfg.message_verbosity_profiles.custom),
            ),
            enum_item(
                "Custom Assistant Max Len",
                "Maximum rendered length for assistant messages in Custom profile.",
                &len_values,
                max_len_to_idx(assistant_max_len_for_profile(&cfg.message_verbosity_profiles.custom)),
            ),
            bool_item(
                "Custom Assistant Syntax Highlight",
                "Whether Custom profile enables assistant syntax highlighting.",
                assistant_syntax_for_profile(&cfg.message_verbosity_profiles.custom),
            ),
            bool_item(
                "Custom Assistant Truncate Prev",
                "Whether Custom profile truncates earlier assistant messages.",
                assistant_truncate_prev_for_profile(&cfg.message_verbosity_profiles.custom),
            ),
            enum_item(
                "Custom Assistant Truncated Len",
                "Rendered length for earlier assistant messages in Custom profile.",
                &len_values,
                max_len_to_idx(assistant_truncated_len_for_profile(
                    &cfg.message_verbosity_profiles.custom,
                )),
            ),
            enum_item(
                "Custom SysInfo Level",
                "Severity threshold shown for SysInfo in Custom profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(sysinfo_level_for_profile(
                    &cfg.message_verbosity_profiles.custom,
                )),
            ),
            enum_item(
                "Custom System Level",
                "Severity threshold shown for System messages in Custom profile.",
                &["Info", "Debug", "Warn", "Error"],
                verbosity_level_to_idx(system_level_for_profile(
                    &cfg.message_verbosity_profiles.custom,
                )),
            ),
            bool_item(
                "Custom Show Init System",
                "Whether Custom profile renders the initial base system message.",
                display_init_for_profile(&cfg.message_verbosity_profiles.custom),
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

        let context_top_k_values = [3_usize, 5, 8, 10, 12, 16, 20, 30, 40, 60, 80, 100, 150, 200];
        let context_per_part_values = [
            32_usize, 64, 96, 128, 160, 256, 320, 512, 768, 1024, 1536, 2048, 3072, 4096,
        ];
        let context_items = vec![
            enum_item(
                "Context Mode",
                "Cycle auto-retrieval intensity (Off/Light/Heavy).",
                &["Off", "Light", "Heavy"],
                match cfg.context_management.mode {
                    CtxMode::Off => 0,
                    CtxMode::Light => 1,
                    CtxMode::Heavy => 2,
                },
            ),
            numeric_item(
                "Light top_k",
                "Retrieved parts count in Light mode.",
                &context_top_k_values,
                cfg.context_management.modes.light.top_k,
            ),
            numeric_item(
                "Light per-part max",
                "Token cap per part in Light mode.",
                &context_per_part_values,
                cfg.context_management.modes.light.per_part_max_tokens,
            ),
            numeric_item(
                "Heavy top_k",
                "Retrieved parts count in Heavy mode.",
                &context_top_k_values,
                cfg.context_management.modes.heavy.top_k,
            ),
            numeric_item(
                "Heavy per-part max",
                "Token cap per part in Heavy mode.",
                &context_per_part_values,
                cfg.context_management.modes.heavy.per_part_max_tokens,
            ),
        ];

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
                name: "Message Verbosity".to_string(),
                items: message_verbosity_items,
            },
            ConfigOverlayCategory {
                name: "Context".to_string(),
                items: context_items,
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

    pub fn adjust_numeric_value(&mut self, delta: i64) -> bool {
        if self.pane != ConfigOverlayPane::Values {
            return false;
        }
        let Some(item) = self.current_item_mut() else {
            return false;
        };
        let current = item
            .values
            .get(item.selected)
            .and_then(|v| v.parse::<i64>().ok());
        let Some(current) = current else {
            return false;
        };
        let mut numeric_values: Vec<i64> = item
            .values
            .iter()
            .map(|v| v.parse::<i64>().ok())
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        if numeric_values.len() != item.values.len() {
            return false;
        }
        let next = (current + delta).max(1);
        if next == current {
            return false;
        }
        if !numeric_values.contains(&next) {
            numeric_values.push(next);
        }
        numeric_values.sort_unstable();
        numeric_values.dedup();
        item.values = numeric_values.iter().map(|v| v.to_string()).collect();
        item.selected = numeric_values.iter().position(|v| *v == next).unwrap_or(0);
        self.value_idx = item.selected.min(item.values.len().saturating_sub(1));
        self.dirty = true;
        true
    }

    pub fn apply_to_runtime_config(&self, cfg: &mut RuntimeConfig) -> bool {
        let mut changed = false;

        if let Some(value) = self.selected_value("UI", "Command Style") {
            let next = match value {
                "Slash" => CommandStyle::Slash,
                "NeoVim" => CommandStyle::NeoVim,
                _ => cfg.command_style,
            };
            if cfg.command_style != next {
                cfg.command_style = next;
                changed = true;
            }
        }
        if let Some(value) = self.selected_value("UI", "Tool Verbosity") {
            let next = match value {
                "Minimal" => ToolVerbosity::Minimal,
                "Normal" => ToolVerbosity::Normal,
                "Verbose" => ToolVerbosity::Verbose,
                _ => cfg.tool_verbosity,
            };
            if cfg.tool_verbosity != next {
                cfg.tool_verbosity = next;
                changed = true;
            }
        }
        if let Some(value) = self.selected_value("UI", "Default Message Verbosity") {
            let next = match value {
                "Minimal" => MessageVerbosityProfile::Minimal,
                "Normal" => MessageVerbosityProfile::Normal,
                "Verbose" => MessageVerbosityProfile::Verbose,
                "Custom" => MessageVerbosityProfile::Custom,
                _ => cfg.default_verbosity,
            };
            if cfg.default_verbosity != next {
                cfg.default_verbosity = next;
                changed = true;
            }
        }
        changed |= apply_message_verbosity_profile_settings(self, cfg);
        if let Some(value) = self.selected_value("Chat", "Retry Without Tools") {
            if let Some(next) = parse_bool(value) {
                if cfg.chat_policy.retry_without_tools_on_404 != next {
                    cfg.chat_policy.retry_without_tools_on_404 = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("RAG", "Strict BM25") {
            if let Some(next) = parse_bool(value) {
                if cfg.rag.strict_bm25_by_default != next {
                    cfg.rag.strict_bm25_by_default = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Context", "Context Mode") {
            let next = match value {
                "Off" => CtxMode::Off,
                "Light" => CtxMode::Light,
                "Heavy" => CtxMode::Heavy,
                _ => cfg.context_management.mode,
            };
            if cfg.context_management.mode != next {
                cfg.context_management.mode = next;
                changed = true;
            }
        }
        if let Some(value) = self.selected_value("Context", "Light top_k") {
            if let Ok(next) = value.parse::<usize>() {
                if cfg.context_management.modes.light.top_k != next {
                    cfg.context_management.modes.light.top_k = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Context", "Light per-part max") {
            if let Ok(next) = value.parse::<usize>() {
                if cfg.context_management.modes.light.per_part_max_tokens != next {
                    cfg.context_management.modes.light.per_part_max_tokens = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Context", "Heavy top_k") {
            if let Ok(next) = value.parse::<usize>() {
                if cfg.context_management.modes.heavy.top_k != next {
                    cfg.context_management.modes.heavy.top_k = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Context", "Heavy per-part max") {
            if let Ok(next) = value.parse::<usize>() {
                if cfg.context_management.modes.heavy.per_part_max_tokens != next {
                    cfg.context_management.modes.heavy.per_part_max_tokens = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Embedding", "Device Preference") {
            let next = match value {
                "Auto" => DevicePreference::Auto,
                "ForceCpu" => DevicePreference::ForceCpu,
                "ForceGpu" => DevicePreference::ForceGpu,
                _ => cfg.embedding_local.device_preference,
            };
            if cfg.embedding_local.device_preference != next {
                cfg.embedding_local.device_preference = next;
                changed = true;
            }
        }
        if let Some(value) = self.selected_value("Embedding", "Allow Fallback") {
            if let Some(next) = parse_bool(value) {
                if cfg.embedding_local.allow_fallback != next {
                    cfg.embedding_local.allow_fallback = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Embedding", "Approximate GELU") {
            if let Some(next) = parse_bool(value) {
                if cfg.embedding_local.approximate_gelu != next {
                    cfg.embedding_local.approximate_gelu = next;
                    changed = true;
                }
            }
        }
        if let Some(value) = self.selected_value("Embedding", "Use .pth Weights") {
            if let Some(next) = parse_bool(value) {
                if cfg.embedding_local.use_pth != next {
                    cfg.embedding_local.use_pth = next;
                    changed = true;
                }
            }
        }

        changed
    }

    pub fn selected_command_style(&self) -> Option<CommandStyle> {
        self.selected_value("UI", "Command Style")
            .and_then(|value| match value {
                "Slash" => Some(CommandStyle::Slash),
                "NeoVim" => Some(CommandStyle::NeoVim),
                _ => None,
            })
    }

    pub fn selected_tool_verbosity(&self) -> Option<ToolVerbosity> {
        self.selected_value("UI", "Tool Verbosity")
            .and_then(|value| match value {
                "Minimal" => Some(ToolVerbosity::Minimal),
                "Normal" => Some(ToolVerbosity::Normal),
                "Verbose" => Some(ToolVerbosity::Verbose),
                _ => None,
            })
    }

    fn selected_value(&self, category: &str, item_label: &str) -> Option<&str> {
        let category = self.categories.iter().find(|cat| cat.name == category)?;
        let item = category
            .items
            .iter()
            .find(|item| item.label == item_label)?;
        item.values.get(item.selected).map(|value| value.as_str())
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

    let footer_height = if cfg.help_visible { 7 } else { 4 };
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
            "Keys: tab/shift+tab=change pane  ↑/↓=navigate  Enter=select value  +/- adjust numbers (shift=10)\n\
             Note: changes apply immediately to runtime config (not yet persisted).\n\
             {desc}"
        )
    } else {
        format!(
            "{} \n ? Help  | Enter select  | +/- adjust (shift=10)  | Tab pane  | q/Esc close",
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

fn numeric_item(
    label: &str,
    description: &str,
    values: &[usize],
    current: usize,
) -> ConfigOverlayItem {
    let mut options: Vec<usize> = values.to_vec();
    if !options.contains(&current) {
        options.push(current);
    }
    options.sort_unstable();
    options.dedup();
    let selected = options.iter().position(|v| *v == current).unwrap_or(0);
    ConfigOverlayItem {
        label: label.to_string(),
        description: description.to_string(),
        values: options.iter().map(|v| v.to_string()).collect(),
        selected,
    }
}

fn parse_bool(value: &str) -> Option<bool> {
    match value {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn verbosity_level_to_idx(level: VerbosityLevel) -> usize {
    match level {
        VerbosityLevel::Info => 0,
        VerbosityLevel::Debug => 1,
        VerbosityLevel::Warn => 2,
        VerbosityLevel::Error => 3,
    }
}

fn parse_verbosity_level(value: &str) -> Option<VerbosityLevel> {
    match value {
        "Info" => Some(VerbosityLevel::Info),
        "Debug" => Some(VerbosityLevel::Debug),
        "Warn" => Some(VerbosityLevel::Warn),
        "Error" => Some(VerbosityLevel::Error),
        _ => None,
    }
}

fn max_len_to_idx(max_len: Option<u32>) -> usize {
    match max_len {
        None => 0,
        Some(120) => 1,
        Some(220) => 2,
        Some(512) => 3,
        Some(1024) => 4,
        Some(2048) => 5,
        Some(4096) => 6,
        Some(8192) => 7,
        Some(_) => 0,
    }
}

fn parse_optional_u32(value: &str) -> Option<Option<u32>> {
    if value == "None" {
        return Some(None);
    }
    value.parse::<u32>().ok().map(Some)
}

fn user_max_len_for_profile(profile: &[MessageVerbosity]) -> Option<u32> {
    for setting in profile {
        if let MessageVerbosity::User { max_len, .. } = setting {
            return *max_len;
        }
    }
    None
}

fn user_syntax_for_profile(profile: &[MessageVerbosity]) -> bool {
    for setting in profile {
        if let MessageVerbosity::User {
            syntax_highlighting, ..
        } = setting
        {
            return *syntax_highlighting;
        }
    }
    false
}

fn assistant_max_len_for_profile(profile: &[MessageVerbosity]) -> Option<u32> {
    for setting in profile {
        if let MessageVerbosity::Assistant { max_len, .. } = setting {
            return *max_len;
        }
    }
    None
}

fn assistant_syntax_for_profile(profile: &[MessageVerbosity]) -> bool {
    for setting in profile {
        if let MessageVerbosity::Assistant {
            syntax_highlighting, ..
        } = setting
        {
            return *syntax_highlighting;
        }
    }
    false
}

fn assistant_truncate_prev_for_profile(profile: &[MessageVerbosity]) -> bool {
    for setting in profile {
        if let MessageVerbosity::Assistant {
            truncate_prev_messages,
            ..
        } = setting
        {
            return *truncate_prev_messages;
        }
    }
    false
}

fn assistant_truncated_len_for_profile(profile: &[MessageVerbosity]) -> Option<u32> {
    for setting in profile {
        if let MessageVerbosity::Assistant { truncated_len, .. } = setting {
            return *truncated_len;
        }
    }
    None
}

fn sysinfo_level_for_profile(profile: &[MessageVerbosity]) -> VerbosityLevel {
    for setting in profile {
        if let MessageVerbosity::SysInfo { verbosity, .. } = setting {
            return *verbosity;
        }
    }
    VerbosityLevel::Info
}

fn system_level_for_profile(profile: &[MessageVerbosity]) -> VerbosityLevel {
    for setting in profile {
        if let MessageVerbosity::System { verbosity, .. } = setting {
            return *verbosity;
        }
    }
    VerbosityLevel::Info
}

fn display_init_for_profile(profile: &[MessageVerbosity]) -> bool {
    for setting in profile {
        if let MessageVerbosity::System { display_init, .. } = setting {
            return *display_init;
        }
    }
    false
}

fn apply_message_verbosity_profile_settings(
    overlay: &ConfigOverlayState,
    cfg: &mut RuntimeConfig,
) -> bool {
    let mut changed = false;
    changed |= apply_profile_user_assistant_settings(
        overlay,
        cfg,
        MessageVerbosityProfile::Minimal,
        "Minimal User Max Len",
        "Minimal User Syntax Highlight",
        "Minimal Assistant Max Len",
        "Minimal Assistant Syntax Highlight",
        "Minimal Assistant Truncate Prev",
        "Minimal Assistant Truncated Len",
    );
    changed |= apply_profile_level(
        overlay,
        cfg,
        MessageVerbosityProfile::Minimal,
        "Minimal SysInfo Level",
        "Minimal System Level",
    );
    changed |= apply_profile_display_init(
        overlay,
        cfg,
        MessageVerbosityProfile::Minimal,
        "Minimal Show Init System",
    );
    changed |= apply_profile_user_assistant_settings(
        overlay,
        cfg,
        MessageVerbosityProfile::Normal,
        "Normal User Max Len",
        "Normal User Syntax Highlight",
        "Normal Assistant Max Len",
        "Normal Assistant Syntax Highlight",
        "Normal Assistant Truncate Prev",
        "Normal Assistant Truncated Len",
    );
    changed |= apply_profile_level(
        overlay,
        cfg,
        MessageVerbosityProfile::Normal,
        "Normal SysInfo Level",
        "Normal System Level",
    );
    changed |= apply_profile_display_init(
        overlay,
        cfg,
        MessageVerbosityProfile::Normal,
        "Normal Show Init System",
    );
    changed |= apply_profile_user_assistant_settings(
        overlay,
        cfg,
        MessageVerbosityProfile::Verbose,
        "Verbose User Max Len",
        "Verbose User Syntax Highlight",
        "Verbose Assistant Max Len",
        "Verbose Assistant Syntax Highlight",
        "Verbose Assistant Truncate Prev",
        "Verbose Assistant Truncated Len",
    );
    changed |= apply_profile_level(
        overlay,
        cfg,
        MessageVerbosityProfile::Verbose,
        "Verbose SysInfo Level",
        "Verbose System Level",
    );
    changed |= apply_profile_display_init(
        overlay,
        cfg,
        MessageVerbosityProfile::Verbose,
        "Verbose Show Init System",
    );
    changed |= apply_profile_user_assistant_settings(
        overlay,
        cfg,
        MessageVerbosityProfile::Custom,
        "Custom User Max Len",
        "Custom User Syntax Highlight",
        "Custom Assistant Max Len",
        "Custom Assistant Syntax Highlight",
        "Custom Assistant Truncate Prev",
        "Custom Assistant Truncated Len",
    );
    changed |= apply_profile_level(
        overlay,
        cfg,
        MessageVerbosityProfile::Custom,
        "Custom SysInfo Level",
        "Custom System Level",
    );
    changed |= apply_profile_display_init(
        overlay,
        cfg,
        MessageVerbosityProfile::Custom,
        "Custom Show Init System",
    );
    changed
}

fn apply_profile_user_assistant_settings(
    overlay: &ConfigOverlayState,
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    user_max_len_label: &str,
    user_syntax_label: &str,
    assistant_max_len_label: &str,
    assistant_syntax_label: &str,
    assistant_truncate_prev_label: &str,
    assistant_truncated_len_label: &str,
) -> bool {
    let mut changed = false;
    if let Some(value) = overlay.selected_value("Message Verbosity", user_max_len_label)
        && let Some(next) = parse_optional_u32(value)
    {
        changed |= set_profile_user_max_len(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", user_syntax_label)
        && let Some(next) = parse_bool(value)
    {
        changed |= set_profile_user_syntax(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", assistant_max_len_label)
        && let Some(next) = parse_optional_u32(value)
    {
        changed |= set_profile_assistant_max_len(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", assistant_syntax_label)
        && let Some(next) = parse_bool(value)
    {
        changed |= set_profile_assistant_syntax(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", assistant_truncate_prev_label)
        && let Some(next) = parse_bool(value)
    {
        changed |= set_profile_assistant_truncate_prev(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", assistant_truncated_len_label)
        && let Some(next) = parse_optional_u32(value)
    {
        changed |= set_profile_assistant_truncated_len(cfg, profile, next);
    }
    changed
}

fn apply_profile_level(
    overlay: &ConfigOverlayState,
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    sysinfo_label: &str,
    system_label: &str,
) -> bool {
    let mut changed = false;
    if let Some(value) = overlay.selected_value("Message Verbosity", sysinfo_label)
        && let Some(next) = parse_verbosity_level(value)
    {
        changed |= set_profile_sysinfo_level(cfg, profile, next);
    }
    if let Some(value) = overlay.selected_value("Message Verbosity", system_label)
        && let Some(next) = parse_verbosity_level(value)
    {
        changed |= set_profile_system_level(cfg, profile, next);
    }
    changed
}

fn apply_profile_display_init(
    overlay: &ConfigOverlayState,
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    item_label: &str,
) -> bool {
    if let Some(value) = overlay.selected_value("Message Verbosity", item_label)
        && let Some(next) = parse_bool(value)
    {
        return set_profile_system_display_init(cfg, profile, next);
    }
    false
}

fn profile_settings_mut(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
) -> &mut Vec<MessageVerbosity> {
    match profile {
        MessageVerbosityProfile::Minimal => &mut cfg.message_verbosity_profiles.minimal,
        MessageVerbosityProfile::Normal => &mut cfg.message_verbosity_profiles.normal,
        MessageVerbosityProfile::Verbose => &mut cfg.message_verbosity_profiles.verbose,
        MessageVerbosityProfile::Custom => &mut cfg.message_verbosity_profiles.custom,
    }
}

fn set_profile_sysinfo_level(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    level: VerbosityLevel,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::SysInfo { verbosity, .. } = entry {
            if *verbosity != level {
                *verbosity = level;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::SysInfo {
        max_len: None,
        verbosity: level,
    });
    true
}

fn set_profile_user_max_len(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    max_len: Option<u32>,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::User {
            max_len: current, ..
        } = entry
        {
            if *current != max_len {
                *current = max_len;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::User {
        max_len,
        syntax_highlighting: false,
    });
    true
}

fn set_profile_user_syntax(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    syntax_highlighting: bool,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::User {
            syntax_highlighting: current,
            ..
        } = entry
        {
            if *current != syntax_highlighting {
                *current = syntax_highlighting;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::User {
        max_len: None,
        syntax_highlighting,
    });
    true
}

fn set_profile_assistant_max_len(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    max_len: Option<u32>,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::Assistant {
            max_len: current, ..
        } = entry
        {
            if *current != max_len {
                *current = max_len;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::Assistant {
        max_len,
        syntax_highlighting: false,
        truncate_prev_messages: false,
        truncated_len: None,
    });
    true
}

fn set_profile_assistant_syntax(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    syntax_highlighting: bool,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::Assistant {
            syntax_highlighting: current,
            ..
        } = entry
        {
            if *current != syntax_highlighting {
                *current = syntax_highlighting;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::Assistant {
        max_len: None,
        syntax_highlighting,
        truncate_prev_messages: false,
        truncated_len: None,
    });
    true
}

fn set_profile_assistant_truncate_prev(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    truncate_prev_messages: bool,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::Assistant {
            truncate_prev_messages: current,
            ..
        } = entry
        {
            if *current != truncate_prev_messages {
                *current = truncate_prev_messages;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::Assistant {
        max_len: None,
        syntax_highlighting: false,
        truncate_prev_messages,
        truncated_len: None,
    });
    true
}

fn set_profile_assistant_truncated_len(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    truncated_len: Option<u32>,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::Assistant {
            truncated_len: current,
            ..
        } = entry
        {
            if *current != truncated_len {
                *current = truncated_len;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::Assistant {
        max_len: None,
        syntax_highlighting: false,
        truncate_prev_messages: false,
        truncated_len,
    });
    true
}

fn set_profile_system_level(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    level: VerbosityLevel,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::System { verbosity, .. } = entry {
            if *verbosity != level {
                *verbosity = level;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::System {
        max_len: None,
        verbosity: level,
        display_init: false,
    });
    true
}

fn set_profile_system_display_init(
    cfg: &mut RuntimeConfig,
    profile: MessageVerbosityProfile,
    display_init: bool,
) -> bool {
    let settings = profile_settings_mut(cfg, profile);
    for entry in settings.iter_mut() {
        if let MessageVerbosity::System {
            display_init: value, ..
        } = entry
        {
            if *value != display_init {
                *value = display_init;
                return true;
            }
            return false;
        }
    }
    settings.push(MessageVerbosity::System {
        max_len: None,
        verbosity: VerbosityLevel::Info,
        display_init,
    });
    true
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
