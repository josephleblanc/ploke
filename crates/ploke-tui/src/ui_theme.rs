use ratatui::style::Color;

#[derive(Debug, Clone)]
pub struct UiTheme {
    pub input_bg: Color,
    pub input_fg: Color,
    pub input_command_fg: Color,
}

impl Default for UiTheme {
    fn default() -> Self {
        Self {
            input_bg: Color::Rgb(90, 90, 90),
            input_fg: Color::Rgb(220, 220, 220),
            input_command_fg: Color::Blue,
        }
    }
}
