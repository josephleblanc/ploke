# Recent Activity

- last_updated: 2026-04-12
- ready_for: bounded Phase 2 diagnostic follow-up under active control plane
- owning_branch: refactor/tool-calls
- review_cadence: update after meaningful workflow-doc changes or handoffs
- update_trigger: update after touching workflow structure, review rules, or active artifact layout

## Guardrails

- **PRODUCTION CODE CHANGES OUTSIDE PLOKE-EVAL REQUIRE EXPLICIT PERMISSION**
  - Before modifying any production code outside `crates/ploke-eval/`:
    1. STOP and ask the user
    2. Wait for explicit permission before proceeding
  - This applies to: `syn_parser`, `ploke-tui`, `ploke-db`, `ploke-llm`, etc.
  - Rationale: Prevent unintended side effects on core infrastructure during eval work

## 2026-04-12

- **`P2G` ACCEPTED; FIRST FORMAL PHASE 2 RUNS EXECUTED**
  - Added [P2G report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2G_report.md)
  - Accepted the narrow `ploke-eval` runner-surface follow-up:
    - explicit arm provenance now distinguishes shell-only control vs structured treatment in runner artifacts
    - selected endpoint provenance now persists in `execution-log.json` and `record.json.gz`
    - provider/model metadata in `record.json.gz` now serializes under `selected_model` / `selected_provider` consistently with `execution-log.json`
  - Executed the first bounded formal ripgrep packet against isolated output roots and observed:
    - `moonshotai/kimi-k2.5` / `friendli` blocked immediately because the pinned provider was unavailable for the model in the refreshed registry
    - `moonshotai/kimi-k2.5` / `baseten` reached real treatment traffic but hit upstream `429` throttling
    - `moonshotai/kimi-k2.5` / `modelrun` captured endpoint provenance with `quantization = "fp4"` but then hit an OpenRouter `402` budget/credit ceiling
    - `x-ai/grok-4-fast` / `xai` produced one aborted attempt with a transient `502`, then later completed treatment retries
  - Operational consequence:
    - the blocking uncertainty is no longer runner-surface ambiguity
    - the next bounded move is CLI-first diagnostic introspection over the completed `BurntSushi__ripgrep-1294` `grok-4-fast` / `xai` retries

- **CLI-FIRST PHASE 2 INTROSPECTION METHOD CONFIRMED**
  - Surveyed `ploke-eval inspect` from `--help` and confirmed the main high-signal surfaces for this case are:
    - `inspect tool-calls`
    - `inspect turn --show messages`
    - `conversations`
  - Used the planning/design docs to tighten the method:
    - treat the next packet as a diagnostic hypothesis, not a broad postmortem
    - identify the earliest blocking layer first
    - use one primary failure code and at most two secondary codes
    - separate outcome metrics from validity/health metrics
  - CLI-first classification on completed `grok-4-fast` / `xai` retries currently supports:
    - primary code `MODEL_STRATEGY`
    - secondary code `TOOL_SEMANTICS`
    - no current evidence that `EVAL_HARNESS` or `RUNTIME_INFRA` explains the retry discrepancy

- **EVAL ORCHESTRATION PROTOCOL ADOPTED** — active control plane created for Phase 1 P0 gaps
  - Created [Eval Orchestration Protocol](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-protocol.md) and compact [templates](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-templates.md)
  - Workers now report claims plus evidence, not self-certified "verified/done" status
  - Verifier passes are bounded; orchestrator is sole acceptance authority
  - [AGENTS.md](../../../../AGENTS.md) now mirrors the cold-start sequence and points directly at the protocol for eval execution

- **EVAL INFRA SPRINT CONTROL PLANE ACTIVE**
  - Active planning doc moved from audit synthesis to [2026-04-12_eval-infra-sprint-control-plane.md](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md)
  - Seeded initial P0 packets:
    - `P0A` SetupPhase schema extension
    - `P0B` SetupPhase capture wiring
    - `P0C` historical DB query support
    - `P0D` turn DB-state lookup
    - `P0E` replay query surface
  - **Permission gate:** `P0C` is blocked pending explicit approval because it touches `crates/ploke-db/`
  - This entry supersedes older implied "Phase 1 complete" claims as current operational truth

