# Current Focus

**Last Updated:** 2026-04-17 (eval closure reached `221` success / `18` fail / `0` missing, protocol closure reached `72` full / `21` partial / `8` fail / `120` missing on fresh recompute, and the main restart-critical discovery is that the campaign-backed protocol pass is finite but far more expensive than advertised because local review procedures fan out into usefulness/redundancy/recoverability adjudications while some failed rows are blocked by a `missing field label` schema mismatch)
**Active Planning Doc:** [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md)

---

## What We're Doing Now

The current pass has shifted from “close the remaining evals” to “understand and redesign the protocol frontier.” Structurally, `ploke-eval` still has the persisted registry and closure surfaces required by the formal sketch, and the local Rust slice remains `239` active targets across `10` dataset families. Operationally, eval closure is now effectively done for this slice: `221` success, `18` explicit failure, `0` missing. Protocol is the live frontier at `72` full, `21` partial, `8` fail, `120` missing on fresh recompute. The important new finding is that `closure advance all` is one eval pass plus one very large protocol frontier walk, not a fixed-point loop, and that the protocol workload is much more expensive than the CLI suggests because per-call and per-segment reviews fan out into usefulness/redundancy/recoverability adjudications. Some failed protocol rows are also blocked by a real artifact schema mismatch (`missing field label`) rather than mere incompleteness.

---

## Immediate Next Step

**The next bounded move is to restart from a design-oriented protocol pass rather than more raw operator frontier walking**:

1. treat the fresh closure recompute as the stable baseline:
   - registry: `239/239`
   - eval: `221` success, `18` fail, `0` missing
   - protocol: `72` full, `21` partial, `8` fail, `120` missing
2. if the old campaign-backed protocol pass is still running when back at the machine, stop it and recompute closure again before doing more work
3. preserve the hard blocker classes separately from mere missing coverage:
   - `clap-rs__clap-1624`
   - `clap-rs__clap-941`
   - `nushell` parser/indexing failures
   - protocol artifact failures with `missing field label`
4. treat the long protocol runtime as a scheduler/design problem, not only an operator problem:
   - `closure advance all` is a bounded two-part pass, not a fixed-point loop
   - protocol reviews are fork/merge procedures with hidden adjudication fan-out
5. restart from the question:
   - how should usefulness / redundancy / recoverability analysis output improve tools and workflow, rather than only filling protocol coverage cells?
6. use the compact restart handoff:
   - [2026-04-17_protocol-design-reset.md](workflow/handoffs/2026-04-17_protocol-design-reset.md)

Current high-priority references:

- [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md)
- [Protocol Design Reset](workflow/handoffs/2026-04-17_protocol-design-reset.md)
- [Clap Baseline Eval Orchestration](agents/2026-04-15_clap-baseline-eval-orchestration.md)
- [Protocol Aggregate CLI](agents/2026-04-15_protocol-aggregate-cli.md)
- [Ploke-Protocol Control Note](agents/2026-04-15_ploke-protocol-control-note.md)
- [Recent Activity](workflow/handoffs/recent-activity.md)
- [Target Capability Registry](workflow/target-capability-registry.md)

---

## Quick Links

| Ask me about... | Check this... |
|-----------------|---------------|
| "What were we up to?" | This doc ↑ |
| "Remind me of next steps" | [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md) |
| "Let's pick up where we left off" | Open [2026-04-17_protocol-design-reset.md](workflow/handoffs/2026-04-17_protocol-design-reset.md), then [recent-activity.md](workflow/handoffs/recent-activity.md), then [2026-04-16_eval-closure-formal-sketch.md](agents/2026-04-16_eval-closure-formal-sketch.md); treat eval as closed, protocol as the active frontier, and the next question as design/use-of-analysis-output rather than more blind frontier walking |
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
