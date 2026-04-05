#[path = "integration/approvals_overlay_keys.rs"]
mod approvals_overlay_keys;

#[path = "integration/approvals_overlay_render.rs"]
mod approvals_overlay_render;

#[path = "integration/command_feedback_policy.rs"]
mod command_feedback_policy;

#[path = "integration/command_quit.rs"]
mod command_quit;

#[path = "integration/command_stress_loom.rs"]
mod command_stress_loom;

#[path = "integration/command_stress_proptest.rs"]
mod command_stress_proptest;

#[path = "integration/command_stress_tokio.rs"]
mod command_stress_tokio;

#[path = "integration/command_verbosity_profile.rs"]
mod command_verbosity_profile;

#[path = "integration/commands_parser_m1.rs"]
mod commands_parser_m1;

#[path = "integration/config_overlay_footer.rs"]
mod config_overlay_footer;

#[path = "integration/config_overlay_message_verbosity.rs"]
mod config_overlay_message_verbosity;

#[path = "integration/editor_command.rs"]
mod editor_command;

#[path = "integration/get_code_edges_regression.rs"]
mod get_code_edges_regression;

#[path = "integration/index_workspace_targets.rs"]
mod index_workspace_targets;

#[path = "integration/indexing_freeze_app_loop.rs"]
mod indexing_freeze_app_loop;

#[path = "integration/indexing_freeze_repro.rs"]
mod indexing_freeze_repro;

#[path = "integration/indexing_non_blocking.rs"]
mod indexing_non_blocking;

#[path = "integration/input_keymap.rs"]
mod input_keymap;

#[path = "integration/input_keymap_command.rs"]
mod input_keymap_command;

#[path = "integration/input_keymap_insert.rs"]
mod input_keymap_insert;

#[path = "integration/input_keymap_normal.rs"]
mod input_keymap_normal;

#[path = "integration/load_db_crate_focus.rs"]
mod load_db_crate_focus;

#[path = "integration/no_workspace_fallback.rs"]
mod no_workspace_fallback;

#[path = "integration/observability_lifecycle.rs"]
mod observability_lifecycle;

#[path = "integration/overlay_fixture_tests.rs"]
mod overlay_fixture_tests;

#[path = "integration/overlay_intents.rs"]
mod overlay_intents;

#[path = "integration/overlay_manager_smoke.rs"]
mod overlay_manager_smoke;

#[path = "integration/post_apply_rescan.rs"]
mod post_apply_rescan;

#[path = "integration/prop_conversation_scroll.rs"]
mod prop_conversation_scroll;

#[path = "integration/prop_input_view.rs"]
mod prop_input_view;

#[path = "integration/prop_message_update.rs"]
mod prop_message_update;

#[path = "integration/proposals_persistence.rs"]
mod proposals_persistence;

#[path = "integration/tokens_estimate_accuracy.rs"]
mod tokens_estimate_accuracy;

#[path = "integration/tool_call_event_ordering.rs"]
mod tool_call_event_ordering;

#[path = "integration/tool_io_roundtrip.rs"]
mod tool_io_roundtrip;

#[path = "integration/tool_ui_payload.rs"]
mod tool_ui_payload;

#[path = "integration/tool_ui_payload_fixture.rs"]
mod tool_ui_payload_fixture;

#[path = "integration/tool_workspace_path_scoping.rs"]
mod tool_workspace_path_scoping;

#[path = "integration/ui_model_browser_snapshot.rs"]
mod ui_model_browser_snapshot;

#[path = "integration/workspace_status_update.rs"]
mod workspace_status_update;

#[path = "integration/workspace_subset_remove.rs"]
mod workspace_subset_remove;
