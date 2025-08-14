I have a few clarifying questions before I modify crates/ploke-tui/src/app/mod.rs and append next steps to docs/plans/bm25/overview.md.

1) StateCommand variants
   - Please confirm the exact names and field signatures of the new StateCommand variants you added in app_state/mod.rs (you dropped this file from the chat). I will use these exact names when updating the UI to dispatch commands to the state manager.
     - Do the variants exist as:
       - `StateCommand::Bm25Rebuild` (no fields)?
       - `StateCommand::Bm25Search { query: String, top_k: usize }`?
       - `StateCommand::HybridSearch { query: String, top_k: usize }`?
     - If the names/fields differ, please paste the enum lines for those variants here (or re-add app_state/mod.rs to the chat).

RESPONSE:
```rust
#[derive(Debug)]
pub enum StateCommand {
    //...

    /// Triggers a rebuild of the BM25 sparse index.
    Bm25Rebuild,

    /// Runs a BM25-only search with the given query and returns top_k results.
    Bm25Search {
        query: String,
        top_k: usize,
    },

    /// Runs a hybrid search (dense + BM25) for the given query.
    HybridSearch {
        query: String,
        top_k: usize,
    },
}
```

2) Dispatch semantics
   - Should the TUI (app/mod.rs) continue to also send an `AddMessageImmediate` feedback message to the UI when a BM25 or hybrid search is requested, or should it only send the `StateCommand` and let the state manager/rag service emit progress/results messages?
   - Current plan: I'll keep a lightweight immediate feedback `AddMessageImmediate` (SysInfo) then send the actual `StateCommand`. Confirm if you prefer that.

RESPONSE: Confirmed

3) Error / result handling expectations
   - When results arrive from state_manager (via EventBus as SysInfo or Rag events), is there a preferred format the UI expects (currently the TUI simply adds SysInfo messages with short text)? Any preference (structured vs plain text) will affect the message we send here.

RESPONSE:
The current system of receiving messages from the `state_manager` is suboptimal. It grew out of a temporary bug fix to a lock caused by the `add_message_immediate` function in `state_manager` due to stacking calls to an `RwLock`. We have been using this system as a workaround until we can get to setting up the event system through the `EventBus` to instead process messages in the `EventBus` and not in the main app drawing loop, which is possibly the very worst place we could be handling all the events.

The messages do not have a preferred format, and simple text works here. If there is additional data we can log it with `tracing`, as you can see in the patterns in that file.

4) Overview.md append
   - I will append a short "Actions performed" note documenting the TUI update and a small list of next steps (implement rag_search fusion, add tests, refine UI results pane). Confirm you'd like that appended to the end of docs/plans/bm25/overview.md.

RESPONSE: Confirm, with caveats
Apply the necessary changes to `app/mod.rs`. The messages appear in a chat interface, in which the user has an input buffer in the bottom of the screen and the messages are presented in the upper part of the screen. There are some issues but it is overall functional for now. Our focus should be on integrating the `rag_search` fusion. The immediate next steps should be to update the events in the UI as needed, then after your next response we will focus more primarily on the `ploke-rag` crate, and handling the logic for making the calls to the dense embeddings hnsw there and implementing the reranker on the hybrid results, likely duplicating some of the logic in `state_command`, which is the current manager of the process that is running the embeddings indexing for the dense embeddings - though this should more properly live in `ploke-rag`. Again a consequence of rapid iterative improvement that will need to be refined.
Our goal is to get the RAG system running as soon as possible while retaining functionality and meeting the bar of a functioning interface. Once the RAG system is up the other refactors will become much easier, since we will not need to manage context window on a file-level, but will be one step closer to implementing the combined RAG retrieval through semantic search and iterative sexpansion of context that will allow for more granular control over context management. This will unlock the possibility to develop an agentic workflow solution that will more rapidly solve the other issues. Defer where possible unless the task is important to ensuring the new changes are hooked up to the implementation of our goals and meeting the minimum bar set by the current implementation.
Keep in mind that we are on a sprint to enable agentic capability. Do not break things, and implement everything as thoroughly and correctly as possible so we can move toward having a production ready system with fewer roadblocks for the future. But also remember where our priorities are - we want to implement a working, sophisticated RAG system that will allow for the agentic system that will bring with it the opportunity for more granular context management and autonomous code edits and refactoring.

Once you confirm (or provide the exact StateCommand enum definition), I will:
- Update crates/ploke-tui/src/app/mod.rs:
  - Replace the "bm25 rebuild" handler to send `StateCommand::Bm25Rebuild` and a small SysInfo confirmation.
  - Replace the "bm25 search"/"hybrid" handler to parse query and top_k, and send `StateCommand::Bm25Search { .. }` or `StateCommand::HybridSearch { .. }` respectively, also sending a SysInfo confirmation.
- Append a "Next steps / Actions performed" section to docs/plans/bm25/overview.md describing the implemented change and remaining tasks.

If you'd rather I proceed with the assumptions listed in (1) above, say "Proceed with assumptions" and I'll produce SEARCH/REPLACE patches for:
- crates/ploke-tui/src/app/mod.rs
- docs/plans/bm25/overview.md
