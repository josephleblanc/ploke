# Event flow for message submission

When the user runs the application, it opens the chat interface and the user sees the input area and an empty chat history. This event flow is what happens when they enter some text to the input buffer and hit "Enter" to submit the message.

Note: Currently submitting a message before loading a workspace as database causes an error. This is not desirable behavior. Instead we should display a warning to the user that no workspace has been selected, provide instructions on how to index a workspace, and then allow the message to be sent to the LLM without performing cosine similarity search.

## Overview

1. `Action::Submit` in `app/mod.rs`
  - sends several messages to kick off the message sending lifecycle
  - `StateCommand::AddUserMessage`: adds the user message to the conversation history
  - `StateCommand::ScanForChange`: scan the loaded workspace to see if it is in sync with the database
  - `StateCommand::EmbedMessage`: embed's the user's submitted message and performs the similarity search + bm25 (if bm25 is loaded, otherwise falls back to vector similarity search only)
  - `StateCommand::AddMessage`: Adds a placeholder message to provide feedback to the user, begins with "Embedding User Message" and is updated upon LLM response.

1.1 `StateCommand::AddUserMessage` handled in `app_state/dispatcher.rs`
  - event carries a one-shot that sends an empty value upon completion
  - the one-shot is received by `StateCommand::EmbedMessage`
  - this allows `StateCommand::ScanForChange` to run concurrently with the message embedding.
  - once the user's message has been added to the chat history, the ID of the message is used to start the process of adding a new LLM message session - see (2) below

1.2 `StateCommand::ScanForChange` handled in `app_state/dispatcher.rs`
  - ultimately processed as `scan_for_change` in `app_state/database.rs`
  - if there is not `crate_focus` set in `AppState` or if the file path to the target `crate_focus` cannot be resolved, then returns an error, but the error is not propogated up to the user, and they instead just see "Embedding User Message" with no further changes
  - TODO: propogate the error message up as a warning + help message with info on how to index a crate or load a previously indexed crate.
  - TODO: Currently no distinction in the errors between a `crate_focus` of `None` and a `crate_focus` whose file path cannot be resolved. We should make these distinct so they can be handled and propogated back to the user with different help messages.
  - If a change is detected and the new scan is successful, or if there is no change, sends a message along a one-shot to the handler of `StateCommand::EmbedMessage`, in `process_with_rag` in `rag/context.rs`

1.3 `StateCommand::EmbedMessage`
  - (add details)

1.4 `StateCommand::AddMessage`
  - simple enough, adds a placeholder `SysInfo` message
  - Q: Where is this updated again? I can't remember but it disappears (as it should) when the LLM message is being successfully processed, or maybe when the user's message is being embedded? Find out.

2. `llm_manager` orchestrates sending message to router in `llm/manager/mod.rs`
  - waits on two events to be received, matching on the `parent_id`, which is the... I think it is the Uuid v4 of the message from the user that the LLM's response will be the child of 
    - waits on `ChatEvt::Request` sent from `add_msg_immediate`, which is ultimately kicked off by `StateCommand::AddUserMessage` in (1.1) above.
    - waits on `ChatEvt::PromptConstructed` from `construct_context_from_rag` in `rag/context.rs`, ultimately kicked off by `StateCommand::EmbedMessage` in (1.3) above
  - start `process_llm_request` on a new thread once both `ChatEvt` are received for the same message (must receive `ChatEvt::Request` first, is this a bug? We only insert on `Request` and don't check for a match, only checking for a match when receiving on `ChatEvt::PromptConstructed`, kind of confusing)
  - note: This might be the best place to add new logic for what to do if there is no `crate_focus`. We can just add the logic to the match on `ChatEvt::Request` if there is a `crate_focus` of `None`, to start off `process_llm_request` without the additional code context, and add a `System` or `SysInfo` message with information on how to index or load a crate's embeddings.

## Questions

1. Will the note in (2) above work?

I think so, but there might be some second-order effects to watch out for. For example, if the LLM tries to call some tools, they will fail, which might be confusing for the user. Maybe we turn off tools if there is no `crate_focus`? This might require changes `llm/manager/session.rs`.

2. If the note in (2) will work, what else might break, if anything?
