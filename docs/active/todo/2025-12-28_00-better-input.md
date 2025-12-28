# Better input checklist No. 4
- part of the overall list of improvements in `ploke/docs/active/todo/2025-12-27_00-general.md`

[ ] Change the input area to be flexible. tracked in  It should:
  - [x] start with a single line and increase input size as the user keeps typing
  up to the limit of half the screen
  - [x] change the input area so it doesn't have a border, but instead has a
  different background color from the rest of the application (light gray)
  - [x] add ghost text for autocomplete on commands
    - [ ] stretch goal (include the pwd for files after an `@` symbol)
  - [x] add a line above the input that appears whenever there are pending edit
  proposals to approve. this will be an area that has a message like "Press
  shift+y to approve all, shift+n to reject all". Allows more ergonomical
  application of code edits.
    - [x] implement a rough edit ordering priority, where only the newer item is
    applied when there is overlap.
  - [ ] Add semantic highlighting to fenced code blocks
    - [ ] add styling to markdown quote blocks
    - [ ] maybe also add header styling
  - [ ] make the input box scrollable (e.g. detect and capture mouse hover + scroll)
