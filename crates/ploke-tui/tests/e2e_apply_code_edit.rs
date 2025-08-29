use std::{sync::Arc, time::Duration};

use ploke_core::{PROJECT_NAMESPACE_UUID, TrackingHash};
use ploke_tui::{
    AppEvent, EventBus, RagEvent,
    app_state::{
        RuntimeConfig,
        commands::StateCommand,
        core::{AppState, ChatState, ConfigState, SystemState},
    },
    event_bus::EventBusCaps,
    llm::{self, LLMParameters},
    system::SystemEvent,
    tracing_setup::{init_tracing, init_tracing_tests},
    user_config::{ModelConfig, ModelRegistry, ProviderType, default_model},
};
use quote::ToTokens;
use tokio::sync::{RwLock, mpsc};
use tracing::Level;
use uuid::Uuid;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn e2e_apply_code_edit_real_llm() {
    // Gate by environment variable to avoid running in CI by default.
    // if std::env::var("PLOKE_TUI_E2E_LLM").unwrap_or_default() != "1" {
    //     eprintln!("Skipping e2e_apply_code_edit_real_llm (set PLOKE_TUI_E2E_LLM=1 to enable).");
    //     return;
    // }
    let _guard = init_tracing_tests(Level::TRACE);
    // Require a real OpenRouter API key in the environment.
    let api_key = match std::env::var("OPENROUTER_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!("Skipping: OPENROUTER_API_KEY not set.");
            return;
        }
    };


}
