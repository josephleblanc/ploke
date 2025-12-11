# Adding a command to the TUI

First created:  2025-12-10
Last edited:    2025-12-10

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

6. Add handling for keybinds when context_browser is open

Input handling is done in `App::on_key_event` located in

- `ploke/crates/ploke-tui/src/app/mod.rs`

To capture input for the overlay we need to add a statement to `on_key_event`

```rust
impl App {
    // other methods
    fn on_key_event(&mut self, key: KeyEvent) {
        // Intercept approvals overlay keys
        if self.approvals.is_some() && self.handle_overlay_key(key) {
            return;
        }
        // Intercept keys for model browser overlay when visible
        if self.model_browser.is_some() {
            input::model_browser::handle_model_browser_input(self, key);
            self.needs_redraw = true;
            return;
        // Intercept keys for context browser overlay when visible
        } else if self.context_browser.is_some() {
            input::context_browser::handle_context_browser_input(self, key);
            self.needs_redraw = true;
            return;
        }
        // other checks for input capture
    }
```

Create a new file in the `crate::app::input` module

This has the key handling for the different navigation keys for the window,
such as what to do with keycode up/down, using char keys like `j` and `k` for
navigation, `Esc` to close window, etc.

- `ploke/crates/ploke-tui/src/app/input/context_browser.rs`

7. debugging

- the good error messaging helped catch an unrelated bug with the hnsw index
not being set up correctly while loading the backup database. This is due to
unrelated changes in `ploke-db` while adding a new feature in a previous
commit.

- focus window issue: expanding multiple items to fill up the overlay such that
the lowest selectable item is not displayed, and then navigating down the
selectable items results in selecting an item that is off-screen.
  - Desired behavior: selecting an item that would otherwise be off-screen
  should adjust the scroll offset such that both the selected item and its
  expanded content are within the screen frame whenever possible. If this is
  not possible due to the height of the total overlay being too small, then the
  selected item should be shown at the top of the overlay and the expanded
  content should reach the bottom of the viewable part of the overlay.

- Try to see about using `emit_error` and similar to try catching some weird
cases, especially at the borders with the RAG search.

8. Adding tests

- add a test for what happens when the overlay tries to be opened at different times:
  - before loading a code graph
  - after attempting to use the overlay without the code graph, then loading a
  code graph and trying again
  - after making a call to the LLM (with code graph)
  - after making a call to the LLM (without code graph)

- add tests with different window widths and resizing to ensure this works correctly.
  - some of these (at least 3) should have cargo insta snapshots for testing on
  reliable fixture targets, e.g. fixture_nodes.

8. Adding nice things

- Nice code display 

The text for code item nodes is not being displayed very well - it all shows up
on one line, which wraps but does not take into account newline characters or
anything. This is probably due to the underlying way the database stores it,
and I need to look into it and show the code in a better way.

The better way to handle this is by showing one line in a clearly identified
preview element, and then expand the rest of the code block at different levels
of verbosity (maybe 5 lines next) and then all the code for that item.

The "goodness" of this feature is also dependent upon having a good way to
handle scrolling for the overall list as well.


9. Notable next steps

- Add a set of "input environment" or "input context" to group similar behavior on some items.

For example, we are using lists and expanding lists in many places. These have
many common keybindings that should be active whenever they are being focused,
like using `G` to go to the last item in the list, or using `gg` to go to the
first item in the list.

- Consider an "expanding list item" type

This would be a useful abstraction to have in many cases, such as for the
context search, but also (to an extent, it doesn't exactly map on as cleanly,
since model_browser requires making calls to an external api to populate its
expanded list items), to model_browser.

- Add a token estimating/counting functionality to the context search.
  - This would display the number of tokens each code item represents.
  - Ideally we do this using the same model that the provider is using to count
  token costs, e.g. from OpenRouter there is a field associated with each model
  that shows the tokenizer used.
  - We could also optionally include a function that handles showing this as a dollar value / million tokens.
    - However, we don't really want to have the dollar counted one on by
    default. I think that kind of token estimation actually stresses people
    out. Maybe? Only if we are far and away cheaper than alternatives for the
    same set of tasks.
  - Could also include a small helper to show the token usage for each included
  code item as a percentage of that model's total context window.

- Add a type or widget for a search bar.
  - There almost certainly are examples of this on the ratatui webpage.
  - Ideally we would add a search bar that can just be plopped into whatever
  other overlay we have up and expose a local editable field for the user.
  - Would work for the context browser to have it search based on the context
  provided to it via the search bar

- Add other hooks to run the context search:
  - When hovering over a message, add a keybind (perhaps something more
  specific like a multi-key hotkey, similar to the way we handle `gg`, where
  `sc` can be "search context" for whatever message is currently selected in
  normal mode).

- Another interesting overlay:
  - Add an overlay that shows (perhaps in a tab on the left/right) the files
  that are currently within the model context, and how many of the tokens
  within those files are being included in the context window for the model.
    - Could also be an expanding list, where the initial files are displayed
    along with the number of tokens from the file being included in the context
    window vs. the total number of tokens in the file.
    - Expanding each file item would then dropdown to show all the included
    items within the file. Clicking to expand again could additionally include
    those code items in the file which are not being included in the context
    window.
      - This might get a bit visually cluttered, but I think it would probably
      be best to display this kind of thing as a tree-like structure that is
      branching off from each parent item.
    - Could turn into a cool jumping off point that would let you navigate to
    other code items, e.g. the return type and similar.

- Something we should potentially add soon: a popup-style dropdown menu that
allows the user to make some kind of action selection on the selected item.
