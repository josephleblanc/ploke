# Review: Run Readiness for Broad Headless TUI Adapter

## 2026-05-13 Update

Commit `bd00056b Add broad harness request fanout` addresses the one-file
materialization limit called out in finding 5. Broad batch publication now
allocates multiple request slots, each admitted broad transaction can carry
multiple changed paths, and each admitted transaction is materialized as one
child artifact entry. The current live 5x3 broad campaign is exercising the
end-to-end adapter path that this review called unproven.

The other cautions remain relevant: backend admission is the authoritative path
check, submitted evidence fields remain projection/guidance rather than source
truth, and the headless TUI loop may still fail operationally under real model
traffic.

## Findings

1. **High: The live broad path depends on an unproven real `ploke-tui` chat/tool loop.**
   `resolve_child_plan` now invokes `run_broad_headless_tui_attempt` when no submitted result exists, then tries to admit the result and publish a child plan (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6630-6669`). The adapter starts `ploke_tui::test_utils::new_test_harness::AppHarness`, adds a user message, and waits for tool/proposal events (`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:45-86`, `crates/ploke-tui/src/test_utils/new_test_harness.rs:52-84`, `crates/ploke-tui/src/test_utils/new_test_harness.rs:140-178`, `crates/ploke-tui/src/test_utils/new_test_harness.rs:190-215`). The current tests cover carrier/admission logic and adapter bounds, but not an end-to-end run where a real headless `ploke-tui` model turn emits an edit proposal, survives the event loop, writes a submitted result, and proceeds to child materialization. This is a run-readiness gap because the live loop can now block or fail inside the adapter rather than cleanly publishing a pending request for an external harness.

2. **Medium: The adapter has two different surface authorities, and only the backend one is authoritative.**
   The headless adapter pre-rejects paths with a hard-coded `crates/ploke-eval` rule and `..` detection (`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:334-365`), while admission later uses `prototype_surface_for_broad_edit_policy` plus `path_matches_surface_policy` over the actual diff (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1583-1603`). The backend check is the correct authority and does reject protected/out-of-policy changes before admission. The adapter pre-check is acceptable as retry feedback, but it is a duplicated policy projection; if broad policy changes, the retry loop can mislead or prematurely deny before the authoritative backend check.

3. **Medium: Submitted result evidence is bound but not semantically used to prove the patch.**
   `SubmittedBroadHarnessResult::verify_request` checks request id/hash, parent, workspace, result path, admission binding, and relpath shape (`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:132-191`, `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness_result.rs:202-218`). Admission then validates the real workspace diff and persists changed files (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1481-1536`). That is safe for authority, but the submitted `change_summary`, rationale, citations, and checks are not cross-checked against the admitted diff. For live selection this is probably fine only if these fields remain guidance/projection, never source truth.

4. **Medium: Child plan projection drops the admitted artifact surface.**
   Admission computes `artifact_surface` on the accepted candidate workspace (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1522-1535`), but `publish_broad_harness_child_plan_from_admitted` only carries base/derived artifact ids and a single target file into `ResolvedTreatmentBranch`/`ChildFiles` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1402-1472`). `validate_requested_broad_harness_child` checks producer id, artifact id presence, and target consistency, but not the admitted surface measurement (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1790-1818`). This avoids the earlier deterministic-surface path, but it means the broad child plan is not carrying the same surface evidence that admission just minted.

5. **Historical, fixed by `bd00056b`: the broad adapter only materialized exactly one changed file.**
   Before `bd00056b`, admission could accept a diff with multiple changed paths, but child-plan publication rejected anything except one changed file (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1402-1410`). Current broad fanout materializes admitted broad transactions as child entries and supports multi-file changed-path sets inside each admitted transaction.

## Open Questions

- Is the live broad path intended to invoke the in-process `AppHarness` directly, or should complete-mode still support a pure request/pending-result handoff when the TUI provider/tool loop is unavailable?
- Should `AdmittedBroadHarnessResult.artifact_surface` become part of the broad `ChildFiles`/History admission path, or is the derived artifact id alone the intended carrier for this wave?

## Verification

- `cargo test -p ploke-eval tui_adapter -- --nocapture 2>&1 | tail -n 60`: 4 passed.
- `cargo test -p ploke-eval broad_harness -- --nocapture 2>&1 | tail -n 80`: 13 passed.

## Bottom Line

Not ready to trust for an unattended live loop yet. The broad path has the right high-level stages and the backend admission gate rejects protected/out-of-surface patches, but the actual headless TUI execution remains unproven end to end, and the admitted artifact surface is not preserved into the materialized child-plan carrier.
