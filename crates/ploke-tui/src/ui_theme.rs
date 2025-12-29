use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct UiTheme {
    pub input_bg: Color,
    pub input_fg: Color,
    pub input_command_fg: Color,
    pub input_ghost_fg: Color,
    pub input_suggestion_bg: Color,
    pub input_suggestion_fg: Color,
    pub input_suggestion_desc_fg: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            input_bg: Color::Rgb(90, 90, 90),
            input_fg: Color::Rgb(220, 220, 220),
            input_command_fg: Color::Blue,
            input_ghost_fg: Color::Rgb(120, 160, 255),
            input_suggestion_bg: Color::Rgb(60, 60, 60),
            input_suggestion_fg: Color::Rgb(210, 210, 210),
            input_suggestion_desc_fg: Color::Rgb(160, 160, 160),
        }
    }
}