- **CONTROL PLANE EXPANDED TO MULTI-LANE PROGRAM**
  - Added active non-blocking sidecar lanes so broader concerns do not fall out of scope:
    - `S1-COHERENCE` for `ploke-eval` API/code-quality audit
    - `S2-LONGITUDINAL` for change-over-time metrics/reporting design
    - `S3-META-PROCESS` for workflow/skills adherence audit
  - Seeded sidecar packets:
    - `S1A` ploke-eval coherence audit
    - `S2A` longitudinal metrics design
    - `S3A` workflow and skills adherence audit
  - Primary lane remains the blocking path; sidecars are active parallel work, not deferred backlog

- **PRIMARY PATCH DISPOSITION STARTED**
  - Reviewed current in-worktree `ploke-eval` changes against `P0A/P0B`
  - Added [P0A/P0B initial verification note](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_initial-verification-note.md)
  - Current state:
    - setup schema/capture look independently checked inside `ploke-eval`
    - the same patch also includes replay/query additions that should remain unaccepted pending `P0C` permission and stronger evidence
  - Accepted sidecar reports:
    - [S2A report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2A_longitudinal-metrics-report.md)
    - [S3A report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3A_workflow-adherence-audit-report.md)

- **S1A ACCEPTED; NEW FOLLOW-UP PACKETS SEEDED**
  - Accepted [S1A coherence audit report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1A_ploke-eval-coherence-audit-report.md)
  - New primary-lane packet added:
    - `P0F` turn-record fidelity and replay-state reconstruction
  - New sidecar follow-up packets added:
    - `S2B` longitudinal metrics ledger and formula definition
    - `S3B` control-plane and handoff template tightening
  - Operational implication:
    - replay/inspection risk is not only historical-query support; current turn persistence inside `ploke-eval` is itself a blocking fidelity issue

- **S2B/S3B ACCEPTED; P0F ACCEPTED AFTER INDEPENDENT CHECK**
  - Accepted [S2B ledger report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2B_longitudinal-metrics-ledger-report.md) and created [longitudinal-metrics.md](../longitudinal-metrics.md) as the central metrics roll-up artifact
  - Accepted [S3B template report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3B_control-plane-and-handoff-template-tightening-report.md) and tightened:
    - [handoff-template.md](../../../../docs/workflow/handoff-template.md)
    - [eval orchestration templates](../../agents/2026-04-12_eval-orchestration-protocol/2026-04-12_eval-orchestration-templates.md)
  - Current primary-lane state:
    - `P0F` retry changes landed in `crates/ploke-eval/src/record.rs` and related tests
    - independent verification completed against targeted `ploke-eval` tests, so `P0F` is accepted on the strength of [P0F retry report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0F_retry-report.md)
    - remaining ambiguity is now concentrated in the mixed-scope `P0A/P0B/P0D/P0E` patch and the `P0C` permission gate

- **S2B LEDGER CREATED**
  - Added [longitudinal metrics ledger](../longitudinal-metrics.md) as the central roll-up surface for formal eval runs
  - Defined explicit formulas, denominators, source expectations, and derivable-now versus blocked metrics
  - Current blocker remains turn-level misuse and recovery capture/aggregation; the ledger now names that gap directly

- **SIDECAR FOLLOW-UP PACKETS ADDED FROM RESTART REVIEW**
  - Added `S2C` to explore lightweight discovery, durable storage, and auto-rollup for new formal runs feeding [longitudinal-metrics.md](../longitudinal-metrics.md)
  - Added `S3C` to inventory available workflow/process evidence sources and frame exploratory hypotheses for protocol adherence and drift
  - These are active sidecar packets, not deferred backlog, but they remain non-blocking relative to the primary P0 lane

