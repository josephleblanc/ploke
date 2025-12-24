//! Overlay manager smoke tests covering:
//! - `close_kind` only closes when the active overlay matches the requested kind.
//! - render routing draws the currently active overlay into the frame by checking for its title text.
//! - config overlay closes on `Esc` and produces no overlay actions.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_tui::app::overlay::{OverlayKind, OverlayManager};
use ploke_tui::app::view::components::config_overlay::ConfigOverlayState;
use ploke_tui::app::view::components::model_browser::ModelBrowserState;
use ploke_tui::app_state::core::RuntimeConfig;
use ploke_tui::user_config::UserConfig;
mod ui_test_helpers;

use ratatui::layout::Rect;

#[test]
fn overlay_manager_closes_config_on_escape() {
    let user_cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(user_cfg);
    let mut manager = OverlayManager::default();
    let overlay = ConfigOverlayState::from_runtime_config(&runtime_cfg);

    assert!(!manager.is_active());
    manager.open_config(overlay);
    assert!(manager.is_active());

    let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
    let actions = manager.handle_input(key);
    assert!(actions.is_empty());
    assert!(!manager.is_active());
}

#[test]
fn overlay_manager_close_kind_only_closes_matching_overlay() {
    let mut manager = OverlayManager::default();
    let state = ModelBrowserState {
        visible: true,
        keyword: "test".to_string(),
        items: Vec::new(),
        selected: 0,
        help_visible: false,
        provider_select_active: false,
        provider_selected: 0,
        vscroll: 0,
        viewport_height: 0,
    };

    manager.open_model_browser(state);
    assert!(manager.is_active());

    manager.close_kind(OverlayKind::EmbeddingBrowser);
    assert!(manager.is_active());

    manager.close_kind(OverlayKind::ModelBrowser);
    assert!(!manager.is_active());
}

#[tokio::test(flavor = "multi_thread")]
async fn overlay_manager_renders_active_overlay() {
    let state = ploke_tui::test_harness::get_state().await;
    let user_cfg = UserConfig::default();
    let runtime_cfg = RuntimeConfig::from(user_cfg);
    let mut manager = OverlayManager::default();
    let area = Rect::new(0, 0, 80, 24);

    let config_overlay = ConfigOverlayState::from_runtime_config(&runtime_cfg);
    manager.open_config(config_overlay);
    let buffer = ui_test_helpers::render_to_buffer(area, |f| {
        manager.render(f, &state);
    });
    let text = ui_test_helpers::buffer_to_string(&buffer);
    assert!(text.contains(" Categories "));

    let model_overlay = ModelBrowserState {
        visible: true,
        keyword: "test".to_string(),
        items: Vec::new(),
        selected: 0,
        help_visible: false,
        provider_select_active: false,
        provider_selected: 0,
        vscroll: 0,
        viewport_height: 0,
    };
    manager.open_model_browser(model_overlay);
    let buffer = ui_test_helpers::render_to_buffer(area, |f| {
        manager.render(f, &state);
    });
    let text = ui_test_helpers::buffer_to_string(&buffer);
    assert!(text.contains(" Model Browser"));
}
