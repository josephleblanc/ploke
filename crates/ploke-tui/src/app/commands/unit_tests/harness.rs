//! ## Overview
//!
//! `TestRuntime` is a type-state builder for setting up isolated integration
//! tests. It incrementally spawns the async actors that make up the app
//! (FileManager, StateManager, EventBus, etc.) while giving you hooks to
//! intercept and observe events/commands.
//!
//! ## The Type Parameters (F, S, E, L, O)
//!
//! Each generic parameter tracks whether an actor is `Spawned` or `NotSpawned`:
//!
//! | Param | Actor           | What it does                                       |
//! |-------|-----------------|----------------------------------------------------|
//! | `F`   | FileManager     | File I/O + watcher                                 |
//! | `S`   | StateManager    | Core state machine (intercepts commands via relay) |
//! | `E`   | EventBus        | Pub/sub for events                                 |
//! | `L`   | LlmManager      | LLM requests                                       |
//! | `O`   | Observability   | Tracing/metrics                                    |
//!
//! This is a **compile-time state machine** — once you call
//! `spawn_state_manager()`, the return type has `S=Spawned` and you can't spawn
//! it again. Prevents double-spawn bugs at compile time.
//!
//! ## Key Design: The Relay Pattern
//!
//! The most important piece is `RelayStateCmd` (lines 40-124). When you spawn the state manager:
//!
//! 1. **Interception**: Your test's `cmd_tx` actually sends to the relay, not
//! the real state manager
//! 2. **Debug emission**: The relay converts every `StateCommand` to a
//! `DebugStateCommand` (string representation) and forwards it to
//! `debug_string_rx`
//! 3. **Proxying**: Commands with `oneshot::Sender` fields get proxied so
//! callers still receive responses
//! 4. **Forwarding**: The actual command goes to the real state manager
//!
//! The presence of the Relay means tests can `recv()` on `debug_string_rx` to
//! assert exactly which `StateCommand` was issued without needing the real
//! state manager to process it.
//!
//! ## Usage Patterns
//!
//! ### 1. Minimal Setup (App handle only, no actors)
//! ```rust,norun
//! let rt = TestRuntime::new(&fixture_db);  // All params = NotSpawned
//! let app = rt.into_app(pwd);              // Get App handle, no spawning needed
//! // Use app.state_cmd_tx() to send commands, but nothing processes them
//! ```
//!
//! ### 2. With State Manager (most command tests)
//! ```rust,norun
//! let rt = TestRuntime::new(&fixture_db)
//!     .spawn_state_manager();              // Returns TestRuntime<_, Spawned, _, _, _>
//!
//! let events = rt.events_builder().build_app_only();
//! let mut debug_rx = events.app_actor_events.debug_string_rx.unwrap();
//!
//! let app = rt.into_app(pwd);
//! // Send command, assert on debug_rx.recv()
//! ```
//!
//! ### 3. Full Stack (for integration tests)
//! ```rust,norun
//! let rt = TestRuntime::new(&fixture_db)
//!     .spawn_file_manager()
//!     .spawn_state_manager()
//!     .spawn_event_bus()
//!     .spawn_llm_manager()
//!     .spawn_observability();
//! ```
//!
//! ## The Events Builder
//!
//! After spawning, `rt.events_builder()` gives you a **type-state builder** for subscribing to channels:
//!
//! ```rust,norun
//! let events = rt.events_builder()
//!     .build_app_only();           // Just app actor events + debug_string_rx
//!     .build_app_io();             // App + I/O manager
//!     .build_app_event_bus();      // App + event bus subscriptions
//!     .build_all();                 // Everything
//! ```
// AI_DOC:written kimi-k2.5 2026-04-04
// AI_DOC:checked JL        2026-04-04
use std::{fmt::Debug, path::PathBuf, sync::Arc, time::Duration};

use color_eyre::Result;
use lazy_static::lazy_static;
use ploke_db::bm25_index;
use ploke_embed::{
    cancel_token::CancellationToken,
    indexer::{self, EmbeddingProcessor, IndexerTask},
};
use ploke_io::FileChangeEvent;
use ploke_llm::router_only::default_model;
use ploke_rag::{RagConfig, RagService, TokenBudget};
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use tempfile::{TempDir, tempdir};
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, oneshot, watch};
use tokio::time::timeout;

use crate::{
    AppEvent, CancelChatToken, ErrorEvent, ErrorSeverity, EventBus, EventBusCaps, EventPriority,
    RagEvent,
    app::App,
    app_state::state_manager,
    app_state::{AppState, ChatState, ConfigState, RuntimeConfig, StateCommand, SystemState},
    app_state::{
        IndexTargetDir,
        commands::{IndexResolution, emit_validation_error, validate_state_command},
    },
    chat_history::ChatHistory,
    context_plan,
    file_man::FileManager,
    llm::manager::llm_manager,
    observability, run_event_bus,
    user_config::{EmbeddingConfig, UserConfig},
};

#[derive(Debug, Clone)]
pub struct DebugStateCommand(String);
impl DebugStateCommand {
    pub fn debug_string_from_ref(cmd: &StateCommand) -> Self {
        let debug_string = format!("{:?}", cmd);
        Self(debug_string)
    }

    /// Returns the debug string representation of the StateCommand.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone)]
pub struct ValidationProbeEvent {
    command: String,
    validation: Option<Result<(), String>>,
    /// User-facing error message (if any)
    error_message: Option<String>,
    /// Recovery suggestion (if any)
    recovery_suggestion: Option<String>,
    /// Resolution failure from `/index` (if any)
    resolve_error: Option<String>,
    /// Structured `/index` resolution when the command maps to a concrete target.
    resolved_index_target: Option<IndexResolution>,
    /// Focus root hint for `/index` when the resolved target should become focused.
    focus_root: Option<PathBuf>,
}

impl ValidationProbeEvent {
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn validation(&self) -> Option<&Result<(), String>> {
        self.validation.as_ref()
    }

    pub fn error_message(&self) -> Option<&str> {
        self.error_message.as_deref()
    }

    pub fn recovery_suggestion(&self) -> Option<&str> {
        self.recovery_suggestion.as_deref()
    }

