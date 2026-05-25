---
name: doc-cleanup
description: Use near session closeout or before a cold restart to move stale docs out of docs/active/agents and into archive without interrupting active work.
---

# Doc Cleanup

Use this as a sidecar check near closeout or before a cold restart.

- Keep only restart-critical docs in `docs/active/agents/`: current control planes, live packets, active handoffs, and recovery pointers.
- Move completed one-off notes, superseded plans, and old reports to `docs/archive/agents/` or the repo's existing archive path.
- Do not move docs still referenced by `CURRENT_FOCUS.md`, an active control plane, or an open packet.
- Update local indexes or pointer docs only if they reference moved files.
- Prefer archive over deletion unless the user explicitly asks to delete a doc and it is not a live reference.
- Run this after substantive work or at session end, not as an interruption.
