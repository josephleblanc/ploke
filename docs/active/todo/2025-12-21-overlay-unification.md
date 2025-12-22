# Overlay Unification - 2025-12-21

## Links
- Plan: docs/active/plans/overlay-unification.md

## Done
- Created/updated overlay unification plan with shared widgets and config overlay details.
- Converted approvals/context search input to emit overlay actions.
- Added overlay manager skeleton and moved config overlay to it.

## In Progress
- None.

## Next
- Route model/embedding/context/approvals through an overlay manager.
- Extract shared widgets (search bar, empty state, diff preview) and migrate one overlay.
- Inventory shared widgets (search bar, diff preview, empty state, list header/footer) for extraction.
- Add tests for overlay intents and manager smoke.

## Notes
- Keep config overlay limited to enum/bool selection for now; avoid inline text edits.
- Ensure config updates are thread-safe and persistable on demand.
