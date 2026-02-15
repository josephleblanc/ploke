Plan: In‑TUI linting of LLM-proposed code blocks with in-place (virtual) context

Goal
Show diagnostics for an LLM’s suggested code block as if it were already spliced into the target file at the specified byte range, without touching the file on disk. Keep diagnostics live while the user edits the suggestion in the chat, with hover to show detailed LSP info.

Ownership and crate boundaries
- ploke-tui (owns)
  - Orchestrates “virtual splice” per chat message.
  - Manages the lifecycle of overlays to the language server, editing, debounce, hover, display, and caching of diagnostics per message.
  - UI mapping from file diagnostics to the snippet view.
- ploke-io (support)
  - Provides read-only file snapshot(s) for the target path (non-blocking, optional hash verification).
  - Validates UTF‑8 boundaries for splice edges.
- ploke-core (types)
  - Shared types for SuggestedEdit metadata, basic Diagnostic wrapper, Position/Range types (byte and line/col forms), and helpers for conversions.
- ploke-lsp-client (new, helper crate)
  - Thin LSP client wrapper around rust-analyzer for ephemeral overlays: open/change/close virtual content for a real path, receive diagnostics, serve hover requests.
  - Used by ploke-tui; does not expose user UI.

Data model (new types in ploke-core)
- SuggestedEdit
  - path: PathBuf
  - start_byte: usize
  - end_byte: usize
  - code: String
  - namespace: Uuid
  - file_tracking_hash: TrackingHash (optional but recommended)
  - correlation_id: Uuid (used later for write origin/watcher echo suppression)
- PlokeDiagnostic
  - severity, code, message, source
  - file_range: { line, utf16_char } positions, file: PathBuf
  - optional byte_range
  - optional extra LSP payload (store original lsp_types::Diagnostic if the crate depends on it)
- Position/Range helpers
  - Byte <-> line/utf16 position conversions
  - Mapping utilities between file ranges and snippet-local ranges

Core flow in ploke-tui (per chat message)
1) Parse the LLM response code block into SuggestedEdit (path, start_byte, end_byte, code).
2) Obtain base file snapshot
   - Call ploke-io: get_file_contents(path, expected_hash: Option<TrackingHash>)
   - If ContentMismatch, surface a ploke-owned diagnostic/banner (“Underlying file changed since indexing — suggested offsets may be stale”).
   - Validate splice boundaries: 0 <= start <= end <= len and is_char_boundary for start/end. If invalid, show immediate diagnostic and don’t proceed to LSP overlay.
3) Build virtual file content
   - B = base file content
   - B′ = B[0..start] + code + B[end..]
   - Compute snippet_line_span in B′ (for filtering and mapping).
4) Open ephemeral LSP overlay
   - ploke-lsp-client:
     - Ensure a rust-analyzer server exists (spawn once per workspace).
     - didOpen file with B′ content (version 1). RA treats it as unsaved buffer content for the real path.
   - Subscribe to diagnostics stream; filter and store latest for this message.
5) Display diagnostics in TUI
   - Filter to diagnostics intersecting the snippet_line_span, but allow toggling to view all file diagnostics (some errors appear slightly outside the snippet but are relevant).
   - Underline/mark in the snippet; side panel lists diagnostics; on hover in the snippet, send hover request to LSP and render the returned markup.
6) Live editing of the LLM snippet in the chat
   - On each edit (debounced ~150–300ms):
     - Rebuild B′ by re-splicing updated snippet.
     - didChange file with new B′ (version++).
     - Update mapping and re-render diagnostics when they arrive.
7) Focus management
   - Only one active overlay per path at a time to avoid conflicting versions in rust-analyzer.
   - When user navigates away, didClose the file or restore disk content via didChange to the real on-disk snapshot.
   - For other messages (not focused), optionally precompute and cache diagnostics by temporarily opening/closing overlays serially (low-rate background preflight), then show cached results in the message UI.

Where and how to implement
A) ploke-io (small addition)
- Add a read snapshot API that returns full file content and optionally verifies the provided TrackingHash.
  - IoManagerHandle::get_file_contents(paths_with_optional_hash) -> Result<Vec<Result<FileSnapshot { content: String, tracking_hash: TrackingHash }, PlokeError>>, RecvError>
- Add a helper for boundary validation (or keep it local to TUI if preferred):
  - validate_utf8_boundaries(content: &str, start_byte, end_byte) -> Result<(), IoError>

B) ploke-lsp-client (new)
- Responsibilities:
  - Start rust-analyzer as a child process (JSON-RPC 2.0 over stdio).
  - Implement initialize/initialized and workspace root handshake.
  - Document lifecycle:
    - open_overlay(path: &Path, text: String) -> version_id
    - change_overlay(path, text, version_id+1)
    - close_overlay(path)
  - Diagnostics stream:
    - subscribe_diagnostics() -> mpsc::Receiver<(PathBuf, Vec<lsp_types::Diagnostic>)>
    - Cache last diagnostics per path.
  - Hover:
    - hover(path, Position) -> Option<lsp_types::Hover>
- Notes:
  - Disable cargo flycheck by default to keep response fast; rely on rust-analyzer internal diagnostics.
  - Debounce change events on the caller side (ploke-tui).
  - One server per workspace; serialize overlays for the same path.