    pub fn resolve_error(&self) -> Option<&str> {
        self.resolve_error.as_deref()
    }

    pub fn resolved_index_target(&self) -> Option<&IndexResolution> {
        self.resolved_index_target.as_ref()
    }

    pub fn focus_root(&self) -> Option<&std::path::Path> {
        self.focus_root.as_deref()
    }
}

pub(crate) struct RelayStateCmd {
    state_cmd_rx: mpsc::Receiver<StateCommand>,
    state_cmd_tx: mpsc::Sender<StateCommand>,
    debug_string_tx: mpsc::Sender<DebugStateCommand>,
}

pub(crate) struct ValidationRelayStateCmd {
    state_cmd_rx: mpsc::Receiver<StateCommand>,
    debug_string_tx: mpsc::Sender<DebugStateCommand>,
    validation_tx: mpsc::Sender<ValidationProbeEvent>,
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
}

async fn relay_oneshot<T: Send + 'static>(
    original: oneshot::Sender<T>,
    relay_rx: oneshot::Receiver<T>,
) {
    match relay_rx.await {
        Ok(v) => {
            let _ = original.send(v);
        }
        Err(_) => {
            // The proxied sender was dropped without sending.
            // The original caller will also see a RecvError, which is fine.
        }
    }
}

impl RelayStateCmd {
    pub(crate) async fn run_relay(self) {
        let RelayStateCmd {
            mut state_cmd_rx,
            state_cmd_tx,
            debug_string_tx,
        } = self;
        while let Some(cmd) = state_cmd_rx.recv().await {
            // 1. Emit debug string first
            let debug_string = DebugStateCommand::debug_string_from_ref(&cmd);
            let _ = debug_string_tx
                .send(debug_string)
                .await
                .inspect_err(|e| tracing::error!("{e}"));

            // 2. Proxy oneshot::Sender variants so the original caller still gets the response
            let proxied_cmd = match cmd {
                StateCommand::AddUserMessage {
                    content,
                    new_user_msg_id,
                    completion_tx,
                } => {
                    let (proxy_tx, proxy_rx) = oneshot::channel();
                    tokio::spawn(relay_oneshot(completion_tx, proxy_rx));
                    StateCommand::AddUserMessage {
                        content,
                        new_user_msg_id,
                        completion_tx: proxy_tx,
                    }
                }
                StateCommand::CreateAssistantMessage {
                    parent_id,
                    new_assistant_msg_id,
                    responder,
                } => {
                    let (proxy_tx, proxy_rx) = oneshot::channel();
                    tokio::spawn(relay_oneshot(responder, proxy_rx));
                    StateCommand::CreateAssistantMessage {
                        parent_id,
                        new_assistant_msg_id,
                        responder: proxy_tx,
                    }
                }
                StateCommand::ScanForChange { scan_tx } => {
                    let (proxy_tx, proxy_rx) = oneshot::channel();
                    tokio::spawn(relay_oneshot(scan_tx, proxy_rx));
                    StateCommand::ScanForChange { scan_tx: proxy_tx }
                }
                // EmbedMessage carries Receivers; we cannot proxy those because
                // the corresponding Senders live in the code that created the command.
                other => other,
            };

            // 3. Forward to the real state manager
            let _ = state_cmd_tx
                .send(proxied_cmd)
                .await
                .inspect_err(|e| tracing::error!("{e}"));
        }
    }
}

impl ValidationRelayStateCmd {
    pub(crate) async fn run_relay(self) {
        let ValidationRelayStateCmd {
            mut state_cmd_rx,
            debug_string_tx,
            validation_tx,
            state,
            event_bus,
        } = self;

        // Subscribe to error events for capturing user-facing errors
        let mut error_rx = event_bus.subscribe(EventPriority::Realtime);

        while let Some(cmd) = state_cmd_rx.recv().await {
            let debug_string = DebugStateCommand::debug_string_from_ref(&cmd);
            let _ = debug_string_tx
                .send(debug_string)
                .await
                .inspect_err(|e| tracing::error!("{e}"));

            let mut resolve_error = None;
            let mut index_recovery_suggestion = None;
            let mut focus_root = None;
            let resolved_index_target = match &cmd {
                StateCommand::Index(cmd) => match cmd.resolve(&state).await {
                    Ok(resolution) => {
                        focus_root = resolution.focus_root.clone();
                        Some(resolution)
                    }
                    Err(err) => {
                        resolve_error = Some(err.user_message());
                        index_recovery_suggestion = Some(err.recovery_suggestion());
                        None
                    }
                },
                StateCommand::IndexTargetDir {
                    target_dir,
                    needs_parse,
                } => target_dir.clone().map(|target_dir| IndexResolution {
                    target_dir,
                    needs_parse: *needs_parse,
                    focus_root: None,
                }),
                _ => None,
            };

            let validation: Option<Result<(), String>> =
                match validate_state_command(&cmd, &state).await {
                    Some(Ok(())) => Some(Ok(())),
                    Some(Err(err)) => {
                        emit_validation_error(&event_bus, err.clone());
                        Some(Err(err.to_string()))
                    }
                    None => None,
                };

            // Capture either a validation error event or the user-facing `/index`
            // resolve failure summary. Use a short timeout to avoid blocking tests.
            let (error_message, recovery_suggestion) =
                if resolve_error.is_some() || index_recovery_suggestion.is_some() {
                    (None, index_recovery_suggestion)
                } else {
                    match timeout(Duration::from_millis(10), error_rx.recv()).await {
                        Ok(Ok(AppEvent::Error(error_event))) => {
                            // Extract error message from ErrorEvent
                            let msg = Some(error_event.message.clone());
                            (msg, None)
                        }
                        _ => (None, None),
                    }
                };

            if validation.is_some()
                || error_message.is_some()
                || recovery_suggestion.is_some()
                || resolve_error.is_some()
                || resolved_index_target.is_some()
                || focus_root.is_some()
            {
                let _ = validation_tx
                    .send(ValidationProbeEvent {
                        command: cmd.discriminant().to_string(),
                        validation,
                        error_message,
                        recovery_suggestion,
                        resolve_error,
                        resolved_index_target,
                        focus_root,
                    })
                    .await
                    .inspect_err(|e| tracing::error!("{e}"));
            }
        }
    }
}

