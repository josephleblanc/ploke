# Overlay Unification - 2025-12-21

## Links
- Plan: docs/active/plans/overlay-unification.md

## Done
- Created/updated overlay unification plan with shared widgets and config overlay details.

## In Progress
- None.

## Next
- Convert context search overlay to intent-based actions.
- Convert approvals overlay to intent-based actions.
- Draft overlay manager interface and skeleton implementation.
- Inventory shared widgets (search bar, diff preview, empty state, list header/footer) for extraction.
- Add tests for overlay intents and manager smoke.

## Notes
- Keep config overlay limited to enum/bool selection for now; avoid inline text edits.
- Ensure config updates are thread-safe and persistable on demand.
