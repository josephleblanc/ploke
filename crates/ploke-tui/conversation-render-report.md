# Conversation History Rendering Report

## Rendering pipeline
- `src/app/mod.rs` drives the frame drawing: `App::draw` obtains the cached path (`ChatHistory::iter_path`) and path length, sets up the layout (chat area, preview, input, status), and passes the iterators into `ConversationView::prepare`/`render` along with the currently selected index derived from `self.list`.
- `src/app/view/components/conversation.rs` owns the scrolling/autofollow state. `ConversationView::prepare` calls `measure_messages` to compute per-message heights for the cached path and then adjusts `offset_y`, `auto_follow`, and other helpers so mouse scroll, paging, and selection stay aligned. `render` then delegates to `measure_messages` again’s counterpart, `render_messages`.
- `App::run_with` and `App::handle*` logic (same file) keep `conversation.last_chat_area`, `conversation.offset`, and `conversation.item_heights` up to date so clicks and scroll events can hit-test the rendered rows and update the `ListState`/`AppState` selection before the next draw.

## Message data / state
- `src/chat_history.rs` defines the `ChatHistory` tree. Messages live in a hash map keyed by `Uuid`, `path_cache` stores the current root→tail path, and helpers such as `iter_path`, `path_len`, and `rebuild_path_cache` are the source for every render. When the UI asks for the path, it just iterates that cached vector and feeds the live `Message` instances straight to the renderer.
- `ChatHistory::Message` (and the `MessageKind` enum) carry the data used for display: `id`, `content`, and `kind` (User/Assistant/System/SysInfo/Tool). `content` is the raw string produced by the LLM or user and is what `render_messages` wraps and paints; `kind` informs the per-message `Style`.
- `src/app/types.rs` defines `RenderableMessage` (for snapshotting) and the `RenderMsg` trait that unifies `Message` and `RenderableMessage` so the renderer only needs `id()`, `kind()`, and `content()`. `App::draw` still feeds the live `Message` nodes directly, but there’s a smaller wrapper already in place if cloning becomes necessary for future parsing/highlighting.

## Rendering implementation
- `src/app/message_item.rs` contains `measure_messages`/`render_messages`. `measure_messages` reserves a one-column gutter (selection bar) and uses `textwrap::wrap` to count the number of wrapped lines per message so scrolling is accurate.
- `render_messages` rewraps each message the same way, skips the `offset_y` prefix, and emits `Paragraph`s built from `Line` → `Span` sequences. The base `Style` is chosen by `MessageKind` (`User` is blue, `Assistant` green, `System` cyan, `SysInfo` magenta, `Tool` low-contrast green/dim). If a message is selected, a white `Span::styled("│", ...)` is prepended to every rendered line.
- The current layout logic already enforces a fixed 1-column gutter and only renders what fits inside `conversation_area.height`, so any highlighting needs to preserve that spacing (screen coordinates, offsets, and wrapping are recomputed every render).

## Dependencies that matter for syntax highlighting
- `ratatui` (widgets, `Paragraph`, `Block`, `Span`, `Line`, `Style`, `Color`, `Layout`) is the basis for drawing text with colors. It already holds the per-message `Style`/`Color` values, so the highlighting work will likely continue to use `Span` sequences (probably with nested `StyledContent`/`Color` per chunk).
- `crossterm` (event stream, mouse handling, terminal mode helpers) is already wired into the draw loop and makes colorized output possible via the backend.
- `textwrap` (used in `app/message_item.rs`) is the current line-wrapping helper; any syntax-highlighting approach that changes the line contents must either reuse its wrap logic or wrap the final styled spans to match the existing measured heights.
- `unicode-width` is already present in the crate (used in other modules such as `context_browser`) and can help if the highlighter needs to account for wide characters once multiple spans appear per line.
- `ploke-core` (with `ArcStr` and other shared types) and `uuid` are already used to track messages; this is less about coloring, but it means the renderer can assume each message has a stable ID and metadata entry even after highlighting logic splits the content internally.
- Shared workspace crates such as `once_cell`, `fxhash`, `lazy_static`, and `ploke-error` are already part of the dependency graph, so bringing in additional helpers (e.g., a lexer crate) should be weighed against the existing dependencies first.
