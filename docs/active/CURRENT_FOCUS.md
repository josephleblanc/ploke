# Current Focus

**Last Updated:** 2026-04-15 (protocol-artifact coverage and the new evidence-reliability `inspect proto` slice are the active restart-critical thread; fresh `ploke-protocol` development remains intentionally shelved)
**Active Planning Doc:** [Protocol Aggregate CLI](agents/2026-04-15_protocol-aggregate-cli.md)

---

## What We're Doing Now

The current pass is still operational, but it has moved from pure hygiene into analysis tooling. We now have persisted protocol artifacts across the finished eval set and a first evidence-reliability `ploke-eval inspect proto ...` surface that aggregates them into terminal-native reports and run summaries. The active concern is making those artifacts inspectable enough to guide targeted intervention decisions without flattening away segment/call structure. `ploke-protocol` development itself remains belayed because that lane still needs a more careful human-LLM collaboration pass than this monitoring window supports.

---

## Immediate Next Step

**The next bounded move is to stabilize and use the new protocol aggregate CLI while remaining coverage work finishes**:

1. finish the remaining protocol-artifact coverage gaps and record the known unrecoverable bug cases
2. keep improving `ploke-eval inspect proto` so the evidence-reliability slice remains readable, filterable, and restart-safe on actual run data
3. use the new CLI surface for ongoing spot-checks and larger-run inspection while deciding what intervention lanes matter most
4. preserve the earlier hygiene/testing/doc audit findings, but do not let them displace the current aggregate-analysis lane

Current high-priority references:

- [Protocol Aggregate CLI](agents/2026-04-15_protocol-aggregate-cli.md)
- [Orchestration Hygiene And Artifact Monitor](agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md)
- [Ploke-Protocol Control Note](agents/2026-04-15_ploke-protocol-control-note.md)
- [Recent Activity](workflow/handoffs/recent-activity.md)
- [Target Capability Registry](workflow/target-capability-registry.md)
- [Docs Hygiene Tracker](agents/2026-04-15_docs-hygiene-tracker.md)

---

## Quick Links

| Ask me about... | Check this... |
|-----------------|---------------|
| "What were we up to?" | This doc ↑ |
| "Remind me of next steps" | [Protocol Aggregate CLI](agents/2026-04-15_protocol-aggregate-cli.md) |
| "Let's pick up where we left off" | Open [recent-activity.md](workflow/handoffs/recent-activity.md), [2026-04-15_protocol-aggregate-cli.md](agents/2026-04-15_protocol-aggregate-cli.md), and the active orchestration note, then continue the aggregate-analysis + coverage pass |
| Overall eval workflow | [workflow/README.md](workflow/README.md) |
| Recent activity log | [workflow/handoffs/recent-activity.md](workflow/handoffs/recent-activity.md) |
| Hypothesis status | [workflow/hypothesis-registry.md](workflow/hypothesis-registry.md) |
| Target/run-policy constraints | [workflow/target-capability-registry.md](workflow/target-capability-registry.md) |

---

## Update Instructions (For Agents)

**When this doc changes:**
1. Update the "Last Updated" date.
2. Update "Active Planning Doc" if the authoritative orchestration/control note changes.
3. Keep "What We're Doing Now" to the current restart-critical thread only.
4. Keep "Immediate Next Step" focused on the next bounded move, not the full historical backlog.

**When the active planning doc changes:**
- update this file immediately in the same change set
- mark the old planning surface superseded with a forward link if appropriate

**When the user asks recovery questions:**
- "What were we up to?" → Read this doc and summarize the current paragraph.
- "Remind me of next steps" → Read the linked active planning doc and report the next uncompleted step.
- "Let's pick up where we left off" → Read this doc plus `recent-activity.md` and resume from the active monitoring/hygiene lane.