C) ploke-tui (main feature work)
- SnippetLintController (new component)
  - Input: SuggestedEdit + optional expected_hash.
  - State: base content B, current snippet code, virtual B′, snippet_line_span in B′, RA version, latest diagnostics.
  - Methods:
    - ensure_overlay(): opens LSP overlay with B′
    - update_overlay(new_snippet_code)
    - close_overlay()
    - current_diagnostics(filter_mode) -> Vec<PlokeDiagnostic>
    - hover(snippet_cursor_pos) -> Hover text (maps to file Position then calls LSP)
  - Mapping utils:
    - Compute file Position for a given snippet-local cursor:
      - Determine file byte position: start_byte + snippet_local_byte_offset (careful with UTF‑8).
      - Convert to LSP Position (line, utf16 char).
    - Map diagnostics’ file ranges to snippet-local ranges for UI highlighting.
- UI integration:
  - When a chat message with code block is selected or opened for edit: SnippetLintController.activate(message_id).
  - Render diagnostics inline and in a side list.
  - On hover, request hover info and show popup.
  - On blur, deactivate and close overlay (or restore baseline).
- Background precompute (optional, phase 2):
  - Serially preflight messages to generate cached diagnostics snapshots.

D) ploke-core (shared types)
- Add SuggestedEdit and PlokeDiagnostic (and Position/Range structs) with conversion helpers.
- Provide byte->(line, utf16 char) utilities or expose traits so TUI can use them safely.

Validation and mapping details
- UTF‑16 character indexing
  - LSP uses UTF‑16 code units for “character” positions. Provide a helper that counts code units in a line up to a byte offset. Test with multi-byte and astral characters.
- Snippet range mapping
  - Keep snippet_line_span = [line_of_start_in_B′, line_of_start + snippet_line_count).
  - Diagnostic filtering: keep any diagnostic whose range intersects snippet_line_span; mark those fully outside as “context diagnostics” (toggable).
- Boundary checks
  - Validate start/end in bounds and at char boundaries before splicing.
  - If invalid, surface immediate diagnostics (no overlay), e.g., PLOKE003 SnippetBoundaryInvalid.

Non-goals here
- Writing changes to disk (apply/commit) — that will be handled later by write APIs in ploke-io.
- Running cargo check/clippy in the loop — keep latency low; rust-analyzer internal diagnostics are sufficient for interactive linting.
- Holding multiple simultaneous overlays for the same path — we serialize per path based on active message focus.

Testing plan
- Unit tests
  - Byte→line/utf16 conversions with diverse Unicode.
  - Splice function correctness and char-boundary validation.
  - Mapping diagnostics to snippet-local ranges.
- Integration tests (behind a test feature that requires rust-analyzer in PATH)
  - Start RA server, open overlay, receive diagnostics for known-invalid snippet, verify debounce.
  - Hover returns expected content for symbol under cursor.
  - Focus switch: open overlay for file A, then close and open for file B, ensure no cross‑talk.
- TUI tests
  - Simulated editing loop with debounced updates; assert that diagnostics update.

Performance and UX
- Debounce LSP changes to ~150–300ms to avoid flooding.
- Prewarm rust-analyzer on TUI launch to reduce first-lint latency.
- Graceful fallback: if RA not found, show ploke-only diagnostics (boundary/hash) and an informative banner.
- Size limits: cap snippet size for live lint; show banner if too large, with “lint on demand” action.

Timeline and complexity
- MVP (about 1–2 weeks)
  - ploke-io: get_file_contents + boundary helper
  - ploke-lsp-client: minimal client with open/change/close/diagnostics/hover
  - ploke-tui: SnippetLintController, splice/mapping, inline diagnostics, hover popup, debounce
  - ploke-core: SuggestedEdit + minimal diagnostics/range types
- Phase 2 (additional 1–2 weeks)
  - Background preflight/caching for non-focused messages
  - Better diagnostics presentation (grouping, quick jump)
  - Configurable debounce and filters
  - Add ploke-specific diagnostics (ContentMismatch banners, etc.) wherever relevant

Why this approach
- Keeps ploke-io focused on safe I/O and verification; no lint logic there.
- Uses rust-analyzer via an internal client to get “LSP-grade” diagnostics and hover for realistic context, without writing files.
- Works naturally with the TUI chat workflow: overlay only the currently selected message, live-update while editing, and cleanly revert on blur.
- Limits complexity: we avoid supporting concurrent overlays for the same path and avoid heavy cargo/clippy in the interactive loop.

Brief change summary
- ploke-core: add SuggestedEdit, Position/Range, PlokeDiagnostic.
- ploke-io: add get_file_contents(path, expected_hash?) -> FileSnapshot; optional boundary helper.
- New ploke-lsp-client: RA LSP wrapper (open/change/close/diagnostics/hover).
- ploke-tui: new SnippetLintController, splice/mapping, debounce, UI rendering of diagnostics and hover.

This is a tractable short-to-medium feature: MVP is achievable soon if we accept “only the focused message is live-linted.” The hardest part is the small LSP client wrapper and precise UTF‑16/byte mapping; both are bounded and testable.
