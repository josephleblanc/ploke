//! Named palettes and persisted app theme state.

use eframe::egui;
use serde::{Deserialize, Serialize};

use super::palette::PaletteTokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NamedScheme {
    #[default]
    TokyoNight,
    Dracula,
    GruvboxDark,
    OneDark,
    GruvboxLight,
    OneLight,
}

impl NamedScheme {
    /// Stable snake_case id for URL (`?theme=`), CLI (`--theme`), and agent docs.
    pub fn theme_id(self) -> &'static str {
        match self {
            Self::TokyoNight => "tokyo_night",
            Self::Dracula => "dracula",
            Self::GruvboxDark => "gruvbox_dark",
            Self::OneDark => "one_dark",
            Self::GruvboxLight => "gruvbox_light",
            Self::OneLight => "one_light",
        }
    }

    /// Parse a theme id from `?theme=` or `--theme` (case-sensitive snake_case).
    pub fn from_theme_id(id: &str) -> Option<Self> {
        match id.trim() {
            "tokyo_night" => Some(Self::TokyoNight),
            "dracula" => Some(Self::Dracula),
            "gruvbox_dark" => Some(Self::GruvboxDark),
            "one_dark" => Some(Self::OneDark),
            "gruvbox_light" => Some(Self::GruvboxLight),
            "one_light" => Some(Self::OneLight),
            _ => None,
        }
    }

    pub const ALL: [Self; 6] = [
        Self::TokyoNight,
        Self::Dracula,
        Self::GruvboxDark,
        Self::OneDark,
        Self::GruvboxLight,
        Self::OneLight,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::TokyoNight => "Tokyo Night",
            Self::Dracula => "Dracula",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::OneDark => "One Dark",
            Self::GruvboxLight => "Gruvbox Light",
            Self::OneLight => "One Light",
        }
    }

    pub fn tokens(self) -> PaletteTokens {
        PaletteTokens::for_scheme(self)
    }

    pub fn cache_key(self) -> u8 {
        self as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppTheme {
    pub scheme: NamedScheme,
}

impl Default for AppTheme {
    fn default() -> Self {
        Self {
            scheme: NamedScheme::default(),
        }
    }
}

impl AppTheme {
    pub fn tokens(self) -> PaletteTokens {
        self.scheme.tokens()
    }

    pub fn apply_to_context(self, ctx: &egui::Context) {
        self.tokens().install_on_context(ctx);
    }

    /// Apply when the scheme changes; returns true if visuals were updated.
    pub fn set_scheme(&mut self, ctx: &egui::Context, scheme: NamedScheme) -> bool {
        if self.scheme == scheme {
            return false;
        }
        self.scheme = scheme;
        self.apply_to_context(ctx);
        true
    }

    pub fn selector_ui(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.label("Theme:");
        egui::ComboBox::from_id_salt("ploke-egui-theme")
            .selected_text(self.scheme.label())
            .show_ui(ui, |ui| {
                for scheme in NamedScheme::ALL {
                    if ui
                        .selectable_value(&mut self.scheme, scheme, scheme.label())
                        .clicked()
                    {
                        changed = true;
                    }
                }
            });
        if changed {
            self.apply_to_context(ui.ctx());
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::NamedScheme;

    #[test]
    fn theme_id_round_trip_for_all_palettes() {
        for scheme in NamedScheme::ALL {
            let id = scheme.theme_id();
            assert_eq!(NamedScheme::from_theme_id(id), Some(scheme));
        }
        assert_eq!(NamedScheme::from_theme_id("unknown"), None);
    }
}

/// Drop egui widget/layout temp state after a palette change (collapsing headers, etc.).
pub fn on_theme_changed(ctx: &egui::Context) {
    ctx.request_discard("ploke theme");
    ctx.memory_mut(|memory| memory.data.clear());
}