- **S3C INVENTORY REPORT COMPLETED**
  - Produced [S3C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3C_report.md) with a signal inventory, noisy/unavailable split, exploratory hypotheses, and a narrow `S3D` recommendation
  - The report treats current-focus, control-plane, recent-activity, evidence-ledger, hypothesis-registry, longitudinal-metrics, EDR, and handoff artifacts as the highest-signal workflow sources
  - The next meta-process experiment should be observational and small rather than a process rewrite

- **PRE-`P0C` QUERY-SURFACE SURVEY ADDED**
  - Added `P0C0` to survey the existing `ploke-db` query-builder and raw-query surface before committing to the historical-query implementation path
  - Rationale: current evidence suggests the builder is real but partial, while many active call sites still bypass it with raw Cozo scripts; the sprint should choose whether to extend, wrap, or deliberately bypass that surface before landing `P0C`
  - `P0C` remains permission-gated for implementation because it touches `crates/ploke-db/`

- **`P0A` / `P0B` / `P0C0` ACCEPTANCE BOUNDARIES CLARIFIED**
  - Accepted `P0A` and `P0B` as setup-only slices on the strength of [P0A/P0B scope separation review](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0AB_scope-separation-review.md)
  - Explicitly kept `DbState`, `lookup`, `query`, `replay_query`, and the mixed replay tests outside that acceptance boundary
  - Accepted `P0C0` on the strength of [query-builder survey report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C0_query-builder-survey-report.md)
  - Chosen direction for `P0C`: use the existing `raw_query_at_timestamp()` / `DbState` helper path rather than extending `QueryBuilder` during the primary P0 lane

- **`P0C` ACCEPTED WITH BASELINE COMPARISON**
  - Accepted the narrow historical-query helper slice on the strength of [P0C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0C_report.md)
  - Acceptance boundary is explicit: `Database::raw_query_at_timestamp()` now requires at least one `@ 'NOW'` marker, rewrites all such markers to the supplied timestamp, and has targeted tests for historical behavior, missing-marker rejection, and multi-marker rewriting
  - Did not accept the whole dirty `crates/ploke-db/src/database.rs` diff by implication; only the helper-contract/test slice is in scope for this packet
  - Pre/post full-workspace regression runs used the same environment overrides and showed no new failures: both runs remained red only on `ploke-tui` integration tests `post_apply_rescan::approve_emits_rescan_sysinfo_under_default_profile` and `post_apply_rescan::approve_emits_rescan_sysinfo_under_verbose_profile`

- **`P0D` / `P0E` ACCEPTED; PRIMARY P0 LANE CLOSED**
  - Accepted `P0D` and `P0E` on the strength of [P0D/P0E verification report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P0DE_verification_report.md)
  - `TurnRecord::db_state()` / `DbState::lookup()` and `RunRecord::replay_query()` now meet their packet criteria on top of accepted `P0C`
  - No code changes were needed in the verification pass; acceptance is based on targeted `ploke-eval` tests over the existing implementation
  - Residual risks were explicitly bounded rather than treated as blockers:
    - `lookup()` is exact-name, fixed-relation, first-hit behavior only
    - `replay_query()` is a thin raw-query wrapper over `P0C`
    - nonexistent-turn handling currently collapses to `TimestampNotFound`
  - Operational consequence: the Phase 1 P0 replay/inspection lane is no longer the blocking item for the eval programme

- **POST-P0 SIDECAR PROMOTION QUEUED**
  - Added `S1B` to promote the accepted `ploke-eval` coherence audit into a bounded cleanup track
  - Added `S1C` to audit the inspect-oriented `ploke-eval` CLI as a frequent internal UX/bootstrap surface for quick eval checks
  - `S2C` and `S3C` remain ready as the longitudinal ingestion/bootstrap and meta-observability follow-ups
  - Intended post-compaction resume point: choose from `S1B`, `S1C`, `S2C`, and `S3C` rather than treating the next step as implicit

