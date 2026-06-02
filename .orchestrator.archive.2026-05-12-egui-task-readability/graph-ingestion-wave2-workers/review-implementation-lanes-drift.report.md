Findings:
- `implementation-lanes.md` is stale for Lane 5 agent-turn status. It still
  says the passive owner, store loader, and graph attachment are missing.
- Current state is more precise:
  - typed passive records exist, but the feature boundary currently pulls
    `ploke-tui`;
  - store loading exists and recursive discovery has been narrowed to direct
    run-root files;
  - graph evidence attachment exists, but weak-join repair is active because
    node-scoped agent-turn artifacts lack runtime/operation coordinates.
- Provider/full-response evidence remains a separate unresolved family; it
  needs a distinct passive owner before `ploke-tree` ingestion.

Changed files: none.
