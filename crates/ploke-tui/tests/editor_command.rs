//! Editor command resolution and args formatting tests
//!
//! Purpose: verify precedence of editor command selection and path:line arg
//! formatting used by open-in-editor actions.

use ploke_tui::app::editor::{build_editor_args, resolve_editor_command};
use ploke_tui::app_state::core::RuntimeConfig;

#[test]
fn resolve_editor_prefers_config_over_env() {
    let mut cfg = RuntimeConfig::default();
    unsafe {
        std::env::remove_var("PLOKE_EDITOR");
    }
    // Config set -> chosen
    cfg.ploke_editor = Some("code".into());
    assert_eq!(resolve_editor_command(&cfg).as_deref(), Some("code"));

    // Config empty but env set -> env chosen
    cfg.ploke_editor = None;
    unsafe {
        std::env::set_var("PLOKE_EDITOR", "hx");
    }
    assert_eq!(resolve_editor_command(&cfg).as_deref(), Some("hx"));

    // Neither set -> None
    unsafe {
        std::env::remove_var("PLOKE_EDITOR");
    }
    assert!(resolve_editor_command(&cfg).is_none());
}

#[test]
fn build_editor_args_formats_path_and_line() {
    let path = std::path::Path::new("/tmp/test.rs");
    let args = build_editor_args(path, None);
    assert_eq!(args, vec!["/tmp/test.rs".to_string()]);

    let args2 = build_editor_args(path, Some(42));
    assert_eq!(args2, vec!["/tmp/test.rs:42".to_string()]);
}
