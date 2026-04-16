# Current Focus

**Last Updated:** 2026-04-16 (the local target registry `T` and the closure control plane are both landed in `ploke-eval`; closure now consumes the persisted registry directly, and the full local Rust slice is `239` targets across `10` dataset families)
**Active Planning Doc:** [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md)

---

## What We're Doing Now

The current pass has two connected layers. Structurally, `ploke-eval` now has both explicit persisted surfaces required by the formal sketch: `registry recompute` / `registry status` write the local typed target registry `T` under `~/.ploke-eval/registries/`, and `closure recompute` / `closure status` write reduced campaign state under `~/.ploke-eval/campaigns/<campaign>/closure-state.json` while consuming that registry directly. Operationally, the local Rust benchmark slice now contains `239` active targets across `10` dataset families; of those, the existing eval layer has `169` completed runs, `2` explicit failures, and `68` still missing eval artifacts, while protocol follow-through remains partial over the completed subset.

---

## Immediate Next Step

**The next bounded move is to use the registry-driven closure surface as the actual control plane for expanding eval coverage and then protocol follow-through over the full local Rust slice**:

1. treat the local Rust benchmark slice as explicitly materialized in the target registry:
   - `239` active entries across `10` dataset families
   - persisted at `~/.ploke-eval/registries/multi-swe-bench-rust.json`
2. preserve the known failed eval cases:
   - `clap-rs__clap-1624`
   - `clap-rs__clap-941`
3. use `ploke-eval closure status --campaign rust-baseline-grok4-xai` as the compact monitoring surface for:
   - registry closure: `239/239`
   - eval closure: `171/239` progressed, `169` success, `2` fail, `68` missing
   - protocol closure: `40/169` progressed, `13` full, `27` partial, `129` missing
4. use that surface to decide whether to:
   - expand eval coverage over the `68` missing targets
   - or continue protocol follow-through over the already-completed `169`
5. if protocol work stays the priority, continue over the completed set using the closure snapshot to distinguish:
   - full
   - partial
   - missing
   - failed
6. only after the closure loop is comfortable and truthful, add sparse semantic event emission rather than low-level tracing noise

Current high-priority references:

- [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md)
- [Clap Baseline Eval Orchestration](agents/2026-04-15_clap-baseline-eval-orchestration.md)
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
| "Remind me of next steps" | [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md) |
| "Let's pick up where we left off" | Open [recent-activity.md](workflow/handoffs/recent-activity.md), [2026-04-16_eval-closure-formal-sketch.md](agents/2026-04-16_eval-closure-formal-sketch.md), and [2026-04-15_protocol-aggregate-cli.md](agents/2026-04-15_protocol-aggregate-cli.md), then choose between expanding eval coverage over the missing `68` targets or continuing protocol follow-through over the completed `169` |
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