// =============================================================================
// EXTENDING THE RELAY: A Guide for Future Developers
// =============================================================================
//
// The `RelayStateCmd` intercepts `StateCommand`s sent to the state manager.
// This is necessary because `StateCommand` cannot implement `Clone` (it contains
// oneshot channels), so we cannot simply tee the stream.
//
// There are TWO patterns for handling oneshots in commands:
//
// -----------------------------------------------------------------------------
// PATTERN 1: Commands WITH oneshot::Sender<T> (OUTBOUND responses)
// -----------------------------------------------------------------------------
// These commands expect a response FROM the state manager BACK to the caller.
// Example: `AddUserMessage { completion_tx: oneshot::Sender<()> }`
//
// The caller creates the oneshot pair, keeps the Receiver, and sends the Sender
// in the command. The state manager sends the response via the Sender.
//
// TO ADD A NEW SENDER VARIANT:
//   1. Add a match arm in `run_relay()` that detects your variant
//   2. Create a new oneshot channel: `(proxy_tx, proxy_rx) = oneshot::channel()`
//   3. Spawn `relay_oneshot(original_sender, proxy_rx)` to forward responses
//   4. Send the command with `proxy_tx` instead of the original
//
// This ensures the original caller still receives the response after the relay
// intercepts the command for debugging/logging.
//
// -----------------------------------------------------------------------------
// PATTERN 2: Commands WITH oneshot::Receiver<T> (INBOUND responses)
// -----------------------------------------------------------------------------
// These commands provide a way for the state manager to RECEIVE a response
// from somewhere else. Example: `EmbedMessage { completion_rx, scan_rx }`
//
// The caller creates the oneshot pair, keeps the Sender, and sends the Receiver
// in the command. The state manager awaits the Receiver.
//
// IMPORTANT: The relay CANNOT proxy these because it doesn't have access to
// the corresponding Senders (they're held by the code that created the command).
// These variants are forwarded as-is.
//
// TO TEST COMMANDS WITH RECEIVERS:
//   If you want to intercept these commands and provide mock responses, you
//   need to create the oneshot pair yourself in the test:
//
//   ```rust
//   let (completion_tx, completion_rx) = oneshot::channel();
//   let (scan_tx, scan_rx) = oneshot::channel();
//
//   let cmd = StateCommand::EmbedMessage {
//       new_msg_id,
//       completion_rx,  // State manager will await this
//       scan_rx,        // State manager will await this
//   };
//
//   // Send the command
//   cmd_tx.send(cmd).await.unwrap();
//
//   // Now you can send responses via the Senders you kept:
//   completion_tx.send(()).unwrap();
//   scan_tx.send(Some(vec![PathBuf::from("/test")])).unwrap();
//   ```
//
//   If you need to intercept these commands from the relay (to observe that
//   they were sent without modifying them), simply add them to the match
//   in `run_relay()` as a no-op forwarding case.
//
// -----------------------------------------------------------------------------
// SUMMARY TABLE
// -----------------------------------------------------------------------------
// | Variant Field        | Relay Action      | Test Strategy                  |
// |----------------------|-------------------|--------------------------------|
// | oneshot::Sender<T>   | Proxy via relay   | Test receives response normally|
// | oneshot::Receiver<T> | Forward as-is     | Test creates pair, keeps Sender|
//
// =============================================================================
// AI_DOC:written kimi-k2.5 2026-04-03
// AI_DOC:checked JL        2026-04-04

// example mock setup of sub-component that would usually interact with external
// resources like filesystem
struct MockUserConfig {
    user_config: UserConfig,
}

impl MockUserConfig {
    pub fn mock_load_default(&self) -> Result<EmbeddingProcessor> {
        self.user_config.load_embedding_processor()
    }

    pub fn mock_load_from_tempdir(&self, mock_path: &TempDir) -> Result<MockUserConfig> {
        let content = std::fs::read_to_string(mock_path)?;
        let user_config: UserConfig = toml::from_str(&content)?;
        Ok(MockUserConfig { user_config })
    }
}

pub trait TestAppAccessor {
    fn state_cmd_tx(&self) -> mpsc::Sender<StateCommand>;
}

impl TestAppAccessor for App {
    fn state_cmd_tx(&self) -> mpsc::Sender<StateCommand> {
        self.cmd_tx.clone()
    }
}

/// Builder for the test helper struct that listens for items that could be
/// recieved by sent by App
pub struct TestInAppActorBuilder {
    pub event_rx: Option<broadcast::Receiver<AppEvent>>,
    pub bg_event_rx: Option<broadcast::Receiver<AppEvent>>,
    pub cancel_chat_rx: Option<watch::Receiver<CancelChatToken>>,
    pub context_plan_history: Option<Arc<std::sync::RwLock<context_plan::ContextPlanHistory>>>,
    pub debug_string_rx: Option<mpsc::Receiver<DebugStateCommand>>,
    pub validation_rx: Option<mpsc::Receiver<ValidationProbeEvent>>,
}

/// Listens for items that could be recieved by sent by App
/// All fields are non-Optional - guaranteed to exist after building
pub struct TestInAppActor {
    pub event_rx: broadcast::Receiver<AppEvent>,
    pub bg_event_rx: broadcast::Receiver<AppEvent>,
    pub cancel_chat_rx: watch::Receiver<CancelChatToken>,
    pub context_plan_history: Option<Arc<std::sync::RwLock<context_plan::ContextPlanHistory>>>,
    pub debug_string_rx: Option<mpsc::Receiver<DebugStateCommand>>,
    pub validation_rx: Option<mpsc::Receiver<ValidationProbeEvent>>,
}

/// Listens for broadcast events that could be sent by the IoManagerHandle
#[derive(Default)]
pub struct TestOutIoManagerHandleBuilder {
    /// Only active in IoManagerHandle with feature "watcher", off by default
    /// but currently enabled in ploke-tui dep features for ploke-io
    /// correspondes to
    /// IoManagerHandle.events_tx: Option<broadcast::Sender<FileChangeEvent>>
    events_tx_receiver: Option<broadcast::Receiver<FileChangeEvent>>,
    // only other field is request_sender: mpsc, can't intercept/sub here
}

