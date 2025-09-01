# Messaging life-cycle

1. User submits message in input box, causing an `Action::Submit` to kick off the event flow in `handle_action` in the main event loop within `App.run`
- Emits 
  - AddUserMessage
  - ScanForChange 
  - EmbedMessage (with oneshot from AddUserMessage)
    - Later update
  - Adds SysInfo message
- ScanForChange and EmbedMessage run off in parallel, but ScanForChange must finish before EmbedMessage gets going (using one-shot to wait on ScanForChange completion success).

2.1 `AddUserMessage` in `state_manager` in `app_state/dispatcher.rs`
- calls `add_user_message` `app_state/handlers/chat.rs`
  - calls `add_message_immediate` in `app_state/handlers/chat.rs`
    - calls `chat_guard.add_message_user` (with write permission on the `RwLock<ChatHistory>`)
      - writes contents of user message to chat history
    - gets id of added message
    - upserts message to database with `handle_event`
      - calls `persist_conversation_turn` with `state.db.upsert_conversation_turn`
    - Sends MessageUpdatedEvent with the new message_id
    - (LLM.1) Sends `AppEvent::Llm(llm::Event::Request)`
  - sends `completion_tx` with oneshot to unblock `StateCommand::ScanForChange`

2.2 `StateCommand::ScanForChange` processed in `state_manager` in `app_state/dispatcher.rs`
- calls `scan_for_change` from `app_state/handlers/db.rs`
  - calls `database::scan_for_change` from `app_state/database.rs`
    - `scan_for_change` reads current files and checks file hashes, comparing against known database values after re-parsing crate, then if any files have changed hashes, reparses changed files and updates database.
      - on success, sends one-shot with `Option<Vec<PathBuf>>` with changed files or `None` if no changes.

2.3 `StateCommand::EmbedMessage` processed in `app_state/dispatcher.rs`
- calls `process_with_rag` from `rag/context.rs`
- Assembles chat history from last user message.
  - Currently not handling any tool messages that could (in theory) be added to the conversation tree, or any tool call events from anywhere. This might be the right place to add previous tool events as part of the history, 
    - Q: using "role: tool"? Not sure whether "role: tool" is recognized by API endpoints, even tool-capable ones.
      - Needs tests
- (LLM.2) Sends `AppEvent::Llm(llm::Event)` with `Event::PromptConstructed`
  - Could add different `llm::Event` types possibly.
  - NOTE: There is an arm to receive in `app/events.rs`, but it is not handled
  - Only receiver is `llm_manager` in `llm/mod.rs`

3. `llm_manager` in `llm/mod.rs`
- Receives (LLM.1) and (LLM.2)
- checks model registry for the `active_model_config`
  - Currently uses `continue` to skip processing if no config found.
  - TODO: Better would be triggering a query for valid models.
    - Careful to give up `state.config.guard` while checking for other models,
    then get write access if something is found.
    - Around `llm/mod.rs:407`
  - spawns new thread for `process_llm_request` with:
    - assembed context (conv. history, code snippets)
    - clones `Client`, which is from reqwest via hyper and internally uses
    `Arc`, so clone is cheap and uses same connection(?)
 
3.1 `process_llm_request` in `llm/mod.rs`
- Creates empty assistant message for later updating with `StateCommand::CreateAssistantMessage`
  - via oneshot created locally
- calls `prepare_and_run_llm_call`
  - collects custom logging
    - logs data to custom files on model behavior. A bit verbose, maybe gate behind verbose cfg.
    - checks for required tool use, early return on error
  - forms vec of tool definitions
  - constructs `RequestSession`
  - calls `session.run()`

3.2 `session.run()` loops and makes API calls
  - trims messages to a char or token limit (default 12000 or 4096)
  - calls LLM with timeout (default 10s, configured in const LLM_TIMEOUT_SECS in lib.rs)
  - uses `build_openai_request` to form request, including tool definitions
  - makes api call
  - checks error codes:
    - retries on 404 if tool_use enabled
