// setup: add one harness per expected initial db state
// - db not loaded
// - db loaded:
//  - no workspace
//      - single crate (use TEST_APP_NODES_CANNONICAL_FRESH)
//  - workspace
//      - single crate (needs fixture setup in fixture db registry)
//      - multiple crates (needs fixture setup in fixture db registry)

use std::{collections::HashSet, str::FromStr, sync::Arc};

use lazy_static::lazy_static;
use ploke_llm::router_only::{RouterVariants, google::Google};
use ploke_test_utils::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant, timeout};
use uuid::Uuid;

use crate::app::App;
use crate::app::commands::exec::execute;
use crate::app::commands::harness::{TestRuntime, setup_test_app_from_db};
use crate::app::commands::parser::Command;
use crate::app_state::StateCommand;
use crate::chat_history::MessageKind;
use crate::llm::manager::ChatEvt;
use crate::llm::manager::LlmEvent;
use crate::llm::manager::events::{ContextPlan, ContextPlanMessage};
use crate::user_config::CommandStyle;

lazy_static! {
    /// A test-only accessible App instance for tests, wrapped in Arc<Mutex<...>>.
    /// This app has a DB loaded with fixture_nodes_canonical data.
    static ref TEST_APP_NODES_CANNONICAL_FRESH: Arc<Mutex<App>> = {
        // This stays registry-backed, but it is intentionally not sourced from the shared immutable
        // fixture cache because TEST_APP_NODES_CANNONICAL_FRESH wires the
        // database into mutable runtime services.
        let fixture_db = Arc::new(
                fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
                    .expect("load fixture_nodes_canonical fresh test db")
            );

        // helper builder
        setup_test_app_from_db(&fixture_db)
    };
    static ref OPENROUTER_API_KEY_TEST_LOCK: Mutex<()> = Mutex::new(());
}

struct OpenRouterApiKeyGuard {
    previous: Option<String>,
}

impl OpenRouterApiKeyGuard {
    fn set_to(value: &str) -> Self {
        let previous = std::env::var("OPENROUTER_API_KEY").ok();
        unsafe {
            std::env::set_var("OPENROUTER_API_KEY", value);
        }
        Self { previous }
    }

    fn clear() -> Self {
        let previous = std::env::var("OPENROUTER_API_KEY").ok();
        unsafe {
            std::env::remove_var("OPENROUTER_API_KEY");
        }
        Self { previous }
    }
}

impl Drop for OpenRouterApiKeyGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            unsafe {
                std::env::set_var("OPENROUTER_API_KEY", previous);
            }
        } else {
            unsafe {
                std::env::remove_var("OPENROUTER_API_KEY");
            }
        }
    }
}

mod decision_tree;

