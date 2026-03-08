use ploke_tui::app::view::components::config_overlay::ConfigOverlayState;
use ploke_tui::app_state::RuntimeConfig;
use ploke_tui::user_config::{MessageVerbosityProfile, VerbosityLevel};

fn set_selected_value(
    overlay: &mut ConfigOverlayState,
    category_name: &str,
    item_label: &str,
    value: &str,
) {
    let category = overlay
        .categories
        .iter_mut()
        .find(|cat| cat.name == category_name)
        .expect("category exists");
    let item = category
        .items
        .iter_mut()
        .find(|item| item.label == item_label)
        .expect("item exists");
    item.selected = item
        .values
        .iter()
        .position(|v| v == value)
        .expect("value exists");
}

#[test]
fn config_overlay_exposes_message_verbosity_controls() {
    let cfg = RuntimeConfig::default();
    let overlay = ConfigOverlayState::from_runtime_config(&cfg);

    let ui = overlay
        .categories
        .iter()
        .find(|cat| cat.name == "UI")
        .expect("ui category");
    assert!(
        ui.items
            .iter()
            .any(|item| item.label == "Default Message Verbosity")
    );

    let message = overlay
        .categories
        .iter()
        .find(|cat| cat.name == "Message Verbosity")
        .expect("message category");
    assert!(
        message
            .items
            .iter()
            .any(|item| item.label == "Minimal SysInfo Level")
    );
    assert!(
        message
            .items
            .iter()
            .any(|item| item.label == "Custom Show Init System")
    );
}

#[test]
fn config_overlay_applies_message_verbosity_to_runtime_config() {
    let mut cfg = RuntimeConfig::default();
    let mut overlay = ConfigOverlayState::from_runtime_config(&cfg);

    set_selected_value(
        &mut overlay,
        "UI",
        "Default Message Verbosity",
        "Custom",
    );
    set_selected_value(
        &mut overlay,
        "Message Verbosity",
        "Custom SysInfo Level",
        "Error",
    );
    set_selected_value(
        &mut overlay,
        "Message Verbosity",
        "Custom System Level",
        "Debug",
    );
    set_selected_value(
        &mut overlay,
        "Message Verbosity",
        "Custom Show Init System",
        "true",
    );

    let changed = overlay.apply_to_runtime_config(&mut cfg);
    assert!(changed);
    assert_eq!(cfg.default_verbosity, MessageVerbosityProfile::Custom);

    let custom = &cfg.message_verbosity_profiles.custom;
    let sysinfo = custom.iter().find_map(|entry| match entry {
        ploke_tui::user_config::MessageVerbosity::SysInfo { verbosity, .. } => Some(*verbosity),
        _ => None,
    });
    let system = custom.iter().find_map(|entry| match entry {
        ploke_tui::user_config::MessageVerbosity::System {
            verbosity,
            display_init,
            ..
        } => Some((*verbosity, *display_init)),
        _ => None,
    });

    assert_eq!(sysinfo, Some(VerbosityLevel::Error));
    assert_eq!(system, Some((VerbosityLevel::Debug, true)));
}