/// Listens for broadcast events that could be sent by the IoManagerHandle
/// Field is non-Optional - guaranteed to exist after building
pub struct TestOutIoManagerHandle {
    /// Only active in IoManagerHandle with feature "watcher", off by default
    /// but currently enabled in ploke-tui dep features for ploke-io
    /// correspondes to
    /// IoManagerHandle.events_tx: Option<broadcast::Sender<FileChangeEvent>>
    pub events_tx_receiver: broadcast::Receiver<FileChangeEvent>,
    // only other field is request_sender: mpsc, can't intercept/sub here
}

/// Only active in IoManagerHandle with feature "watcher", off by default but
/// currently enabled in ploke-tui dep features for ploke-io
impl TestOutIoManagerHandleBuilder {
    pub fn with_events_tx_receiver(&mut self, events_tx: broadcast::Sender<FileChangeEvent>) {
        let rx = events_tx.subscribe();
        self.events_tx_receiver = Some(rx);
    }

    pub fn from_io(io_handle: &ploke_io::IoManagerHandle) -> Self {
        Self {
            events_tx_receiver: Some(io_handle.subscribe_file_events()),
        }
    }

    pub fn build(self) -> TestOutIoManagerHandle {
        TestOutIoManagerHandle {
            events_tx_receiver: self.events_tx_receiver.expect("events_tx_receiver not set"),
        }
    }
}

/// Builds items that could be sent by EventBus for testing
#[derive(Default)]
pub struct TestOutEventBusBuilder {
    pub realtime_tx_rx: Option<broadcast::Receiver<AppEvent>>,
    pub background_tx_rx: Option<broadcast::Receiver<AppEvent>>,
    pub error_tx_rx: Option<broadcast::Receiver<ErrorEvent>>,
    pub index_tx_rx: Option<Arc<broadcast::Receiver<indexer::IndexingStatus>>>,
}

/// Items that could be sent by EventBus for testing
/// All fields are non-Optional - guaranteed to exist after building
pub struct TestOutEventBus {
    pub realtime_tx_rx: broadcast::Receiver<AppEvent>,
    pub background_tx_rx: broadcast::Receiver<AppEvent>,
    pub error_tx_rx: broadcast::Receiver<ErrorEvent>,
    pub index_tx_rx: Arc<broadcast::Receiver<indexer::IndexingStatus>>,
}

impl TestOutEventBusBuilder {
    pub fn with_subscribe_realtime(&mut self, event_tx: broadcast::Sender<AppEvent>) {
        let rx = event_tx.subscribe();
        self.realtime_tx_rx = Some(rx);
    }

    pub fn with_subscribe_background(&mut self, event_tx: broadcast::Sender<AppEvent>) {
        let rx = event_tx.subscribe();
        self.background_tx_rx = Some(rx);
    }

    pub fn with_subscribe_error(&mut self, error_tx: broadcast::Sender<ErrorEvent>) {
        let rx = error_tx.subscribe();
        self.error_tx_rx = Some(rx);
    }

    pub fn with_subscribe_index(&mut self, index_tx: broadcast::Sender<indexer::IndexingStatus>) {
        let rx = index_tx.subscribe();
        self.index_tx_rx = Some(Arc::new(rx));
    }

    pub fn from_event_bus(event_bus: &EventBus) -> Self {
        Self {
            realtime_tx_rx: Some(event_bus.subscribe(EventPriority::Realtime)),
            background_tx_rx: Some(event_bus.subscribe(EventPriority::Background)),
            error_tx_rx: Some(event_bus.error_subscriber()),
            index_tx_rx: Some(Arc::new(event_bus.index_subscriber())),
        }
    }

    pub fn build(self) -> TestOutEventBus {
        TestOutEventBus {
            realtime_tx_rx: self.realtime_tx_rx.expect("realtime_tx_rx not set"),
            background_tx_rx: self.background_tx_rx.expect("background_tx_rx not set"),
            error_tx_rx: self.error_tx_rx.expect("error_tx_rx not set"),
            index_tx_rx: self.index_tx_rx.expect("index_tx_rx not set"),
        }
    }
}

impl Default for TestInAppActorBuilder {
    fn default() -> Self {
        Self {
            event_rx: None,
            bg_event_rx: None,
            cancel_chat_rx: None,
            context_plan_history: None,
            debug_string_rx: None,
            validation_rx: None,
        }
    }
}

impl TestInAppActorBuilder {
    pub fn with_subscribe_event(&mut self, event_tx: broadcast::Sender<AppEvent>) {
        let rx = event_tx.subscribe();
        self.event_rx = Some(rx);
    }

    pub fn with_subscribe_bg_event(&mut self, event_tx: broadcast::Sender<AppEvent>) {
        let rx = event_tx.subscribe();
        self.bg_event_rx = Some(rx);
    }

    pub fn with_subscribe_cancel_chat(&mut self, cancel_chat_tx: watch::Sender<CancelChatToken>) {
        let rx = cancel_chat_tx.subscribe();
        self.cancel_chat_rx = Some(rx);
    }

    pub fn with_context_plan(
        &mut self,
        context_plan_history: Arc<std::sync::RwLock<context_plan::ContextPlanHistory>>,
    ) {
        let context_plan_hook = Arc::clone(&context_plan_history);
        self.context_plan_history = Some(context_plan_hook);
    }

    pub fn from_app(
        event_bus: &EventBus,
        cancel_tx: &watch::Sender<CancelChatToken>,
        debug_string_rx: Option<mpsc::Receiver<DebugStateCommand>>,
        validation_rx: Option<mpsc::Receiver<ValidationProbeEvent>>,
    ) -> Self {
        Self {
            event_rx: Some(event_bus.subscribe(EventPriority::Realtime)),
            bg_event_rx: Some(event_bus.subscribe(EventPriority::Background)),
            cancel_chat_rx: Some(cancel_tx.subscribe()),
            context_plan_history: None,
            debug_string_rx,
            validation_rx,
        }
    }

