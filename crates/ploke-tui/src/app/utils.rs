/*!
Small utility helpers used by the `app` module and its submodules.

Intended usage:
- Rendering code uses `truncate_uuid` to format compact labels.
- Event formatting uses `display_file_info` to show optional file paths.
*/

use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

/// Return the first 8 characters of a UUID for compact display.
pub fn truncate_uuid(id: Uuid) -> String {
    id.to_string().chars().take(8).collect()
}

/// Safely display an optional file path, falling back to a friendly message.
pub fn display_file_info(file: Option<&Arc<PathBuf>>) -> String {
    file.map(|f| f.display().to_string())
        .unwrap_or_else(|| "File not found.".to_string())
}