- **S1B CLEANUP SLICE REPORTED**
  - Removed the redundant standalone `crates/ploke-eval/tests/test_introspection.rs` smoke test because `introspection_integration.rs` already carries the canonical, stronger introspection assertions
  - Trimmed one stray diagnostic `println!` from the canonical introspection suite so the test output is quieter and easier to scan
  - Test signal remains in `crates/ploke-eval/tests/introspection_integration.rs`; the cleanup did not touch accepted P0 runtime behavior

- **FIRST POST-P0 SIDECAR WAVE ACCEPTED**
  - Accepted [S1B report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1B_report.md) as a narrow `ploke-eval` cleanup slice: the redundant standalone introspection smoke test is gone, and `introspection_integration.rs` remains the canonical stronger suite
  - Accepted [S1C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1C_inspect-cli-ux-audit-report.md): the inspect CLI is usable as a bootstrap surface, but `inspect turn --show messages` still exposes a placeholder gap and is the cleanest polish follow-up
  - Accepted [S2C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2C_report.md): the longitudinal metrics path now specifies an append-only JSONL companion plus regenerated markdown ledger as the lightest-weight ingestion/bootstrap design
  - Accepted [S2D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_report.md): a tiny real sample now proves the companion-row + regenerated-markdown shape, while canonical manifest keys and a few telemetry fields remain intentionally hypothetical
  - Accepted [S3C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3C_report.md): the highest-signal workflow sources are now explicit, and the recommended next step is a narrow observational `S3D` packet rather than a broad process rewrite
  - Operational consequence: the next decision is between bounded follow-up packets, not rediscovery of the primary lane or the first sidecar wave

- **POST-SIDECAR FOLLOW-UP PACKETS SEEDED**
  - Added [S1D](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1D_inspect-cli-polish.md) for the smallest inspect-CLI polish work exposed by `S1C`
  - Added [S2D](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_metrics-backfill-prototype.md) to validate the proposed JSONL-companion/regenerated-ledger path against a small real sample
  - Added [S3D](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3D_restart-rubric-sample.md) for a narrow restart-rubric observational pass over recent workflow artifacts

- **S1D ACCEPTED**
  - Replaced the misleading `inspect turn --show messages` placeholder with structured JSON output and kept bootstrap discoverability explicit in help text
  - Accepted [S1D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1D_report.md) on targeted test and live-command evidence

- **S2D ACCEPTED**
  - Added [S2D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S2D_report.md) plus a tiny sample companion row and regenerated markdown excerpt
  - The sample shows that the backfill/render loop is viable on the current run-directory artifact set, but canonical manifest keys and some telemetry fields stay hypothetical until the formal manifest path lands

- **S3D ACCEPTED**
  - Added [S3D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S3D_report.md) after sampling `CURRENT_FOCUS.md`, the control plane, and `recent-activity.md` against a restart rubric
  - The sample supports keeping the current recovery chain unchanged for now; no additional process change is justified yet

- **S1E SEEDED**
  - Added [S1E](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1E_setup-phase-test-cleanup.md) as the next narrow `ploke-eval` cleanup packet
  - Scope is limited to the duplicated setup/helper path inside `crates/ploke-eval/tests/setup_phase_integration.rs`

- **S1E ACCEPTED WITH NO-CHANGE OUTCOME**
  - Added [S1E report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_S1E_report.md)
  - The suspected duplication in `setup_phase_integration.rs` is a single shared test-only fixture builder, not shallow redundancy worth removing
  - Operational consequence: this is not the next high-value `ploke-eval` cleanup target

- **TARGET CAPABILITY REGISTRY PROPOSAL ADDED**
  - Added durable schema/rules doc at [docs/workflow/target-capability-registry.md](../../../../docs/workflow/target-capability-registry.md)
  - Added living registry at [target-capability-registry.md](../target-capability-registry.md)
  - Added proposal note at [2026-04-12_target-capability-registry-proposal.md](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_target-capability-registry-proposal.md)
  - Purpose: record parser blockers, modeling coverage gaps, and scaling constraints as target/task run-policy annotations so unfair targets can be skipped by default and revisited deliberately when new features bring them into scope
  - Ripgrep is recorded as the example resolved-blocker case: the mixed-edition parser issue is no longer active, but the target remains useful as a regression/sentinel recheck

