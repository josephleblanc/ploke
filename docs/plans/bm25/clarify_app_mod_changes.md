I have a few clarifying questions before I modify crates/ploke-tui/src/app/mod.rs and append next steps to docs/plans/bm25/overview.md.

1) StateCommand variants
   - Please confirm the exact names and field signatures of the new StateCommand variants you added in app_state/mod.rs (you dropped this file from the chat). I will use these exact names when updating the UI to dispatch commands to the state manager.
     - Do the variants exist as:
       - `StateCommand::Bm25Rebuild` (no fields)?
       - `StateCommand::Bm25Search { query: String, top_k: usize }`?
       - `StateCommand::HybridSearch { query: String, top_k: usize }`?
     - If the names/fields differ, please paste the enum lines for those variants here (or re-add app_state/mod.rs to the chat).

2) Dispatch semantics
   - Should the TUI (app/mod.rs) continue to also send an `AddMessageImmediate` feedback message to the UI when a BM25 or hybrid search is requested, or should it only send the `StateCommand` and let the state manager/rag service emit progress/results messages?
   - Current plan: I'll keep a lightweight immediate feedback `AddMessageImmediate` (SysInfo) then send the actual `StateCommand`. Confirm if you prefer that.

3) Error / result handling expectations
   - When results arrive from state_manager (via EventBus as SysInfo or Rag events), is there a preferred format the UI expects (currently the TUI simply adds SysInfo messages with short text)? Any preference (structured vs plain text) will affect the message we send here.

4) Overview.md append
   - I will append a short "Actions performed" note documenting the TUI update and a small list of next steps (implement rag_search fusion, add tests, refine UI results pane). Confirm you'd like that appended to the end of docs/plans/bm25/overview.md.

Once you confirm (or provide the exact StateCommand enum definition), I will:
- Update crates/ploke-tui/src/app/mod.rs:
  - Replace the "bm25 rebuild" handler to send `StateCommand::Bm25Rebuild` and a small SysInfo confirmation.
  - Replace the "bm25 search"/"hybrid" handler to parse query and top_k, and send `StateCommand::Bm25Search { .. }` or `StateCommand::HybridSearch { .. }` respectively, also sending a SysInfo confirmation.
- Append a "Next steps / Actions performed" section to docs/plans/bm25/overview.md describing the implemented change and remaining tasks.

If you'd rather I proceed with the assumptions listed in (1) above, say "Proceed with assumptions" and I'll produce SEARCH/REPLACE patches for:
- crates/ploke-tui/src/app/mod.rs
- docs/plans/bm25/overview.md
