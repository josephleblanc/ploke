mod exec;
pub mod parser;

use crate::app::App;
pub use exec::HELP_COMMANDS;

/// Entry point for command handling: parse then execute.
pub fn execute_command(app: &mut App) {
    let style = app.command_style;
    let cmd = app.input_buffer.clone();
    let command = parser::parse(&cmd, style);
    exec::execute(app, command);
}