async fn wait_for_google_router(state: &Arc<crate::app_state::AppState>) {
    timeout(Duration::from_secs(2), async {
        loop {
            if matches!(
                state.config.read().await.active_router,
                RouterVariants::Google(_)
            ) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for active_router=google");
}

async fn wait_for_active_model(
    state: &Arc<crate::app_state::AppState>,
    expected: &crate::llm::ModelId,
) {
    timeout(Duration::from_secs(2), async {
        loop {
            if state.config.read().await.active_model == *expected {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("timed out waiting for active model selection");
}

#[cfg(feature = "live_api_tests")]
async fn assistant_ids(state: &Arc<crate::app_state::AppState>) -> HashSet<Uuid> {
    let chat = state.chat.0.read().await;
    chat.messages
        .values()
        .filter(|message| message.kind == MessageKind::Assistant)
        .map(|message| message.id)
        .collect()
}

#[cfg(feature = "live_api_tests")]
async fn assistant_snapshot(state: &Arc<crate::app_state::AppState>) -> Vec<String> {
    let chat = state.chat.0.read().await;
    let mut snapshot = chat
        .messages
        .values()
        .filter(|message| message.kind == MessageKind::Assistant)
        .map(|message| {
            let mut chars = message.content.chars();
            let preview: String = chars.by_ref().take(240).collect();
            let suffix = if chars.next().is_some() { "..." } else { "" };
            format!(
                "id={} status={:?} content={:?}{}",
                message.id, message.status, preview, suffix
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort();
    snapshot
}

#[cfg(feature = "live_api_tests")]
async fn wait_for_final_assistant_content(
    state: &Arc<crate::app_state::AppState>,
    existing_assistant_ids: &HashSet<Uuid>,
    placeholder_assistant_id: Uuid,
    expected: &str,
) -> Result<String, Vec<String>> {
    // This is a harness-specific final-response wait, not a generic chat-history helper.
    // `ChatTurnFinished.assistant_message_id` is the placeholder created before the first
    // provider request. A tool-call step can update that placeholder to "Calling tools..."
    // and mark it Completed; the final provider answer may then be inserted as a new
    // assistant message. Do not collapse this back to "only check the placeholder id"
    // unless the chat-loop message contract changes.
    let expected = expected.to_ascii_lowercase();
    let deadline = Instant::now() + Duration::from_secs(30);

    loop {
        let found = {
            let chat = state.chat.0.read().await;
            chat.messages
                .values()
                .filter(|message| {
                    message.kind == MessageKind::Assistant
                        && (message.id == placeholder_assistant_id
                            || !existing_assistant_ids.contains(&message.id))
                })
                .find_map(|message| {
                    message
                        .content
                        .to_ascii_lowercase()
                        .contains(&expected)
                        .then(|| message.content.clone())
                })
        };

        if let Some(content) = found {
            return Ok(content);
        }

        if Instant::now() >= deadline {
            return Err(assistant_snapshot(state).await);
        }

        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(feature = "live_api_tests")]
fn live_google_chat_model() -> String {
    let raw = std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL")
        .unwrap_or_else(|_| "google/gemini-2.5-flash".to_string());
    if raw.contains('/') {
        raw
    } else {
        format!("google/{raw}")
    }
}

// ============================================================================
// TEST CASE 1: /index with no db loaded at workspace root
// ============================================================================
// Decision tree path: "pwd is workspace root" → "no db loaded" → "/index"
//
// Current behavior:
//   Parser returns: Command::Index { mode: Auto, target: None }
//   Executor: forwards StateCommand::Index
//   Result: state layer handles indexing effects
//
// NOTE: This test now documents the UPDATED behavior after parser/executor changes.

#[tokio::test]
async fn test_index_no_db_workspace_root_current_behavior() {
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    // Set up input and parse command
    app.input_buffer = "/index".to_string();
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index", style);

    // UPDATED: Command is now parsed as Command::Index
    match &command {
        Command::Index { mode, target } => {
            assert!(matches!(mode, crate::app_state::commands::IndexMode::Auto));
            assert!(target.is_none(), "Expected target=None for bare /index");
            println!(
                "Command parsed as Command::Index {{ mode: {:?}, target: {:?} }}",
                mode, target
            );
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    // Execute command - now runs update/indexing logic
    execute(&mut app, command);

    // The executor emits AddMessageImmediate with scanning message
    // Full validation is done in the decision_tree test suite
}

#[tokio::test]
async fn test_index_workspace_dot_normalizes_to_current_workspace() {
    let app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index workspace .", style);

    match &command {
        Command::Index { mode, target } => {
            assert!(matches!(
                mode,
                crate::app_state::commands::IndexMode::Workspace
            ));
            assert!(
                target.is_none(),
                "Expected target=None for `/index workspace .`"
            );
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }
}

#[tokio::test]
async fn test_model_router_parser_show_and_set() {
    let app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    let show = crate::app::commands::parser::parse(&app, "/model router", CommandStyle::Slash);
    assert!(matches!(show, Command::ModelRouter(None)));

    let set_google =
        crate::app::commands::parser::parse(&app, "/model router google", CommandStyle::Slash);
    match set_google {
        Command::ModelRouter(Some(router)) => assert_eq!(router, "google"),
        other => panic!("unexpected command variant: {other:?}"),
    }
}

#[tokio::test]
#[cfg(feature = "live_api_tests")]
#[ignore = "live Google route/tool-call test; requires GOOGLE_PROJECT_ID/GOOGLE_REGION and ADC"]
async fn live_google_harness_router_command_runs_list_dir_through_llm_manager() {
    // This test is the live command-harness surface:
    // `/model router google` -> `/model use google/...` -> `AddUserMessage` ->
    // `llm_manager` -> live Google -> EventBus tool dispatch -> `list_dir` ->
    // final assistant response. Do not replace it with the direct
    // `ChatSession<Google>` canary or a non-live command test.
    //
    // It intentionally has no route/auth skip helper. With `live_api_tests`
    // enabled, missing Google route config or ADC auth is a live-test setup
    // failure, not a reason to silently pass this surface.
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));
    let rt = TestRuntime::new(&fixture_db)
        .spawn_state_manager()
        // The direct `ChatSession<Google>` canary manually processes tool calls.
        // This harness test must use the real EventBus runner so `llm_manager`
        // dispatches `ToolCallRequested` into the normal tool processor.
        .spawn_event_bus()
        .spawn_llm_manager();
    let state = rt.state_arc();
    let event_bus = rt.event_bus_arc();
    let cmd_tx = rt.command_sender();
    let mut realtime_rx = event_bus.subscribe(crate::EventPriority::Realtime);
    let mut background_rx = event_bus.subscribe(crate::EventPriority::Background);
    let workspace_root = std::env::current_dir().expect("current dir");
    rt.setup_loaded_workspace(
        workspace_root.clone(),
        vec![workspace_root.clone()],
        Some(workspace_root.clone()),
    )
    .await;
    let mut app = rt.into_app(workspace_root);

    app.run_command_text("/model router google").await;
    wait_for_google_router(&state).await;

    let model = live_google_chat_model();
    let expected_model =
        crate::llm::ModelId::from_str(&model).expect("live Google model id parses");
    app.run_command_text(&format!("/model use {model}")).await;
    wait_for_active_model(&state, &expected_model).await;

    let existing_assistant_ids = assistant_ids(&state).await;
    let user_msg_id = Uuid::new_v4();
    let (completion_tx, completion_rx) = tokio::sync::oneshot::channel();
    let prompt = "Call the list_dir tool exactly once with dir \".\" and max_entries 3. After the tool result, reply with the word listed.";
    cmd_tx
        .send(StateCommand::AddUserMessage {
            content: prompt.to_string(),
            new_user_msg_id: user_msg_id,
            completion_tx,
        })
        .await
        .expect("state command channel accepts user message");
    completion_rx.await.expect("user message is inserted");

    timeout(Duration::from_secs(5), async {
        loop {
            match background_rx.recv().await.expect("event bus open") {
                crate::AppEvent::Llm(LlmEvent::ChatCompletion(ChatEvt::Request {
                    parent_id,
                    ..
                })) if parent_id == user_msg_id => return,
                _ => {}
            }
        }
    })
    .await
    .expect("timed out waiting for ChatEvt::Request from AddUserMessage");

    // The command harness does not run the prompt-construction task itself.
    // `AddUserMessage` above proves the state manager emitted the live request;
    // this event bridges that request into `llm_manager` without bypassing route
    // selection, provider execution, EventBus tool dispatch, or chat-state updates.
    let estimated_tokens = prompt.len() / 4;
    let context_plan = ContextPlan {
        plan_id: Uuid::new_v4(),
        parent_id: user_msg_id,
        estimated_total_tokens: estimated_tokens,
        included_messages: vec![ContextPlanMessage {
            message_id: Some(user_msg_id),
            kind: MessageKind::User,
            estimated_tokens,
        }],
        excluded_messages: Vec::new(),
        included_rag_parts: Vec::new(),
        rag_stats: None,
    };
    event_bus.send(crate::AppEvent::Llm(LlmEvent::ChatCompletion(
        ChatEvt::PromptConstructed {
            parent_id: user_msg_id,
            formatted_prompt: vec![crate::llm::RequestMessage::new_user(prompt.to_string())],
            context_plan,
        },
    )));

    let mut requested_tools = 0usize;
    let mut completed_tools = 0usize;
    let mut requested_tool_names = Vec::new();

    let (outcome, attempts, assistant_message_id, summary) =
        timeout(Duration::from_secs(120), async {
            loop {
                match realtime_rx.recv().await.expect("event bus open") {
                    crate::AppEvent::System(crate::SystemEvent::ToolCallRequested {
                        tool_call,
                        ..
                    }) => {
                        requested_tools += 1;
                        requested_tool_names.push(tool_call.function.name);
                    }
                    crate::AppEvent::System(crate::SystemEvent::ToolCallCompleted { .. }) => {
                        completed_tools += 1;
                    }
                    crate::AppEvent::System(crate::SystemEvent::ChatTurnFinished {
                        outcome,
                        attempts,
                        assistant_message_id,
                        summary,
                        ..
                    }) => return (outcome, attempts, assistant_message_id, summary),
                    _ => {}
                }
            }
        })
        .await
        .expect("timed out waiting for live Google chat turn to finish");

    assert_eq!(outcome, "completed", "chat turn summary: {summary}");
    assert_eq!(
        requested_tool_names,
        vec![crate::tools::ToolName::ListDir],
        "expected exactly one list_dir request"
    );
    assert_eq!(requested_tools, 1, "chat turn summary: {summary}");
    assert_eq!(completed_tools, 1, "chat turn summary: {summary}");
    assert_eq!(
        attempts, 2,
        "tool session should include one Google tool-call step and one final-response step"
    );

    // Final assistant text is part of this test's acceptance surface. Counting a
    // completed chat turn and a completed tool call is not enough.
    let assistant_content = wait_for_final_assistant_content(
        &state,
        &existing_assistant_ids,
        assistant_message_id,
        "listed",
    )
    .await
    .unwrap_or_else(|assistant_messages| {
        panic!(
            "timed out waiting for final assistant response containing `listed`; \
             placeholder_assistant_message_id={assistant_message_id}; \
             requested_tools={requested_tools}; completed_tools={completed_tools}; \
             requested_tool_names={requested_tool_names:?}; chat turn summary: {summary}; \
             assistant_messages={assistant_messages:#?}"
        )
    });
    assert!(
        assistant_content.to_ascii_lowercase().contains("listed"),
        "unexpected assistant content: {assistant_content:?}"
    );
}

#[tokio::test]
async fn test_index_pause_stays_on_legacy_feedback_path() {
    let app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/index pause", style);

    match &command {
        Command::Raw(raw) => {
            assert_eq!(raw, "index pause");
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }
}

// ============================================================================
// TEST CASE 2: /save db with no db loaded (should error)
// ============================================================================
// Decision tree path: "no db loaded" → "/save db" → error
//
// Expected: Error event emitted: "No crate/workspace in db to save"
// Current: Silently succeeds or creates empty backup

#[tokio::test]
async fn test_save_db_no_db_loaded_should_error() {
    // TODO: Use TEST_APP_NO_DB harness
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    // TODO: Subscribe to events
    // let mut event_rx = app.subscribe(EventPriority::Realtime);

    // Parse and execute
    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/save db", style);

    match &command {
        Command::Save { kind } => {
            assert!(matches!(kind, crate::app::commands::parser::SaveKind::Db))
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    execute(&mut app, command);

    // TODO: Assert error event received
    // Expected: AppEvent::Error(ErrorEvent {
    //     message: "No crate/workspace in db to save",
    //     severity: ErrorSeverity::Error,
    // })
}

// ============================================================================
// TEST CASE 3: /load crate <nonexistent> should suggest /index
// ============================================================================
// Decision tree path: "no db loaded" → "/load crate X" → not in registry → suggest index

#[tokio::test]
async fn test_load_crate_nonexistent_should_suggest_index() {
    // TODO: Use TEST_APP_NO_DB harness
    let mut app = TEST_APP_NODES_CANNONICAL_FRESH.lock().await;

    let style = CommandStyle::Slash;
    let command = crate::app::commands::parser::parse(&app, "/load crate nonexistent_xyz", style);

    match &command {
        Command::Load { kind, name, force } => {
            assert!(matches!(
                kind,
                crate::app::commands::parser::LoadKind::Crate
            ));
            assert_eq!(name.as_deref(), Some("nonexistent_xyz"));
            assert!(!force);
        }
        _ => panic!("Unexpected command variant: {:?}", command),
    }

    execute(&mut app, command);

    // TODO: Assert error event with recovery suggestion
    // Expected: AppEvent::Error(ErrorEvent {
    //     message: "Crate 'nonexistent_xyz' not found in registry",
    //     severity: ErrorSeverity::Error,
    // })
    // Plus a message suggesting: "Use `/index crate <path>` to index it first"
}

#[tokio::test]
async fn test_check_api_reports_openrouter_key_prefix() {
    let _guard = OPENROUTER_API_KEY_TEST_LOCK.lock().await;
    let _env_guard = OpenRouterApiKeyGuard::set_to("sk_test_abcdef123456");
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));

    let rt = TestRuntime::new(&fixture_db).spawn_state_manager();
    let mut events = rt.events_builder().build_app_only();
    let mut debug_rx = events
        .app_actor_events
        .debug_string_rx
        .take()
        .expect("debug_string_rx should be available after spawn_state_manager");
    let pwd = std::env::current_dir().expect("current dir");
    let mut app = rt.into_app(pwd);

    let command = crate::app::commands::parser::parse(&app, "/check api", CommandStyle::Slash);
    assert!(matches!(command, Command::CheckApi));

    execute(&mut app, command);

    let debug_cmd = timeout(Duration::from_millis(500), debug_rx.recv())
        .await
        .expect("debug recv timeout")
        .expect("debug channel closed")
        .as_str()
        .to_string();
    assert!(
        debug_cmd.contains("AddMessageImmediate"),
        "Expected AddMessageImmediate, got: {debug_cmd}"
    );
    assert!(
        debug_cmd.contains("OpenRouter API key found: sk_tes..."),
        "Expected masked OpenRouter key prefix, got: {debug_cmd}"
    );
}

#[tokio::test]
async fn test_check_api_reports_missing_openrouter_key() {
    let _guard = OPENROUTER_API_KEY_TEST_LOCK.lock().await;
    let _env_guard = OpenRouterApiKeyGuard::clear();
    let fixture_db =
        Arc::new(fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL).expect("load fixture db"));

    let rt = TestRuntime::new(&fixture_db).spawn_state_manager();
    let mut events = rt.events_builder().build_app_only();
    let mut debug_rx = events
        .app_actor_events
        .debug_string_rx
        .take()
        .expect("debug_string_rx should be available after spawn_state_manager");
    let pwd = std::env::current_dir().expect("current dir");
    let mut app = rt.into_app(pwd);

    let command = crate::app::commands::parser::parse(&app, "/check api", CommandStyle::Slash);
    assert!(matches!(command, Command::CheckApi));

    execute(&mut app, command);

    let debug_cmd = timeout(Duration::from_millis(500), debug_rx.recv())
        .await
        .expect("debug recv timeout")
        .expect("debug channel closed")
        .as_str()
        .to_string();
    assert!(
        debug_cmd.contains("OpenRouter API key not found in OPENROUTER_API_KEY."),
        "Expected missing-key message, got: {debug_cmd}"
    );
}

// ============================================================================
// MISSING INFRASTRUCTURE - Implementation Checklist
// ============================================================================

// [ ] 1. EXTEND Command ENUM (app/commands/parser.rs)
//     Add variants:
//     - Command::Index { mode: IndexMode, target: Option<String> }
//       where IndexMode = Auto | Workspace | Crate
//     - Command::Load { kind: LoadKind, name: String, force: bool }
//       where LoadKind = Crate | Workspace
//     - Command::Save { kind: SaveKind }
//       where SaveKind = Db | History | Config
//     - Command::Update { scope: UpdateScope }
//       where UpdateScope = Auto | Focused | All

// [ ] 2. ADD PARSER RULES (app/commands/parser.rs)
//     Match patterns:
//     - "index" → Command::Index { scope: Auto, target: None }
//     - "index workspace" → Command::Index { scope: Workspace, target: None }
//     - "index crate <name>" → Command::Index { scope: Crate, target: Some(name) }
//     - "load crate <name>" → Command::Load { kind: Crate, name, force: false }
//     - "load crate <name> --force" → Command::Load { kind: Crate, name, force: true }
//     - etc.

// [ ] 3. CREATE CommandValidator (NEW: app/commands/validator.rs)
//     - Takes (&Command, &AppStateSnapshot) -> ValidationResult
//     - Implements decision tree logic from spec
//     - Returns ValidationResult::Success | ValidationResult::Error { reason, recovery }
//     - Does NOT execute expensive operations (just validates)

// [ ] 4. EXTEND App FOR TEST ACCESS (app/mod.rs)
//     Add method:
//     - pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent>
//       Returns clone of event_rx or new subscription from event_bus
//     - Or make event_bus accessible via AppState

// [ ] 5. CREATE TEST HARNESS VARIANTS (unit_tests/harness.rs)
//     Add lazy_static refs:
//     - TEST_APP_NO_DB: App with no loaded crates/workspace
//       (empty system_state, no db backup loaded)
//     - TEST_APP_STANDALONE: Single crate loaded, NOT workspace member
//     - TEST_APP_WORKSPACE_SINGLE: Workspace with 1 member
//     - TEST_APP_WORKSPACE_MULTI: Workspace with 2+ members

// [ ] 6. PWD CONTEXT DETECTION (utility function)
//     - fn detect_pwd_context() -> PwdContext
//     - PwdContext::WorkspaceRoot { path, members }
//     - PwdContext::CrateRoot { path, parent_workspace: Option<PathBuf> }
//     - PwdContext::Other { path }
//     - Uses syn_parser::discovery to detect Cargo.toml structure

// [ ] 7. DECISION TREE IMPLEMENTATION (validator.rs)
//     For each decision tree branch:
//     - Check current state (loaded_crates, loaded_workspace)
//     - Check pwd context if needed
//     - Check registry if needed
//     - Check unsaved changes if needed
//     - Return appropriate ValidationResult

// ============================================================================
// DECISION TREE COVERAGE MAP
// ============================================================================
// Section 1: pwd workspace root, no db (12 cases)
//   - /index → IndexWorkspace
//   - /index workspace → IndexWorkspace
//   - /index crate <member> → IndexCrate
//   - /index crate → ListMembers
//   - /load crate <exists> → LoadCrate
//   - /load crate <not exists> → Error + suggest index
//   - /load workspace → Error + suggest index
//   - /save db → Error (no db)
//   - /update → Error (no db)
//   - /index start/pause/resume/cancel → Control indexing
//
// Section 2-8: [additional sections similar...]
// Total: ~65 test cases

// ============================================================================
// TEST PATTERN TEMPLATE
// ============================================================================
// async fn test_<scenario>() {
//     // 1. Setup app in required state
//     let mut app = TEST_APP_<STATE>.lock().await;
//
//     // 2. Subscribe to events
//     let mut event_rx = app.subscribe(EventPriority::Realtime);
//
//     // 3. Parse command
//     let command = parser::parse(&app, "/command args", CommandStyle::Slash);
//
//     // 4. Validate (if testing validation)
//     let validation = validator::validate(&command, &app.state_snapshot());
//
//     // 5. Execute (if testing execution)
//     execute(&mut app, command);
//
//     // 6. Assert on events
//     let event = timeout(Duration::from_millis(100), event_rx.recv()).await;
//     assert_matches!(event, Ok(Ok(AppEvent::...)));
// }
