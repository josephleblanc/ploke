//! Compile-time palettes and runtime theme application (cold path only).

mod palette;
mod scheme;

pub use palette::{DiffLineColors, PaletteTokens, VerdictColors, tokens_from_ctx, tokens_from_ui};
pub use scheme::{AppTheme, NamedScheme, on_theme_changed};
