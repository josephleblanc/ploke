# Current Focus

**Last Updated:** 2026-04-17 (repo-cache layout was normalized for all `10` Rust dataset families, eval/protocol work resumed under the closure control plane, and the current restart-critical state is mixed: eval closure advanced, protocol closure did not, and the live `rayon` batch is provider-suspect because a run launched with `--provider xai` emitted an `openrouter.ai` request warning)
**Active Planning Doc:** [Eval Closure Formal Sketch](agents/2026-04-16_eval-closure-formal-sketch.md)

---

## What We're Doing Now

The current pass has two connected layers. Structurally, `ploke-eval` now has both explicit persisted surfaces required by the formal sketch: `registry recompute` / `registry status` write the local typed target registry `T` under `~/.ploke-eval/registries/`, and `closure recompute` / `closure status` write reduced campaign state under `~/.ploke-eval/campaigns/<campaign>/closure-state.json` while consuming that registry directly. Operationally, the local Rust benchmark slice now contains `239` active targets across `10` dataset families and the repo cache has been normalized into the `<org>/<repo>` layout expected by `ploke-eval`. Closure now shows `186/239` eval rows progressed with `169` success, `16` explicit failure, `1` partial, and `53` still missing. Protocol follow-through remains unchanged at `40/169` progressed with `13` full, `27` partial, and `129` missing. The main new failure concentration is `nushell`, while the live `rayon` batch must be treated carefully because a model-turn warning referenced `https://openrouter.ai/api/v1/chat/completions` even though the batch was launched with `--provider xai`.

---

## Immediate Next Step

**The next bounded move is to re-enter the eval/protocol pass from a stricter operator stance, starting by validating provider fidelity on the live `rayon` batch before trusting any new eval artifacts, then continuing direct operator-driven eval batches and only afterward returning to protocol follow-through**:

1. treat the local Rust benchmark slice as explicitly materialized in the target registry:
   - `239` active entries across `10` dataset families
   - persisted at `~/.ploke-eval/registries/multi-swe-bench-rust.json`
2. preserve the known failed eval cases and new explicit failure classes:
   - `clap-rs__clap-1624`
   - `clap-rs__clap-941`
   - `nushell` parser/indexing failures:
     - duplicate `crate::commands` module-tree path
     - `generic_lifetime` relation failure
     - `indexing_completed` timeouts
3. use `ploke-eval closure status --campaign rust-baseline-grok4-xai` as the compact monitoring surface for:
   - registry closure: `239/239`
   - eval closure: `186/239` progressed, `169` success, `16` fail, `1` partial, `53` missing
   - protocol closure: `40/169` progressed, `13` full, `27` partial, `129` missing
4. before trusting the currently running `rayon` family, verify whether the observed `openrouter.ai` warning was:
   - a true provider mismatch
   - or only a misleading internal logging path
5. if the `rayon` run is valid, continue direct operator-driven eval batches over:
   - `rayon`
   - `serde`
   - `bat`
   - `fd`
   - `bytes`
   - `tracing`
   while leaving `nushell` aside as a low-yield failure family for now
6. return to protocol follow-through only after the eval lane is back under direct control and write-producing commands are being run rather than read-only inspection

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
| "Let's pick up where we left off" | Open [recent-activity.md](workflow/handoffs/recent-activity.md) and [2026-04-16_eval-closure-formal-sketch.md](agents/2026-04-16_eval-closure-formal-sketch.md), verify provider fidelity on the live `rayon` batch, then continue direct operator-driven eval expansion over the non-`nushell` missing families before returning to protocol follow-through |
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
