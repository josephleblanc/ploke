mod database;
pub use database::IndexTargetDir;
mod models;

// Re-export crate root items so submodules can `use super::*` like the prior monolithic file.
use super::*;

// Internal modules for a modularized AppState
pub mod commands;
pub mod core;
mod dispatcher;
pub mod events;
pub mod handlers;
mod helpers;

// Public re-exports to keep external API stable
pub use commands::{ListNavigation, StateCommand, StateError};
pub use core::{
    AppState, ChatState, ConfigState, IndexingState, LoadedCrateState, RuntimeConfig, SystemState,
    SystemStatus,
};
pub use events::SystemMutation;
pub use dispatcher::state_manager;
pub use events::MessageUpdatedEvent;

// Test-only exports (use the crate feature so integration tests can call these helpers)
#[cfg(feature = "test_harness")]
pub use database::test_set_crate_focus_from_db;
#[cfg(feature = "test_harness")]
pub use database::{
    load_workspace_crates_for_test, workspace_remove_for_test, workspace_status_for_test,
    workspace_update_for_test,
};
#[cfg(feature = "test_harness")]
pub use handlers::indexing::set_indexing_test_delay_ms;

// Keep tests colocated under app_state after refactor
#[cfg(test)]
mod tests;
