# Migration log: prototype1-state API-surface cleanup (PR1–4)

This document tracks internal API changes in `ploke-eval` and their impact on downstream
consumers. **Downstream crates never import eval `Prototype1*` types**; they read passive
`ploke-records` DTOs via `ploke-tree::FsRunStore` → `Graph::from_records` → `ploke-egui`
inspector projections.

## Downstream consumer map

| Consumer | Coupling surface | Risk in this refactor |
|----------|------------------|----------------------|
| `ploke-tree` | `FsRunStore` file→type mapping, `Graph::from_records` field joins | Low if serde shapes unchanged |
| `ploke-egui` | `ploke_tree::Graph` + direct reads of `child_plan`, `history::payload`, `selection`, `run_record` | Low if serde shapes unchanged |
| `ploke-eval` `replay/` | `tui_adapter` entrypoints | Updated in PR1 |

### Serde shapes most at risk

- `ploke_records::history::payload::{SurfaceEvidenceRecord, SurfaceProposalProducerRecord, RequestPolicyReceiptRecord}`
- `.headless-tui.json` (`tui_adapter::evidence::Summary`)
- `.turn-live/` bundles

## PR log

### PR1 — Typed attempt request

**Internal changes**
- New `tui_adapter::Attempt` + `Capture` enum replace telescoping `run_headless*` entrypoints.
- Retry loop uses `harness_io::{Retry, Step, Terminal, Feedback}` instead of hand-rolled `advance_turn`.
- Test fixture injection moves from `#[cfg(test)]` early-return to harness selection.

**Downstream record schema: unchanged** (no persisted type changes).

**Verification** (2026-06-09)
- `cargo test -p ploke-eval tui_adapter`: 58 passed, 10 ignored (live).
- `cargo test -p ploke-eval replay`: 61 passed, 4 ignored.
- Live canary: `live_attempt_run_google_direct_with_embeddings` (ignored; requires ADC + `OPENROUTER_API_KEY`).

### PR2 — Structured boundary error

**Internal changes**
- Broad-attempt boundary returns `Result<AttemptOutcome, tui_adapter::Error>` instead of stringizing into `PrepareError`.
- `AttemptOutcome` is built from `HeadlessTerminal` taxonomy (in-memory only).

**Downstream record schema: unchanged** (`AttemptOutcome` is not persisted).

### PR3 — Harness port cleanup

**Internal changes**
- Policy (retry, validation gating, terminal classification) moves above `Harness` trait.
- `TuiHarness` / slimmed `tui_bridge` own ploke-tui mechanics only.
- `FixtureHarness` enables driver-level unit tests without `WorkspaceTuiRuntime`.

**Downstream record schema: unchanged**.

### PR4 — SurfacePolicy unification (keystone)

**Internal changes**
- `BroadEditPolicy` replaced by typed `SurfacePolicy` backed by Model A (`surface::Grant`).
- `WriteScope` and post-apply path checks derive from `SurfacePolicy` via `grant.check(Draft)`.
- `SurfacePolicy::GraphNeighborhood(Grant)` named as feature-2 seam; not implemented.

**Downstream record schema: unchanged** (verified by `surface_policy_roundtrips_legacy_serde_label` and `surface_evidence_record_policy_label_unchanged` in `surface_policy.rs`).

**If schema had changed** (not expected): would require `ploke-tree` `FsRunStore` update,
`ploke-egui` inspector read-site audit, and backup-fixture review per
[`docs/testing/BACKUP_DB_FIXTURES.md`](../../../testing/BACKUP_DB_FIXTURES.md).

### Post-review fixes (multi-model review, 2026-06-09)

**C1 — `Capture::Responses` tap-guard lifetime (correctness, behavior-affecting)**
- `Attempt::run` bound the process-global `ResponseTapGuard` inside the `Capture::Responses`
  match arm, so it dropped (and cleared the tap) before `AttemptDriver::run().await`. Captured
  full-response sidecars were silently empty.
- Fix: bind the guard in the `Attempt::run` function scope so it lives across the await.
- **Downstream impact on persisted shapes**: this *restores* population of `.turn-live/` full-response
  records when capture is requested; it does not change the serde shape. Consumers that read
  turn-live sidecars now see the records they expected (previously empty under the bug).
- Regression test: `attempt_capture_responses_keeps_tap_installed_across_run`
  (`tui_adapter/tests.rs`, recorded-tape, runs in the default suite) asserts
  `AttemptOutcome.run.full_response_records()` is non-empty.

**W1 — no-edit retry feedback (behavior, prompt content)**
- `retry_outcome_from_end` fed the generic turn `summary` into the next prompt on a retryable
  `RetryNoEdit`, instead of the curated `retry_feedback(feedback)` the old hand-rolled loop used.
- Fix: map `RetryNoEdit` to `Outcome::NoEdit(NoEdit::new(retry_feedback(feedback)))`. The
  exhaustion terminal (`CompletedWithoutEdit { outcome, summary }`) is unchanged — it still uses
  the original `end`'s `summary`/`outcome`.
- **Downstream record schema: unchanged** (affects in-loop prompt text only).
- Regression test: `retry_no_edit_uses_curated_feedback_not_summary_for_next_prompt`
  (`tui_adapter/driver.rs`).
