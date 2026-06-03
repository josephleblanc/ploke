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
