# Integrating workspace-aware code graph to TUI

- see outstanding todos in comments marked `TODO:workspaces-tui`

- [in progress] Change `IndexWorkspace` to `IndexTargetDir`, update callsites 
  - also affects:
    - `SystemEvent::ReIndex` in app/events.rs around line 250
    - `handlers::indexing::index_workspace` in app_state/dispatcher.rs
    - `ParseFailure` in `ploke-tui::core`
  - not addressed now, instead temporarily allowing `IndexWorkspace` to be
  passed as `PathBuf`, but should be addressed later
    - `ploke-tui::resolve_index_target`: looks like an older attempt at a
    unified system state, but not thoroughly propogated through the code base.
  - not addressed now, but should be brought into alignment with
  [ADR-023](docs/design/ADRs/proposed/ADR-023-refactor-crate-focus.md)
    - `app_state::database::workspace_update`

- Change `SystemState` to no longer contain a `crate_focus`, and instead store
loaded crates + metadata (loaded, directory, any parsing artifacts) as
`loaded_crates` and add a `loaded_workspace` field to track similar info on
workspace such as directory, members, etc
