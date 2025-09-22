# Implementation log 014 — AppState propagation + relative path previews (2025-08-20)

Summary
- Finished propagating AppState changes across constructors:
  - Added in-memory proposals store initialization in lib.rs and in database.rs test setup.
- Continued M1 implementation:
  - Staged edit proposals already in place; improved preview UX by normalizing file paths relative to current crate focus.
  - Kept previews as code-blocks; unified diff remains optional (planned via a small dependency in a follow-up).
- Maintained bridge semantics for tool events; completion/failed emitted only on approve/deny.

User-visible changes
- Staged edit summaries now list files relative to the focused workspace when available.
- Approve/Deny commands unchanged:
  - edit approve <request_id>
  - edit deny <request_id>

Internals
- AppState now consistently includes proposals: RwLock<HashMap<Uuid, EditProposal>> across runtime and tests.
- Previews normalize paths using SystemState.crate_focus if set; falls back to absolute paths otherwise.

Next steps
- Optional unified diff preview (via similar crate), gated by a config flag.
- Config knobs for editing.auto_confirm_edits and editing.preview_mode.
- Persist proposal lifecycle to ploke-db once code_edit_proposal/outcome APIs land.

Risks/notes
- Continue to rely on the SystemEvent compatibility bridge in M1; plan removal in M2 after typed events are fully adopted.
