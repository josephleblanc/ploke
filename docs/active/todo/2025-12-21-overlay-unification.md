# Overlay Unification - 2025-12-21

## Links
- Plan: docs/active/plans/overlay-unification.md

## Done
- Created/updated overlay unification plan with shared widgets and config overlay details.
- Converted approvals/context search input to emit overlay actions.
- Added overlay manager skeleton and moved config overlay to it.
- Routed model/embedding/context/approvals overlays through the overlay manager.
- Added shared overlay widgets (search bar, empty state, diff preview) and migrated context/approvals.
- Added overlay manager smoke test, overlay intent tests, and config overlay footer height test.

## In Progress
- None.

## Next
- Expand overlay manager tests for render routing and close behavior per overlay.
- Extract any remaining shared widgets (list header/footer, key-hint footer) and migrate overlays that still duplicate UI.
- Document overlay manager invariants and overlay command boundaries in the plan (if missing).

## Notes
- Keep config overlay limited to enum/bool selection for now; avoid inline text edits.
- Ensure config updates are thread-safe and persistable on demand.
