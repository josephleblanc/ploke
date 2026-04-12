# Current Focus

**Last Updated:** 2026-04-12 (Eval Infra Sprint Multi-Lane Control Plane Activated)
**Active Planning Doc:** [Eval Infra Sprint Control Plane](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)

---

## What We're Doing Now

We are now running an **eval-infra sprint under an explicit multi-lane orchestration protocol**. The blocking primary lane is still the Phase 1 P0 replay/inspection work identified by the audit, but active sidecar lanes now also cover `ploke-eval` API/code-quality audit, longitudinal metrics/change-over-time reporting, and workflow/skills adherence. This keeps the measurement layer in front of parser/tool optimization work without letting the broader programme concerns disappear from the active plan.

---

## Immediate Next Step

**The Phase 1 P0 replay/inspection lane is now accepted; choose the next active lane deliberately**:

1. **`P0A` and `P0B` are accepted with a strict boundary** - setup schema/capture work in `ploke-eval` is accepted, while `DbState`/lookup/query/replay surfaces remain outside that acceptance slice
2. **`P0C0` is accepted** - the pre-implementation survey recommends using the existing `raw_query_at_timestamp()` / `DbState` path rather than pulling `QueryBuilder` into the sprint
3. **`P0C` is accepted with a strict boundary** - the explicit historical-query helper contract now lives in `ploke-db`, and before/after workspace baselines showed no new failures beyond the same two pre-existing `ploke-tui` integration failures
4. **`P0D` and `P0E` are accepted** - `turn.db_state().lookup()` and `replay_query(turn, query)` meet their packet criteria on top of accepted `P0C`
5. **`P0F` is accepted** - turn-record fidelity and replay-state reconstruction inside `ploke-eval` no longer block the primary lane
6. **Next step is a program decision, not a hidden blocker** - promote one or more ready sidecars (`S1B` cleanup, `S1C` inspect CLI UX audit, `S2C` metrics ingestion bootstrap, `S3C` meta-observability inventory) or advance to the next eval-design phase with the new inspection/replay surface in place

**Critical findings from audit:**
- `turn.db_state().lookup()` - **NOT IMPLEMENTED** (claimed complete in Phase 1F)
- `replay_query(turn, query)` - **NOT IMPLEMENTED**  
- SetupPhase - **NEVER POPULATED** (verified `null` in `record.json.gz`)
- Historical DB queries - **NOT POSSIBLE** (all queries hardcode `@ 'NOW'`)

**Control plane:** [2026-04-12_eval-infra-sprint-control-plane.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)
**Evidence base:** [AUDIT_SYNTHESIS.md](agents/phase-1-audit/AUDIT_SYNTHESIS.md)
**Current verification note:** [2026-04-12_P0AB_initial-verification-note.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_initial-verification-note.md)
**Current setup-boundary review:** [2026-04-12_P0AB_scope-separation-review.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_scope-separation-review.md)
**Current query-surface survey:** [2026-04-12_P0C0_query-builder-survey-report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C0_query-builder-survey-report.md)
**Current accepted historical-query packet:** [2026-04-12_P0C_report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C_report.md)
**Current accepted lookup/replay packet:** [2026-04-12_P0DE_verification_report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0DE_verification_report.md)
**Current accepted fidelity packet:** [2026-04-12_P0F_retry-report.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_P0F_retry-report.md)
**Accepted sidecar findings:** [S1A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S1A_ploke-eval-coherence-audit-report.md), [S2A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md), [S2B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S2B_longitudinal-metrics-ledger-report.md), [S3A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3A_workflow-adherence-audit-report.md), [S3B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_S3B_control-plane-and-handoff-template-tightening-report.md)

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