    pub fn build(self) -> TestInAppActor {
        TestInAppActor {
            event_rx: self.event_rx.expect("event_rx not set"),
            bg_event_rx: self.bg_event_rx.expect("bg_event_rx not set"),
            cancel_chat_rx: self.cancel_chat_rx.expect("cancel_chat_rx not set"),
            context_plan_history: self.context_plan_history,
            debug_string_rx: self.debug_string_rx,
            validation_rx: self.validation_rx,
        }
    }
}

pub struct Present<T>(T);
pub struct Missing;

pub struct TestEventsBuilder<A, I, E> {
    app: A,
    io: I,
    event_bus: E,
}

impl Default for TestEventsBuilder<Missing, Missing, Missing> {
    fn default() -> Self {
        Self {
            app: Missing,
            io: Missing,
            event_bus: Missing,
        }
    }
}

impl<A, I, E> TestEventsBuilder<A, I, E> {
    pub fn with_app(
        self,
        app: TestInAppActorBuilder,
    ) -> TestEventsBuilder<Present<TestInAppActorBuilder>, I, E> {
        TestEventsBuilder {
            app: Present(app),
            io: self.io,
            event_bus: self.event_bus,
        }
    }

    pub fn with_io(
        self,
        io: TestOutIoManagerHandleBuilder,
    ) -> TestEventsBuilder<A, Present<TestOutIoManagerHandleBuilder>, E> {
        TestEventsBuilder {
            app: self.app,
            io: Present(io),
            event_bus: self.event_bus,
        }
    }

    pub fn with_event_bus(
        self,
        event_bus: TestOutEventBusBuilder,
    ) -> TestEventsBuilder<A, I, Present<TestOutEventBusBuilder>> {
        TestEventsBuilder {
            app: self.app,
            io: self.io,
            event_bus: Present(event_bus),
        }
    }
}

impl<I, E> TestEventsBuilder<Present<TestInAppActorBuilder>, I, E> {
    pub fn build_app_only(self) -> TestActorEventsOnly {
        TestActorEventsOnly {
            app_actor_events: self.app.0.build(),
        }
    }
}

impl<A, E> TestEventsBuilder<A, Present<TestOutIoManagerHandleBuilder>, E> {
    pub fn build_io_only(self) -> TestIoEventsOnly {
        TestIoEventsOnly {
            io_manager_events: self.io.0.build(),
        }
    }
}

impl<A, I> TestEventsBuilder<A, I, Present<TestOutEventBusBuilder>> {
    pub fn build_event_bus_only(self) -> TestEventBusEventsOnly {
        TestEventBusEventsOnly {
            event_bus_events: self.event_bus.0.build(),
        }
    }
}

impl<E>
    TestEventsBuilder<Present<TestInAppActorBuilder>, Present<TestOutIoManagerHandleBuilder>, E>
{
    pub fn build_app_io(self) -> TestActorIoEvents {
        TestActorIoEvents {
            app_actor_events: self.app.0.build(),
            io_manager_events: self.io.0.build(),
        }
    }
}

impl<I> TestEventsBuilder<Present<TestInAppActorBuilder>, I, Present<TestOutEventBusBuilder>> {
    pub fn build_app_event_bus(self) -> TestActorEventBusEvents {
        TestActorEventBusEvents {
            app_actor_events: self.app.0.build(),
            event_bus_events: self.event_bus.0.build(),
        }
    }
}

impl<A>
    TestEventsBuilder<A, Present<TestOutIoManagerHandleBuilder>, Present<TestOutEventBusBuilder>>
{
    pub fn build_io_event_bus(self) -> TestIoEventBusEvents {
        TestIoEventBusEvents {
            io_manager_events: self.io.0.build(),
            event_bus_events: self.event_bus.0.build(),
        }
    }
}

impl
    TestEventsBuilder<
        Present<TestInAppActorBuilder>,
        Present<TestOutIoManagerHandleBuilder>,
        Present<TestOutEventBusBuilder>,
    >
{
    pub fn build_all(self) -> TestAllEvents {
        TestAllEvents {
            app_actor_events: self.app.0.build(),
            io_manager_events: self.io.0.build(),
            event_bus_events: self.event_bus.0.build(),
        }
    }
}

pub struct TestActorEventsOnly {
    pub app_actor_events: TestInAppActor,
}

pub struct TestIoEventsOnly {
    pub io_manager_events: TestOutIoManagerHandle,
}

pub struct TestEventBusEventsOnly {
    pub event_bus_events: TestOutEventBus,
}

pub struct TestActorIoEvents {
    pub app_actor_events: TestInAppActor,
    pub io_manager_events: TestOutIoManagerHandle,
}

pub struct TestActorEventBusEvents {
    pub app_actor_events: TestInAppActor,
    pub event_bus_events: TestOutEventBus,
}

pub struct TestIoEventBusEvents {
    pub io_manager_events: TestOutIoManagerHandle,
    pub event_bus_events: TestOutEventBus,
}

pub struct TestAllEvents {
    pub app_actor_events: TestInAppActor,
    pub io_manager_events: TestOutIoManagerHandle,
    pub event_bus_events: TestOutEventBus,
}

pub struct Spawned;
pub struct NotSpawned;

struct TestRuntimeInner {
    command_style: crate::user_config::CommandStyle,
    tool_verbosity: crate::tools::ToolVerbosity,
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    cmd_tx: mpsc::Sender<StateCommand>,
    cmd_rx: std::sync::Mutex<Option<mpsc::Receiver<StateCommand>>>,
    debug_string_rx: std::sync::Mutex<Option<mpsc::Receiver<DebugStateCommand>>>,
    validation_rx: std::sync::Mutex<Option<mpsc::Receiver<ValidationProbeEvent>>>,
    rag_event_tx: mpsc::Sender<RagEvent>,
    cancel_tx: watch::Sender<CancelChatToken>,
}

/// Type-state test harness that tracks which background actors have been spawned.
/// Creating the runtime is cheap (only channels and Arcs). Actors are spawned
/// on-demand via `spawn_*` methods.
pub struct TestRuntime<
    F = NotSpawned,
    S = NotSpawned,
    E = NotSpawned,
    L = NotSpawned,
    O = NotSpawned,
> {
    inner: Arc<TestRuntimeInner>,
    _file_manager: std::marker::PhantomData<F>,
    _state_manager: std::marker::PhantomData<S>,
    _event_bus_runner: std::marker::PhantomData<E>,
    _llm_manager: std::marker::PhantomData<L>,
    _observability: std::marker::PhantomData<O>,
}

