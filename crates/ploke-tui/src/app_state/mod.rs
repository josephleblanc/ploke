mod database;
mod models;

// Re-export crate root items so submodules can `use super::*` like the prior monolithic file.
use super::*;

// Internal modules for a modularized AppState
pub mod commands;
pub mod core;
mod dispatcher;
mod events;
pub mod handlers;
mod helpers;

// Public re-exports to keep external API stable
pub use commands::{ListNavigation, StateCommand, StateError};
pub use core::{
    AppState, ChatState, ConfigState, IndexingState, SystemState, SystemStatus,
};
pub use dispatcher::state_manager;
pub use events::MessageUpdatedEvent;

// Back-compat wrapper for legacy tests/calls that constructed Config via struct literal
// before the 'editing' field existed. Converts into core::Config with defaults.
#[derive(Debug, Clone, Default)]
pub struct Config {
    pub llm_params: crate::llm::LLMParameters,
    pub provider_registry: crate::user_config::ProviderRegistry,
}

impl From<Config> for core::Config {
    fn from(c: Config) -> core::Config {
        core::Config {
            llm_params: c.llm_params,
            provider_registry: c.provider_registry,
            editing: core::EditingConfig::default(),
        }
    }
}

// Keep tests colocated under app_state after refactor
#[cfg(test)]
mod tests;
