# 2026-05-18 ploke-egui allocation cache layout regression

## Trigger

The user reported: "you just made the UI much worse" and specifically that the
graph view became tiny after an allocation-reduction change.

## User-visible failure

The left-panel diagnostic/render cache used pre-laid-out no-wrap galleys for
long diagnostic rows. That increased layout pressure from the side panel and
shrunk the central graph viewport, making the primary graph view much worse.

## Touched code surface

- `crates/ploke-egui/src/ui/app/mod.rs`
- `crates/ploke-egui/src/ui/view/mod.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`

## What the agent did

The agent optimized per-frame allocation churn by caching diagnostics and run
navigation labels as render artifacts. The implementation used `layout_no_wrap`
galleys in the left panel and treated allocation measurements as the main
acceptance surface.

## Skipped docs / skills / instructions

- The `ploke-egui-benchmarking` skill was followed for allocation measurement,
  but the visual layout effect of cached render artifacts was not checked before
  continuing.
- The repo frontend/layout guidance requires preserving coherent UI layout and
  avoiding text overflow or overlap. The no-wrap galley cache violated that
  intent by letting diagnostic text drive panel width.

## Why this was risky

`ploke-egui` is primarily an operator graph UI. A change that reduces string or
layout churn is not acceptable if it damages the central graph viewport. Native
allocation reports can show performance movement while missing a visual layout
regression unless the rendered UI is also inspected.

## Prevention rule

Do not cache text galleys in persistent side panels unless the cached widget
preserves the same wrapping and width behavior as the original label. For
`ploke-egui` layout-affecting allocation work, preserve the central graph
viewport as an explicit acceptance condition before treating benchmark numbers
as useful.

## Memory hypothesis

The memory guidance correctly emphasized native-window allocation proof, but it
did not prevent optimizing a render boundary in a way that changed layout
behavior. Future memory/skill guidance should pair allocation acceptance with a
visual layout sanity check for side panels and the central graph viewport.