impl<F, S, E, L, O> TestRuntime<F, S, E, L, O> {
    /// Safe because F/S/E/L/O are only PhantomData; layout is identical.
    pub fn _cast<X, Y, Z, W, V>(self) -> TestRuntime<X, Y, Z, W, V> {
        TestRuntime {
            inner: self.inner,
            _file_manager: std::marker::PhantomData,
            _state_manager: std::marker::PhantomData,
            _event_bus_runner: std::marker::PhantomData,
            _llm_manager: std::marker::PhantomData,
            _observability: std::marker::PhantomData,
        }
    }

    /// Build the [`App`] handle. This does **not** require any actors to be spawned.
    pub fn into_app(self, pwd: PathBuf) -> App {
        App::new(
            self.inner.command_style,
            Arc::clone(&self.inner.state),
            self.inner.cmd_tx.clone(),
            &self.inner.event_bus,
            default_model(),
            self.inner.tool_verbosity,
            self.inner.cancel_tx.clone(),
            pwd,
        )
    }

    /// Build the [`App`] handle after seeding `SystemState.pwd` for fast-path tests.
    pub async fn into_app_with_state_pwd(self, pwd: PathBuf) -> App {
        self.inner.state.system.set_pwd_for_test(pwd.clone()).await;
        self.into_app(pwd)
    }

    /// Convenience wrapper that returns the app wrapped in `Arc<Mutex<App>>`.
    pub fn into_app_arc(self, pwd: PathBuf) -> Arc<Mutex<App>> {
        Arc::new(Mutex::new(self.into_app(pwd)))
    }

    /// Set up SystemStatus to reflect a loaded workspace/crate state.
    /// This simulates the state after a successful `/load` command without requiring
    /// the actual load process (registry lookup, DB restore, etc.).
    ///
    /// # Arguments
    /// * `workspace_root` - Path to the workspace root directory
    /// * `member_roots` - Paths to workspace member crate roots
    /// * `focused_root` - Path to the initially focused crate (must be in member_roots)
    ///
    /// # Example
    /// ```rust,ignore
    /// rt.setup_loaded_workspace(
    ///     "/path/to/workspace".into(),
    ///     vec!["/path/to/workspace/member_a".into(), "/path/to/workspace/member_b".into()],
    ///     Some("/path/to/workspace/member_a".into()),
    /// ).await;
    /// ```
    pub async fn setup_loaded_workspace(
        &self,
        workspace_root: PathBuf,
        member_roots: Vec<PathBuf>,
        focused_root: Option<PathBuf>,
    ) {
        let _ = self
            .inner
            .state
            .with_system_raw(|sys| {
                sys.set_loaded_workspace(workspace_root, member_roots, focused_root);
            })
            .await;
    }

    /// Set up SystemStatus to reflect a loaded standalone crate (no workspace).
    /// Standalone crates are loaded as a single-member workspace.
    ///
    /// # Arguments
    /// * `crate_root` - Path to the crate root directory
    pub async fn setup_loaded_standalone_crate(&self, crate_root: PathBuf) {
        // A standalone crate is treated as a single-member workspace
        self.setup_loaded_workspace(crate_root.clone(), vec![crate_root], None)
            .await;
    }

    /// Returns a fully-populated [`TestEventsBuilder`] wired to this runtime's channels.
    pub fn events_builder(
        &self,
    ) -> TestEventsBuilder<
        Present<TestInAppActorBuilder>,
        Present<TestOutIoManagerHandleBuilder>,
        Present<TestOutEventBusBuilder>,
    > {
        let debug_string_rx = self
            .inner
            .debug_string_rx
            .lock()
            .expect("debug_string_rx mutex poisoned")
            .take();
        let validation_rx = self
            .inner
            .validation_rx
            .lock()
            .expect("validation_rx mutex poisoned")
            .take();
        TestEventsBuilder::default()
            .with_app(TestInAppActorBuilder::from_app(
                &self.inner.event_bus,
                &self.inner.cancel_tx,
                debug_string_rx,
                validation_rx,
            ))
            .with_io(TestOutIoManagerHandleBuilder::from_io(
                &self.inner.state.io_handle,
            ))
            .with_event_bus(TestOutEventBusBuilder::from_event_bus(
                &self.inner.event_bus,
            ))
    }
}