- **TARGET CAPABILITY REGISTRY INTEGRATED INTO RESTART PATH**
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md) so the registry is treated as a live workflow artifact, not only a proposal
  - Updated [workflow/README.md](../README.md) so target selection, run-policy decisions, and fairness interpretation explicitly consult [target-capability-registry.md](../target-capability-registry.md)
  - Updated the active [eval infra sprint control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) resume path to read the registry before target-sensitive planning
  - Operational consequence: cold starts and run-planning passes now have an explicit place to check known parser/modeling/scaling constraints before scheduling formal work

- **PHASE 2 ENTRY PACKET SEEDED**
  - Added [P2A - Phase 2 Entry Run Planning](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2A_phase-2-entry-run-planning.md)
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md) and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) so `P2A` is the default resume point
  - Purpose: convert the accepted Phase 1 substrate plus the live target capability registry into a bounded recommendation for the first Phase 2 baseline/control planning slice
  - Expected output: candidate targets or subsets, explicit run-policy notes, remaining blockers, and one clear next packet recommendation

- **P2A ACCEPTED; P2B SEEDED**
  - Added [P2A report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2A_report.md)
  - `P2A` recommends a conservative ripgrep-first Phase 2 entry path:
    - `BurntSushi__ripgrep-1294` stays the documented mixed-edition `A2` sentinel
    - `BurntSushi__ripgrep-2209` remains the replay/introspection reference artifact
    - broader formal baseline/control scheduling stays blocked until `A2`, validity-guard policy, and manifest convergence are clearer
  - Added [P2B - Ripgrep A2 Validation](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_ripgrep-a2-validation.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) to reflect the new blocker order

