# todos for the model browser

Most Relevant files (not everywhere this overlay is concerned with, but these
are the big ones):
- `ploke/crates/ploke-tui/src/app/view/components/context_browser.rs`
- `ploke/crates/ploke-tui/src/app/view/components/context_browser.rs`

## TODO:

### Soon

- add a way to have a keypress that will add the currently selected message in
normal mode to be the subject of the model search.

- Change the "loading..." to replace the current code items in the displayed list instead of appearing at the top.

- Display issue of different items:
  - Currently we are showing the "title" in the middle of the line border, which would be fine but because it also appears below the "query, it appears weird in the overlay.
  - A better design would be to have boht the search bar and the results inside one box (with a border) and then have no border around the search results but retain the border around the search bar (or only use the bottom border.

- On color/emphasis and Insert/Normal modes
  - We should add color changes depending on the input mode.
    - Normal mode should keep the currently used list item highlighting.
    - Insert mode should remove the background color added for the currently
    selected item (but retain the `>` indicator), and should change the color
    of the search input bar, maybe to same color as input used for the
    conversation input box

- Error handling
  - Currently when a search fails, it is shown in the conversation history.
  - Instead, when the search error occurrs due to an interaction with the model
  search overlay, we should show the error message in the area used to show
  search results, and it should disappear if a search succeeds.
  - If no database has yet been set up, a helpful message indicating what might
  be wrong and what the user can do about it should be shown to the user
  instead of just saying the search failed.

- Adding more modes
  - We should add the notion of a search mode to the search bar, which should be toggled through a cycle with the `Tab` key.
    - Modes should match those available for the rag search kinds (see `ploke-rag`)

- We should add a key that will be used as input to make the contents of the
currently selected item the content for a new search that overrides the current
contents in the input bar.

### Later

- It would be cool to add a way to add items shown in the search results to the
current context window that is being used for the model. We should assess how
this would work exactly. Maybe just adding it to the conversation history as a
system message.

- Once we have the notion of a popup menu or similar, we should add a way for
the user to do a follow-up search that uses a cozo query for graph traversal,
to do something like find all the child code items for a given code item, or
its containing parent, or its siblings.

- Once we have added the tachyonfx crate as a dependency and experimented with
it a bit, we should add a subtle indicator when a search is loading. 