impl TestRuntime<NotSpawned, NotSpawned, NotSpawned, NotSpawned, NotSpawned> {
    /// Create a lightweight runtime backed by `fixture_db`. No tasks are spawned yet.
    pub fn new(fixture_db: &Arc<ploke_db::Database>) -> Self {
        let config = UserConfig::default();
        let runtime_cfg: RuntimeConfig = config.clone().into();
        let tool_verbosity = runtime_cfg.tool_verbosity;

        let db_handle = Arc::clone(fixture_db);

        let processor = config
            .load_embedding_processor()
            .expect("load embedding processor");
        let embedding_runtime = Arc::new(ploke_embed::runtime::EmbeddingRuntime::from_shared_set(
            Arc::clone(&db_handle.active_embedding_set),
            processor,
        ));

        let io_handle = ploke_io::IoManagerHandle::builder()
            .enable_watcher(true)
            .build();

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        let bm25_cmd = bm25_index::bm25_service::start(Arc::clone(&db_handle), 0.0)
            .expect("start bm25 service");

        let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
        let indexer_task = IndexerTask::new(
            db_handle.clone(),
            io_handle.clone(),
            Arc::clone(&embedding_runtime),
            index_cancellation_token,
            index_cancel_handle,
            None,
        )
        .with_bm25_tx(bm25_cmd);
        let indexer_task = Arc::new(indexer_task);

        let rag = match RagService::new_full(
            db_handle.clone(),
            Arc::clone(&embedding_runtime),
            io_handle.clone(),
            RagConfig::default(),
        ) {
            Ok(svc) => Some(Arc::new(svc)),
            Err(_e) => None,
        };

        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(runtime_cfg),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(Arc::clone(&indexer_task)),
            indexing_control: Arc::new(Mutex::new(None)),
            db: db_handle,
            embedder: Arc::clone(&embedding_runtime),
            io_handle,
            proposals: RwLock::new(std::collections::HashMap::new()),
            create_proposals: RwLock::new(std::collections::HashMap::new()),
            rag,
            budget: TokenBudget::default(),
        });

        let (rag_event_tx, _rag_event_rx) = mpsc::channel(10);
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(1024);
        let (cancel_tx, _cancel_rx) = watch::channel(CancelChatToken::KeepOpen);

        Self {
            inner: Arc::new(TestRuntimeInner {
                command_style: config.command_style,
                tool_verbosity,
                state,
                event_bus,
                cmd_tx,
                cmd_rx: std::sync::Mutex::new(Some(cmd_rx)),
                debug_string_rx: std::sync::Mutex::new(None),
                validation_rx: std::sync::Mutex::new(None),
                rag_event_tx,
                cancel_tx,
            }),
            _file_manager: std::marker::PhantomData,
            _state_manager: std::marker::PhantomData,
            _event_bus_runner: std::marker::PhantomData,
            _llm_manager: std::marker::PhantomData,
            _observability: std::marker::PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Spawn methods (each can only be called once because the method disappears
// after the type parameter becomes Spawned)
// ---------------------------------------------------------------------------

impl<F, S, E, L, O> TestRuntime<F, S, E, L, O> {
    pub fn spawn_file_manager(self) -> TestRuntime<Spawned, S, E, L, O> {
        let pwd = std::env::current_dir().expect("current dir");
        let fm = FileManager::new(
            self.inner.state.io_handle.clone(),
            self.inner.event_bus.subscribe(EventPriority::Background),
            self.inner.event_bus.background_tx.clone(),
            self.inner.rag_event_tx.clone(),
            self.inner.event_bus.realtime_tx.clone(),
            pwd,
        );
        tokio::spawn(fm.run());
        self._cast()
    }

    pub fn spawn_state_manager(self) -> TestRuntime<F, Spawned, E, L, O> {
        let cmd_rx = self
            .inner
            .cmd_rx
            .lock()
            .expect("cmd_rx mutex poisoned")
            .take()
            .expect("cmd_rx already consumed; state_manager can only be spawned once");

        let (state_cmd_relay_tx, state_cmd_relay_rx) = mpsc::channel::<StateCommand>(1024);
        let (debug_string_tx, debug_string_rx) = mpsc::channel::<DebugStateCommand>(1024);
        let debug_relay = RelayStateCmd {
            state_cmd_rx: cmd_rx,
            state_cmd_tx: state_cmd_relay_tx,
            debug_string_tx,
        };

        *self
            .inner
            .debug_string_rx
            .lock()
            .expect("debug_string_rx mutex poisoned") = Some(debug_string_rx);

        tokio::spawn(debug_relay.run_relay());
        tokio::spawn(state_manager(
            Arc::clone(&self.inner.state),
            state_cmd_relay_rx,
            Arc::clone(&self.inner.event_bus),
            self.inner.rag_event_tx.clone(),
        ));
        self._cast()
    }

    pub fn spawn_validation_probe(self) -> TestRuntime<F, Spawned, E, L, O> {
        let cmd_rx = self
            .inner
            .cmd_rx
            .lock()
            .expect("cmd_rx mutex poisoned")
            .take()
            .expect("cmd_rx already consumed; validation probe can only be spawned once");

        let (debug_string_tx, debug_string_rx) = mpsc::channel::<DebugStateCommand>(1024);
        let (validation_tx, validation_rx) = mpsc::channel::<ValidationProbeEvent>(1024);
        let probe = ValidationRelayStateCmd {
            state_cmd_rx: cmd_rx,
            debug_string_tx,
            validation_tx,
            state: Arc::clone(&self.inner.state),
            event_bus: Arc::clone(&self.inner.event_bus),
        };

        *self
            .inner
            .debug_string_rx
            .lock()
            .expect("debug_string_rx mutex poisoned") = Some(debug_string_rx);
        *self
            .inner
            .validation_rx
            .lock()
            .expect("validation_rx mutex poisoned") = Some(validation_rx);

        tokio::spawn(probe.run_relay());
        self._cast()
    }

    pub fn spawn_event_bus(self) -> TestRuntime<F, S, Spawned, L, O> {
        tokio::spawn(run_event_bus(Arc::clone(&self.inner.event_bus)));
        self._cast()
    }

    pub fn spawn_llm_manager(self) -> TestRuntime<F, S, E, Spawned, O> {
        let cancel_rx = self.inner.cancel_tx.subscribe();
        tokio::spawn(llm_manager(
            self.inner.event_bus.subscribe(EventPriority::Realtime),
            self.inner.event_bus.subscribe(EventPriority::Background),
            Arc::clone(&self.inner.state),
            self.inner.cmd_tx.clone(),
            Arc::clone(&self.inner.event_bus),
            cancel_rx,
        ));
        self._cast()
    }

    pub fn spawn_observability(self) -> TestRuntime<F, S, E, L, Spawned> {
        tokio::spawn(observability::run_observability(
            Arc::clone(&self.inner.event_bus),
            Arc::clone(&self.inner.state),
        ));
        self._cast()
    }
}

// ---------------------------------------------------------------------------
// Back-compat convenience
// ---------------------------------------------------------------------------

pub(super) fn setup_test_app_from_db(fixture_db: &Arc<ploke_db::Database>) -> Arc<Mutex<App>> {
    let pwd = std::env::current_dir().expect("current dir");
    TestRuntime::new(fixture_db)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus()
        .spawn_llm_manager()
        .spawn_observability()
        .into_app_arc(pwd)
}

// ---------------------------------------------------------------------------
// Example tests demonstrating the harness
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};

    #[tokio::test]
    async fn test_runtime_can_build_app_without_spawning_actors() {
        let fixture_db =
            Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
        let pwd = std::env::current_dir().expect("current dir");
        let app = TestRuntime::new(&fixture_db).into_app(pwd);
        // Just verify we got a valid app handle
        assert_eq!(app.input_buffer, "");
    }

    #[tokio::test]
    async fn test_runtime_spawn_file_manager_and_state_manager_only() {
        let fixture_db =
            Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
        let rt = TestRuntime::new(&fixture_db)
            .spawn_file_manager()
            .spawn_state_manager();

        // Grab event subscribers before consuming the runtime
        let events = rt.events_builder().build_app_event_bus();
        // Event receivers are guaranteed to exist after building
        let _ = events.app_actor_events.event_rx;
        let _ = events.event_bus_events.realtime_tx_rx;

        // We can still extract the app afterwards
        let pwd = std::env::current_dir().expect("current dir");
        let _app = rt.into_app(pwd);
    }

    #[tokio::test]
    async fn test_runtime_full_stack_matches_legacy_setup() {
        let fixture_db =
            Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
        let pwd = std::env::current_dir().expect("current dir");
        let app = TestRuntime::new(&fixture_db)
            .spawn_file_manager()
            .spawn_state_manager()
            .spawn_event_bus()
            .spawn_llm_manager()
            .spawn_observability()
            .into_app_arc(pwd);

        let mut locked = app.lock().await;
        locked.input_buffer = "/index".to_string();
        assert_eq!(locked.input_buffer, "/index");
    }

    #[tokio::test]
    async fn test_relay_intercepts_and_proxies_oneshot() {
        use crate::chat_history::MessageKind;
        use tokio::time::{Duration, timeout};
        use uuid::Uuid;

        let fixture_db =
            Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));

        // Spawn just the state_manager (which includes the relay)
        let rt = TestRuntime::new(&fixture_db).spawn_state_manager();

        // Get debug receiver before consuming rt
        let mut events = rt.events_builder().build_app_only();
        let mut debug_rx = events
            .app_actor_events
            .debug_string_rx
            .take()
            .expect("debug_string_rx should be available after spawn_state_manager");

        // Now get the app handle (consumes rt)
        let pwd = std::env::current_dir().expect("current dir");
        let app = rt.into_app(pwd);
        let cmd_tx = app.state_cmd_tx();

        // Create a command with an oneshot
        let (completion_tx, completion_rx) = oneshot::channel();
        let new_user_msg_id = Uuid::new_v4();
        let cmd = StateCommand::AddUserMessage {
            content: "Hello from test".to_string(),
            new_user_msg_id,
            completion_tx,
        };

        // Send the command
        cmd_tx.send(cmd).await.expect("send command");

        // 1. Verify the relay intercepted it via debug channel
        let debug_cmd = timeout(Duration::from_millis(500), debug_rx.recv())
            .await
            .expect("debug recv timeout")
            .expect("debug channel closed")
            .0;
        assert!(
            debug_cmd.contains("AddUserMessage"),
            "Debug should show AddUserMessage, got: {}",
            debug_cmd
        );

        // 2. Verify the oneshot response was proxied correctly
        let result = timeout(Duration::from_millis(500), completion_rx)
            .await
            .expect("oneshot recv timeout")
            .expect("oneshot channel closed");
        assert_eq!(
            result,
            (),
            "oneshot should receive () after state_manager processes command"
        );
    }

    /// Demonstrates PATTERN 2: Sending commands that contain oneshot::Receiver fields.
    ///
    /// When a command carries `oneshot::Receiver<T>`, the relay cannot proxy it (it doesn't
    /// have the corresponding Sender). Instead, the TEST creates the oneshot pair, keeps
    /// the Sender, and provides the response after the command is sent.
    #[tokio::test]
    async fn test_relay_pattern_2_receiver_commands() {
        use std::path::PathBuf;
        use tokio::time::{Duration, timeout};
        use uuid::Uuid;

        let fixture_db =
            Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));

        let rt = TestRuntime::new(&fixture_db).spawn_state_manager();

        // Get debug receiver to observe the command was sent
        let mut events = rt.events_builder().build_app_only();
        let mut debug_rx = events
            .app_actor_events
            .debug_string_rx
            .take()
            .expect("debug_string_rx should be available");

        let pwd = std::env::current_dir().expect("current dir");
        let app = rt.into_app(pwd);
        let cmd_tx = app.state_cmd_tx();

        // Create the oneshot pairs. We keep the Senders and put Receivers in the command.
        let (completion_tx, completion_rx) = oneshot::channel::<()>();
        let (scan_tx, scan_rx) = oneshot::channel::<Option<Vec<PathBuf>>>();

        let new_msg_id = Uuid::new_v4();
        let cmd = StateCommand::EmbedMessage {
            new_msg_id,
            completion_rx, // State manager will await this
            scan_rx,       // State manager will await this
        };

        // Send the command
        cmd_tx.send(cmd).await.expect("send command");

        // Verify the relay observed it (forwarded as-is, not proxied)
        let debug_cmd = timeout(Duration::from_millis(500), debug_rx.recv())
            .await
            .expect("debug recv timeout")
            .expect("debug channel closed")
            .0;
        assert!(
            debug_cmd.contains("EmbedMessage"),
            "Debug should show EmbedMessage, got: {}",
            debug_cmd
        );

        // Now provide the responses via the Senders we kept.
        // In a real test, you might wait for side effects or use a mock.
        completion_tx.send(()).expect("send completion");
        scan_tx
            .send(Some(vec![PathBuf::from("/mock/path")]))
            .expect("send scan result");

        // The state manager would now receive these values when it awaits the Receivers.
        // (In this test, we're not waiting for processing; we're just demonstrating the pattern.)
    }

    /// Tests that the WS_FIXTURE_01_MEMBER_SINGLE fixture can be loaded and used.
    /// This fixture contains only one crate from a multi-member workspace.
    #[tokio::test]
    async fn test_runtime_with_workspace_member_single_fixture() {
        use ploke_test_utils::WS_FIXTURE_01_MEMBER_SINGLE;

        let fixture_db = Arc::new(
            fresh_backup_fixture_db(&WS_FIXTURE_01_MEMBER_SINGLE).expect("load fixture db"),
        );

        let rt = TestRuntime::new(&fixture_db).spawn_state_manager();

        // Verify we can get event subscribers (receivers are guaranteed to exist after building)
        let events = rt.events_builder().build_app_only();
        let _ = events.app_actor_events.event_rx;

        // Verify we can create an app
        let pwd = std::env::current_dir().expect("current dir");
        let _app = rt.into_app(pwd);
    }
}
