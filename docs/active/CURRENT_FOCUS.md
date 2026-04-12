# Current Focus

**Last Updated:** 2026-04-12 (S1D, S2D, and S3D follow-up packets accepted)
**Active Planning Doc:** [Eval Infra Sprint Control Plane](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)

---

## What We're Doing Now

We are now running an **eval-infra sprint under an explicit multi-lane orchestration protocol** with the blocking Phase 1 P0 replay/inspection lane closed. The active work has shifted to post-P0 sidecars: `ploke-eval` coherence cleanup, inspect CLI UX, longitudinal metrics ingestion/bootstrap, and meta-process observability. This keeps the measurement layer intact while turning the accepted P0 substrate into a usable day-to-day eval workflow.

---

## Immediate Next Step

**The Phase 1 P0 replay/inspection lane is accepted; the next step is a deliberate post-P0 lane choice**:

1. **`P0A` and `P0B` are accepted with a strict boundary** - setup schema/capture work in `ploke-eval` is accepted, while `DbState`/lookup/query/replay surfaces remain outside that acceptance slice
2. **`P0C0` is accepted** - the pre-implementation survey recommends using the existing `raw_query_at_timestamp()` / `DbState` path rather than pulling `QueryBuilder` into the sprint
3. **`P0C` is accepted with a strict boundary** - the explicit historical-query helper contract now lives in `ploke-db`, and before/after workspace baselines showed no new failures beyond the same two pre-existing `ploke-tui` integration failures
4. **`P0D` and `P0E` are accepted** - `turn.db_state().lookup()` and `replay_query(turn, query)` meet their packet criteria on top of accepted `P0C`
5. **`P0F` is accepted** - turn-record fidelity and replay-state reconstruction inside `ploke-eval` no longer block the primary lane
6. **Accepted sidecar outputs now shape the next move**:
   - `S1B` accepted a narrow `ploke-eval` cleanup slice by removing a redundant introspection smoke test and keeping `introspection_integration.rs` as the canonical stronger suite
   - `S1C` accepted the inspect CLI audit and `S1D` now removes the empty `inspect turn --show messages` placeholder in favor of structured JSON output plus a direct bootstrap example
   - `S2C` accepted the longitudinal metrics ingestion/bootstrap design, centered on an append-only JSONL companion plus regenerated markdown ledger
   - `S2D` accepted a tiny real-sample prototype that validates the JSONL-companion/regenerated-markdown shape while keeping canonical manifest keys and some telemetry fields explicitly hypothetical
   - `S3C` accepted the meta-observability inventory, and `S3D` now validates that the current recovery chain is working well enough that no new process change is supported yet
7. **Current program decision** - the cleanest next move is either to advance to the next eval-design phase with the accepted sidecar findings in hand, or to open one more narrow `ploke-eval` cleanup/polish packet if you want to keep hardening the local eval UX before moving up-phase

**Control plane:** [2026-04-12_eval-infra-sprint-control-plane.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)
**Evidence base:** [AUDIT_SYNTHESIS.md](agents/phase-1-audit/AUDIT_SYNTHESIS.md)
**Current verification note:** [2026-04-12_P0AB_initial-verification-note.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_initial-verification-note.md)
**Current setup-boundary review:** [2026-04-12_P0AB_scope-separation-review.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_scope-separation-review.md)
**Current query-surface survey:** [2026-04-12_P0C0_query-builder-survey-report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C0_query-builder-survey-report.md)
**Current accepted historical-query packet:** [2026-04-12_P0C_report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C_report.md)
**Current accepted lookup/replay packet:** [2026-04-12_P0DE_verification_report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0DE_verification_report.md)
**Current accepted fidelity packet:** [2026-04-12_P0F_retry-report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0F_retry-report.md)
**Accepted sidecar findings:** [S1A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S1A_ploke-eval-coherence-audit-report.md), [S1B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S1B_report.md), [S1C report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S1C_inspect-cli-ux-audit-report.md), [S1D report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S1D_report.md), [S2A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md), [S2B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2B_longitudinal-metrics-ledger-report.md), [S2C report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2C_report.md), [S2D report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_report.md), [S3A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3A_workflow-adherence-audit-report.md), [S3B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3B_control-plane-and-handoff-template-tightening-report.md), [S3C report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3C_report.md), [S3D report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3D_report.md)

**Recently completed:**
- **Phase 1 Audit** - 4 sub-agents parallel investigation
- Gap priority matrix and revised exit criteria defined
- Eval orchestration protocol, packet templates, and multi-lane control plane

---

## Quick Links

| Ask me about... | Check this... |
|-----------------|---------------|
| "What were we up to?" | This doc ↑ |
| "Remind me of next steps" | [Eval Infra Sprint Control Plane](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) |
| "Let's pick up where we left off" | Open the highest-priority non-accepted packet in the control plane |
| Overall eval workflow | [workflow/README.md](workflow/README.md) |
| Recent activity log | [workflow/handoffs/recent-activity.md](workflow/handoffs/recent-activity.md) |
| Hypothesis status | [workflow/hypothesis-registry.md](workflow/hypothesis-registry.md) |

---

## Update Instructions (For Agents)

**When this doc changes:**
1. Update the "Last Updated" date
2. Update "Active Planning Doc" link
3. Update the "What We're Doing Now" paragraph (keep it to 3-5 sentences)
4. Update "Immediate Next Step" with the current actionable task

**When the active planning doc changes:**
- This doc should be updated immediately to point to the new planning doc
- The old planning doc should be marked complete/superseded with a link to the new one

**When the user asks recovery questions:**
- "What were we up to?" → Read this doc, summarize Current Focus paragraph
- "Remind me of next steps" → Read linked planning doc, summarize next uncompleted task
- "Let's pick up where we left off" → Read linked planning doc, identify where work paused, suggest resume point
