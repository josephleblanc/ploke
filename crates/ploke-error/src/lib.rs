pub mod context;
pub mod fatal;
pub mod internal;
pub mod warning;

pub use context::ErrorContext;
pub use fatal::FatalError;
pub use warning::WarningError;

use std::path::PathBuf;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}
