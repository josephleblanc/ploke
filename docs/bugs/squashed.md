## Squashed Bugs

 ### OpenRouter 500 caused by incorrect message field name

Date found: Jul 22, 2025
Date fixed: Jul 22, 2025

**Description**:
The OpenRouter API expects each chat message to contain a field named `"role"`
whose value is one of `"system"`, `"user"`, or `"assistant"`.
Our code was serializing messages with a field named `"kind"` instead of
`"role"`.
OpenRouter therefore rejected every request with an opaque 500 Internal Server
Error.

**The Fix**:
Change the serialization attribute in `RequestMessage` from `kind` to `role`, a
update the field name accordingly.

**Files changed**:
`crates/ploke-tui/src/llm/mod.rs`

**Added Tests**:
None yet (manual verification with OpenRouter endpoint required).

### most recent user message not included in request to LLM 

Date found: Jul 20, 2025
Date fixed: Jul 20, 2025

**Description**: The most recent user message was being sent through `AddUserMessage` at the same time as `EmbedMessage`. Because `AddUserMessage` writes a message to state.chat, and the `EmbedMessage` needs to read from state.chat (which is an RwLock), a race condition occurs between the two. 

This results in a non-deterministic bug where sometimes the `EmbedMessage` would read first, and sometimes the `AddUserMessage` would write first.

**The Fix**: Added a `tokio::sync::oneshot` Sender to the `AddUserMessage` and a Receiver to `EmbedMessage`, the make `EmbedMessage` await on the Receiver. This results in the `EmbedMessage` being guaranteed not to read from the state.chat until *after* the user's message has been written to the chat.

**Sanity Check**: A `tokio::sleep` was placed in the `AddUserMessage` event handling within the `match` for the even within `state_manager`. This meant that for this sleep duration (2 seconds), the user message was not added to the chat history. After these two seconds, the message appeared, and shortly afterwards was answered correctly by the LLM ("tell me a haiku").

**Question**: Why is this a successful fix? It seems like because both events are being sent from the same thread where `App::run` is being processed to a second thread where `state_manager` is being processed, there would still be a race condition. But that isn't the case, as proved the **sanity check**. Why?

**Added Tests**:
test app_state::tests::test_race_condition_without_oneshot
test app_state::tests::test_fix_with_oneshot
test app_state::tests::test_concurrency_with_fuzzing
