# Current Focus

**Last Updated:** 2026-04-14 (`ploke-protocol` bootstrap landed; evalnomicon conceptual framework and protocol architecture are the active restart-critical thread)
**Active Planning Doc:** [Eval Infra Sprint Control Plane](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)

---

## What We're Doing Now

We are still under the same eval-infra sprint control plane, but the active restart-critical thread has shifted into **conceptual framework and protocol architecture work for eval introspection**. The broad batch-execution path is no longer the immediate blocker: `ripgrep` has a usable artifact set, `tokio-rs-all` completed `25/25`, and the current working surface is designing sharper NOM/introspection methods while bootstrapping a new `ploke-protocol` crate that can turn bounded review procedures into typed, executable protocol steps. The immediate concern is preserving and refining the conceptual thread behind that work so restart does not collapse it into "we added a crate and a command."

---

## Immediate Next Step

**The next bounded move is to continue the evalnomicon + `ploke-protocol` line cleanly, then fold that back into the broader post-batch evaluation programme**:

0. **Restart-critical sources for this subtrack**:
   - [Protocol Operationalization Memory](/home/brasides/code/ploke/docs/workflow/evalnomicon/src/meta-experiments/protocol-operationalization-memory.md)
   - [Ploke-Protocol Bootstrap Handoff](agents/2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md)
   - [Conceptual Framework](../workflow/evalnomicon/src/core/conceptual-framework.md)
1. **Treat `ploke-protocol` as the architectural home for bounded NOM procedures**:
   - current bootstrap exists in [crates/ploke-protocol](/home/brasides/code/ploke/crates/ploke-protocol)
   - `ploke-eval` now depends on it and exposes `ploke-eval protocol tool-call-review`
   - the next concrete engineering slices are protocol artifact persistence, richer input packets, and a second bounded protocol only after the first path feels stable
2. **Keep the broader eval sprint context in view**:
   - the active planning/control plane remains [2026-04-12_eval-infra-sprint-control-plane.md](agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)
   - the completed `tokio-rs` artifacts are still waiting for a cleaner post-batch evaluation pass
   - the protocol/introspection work should sharpen that later pass rather than drift into a disconnected side project

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
7. **Current program state** - `S1E` came back as a no-change result, which means the suspected setup-phase test duplication is not a high-value cleanup target. [P2A](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2A_phase-2-entry-run-planning.md), [P2B](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_report.md), [P2C](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2C_report.md), [P2D](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2D_report.md), [P2E](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2E_report.md), [P2F](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2F_report.md), and [P2G](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2G_report.md) are now accepted: ripgrep has moved from mixed-edition sentinel uncertainty to baseline candidacy, the validity-guard ambiguity is resolved into an explicit adoption policy, the first formal packet has a bounded provenance/config entry surface, the runner now persists explicit arm and endpoint provenance, and completed `grok-4-fast` / `xai` treatment retries show that the next uncertainty is model-strategy variance rather than artifact inconsistency.
8. **Current immediate working track** - the ripgrep batch execution loop has now been rolled forward into a usable 14-run artifact set, and `tokio-rs-all` has completed `25/25`; see [ripgrep batch rollup and next target](agents/2026-04-12_eval-infra-sprint/2026-04-13_ripgrep-batch-rollup-and-next-target.md) and [tokio-rs probe and batch entry](agents/2026-04-12_eval-infra-sprint/2026-04-13_tokio-rs-probe-and-batch-entry.md). The next bounded move is:
   - do a cleaner evaluation pass over the tokio runs with explicit scoring for gross tool failures, retrieval drift, and context bloat
   - keep `tokio-rs__tokio` at `watch` / `default_run`, because the batch was operationally successful but still surfaced execution-quality issues in the tool/patch loop
   - choose the next target or next intervention only after that tighter read, rather than treating the first successful full batch as a sufficient optimization signal

**Active surgical note:** [LLM full response trace stopgap](agents/2026-04-12_eval-infra-sprint/2026-04-13_llm-full-response-trace-stopgap.md)
**Current batch execution note:** [Ripgrep batch rollup and next target](agents/2026-04-12_eval-infra-sprint/2026-04-13_ripgrep-batch-rollup-and-next-target.md)
**Current second-target entry note:** [Tokio-rs probe and batch entry](agents/2026-04-12_eval-infra-sprint/2026-04-13_tokio-rs-probe-and-batch-entry.md)

**Current planning proposal:** [Target Capability Registry Proposal](agents/2026-04-12_eval-infra-sprint/2026-04-12_target-capability-registry-proposal.md)
**Live run-policy artifact:** [workflow/target-capability-registry.md](workflow/target-capability-registry.md)
**Current phase-entry reports:** [P2A report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2A_report.md), [P2B report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_report.md), [P2C report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2C_report.md), [P2D report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2D_report.md), [P2E report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2E_report.md), [P2F report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2F_report.md), [P2G report](agents/2026-04-12_eval-infra-sprint/2026-04-12_P2G_report.md)
**Current active packet:** fresh `tokio-rs` batch launch after the now-accepted second-target probe

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
| "Let's pick up where we left off" | Open [target-capability-registry.md](workflow/target-capability-registry.md), [recent-activity.md](workflow/handoffs/recent-activity.md), and [tokio-rs probe and batch entry](agents/2026-04-12_eval-infra-sprint/2026-04-13_tokio-rs-probe-and-batch-entry.md), then start the tighter post-batch evaluation pass on the completed `tokio-rs` artifacts |
| Overall eval workflow | [workflow/README.md](workflow/README.md) |
| Recent activity log | [workflow/handoffs/recent-activity.md](workflow/handoffs/recent-activity.md) |
| Hypothesis status | [workflow/hypothesis-registry.md](workflow/hypothesis-registry.md) |
| Target/run-policy constraints | [workflow/target-capability-registry.md](workflow/target-capability-registry.md) |

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
