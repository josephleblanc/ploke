# Adding a command to the TUI

Adding a new command to the TUI, such that the command is executed as a command
line-style command, e.g. `/model search <name>`, the following will need to be
adjusted:

1. Add an Action 

- `ploke/crates/ploke-tui/src/app/input/keymap.rs`

Add an enum variant for the new command's action

(Optionally add keybind in `to_action` function in same file)

2. For execution with `/<some-command>` 

- `ploke/crates/ploke-tui/src/app/commands/parser.rs`

Add an enum variant for the new command, and optionally a wrapped field, e.g.

```rust
pub enum Command {
    // other commands
    SearchContext(String),
}
```

Add to the match statement in `parse`, e.g.

```rust
/// Parse the input buffer into a Command, stripping the style prefix.
pub fn parse(app: &App, input: &str, style: CommandStyle) -> Command {
    match trimmed {
        // other command handling
        s if s.starts_with("search ") => {
            let search_term = s.trim_start_matches("search ").trim();
            Command::SearchContext(search_term.to_string())
        }
    }
}
```

Add a new match arm and function for the command handling in
`ploke/crates/ploke-tui/src/app/commands/exec.rs`, e.g.

```rust
/// Execute a parsed command. Falls back to legacy handler for commands
/// not yet migrated to structured parsing.
pub fn execute(app: &mut App, command: Command) {
    match command {
        Command::Help => show_command_help(app),
        // other commands

        Command::SearchContext(search_term) => new_function(search_term),
```

---

3. To open an overlay

3.1 Add `build_context_search_items` to `App` impl

- `ploke/crates/ploke-tui/src/app/commands/exec.rs`

- takes the input received from an external process, e.g. from a ploke-rag retrieval process or `llm` process and processes it into the items that will be stored in the state for the overlay

3.2 `open_context_search` added to `App` impl

- `ploke/crates/ploke-tui/src/app/commands/exec.rs`

- adds a new fields to the `App` struct
  - `context_browser: Option<ContextSearchState>,`

- uses `tokio::spawn` to create a new thread that will run the search
  - Needs to add a new event that will send the results from the search
  somewhere that will be listening to add them later to the browser

- takes the processed items for the overlay and builds the initial overlay state

- changes the `App` field to hold the new overlay state (previously `None`)

3.3 Wire `app.open_context_search` into the `execute` function

- `ploke/crates/ploke-tui/src/app/commands/exec.rs`

- uses the `Command::SearchContext` added in (2) to open the overlay

4. Create a new event type to route the output of the context search

- `ploke/crates/ploke-tui/src/lib.rs`

- Added `AppEvent::ContextSearch(SearchEvent)`

- Defined new enum for `SearchEvent` that wraps the response from the rag search
```rust
#[derive(Clone, Debug)]
pub enum SearchEvent {
    SearchResults(AssembledContext),
}
```

- updated `AppEvent::priority` with a priority of `Realtime` (reasoning that
this is an event with UI implications, and therefore needs to be prioritized)

5. update `handle_event` in `event.rs` to match on the new `AppEvent::ContextSearch`

- `ploke/crates/ploke-tui/src/app/events.rs`

- new arm now takes assembled_context payload and makes them the contents of
the `app.context_browser`

```rust
/// Handle AppEvent routing in a lightweight way. This keeps the UI loop lean.
pub(crate) async fn handle_event(app: &mut App, app_event: AppEvent) {
    // Forward to view components that subscribe to events
    app.conversation.on_event(&app_event);
    app.input_view.on_event(&app_event);
    match app_event {
        AppEvent::Quit => {
            app.quit();
        }
        // other methods..
        AppEvent::ContextSearch(SearchEvent::SearchResults(assembled_context)) => {
            if let Some(ctx_browser) = app.context_browser.as_mut() {
                let AssembledContext { parts, stats } = assembled_context;
                info!(
                    "ContextSearch event completed with search results.
                    AssembledContext with stats:
                    {stats:#?}"
                );
                ctx_browser.items = App::build_context_search_items(parts);
            }
        }
    }
}
```

- update `open_context_search` to use the new event `AppEvent::ContextSearch` to send the results of the rag process so it will be received by the UI.
  - note: uses `emit_app_event` function, which uses a global event bus. Not
  sure about whether this is good practice or not, might want to look at it
  again later.

6. debugging

- the good error messaging helped catch an unrelated bug with the hnsw index
not being set up correctly while loading the backup database. This is due to
unrelated changes in `ploke-db` while adding a new feature in a previous
commit.
