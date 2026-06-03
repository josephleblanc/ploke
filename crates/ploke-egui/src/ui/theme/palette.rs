//! Semantic color tokens derived from named palettes.

use eframe::egui::{self, Color32};

use super::scheme::NamedScheme;
use crate::ui::view::StatusColors;

/// Stable id for [`PaletteTokens`] stored on the egui context after theme apply.
pub fn theme_tokens_id() -> egui::Id {
    egui::Id::new("ploke_theme_tokens")
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteTokens {
    pub background: Color32,
    pub panel: Color32,
    pub text: Color32,
    pub text_muted: Color32,
    pub border: Color32,
    pub accent: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub error: Color32,
    pub info: Color32,
    pub graph_synthesized: Color32,
    pub graph_opened_from: Color32,
    pub graph_selected: Color32,
    pub graph_applied: Color32,
    pub graph_restored: Color32,
    pub graph_dropped: Color32,
    pub verdict_good: Color32,
    pub verdict_neutral: Color32,
    pub verdict_concerning: Color32,
    pub diff_add: Color32,
    pub diff_remove: Color32,
    pub diff_hunk: Color32,
    pub diff_meta: Color32,
    pub badge_parent: Color32,
    pub badge_child: Color32,
    pub badge_text: Color32,
    pub is_dark: bool,
    pub theme_key: u8,
}

impl PaletteTokens {
    pub(crate) const fn tokyo_night() -> Self {
        Self {
            background: Color32::from_rgb(26, 27, 38),
            panel: Color32::from_rgb(36, 40, 59),
            text: Color32::from_rgb(192, 202, 245),
            text_muted: Color32::from_rgb(154, 163, 194),
            border: Color32::from_rgb(65, 72, 104),
            accent: Color32::from_rgb(122, 162, 247),
            success: Color32::from_rgb(158, 206, 106),
            warning: Color32::from_rgb(224, 175, 104),
            error: Color32::from_rgb(247, 118, 142),
            info: Color32::from_rgb(125, 207, 255),
            graph_synthesized: Color32::from_rgb(118, 128, 142),
            graph_opened_from: Color32::from_rgb(224, 175, 104),
            graph_selected: Color32::from_rgb(158, 206, 106),
            graph_applied: Color32::from_rgb(122, 162, 247),
            graph_restored: Color32::from_rgb(147, 153, 178),
            graph_dropped: Color32::from_rgb(247, 118, 142),
            verdict_good: Color32::from_rgb(158, 206, 106),
            verdict_neutral: Color32::from_rgb(224, 175, 104),
            verdict_concerning: Color32::from_rgb(247, 118, 142),
            diff_add: Color32::from_rgb(64, 190, 110),
            diff_remove: Color32::from_rgb(230, 92, 92),
            diff_hunk: Color32::from_rgb(108, 151, 255),
            diff_meta: Color32::from_rgb(120, 170, 220),
            badge_parent: Color32::from_rgb(187, 154, 247),
            badge_child: Color32::from_rgb(122, 162, 247),
            badge_text: Color32::from_rgb(26, 27, 38),
            is_dark: true,
            theme_key: 0,
        }
    }

    pub(crate) const fn dracula() -> Self {
        Self {
            background: Color32::from_rgb(40, 42, 54),
            panel: Color32::from_rgb(68, 71, 90),
            text: Color32::from_rgb(248, 248, 242),
            text_muted: Color32::from_rgb(189, 189, 197),
            border: Color32::from_rgb(98, 102, 128),
            accent: Color32::from_rgb(189, 147, 249),
            success: Color32::from_rgb(80, 250, 123),
            warning: Color32::from_rgb(255, 184, 108),
            error: Color32::from_rgb(255, 85, 85),
            info: Color32::from_rgb(139, 233, 253),
            graph_synthesized: Color32::from_rgb(139, 141, 160),
            graph_opened_from: Color32::from_rgb(255, 184, 108),
            graph_selected: Color32::from_rgb(80, 250, 123),
            graph_applied: Color32::from_rgb(139, 233, 253),
            graph_restored: Color32::from_rgb(191, 146, 115),
            graph_dropped: Color32::from_rgb(255, 85, 85),
            verdict_good: Color32::from_rgb(80, 250, 123),
            verdict_neutral: Color32::from_rgb(255, 184, 108),
            verdict_concerning: Color32::from_rgb(255, 85, 85),
            diff_add: Color32::from_rgb(80, 250, 123),
            diff_remove: Color32::from_rgb(255, 85, 85),
            diff_hunk: Color32::from_rgb(189, 147, 249),
            diff_meta: Color32::from_rgb(139, 233, 253),
            badge_parent: Color32::from_rgb(189, 147, 249),
            badge_child: Color32::from_rgb(139, 233, 253),
            badge_text: Color32::from_rgb(40, 42, 54),
            is_dark: true,
            theme_key: 0,
        }
    }

    pub(crate) const fn gruvbox_dark() -> Self {
        Self {
            background: Color32::from_rgb(40, 40, 40),
            panel: Color32::from_rgb(60, 56, 54),
            text: Color32::from_rgb(235, 219, 178),
            text_muted: Color32::from_rgb(189, 174, 147),
            border: Color32::from_rgb(80, 73, 69),
            accent: Color32::from_rgb(131, 165, 152),
            success: Color32::from_rgb(184, 187, 38),
            warning: Color32::from_rgb(250, 189, 47),
            error: Color32::from_rgb(251, 73, 52),
            info: Color32::from_rgb(142, 192, 124),
            graph_synthesized: Color32::from_rgb(146, 131, 116),
            graph_opened_from: Color32::from_rgb(250, 189, 47),
            graph_selected: Color32::from_rgb(184, 187, 38),
            graph_applied: Color32::from_rgb(131, 165, 152),
            graph_restored: Color32::from_rgb(168, 153, 132),
            graph_dropped: Color32::from_rgb(251, 73, 52),
            verdict_good: Color32::from_rgb(184, 187, 38),
            verdict_neutral: Color32::from_rgb(250, 189, 47),
            verdict_concerning: Color32::from_rgb(251, 73, 52),
            diff_add: Color32::from_rgb(142, 192, 124),
            diff_remove: Color32::from_rgb(251, 73, 52),
            diff_hunk: Color32::from_rgb(131, 165, 152),
            diff_meta: Color32::from_rgb(250, 189, 47),
            badge_parent: Color32::from_rgb(211, 134, 155),
            badge_child: Color32::from_rgb(131, 165, 152),
            badge_text: Color32::from_rgb(40, 40, 40),
            is_dark: true,
            theme_key: 0,
        }
    }

    pub(crate) const fn one_dark() -> Self {
        Self {
            background: Color32::from_rgb(40, 44, 52),
            panel: Color32::from_rgb(33, 37, 43),
            text: Color32::from_rgb(171, 178, 191),
            text_muted: Color32::from_rgb(130, 137, 151),
            border: Color32::from_rgb(76, 82, 99),
            accent: Color32::from_rgb(97, 175, 239),
            success: Color32::from_rgb(152, 195, 121),
            warning: Color32::from_rgb(229, 192, 123),
            error: Color32::from_rgb(224, 108, 117),
            info: Color32::from_rgb(86, 182, 194),
            graph_synthesized: Color32::from_rgb(130, 137, 151),
            graph_opened_from: Color32::from_rgb(229, 192, 123),
            graph_selected: Color32::from_rgb(152, 195, 121),
            graph_applied: Color32::from_rgb(97, 175, 239),
            graph_restored: Color32::from_rgb(171, 163, 148),
            graph_dropped: Color32::from_rgb(224, 108, 117),
            verdict_good: Color32::from_rgb(152, 195, 121),
            verdict_neutral: Color32::from_rgb(229, 192, 123),
            verdict_concerning: Color32::from_rgb(224, 108, 117),
            diff_add: Color32::from_rgb(152, 195, 121),
            diff_remove: Color32::from_rgb(224, 108, 117),
            diff_hunk: Color32::from_rgb(97, 175, 239),
            diff_meta: Color32::from_rgb(86, 182, 194),
            badge_parent: Color32::from_rgb(198, 120, 221),
            badge_child: Color32::from_rgb(97, 175, 239),
            badge_text: Color32::from_rgb(40, 44, 52),
            is_dark: true,
            theme_key: 0,
        }
    }

    pub(crate) const fn gruvbox_light() -> Self {
        Self {
            background: Color32::from_rgb(251, 241, 199),
            panel: Color32::from_rgb(242, 229, 188),
            text: Color32::from_rgb(60, 56, 54),
            text_muted: Color32::from_rgb(124, 111, 100),
            border: Color32::from_rgb(213, 196, 161),
            accent: Color32::from_rgb(69, 133, 136),
            success: Color32::from_rgb(152, 151, 26),
            warning: Color32::from_rgb(215, 153, 33),
            error: Color32::from_rgb(204, 36, 29),
            info: Color32::from_rgb(104, 157, 106),
            graph_synthesized: Color32::from_rgb(146, 131, 116),
            graph_opened_from: Color32::from_rgb(215, 153, 33),
            graph_selected: Color32::from_rgb(152, 151, 26),
            graph_applied: Color32::from_rgb(69, 133, 136),
            graph_restored: Color32::from_rgb(168, 153, 132),
            graph_dropped: Color32::from_rgb(204, 36, 29),
            verdict_good: Color32::from_rgb(152, 151, 26),
            verdict_neutral: Color32::from_rgb(215, 153, 33),
            verdict_concerning: Color32::from_rgb(204, 36, 29),
            diff_add: Color32::from_rgb(104, 157, 106),
            diff_remove: Color32::from_rgb(204, 36, 29),
            diff_hunk: Color32::from_rgb(69, 133, 136),
            diff_meta: Color32::from_rgb(215, 153, 33),
            badge_parent: Color32::from_rgb(177, 98, 134),
            badge_child: Color32::from_rgb(69, 133, 136),
            badge_text: Color32::from_rgb(251, 241, 199),
            is_dark: false,
            theme_key: 0,
        }
    }

    pub(crate) const fn one_light() -> Self {
        Self {
            background: Color32::from_rgb(250, 250, 250),
            panel: Color32::from_rgb(240, 240, 240),
            text: Color32::from_rgb(56, 58, 66),
            text_muted: Color32::from_rgb(130, 137, 151),
            border: Color32::from_rgb(220, 223, 228),
            accent: Color32::from_rgb(64, 120, 242),
            success: Color32::from_rgb(80, 161, 79),
            warning: Color32::from_rgb(193, 132, 1),
            error: Color32::from_rgb(228, 86, 73),
            info: Color32::from_rgb(0, 132, 180),
            graph_synthesized: Color32::from_rgb(130, 137, 151),
            graph_opened_from: Color32::from_rgb(193, 132, 1),
            graph_selected: Color32::from_rgb(80, 161, 79),
            graph_applied: Color32::from_rgb(64, 120, 242),
            graph_restored: Color32::from_rgb(171, 163, 148),
            graph_dropped: Color32::from_rgb(228, 86, 73),
            verdict_good: Color32::from_rgb(80, 161, 79),
            verdict_neutral: Color32::from_rgb(193, 132, 1),
            verdict_concerning: Color32::from_rgb(228, 86, 73),
            diff_add: Color32::from_rgb(80, 161, 79),
            diff_remove: Color32::from_rgb(228, 86, 73),
            diff_hunk: Color32::from_rgb(64, 120, 242),
            diff_meta: Color32::from_rgb(0, 132, 180),
            badge_parent: Color32::from_rgb(166, 38, 164),
            badge_child: Color32::from_rgb(64, 120, 242),
            badge_text: Color32::from_rgb(250, 250, 250),
            is_dark: false,
            theme_key: 0,
        }
    }

    pub fn for_scheme(scheme: NamedScheme) -> Self {
        let key = scheme.cache_key();
        let mut tokens = match scheme {
            NamedScheme::TokyoNight => Self::tokyo_night(),
            NamedScheme::Dracula => Self::dracula(),
            NamedScheme::GruvboxDark => Self::gruvbox_dark(),
            NamedScheme::OneDark => Self::one_dark(),
            NamedScheme::GruvboxLight => Self::gruvbox_light(),
            NamedScheme::OneLight => Self::one_light(),
        };
        tokens.theme_key = key;
        tokens
    }

    pub fn cache_theme_key(self) -> u8 {
        self.theme_key
    }

    pub fn status_colors(self) -> StatusColors {
        StatusColors {
            synthesized: self.graph_synthesized,
            opened_from: self.graph_opened_from,
            selected: self.graph_selected,
            applied: self.graph_applied,
            restored: self.graph_restored,
            dropped: self.graph_dropped,
        }
    }

    pub fn to_visuals(self) -> egui::Visuals {
        let mut visuals = if self.is_dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.dark_mode = self.is_dark;
        visuals.override_text_color = Some(self.text);
        visuals.panel_fill = self.panel;
        visuals.window_fill = self.background;
        visuals.extreme_bg_color = self.background;
        visuals.faint_bg_color = self.panel.gamma_multiply(0.85);
        visuals.widgets.noninteractive.bg_fill = self.panel;
        visuals.widgets.inactive.bg_fill = self.panel;
        visuals.widgets.hovered.bg_fill = self.panel.gamma_multiply(1.08);
        visuals.widgets.active.bg_fill = self.panel.gamma_multiply(1.12);
        visuals.widgets.noninteractive.fg_stroke.color = self.text_muted;
        visuals.widgets.inactive.fg_stroke.color = self.text;
        visuals.widgets.hovered.fg_stroke.color = self.text;
        visuals.widgets.active.fg_stroke.color = self.text;
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke.color = self.accent;
        visuals.hyperlink_color = self.info;
        visuals.warn_fg_color = self.warning;
        visuals.error_fg_color = self.error;
        visuals.widgets.noninteractive.bg_stroke.color = self.border;
        visuals.widgets.inactive.bg_stroke.color = self.border;
        visuals.window_stroke.color = self.border;
        visuals
    }

    pub fn edge_label_text(self) -> Color32 {
        self.text
    }

    pub fn edge_label_background(self) -> Color32 {
        let base = if self.is_dark {
            self.panel
        } else {
            self.background
        };
        tint_alpha(base, if self.is_dark { 210 } else { 240 })
    }

    pub fn install_on_context(self, ctx: &egui::Context) {
        ctx.set_visuals(self.to_visuals());
        ctx.data_mut(|data| data.insert_temp(theme_tokens_id(), self));
        ctx.request_discard("theme palette");
    }
}

pub fn tokens_from_ctx(ctx: &egui::Context) -> PaletteTokens {
    ctx.data(|data| {
        data.get_temp::<PaletteTokens>(theme_tokens_id())
            .unwrap_or_else(|| NamedScheme::default().tokens())
    })
}

pub fn tokens_from_ui(ui: &egui::Ui) -> PaletteTokens {
    tokens_from_ctx(ui.ctx())
}

#[derive(Debug, Clone, Copy)]
pub struct StatusColorsFromPalette(pub StatusColors);

#[derive(Debug, Clone, Copy)]
pub struct VerdictColors {
    pub good: Color32,
    pub neutral: Color32,
    pub concerning: Color32,
}

impl From<PaletteTokens> for VerdictColors {
    fn from(tokens: PaletteTokens) -> Self {
        Self {
            good: tokens.verdict_good,
            neutral: tokens.verdict_neutral,
            concerning: tokens.verdict_concerning,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiffLineColors {
    pub add: Color32,
    pub remove: Color32,
    pub hunk_fg: Color32,
    pub meta_fg: Color32,
}

impl From<PaletteTokens> for DiffLineColors {
    fn from(tokens: PaletteTokens) -> Self {
        Self {
            add: tokens.diff_add,
            remove: tokens.diff_remove,
            hunk_fg: tokens.diff_hunk,
            meta_fg: tokens.diff_meta,
        }
    }
}

impl PaletteTokens {
    pub(crate) fn verdict_colors(self) -> VerdictColors {
        VerdictColors::from(self)
    }

    pub fn diff_hunk_bg(self) -> Color32 {
        tint_alpha(self.diff_hunk, if self.is_dark { 38 } else { 48 })
    }

    pub fn diff_meta_bg(self) -> Color32 {
        tint_alpha(self.text_muted, if self.is_dark { 18 } else { 24 })
    }

    pub fn diff_header_bg(self) -> Color32 {
        tint_alpha(self.diff_meta, if self.is_dark { 24 } else { 32 })
    }

    pub fn diff_add_bg(self) -> Color32 {
        tint_alpha(self.diff_add, if self.is_dark { 38 } else { 48 })
    }

    pub fn diff_remove_bg(self) -> Color32 {
        tint_alpha(self.diff_remove, if self.is_dark { 42 } else { 52 })
    }

    pub fn diff_context_bg(self) -> Color32 {
        tint_alpha(self.text_muted, if self.is_dark { 10 } else { 14 })
    }
}

fn tint_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}
