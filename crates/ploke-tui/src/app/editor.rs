use std::path::Path;

use crate::app_state::core::RuntimeConfig;

/// Resolve editor command from config or environment.
/// Precedence: RuntimeConfig.ploke_editor -> PLOKE_EDITOR env -> None.
pub fn resolve_editor_command(cfg: &RuntimeConfig) -> Option<String> {
    if let Some(cmd) = cfg.ploke_editor.clone()
        && !cmd.trim().is_empty()
    {
        return Some(cmd);
    }
    std::env::var("PLOKE_EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
}

/// Build editor CLI args for a given path and optional line number.
/// Default format: "{path}" or "{path}:{line}" (common for editors like Vim/Helix).
pub fn build_editor_args(path: &Path, line: Option<u32>) -> Vec<String> {
    let mut target = path.display().to_string();
    if let Some(l) = line {
        target.push(':');
        target.push_str(&l.to_string());
    }
    vec![target]
}
