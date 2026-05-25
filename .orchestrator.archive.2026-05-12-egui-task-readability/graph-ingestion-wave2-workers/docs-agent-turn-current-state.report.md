Completed:
- Updated the graph ingestion inventory and README around agent-turn ingestion.
- Added an agent-turn ingestion state note dated 2026-05-12.

Changed docs:
- `docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md`
- `docs/active/agents/ploke-tree-graph-ingestion/2026-05-12-agent-turn-ingestion-state.md`
- `docs/active/agents/ploke-tree-graph-ingestion/README.md`

Documented gaps:
- Feature boundary still needs repair so passive agent-turn readers do not pull
  in `ploke-tui`.
- Provider attempts, retries, timeouts, and full responses are a separate
  passive record family, not part of agent-turn ingestion.
- Eval CLI-local projections must not become graph authority.

Follow-up:
- `implementation-lanes.md` still needs a more precise lane status update from
  the lane-drift review.