- **P2B ACCEPTED; P2C SEEDED**
  - Added [P2B report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2B_report.md)
  - Ran the ripgrep sentinel re-entry check through the current code path instead of relying on the ambiguous older `BurntSushi__ripgrep-1294` run directory:
    - targeted `syn_parser` edition-2015 repro tests passed
    - a fresh temp-root `BurntSushi__ripgrep-1294` `run-msb-single` completed indexing and snapshotting
    - no fresh `parse-failure.json` was emitted for the old `globset` failure path
  - Operational consequence:
    - the old ripgrep mixed-edition parser blocker is no longer the active Phase 2 gate
    - ripgrep remains a useful regression sentinel, but it can also stay in the bounded baseline-candidate set
    - formal baseline/control work is still blocked on validity-guard policy and manifest convergence
  - Added [P2C - Validity-Guard Policy](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2C_validity-guard-policy.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [hypothesis-registry.md](../hypothesis-registry.md), [target-capability-registry.md](../target-capability-registry.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) to reflect the new blocker order

- **P2C ACCEPTED; P2D SEEDED**
  - Added [P2C report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2C_report.md)
  - Reviewed the live workflow/design/config artifacts and made the validity-guard policy explicit:
    - the draft experiment config already contains example numeric guards for provider and setup failures
    - those numbers are not yet globally binding because [readiness-status.md](../readiness-status.md) still says current numeric validity guards remain draft unless adopted in an EDR or experiment config used for a formal run
    - operational consequence: formal baseline/control work remains blocked, but now for a narrower reason than before; the ambiguity is no longer "what is the policy?" but "which concrete manifest/config surface will adopt it first?"
  - Added [P2D - Manifest And Config Convergence](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2D_manifest-config-convergence.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [hypothesis-registry.md](../hypothesis-registry.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) so manifest/config convergence is now the leading Phase 2 gate

- **P2D ACCEPTED; P2E SEEDED**
  - Added [P2D report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2D_report.md)
  - Compared the draft run-manifest/config schemas against representative current `ploke-eval` runs and the current CLI/runner surfaces
  - Accepted one bounded formal-run entry surface for the first formal Phase 2 packet:
    - `run.json` as identity/budget anchor
    - `execution-log.json` as model/provider execution source
    - `repo-state.json`, `indexing-status.json`, and `snapshot-status.json` as provenance/validity sidecars
    - `multi-swe-bench-submission.jsonl` as benchmark-facing output
    - `record.json.gz` as optional replay-grade support when present
  - Operational consequence:
    - the programme no longer needs to rediscover where formal provenance and validity evidence live before opening the first real Phase 2 packet
    - validity guards should be adopted in the first formal-run experiment config plus EDR, with explicit waivers for draft-only fields that are not yet harness-frozen
  - Added [P2E - Phase 2 Formal Entry Planning](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2E_phase-2-formal-entry-planning.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [hypothesis-registry.md](../hypothesis-registry.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) so formal-entry planning is now the leading Phase 2 gate

- **P2E ACCEPTED; P2F SEEDED**
  - Added [P2E report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2E_report.md)
  - Accepted the first narrow formal-entry plan:
    - `BurntSushi__ripgrep-1294` is the single live anchor target for the first formal packet
    - `BurntSushi__ripgrep-2209` remains reference-only for replay/introspection support
    - one concrete experiment config is the binding runtime contract
    - one paired EDR is the durable decision and waiver record
  - Accepted the minimum first-packet validity guards:
    - `max_provider_failure_rate`
    - `max_setup_failure_rate`
    - `require_full_telemetry`
    - `require_frozen_subset`
  - Accepted the first-packet waiver boundary for draft-only fields not frozen by current harness artifacts, including prompt provenance, tool-schema/policy fields, retry/timeout IDs, and observed wall time
  - Added [P2F - Ripgrep First Formal Phase 2 Packet](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2F_ripgrep-formal-packet.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [hypothesis-registry.md](../hypothesis-registry.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) so the next move is authoring the first real formal packet rather than more pre-planning

- **P2F ACCEPTED; P2G SEEDED**
  - Added [2026-04-12_exp-001-ripgrep-1294-phase2-entry.config.json](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_exp-001-ripgrep-1294-phase2-entry.config.json) as the first concrete formal Phase 2 config artifact
  - Added active [EDR-0001-ripgrep-1294-phase2-entry.md](../edr/EDR-0001-ripgrep-1294-phase2-entry.md) and updated the active EDR index
  - Added [P2F report](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2F_report.md)
  - Accepted the first real formal packet at the workflow level:
    - one config
    - one EDR
    - explicit adopted validity guards
    - explicit waiver list
  - Operational consequence:
    - the remaining blocker is now a narrow `ploke-eval` execution-surface issue
    - the current runner still hardcodes benchmark chat policy and does not yet expose a concrete per-arm shell-only versus structured control surface
  - Added [P2G - Runner Arm Surface](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_P2G_runner-arm-surface.md) as the new default resume packet
  - Updated [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md), [hypothesis-registry.md](../hypothesis-registry.md), [priority-queue.md](../priority-queue.md), and the active [control plane](../../agents/2026-04-12_eval-infra-sprint/2026-04-12_eval-infra-sprint-control-plane.md) so the next move is the `ploke-eval` runner prerequisite rather than more workflow planning

## 2026-04-11 (Late Evening)

- **PHASE 1 AUDIT COMPLETE** — Critical gaps identified
  - 4 sub-agents parallel investigation of claimed vs actual implementation
  - **Key finding:** `turn.db_state().lookup()` was claimed complete but is **NOT IMPLEMENTED**
  - **Key finding:** SetupPhase is **NEVER POPULATED** (verified `null` in record.json.gz)
  - **Key finding:** Historical DB queries **NOT POSSIBLE** (all queries hardcode `@ 'NOW'`)
  - **Phase 1 status:** INCOMPLETE - requires 3-4 days additional work
  - **Audit docs:** [PHASE_1_AUDIT_MASTER.md](../../agents/phase-1-audit/PHASE_1_AUDIT_MASTER.md), [AUDIT_SYNTHESIS.md](../../agents/phase-1-audit/AUDIT_SYNTHESIS.md)

- **DUAL SYN VERSION SUPPORT IMPLEMENTED** — A2 parser issue (code complete, validation blocked)
  - syn1 dispatch, conversion layer, DRY refactoring complete
  - 378 unit tests passing
  - **BLOCKED on Phase 1 gaps:** Cannot validate parse results without SetupPhase population
  - Need to complete P0 audit items before claiming A2 validated
  - Created syn1 versions of visitor files (code_visitor_syn1.rs, attribute_processing_syn1.rs, type_processing_syn1.rs)
  - Added edition-based dispatch: syn1 for Rust 2015, syn2 for 2018+
  - **IN PROGRESS:** Syn1→syn2 type conversion in `parser/utils.rs` to enable code reuse
    - Completed: Type, Path, GenericArgument, TypeParamBound, ReturnType, BoundLifetimes, Abi, Macro
    - Added: `Syn1ToSyn2AttributeConversion` error variant for proper error handling
    - Remaining: Fix AssocType/Constraint field mismatches, Attribute conversions
  - All 378 tests pass
  - Rust 2015 bare trait objects (`Arc<Fn(...)>`) now parse successfully
  - Rust 2015 async identifiers (`fn async(&self)`) now parse successfully
  - **Handoff:** [2026-04-11_dual-syn-implementation-handoff.md](2026-04-11_dual-syn-implementation-handoff.md)
  - **Next:** Complete syn1→syn2 conversion, then integrate into `process_fn_arg_syn1`

## 2026-04-10 (Afternoon)

- **A4/A5 VALIDATED** — RunRecord implementation verified with real data
  - Examined existing `record.json.gz` from `BurntSushi__ripgrep-2209` run
  - Schema v1 present, all required fields captured:
    - `conversation`: message history ✓
    - `db_time_travel_index`: Cozo timestamps for replay ✓
    - `phases.agent_turns`: 1 turn with 97 events ✓
  - All 16 record-related tests pass:
    - A4 schema tests: roundtrip, compression, event capture ✓
    - A5 introspection tests: 10 methods all passing ✓
  - **Next:** Can now query runs without re-running (A5 achieved)

- **A2 ISSUE IDENTIFIED** — globset crate fails to parse
  - Attempted live run on `BurntSushi__ripgrep-1294`
  - 6 of 9 ripgrep crates indexed successfully
  - `globset` crate failed: "Partial parsing success: 6 succeeded, 1 failed"
  - Root cause: syn 2.x rejects Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`)
  - **Solution selected:** Dual syn versions (syn 1.x for Rust 2015, syn 2.x for modern)
  - **Bug report:** [docs/active/bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md](../../../active/bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
  - **Status:** Awaiting implementation post-context-compaction

- **Qwen Deserialization Bug Fixed**
  - Fixed `RESPONSE_DESERIALIZATION_FAILED` when qwen returns `reasoning` without `content`
  - Feature flag `qwen_reasoning_fix` in `ploke-llm` coalesces reasoning→content when content missing
  - Tests use real captured response from `BurntSushi__ripgrep-2209` run
  - Bug documented in `docs/active/bugs/2026-04-10-qwen-reasoning-content-deserialization-failure.md`

## 2026-04-10 (Morning)

- **Phase 1C COMPLETE** — conversation capture refactored to use event channels
  - Removed `capture_conversation()` function that read from `state.chat` (required write lock, caused TTL mutations)
  - Modified `AgentTurnArtifact`: replaced `conversation` field with `llm_prompt: Vec<RequestMessage>` and `llm_response: Option<String>`
  - Updated `handle_benchmark_event` to capture `ChatEvt::PromptConstructed` and `ChatEvt::Response` events
  - This captures what the LLM actually sees/sends without side effects
  - All 33 tests pass
  - Reference: [2026-04-10_conversation-capture-design.md](./2026-04-10_conversation-capture-design.md)

- **Phase 1D COMPLETE** — structured LLM event capture
  - Added `LlmResponse(LlmResponseRecord)` variant to `ObservedTurnEvent` enum
  - Modified `handle_benchmark_event` to capture structured data from `ChatEvt::Response`
  - Captures: content, model, token usage (prompt/completion/total), finish reason, full metadata
  - No more debug strings for Response events — all data is structured
  - Added test: `handle_benchmark_event_captures_structured_llm_response`
  - All 34 tests pass (1 new test added)

- **Fixed pre-existing test failures in ploke-tui**
  - `schema_guidance_mentions_method_targets`: Updated assertion to match actual schema description
  - `de_to_value` (request_code_context): Fixed test expectation to match implementation typo ("guide" → "guides")
  - Both schema tests now pass

- **Phase 1E COMPLETE** — RunRecord emission and compression
  - Added `flate2` dependency for gzip compression
  - Implemented `write_compressed_record()` and `read_compressed_record()` helpers
  - Wired RunRecord collection in `RunMsbAgentSingleRequest::run`:
    - Initialize at run start: `RunRecord::new(&prepared)`
    - Capture turn data after `run_benchmark_turn()` completes
    - Emit `record.json.gz` at end of run
  - `RunArtifactPaths.record_path` now populated with path to compressed record
  - Added tests: `write_and_read_compressed_record_roundtrip`, `compressed_record_achieves_compression_ratio`

- **Phase 1F COMPLETE** — Introspection API
  - Implemented 9 introspection methods on `RunRecord`:
    - `timestamp_for_turn()` — Get Cozo DB timestamp for historical queries
    - `turn_record()` — Get full TurnRecord for a turn
    - `tool_calls_in_turn()` — Get tool calls from a specific turn
    - `llm_response_at_turn()` — Get LLM response from a turn
    - `replay_state_at_turn()` — Reconstruct complete state for replay
    - `total_token_usage()` — Sum tokens across all turns
    - `turn_count()` — Get total number of turns
    - `was_tool_used()` — Check if a tool was used anywhere
    - `turns_with_tool()` — Find all turns using a specific tool
    - `outcome_summary()` — Get high-level run statistics
  - Added `ReplayState` struct for state reconstruction
  - Added 10 comprehensive tests for all introspection methods

- **Phase 1 COMPLETE** — All RunRecord deliverables finished
  - 46 tests passing in ploke-eval (was 34, added 12 new)
  - No changes required outside ploke-eval crate

## 2026-04-09

- formalized the split between [docs/workflow](../../../workflow) and [docs/active/workflow](..)
- created durable workflow docs for manifests, experiment config, EDRs, checklists, and skills
- populated the living workflow artifacts for the programme charter, registry, evidence ledger, taxonomy, and active EDR area
- converted the lab book into an `mdbook` and added an explicit archive-boundary chapter
- added `owning_branch`, `review_cadence`, and `update_trigger` metadata to the active workflow artifacts
- ran five independent doc-review passes and folded the highest-signal issues into the workflow docs; see [2026-04-09-doc-review-followups.md](2026-04-09-doc-review-followups.md)
- **AGENTS.md** now references eval workflow documentation
- **A5** marked as hard gate for H0 interpretation in hypothesis registry
- **Diagnostic hypotheses** added to registry with `D-{DOMAIN}-{NNN}` format (Option C)
- **Cozo time travel** clarified for DB snapshot strategy — see [2026-04-09_run-manifest-design-note.md](../../agents/2026-04-09_run-manifest-design-note.md)
- **Run manifest vs run record** design converged — manifest is lightweight/differentiating, record is comprehensive with Cozo timestamps
- **Type inventory** created — complete catalog of serializable types for run record implementation — see [2026-04-09_run-record-type-inventory.md](../../agents/2026-04-09_run-record-type-inventory.md)
- **Handoff doc** created — [2026-04-09_run-record-design-handoff.md](./2026-04-09_run-record-design-handoff.md)
- **Phase 1 tracking** created — [phase-1-runrecord-tracking.md](../../plans/evals/phase-1-runrecord-tracking.md) — implementation plan validated, ready to begin
