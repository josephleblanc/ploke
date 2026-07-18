# Prototype 1 Headless TUI Runtime Actor Leak

Status: reopened by the 2026-07-18 V26 OOM; sparse eval-runtime fix in source,
pending fresh live-run validation

## Broken Contract

Each broad headless-TUI patch attempt must release the TUI runtime it created when the attempt finishes; completed or abandoned attempts must not leave detached file/state/event/LLM/observability actors holding `AppState`, in-memory database, index, or chat state.

## Evidence

- Live run: `/home/brasides/.ploke-eval/campaigns/p1-g35f-10g5c-e1-20260604-105225`.
- Matching eval log: `/home/brasides/.ploke-eval/logs/ploke_eval_20260604_110949_1896803.log`.
- OOM event: kernel killed PID `1896803` (`ploke-eval`) at `2026-06-04 11:39:42` with about `11339188kB` anonymous RSS and about `1321930` swapped pages. The system had effectively exhausted swap.
- Run profile: `children.max = 5`, `fresh_slots_per_child = 2`, and `patch_generation_parallel_cap = 5`, so broad patch generation admitted five concurrent slots at a time.
- First wave results existed under `prototype1/messages/edit-harness-result/` for slots `node-cbaa86c2efa8b272` through `r5`.
- Second wave target directories existed for `r6` through `r10`, with several `target/` directories in multi-GB build states while the parent `ploke-eval` process remained large.
- Fresh validation run: `/home/brasides/.ploke-eval/campaigns/p1-memfix-0604a`.
  The run used the fixed worktree-local binary and produced ten broad
  headless-TUI result sidecars under
  `prototype1/messages/edit-harness-result/`.
- The fresh run's child plan
  `prototype1/messages/child-plan/node-a212c1db6c2774de.json` had
  `children = []` and ten `rejected_surface_attempts`. This is a broad
  admission failure, not missing patch-generation output.
- Fresh result terminal distribution was eight `applied_timed_out`, one
  `applied_validation_missing`, and one plain `timed_out`. That confirms the
  post-apply terminal-classification fix was active, while admission correctly
  refused every non-contract-clean candidate.
- The same fresh run still emitted a teardown panic after slot rejection:
  `state manager must be running: SendError { .. }` from
  `crates/ploke-tui/src/llm/manager/session.rs`, followed by
  `INVALID_MODEL_RESPONSE` warnings whose diagnostic was
  `ChatStep Error: Cancelled by user.`
- Recurrence campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-v26-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260718-041034`.
- Matching walk-server log:
  `/home/brasides/.ploke-eval/logs/ploke_eval_20260718_041900_1054149.log`.
  It contains 627,648 records and stopped at `2026-07-18 05:55:37`.
- The V26 broad slots started dense indexers over 4,000-5,000 unembedded
  nodes even though the eval runtime was configured for sparse-strict
  retrieval. Several remained active after their corresponding
  `broad_slot_run_headless_tui_done` record; the last two did not stop until
  about 18 minutes after the final broad slot completed.
- At `2026-07-18 07:15:15`, the kernel OOM killer terminated V26 PID
  `1054149` with `10853644kB` anonymous RSS. The process had remained alive as
  a recovery-required walk server after its effectful job stopped progressing.
- V27 was advancing a separate R10-to-R12 job in the same terminal scope when
  V26 exhausted memory. Its interrupted operation has no durable receipt and
  remains preserved rather than being resumed or rewritten.

## Source Trace

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  `admit_broad_harness_batch` spawns up to `batch.patch_generation_parallel_cap` concurrent `run_broad_slot_for_admission` tasks.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  `run_broad_slot_for_admission` calls `run_broad_headless_tui_attempt` for each slot.
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`
  `run_headless_with_model_inner` creates a fresh headless runtime for each attempt and then drops the runtime wrapper after draining events.
- `crates/ploke-eval/src/runner.rs`
  `setup_workspace_tui_runtime_with_read_roots` builds `WorkspaceTuiRuntime` from `TestRuntime`.
- `crates/ploke-tui/src/app/commands/unit_tests/harness.rs`
  `spawn_file_manager`, `spawn_state_manager`, `spawn_event_bus`, `spawn_llm_manager`, and `spawn_observability` called `tokio::spawn(...)` and discarded every `JoinHandle`.

The broken ownership boundary was that `WorkspaceTuiRuntime` owned the visible `App`, receivers, config guard, and `Arc<AppState>`, but not the spawned tasks that also held runtime state. Dropping the wrapper therefore did not terminate the background actors.

The fresh validation run exposed a second lifecycle boundary: once
`TestRuntimeActorGuard` sends `CancelChatToken::Close` and tears the runtime
down, LLM-manager error reporting can still race with state-manager shutdown.
Those late UI-message sends should be best-effort diagnostics during teardown,
not panics in `tokio-rt-worker`.

The V26 recurrence exposed a third boundary. The guard owns the six top-level
actors, but the state manager spawns indexing jobs whose handles are detached
from that guard. The eval-specific constructor also created an `IndexerTask`
and its BM25 service while `RagService::new_full` created the sparse BM25
service actually used by the adapter. Post-apply edits therefore launched
unnecessary dense indexers that could outlive the top-level actor which
started them.

## Docs/Policy Expectation

Prototype 1 broad patch generation intentionally uses parallel parent patch attempts when `patch_generation_parallel_cap > 1`. That parallelism assumes each slot's headless runtime is slot-scoped. The configured cap limits simultaneous active slots; it is not supposed to accumulate prior slot runtimes across completed attempts.

## Current Repro Coverage

- Source inspection proves spawned actor handles were detached before this fix.
- Live OOM evidence proves `ploke-eval` retained far more memory than a resting or indexing `ploke-tui` instance would normally hold.
- A focused harness regression now proves a full-stack `TestRuntime` can transfer ownership of the six expected actor handles to a `TestRuntimeActorGuard`.
- A focused session regression now proves a closed state-manager channel during
  late chat-session message emission is nonfatal.
- The sparse headless constructor regression now proves that the eval runtime
  retains its `RagService` while exposing no dense `IndexerTask`.
- The production adapter refresh regression now asserts the same constructor
  invariant before proving post-apply BM25 freshness.

## Missing Repro / Validation

- Fresh live-run validation should confirm that after a broad batch finishes,
  the parent `ploke-eval` RSS drops instead of retaining each completed slot's
  runtime.
- A fresh run using the follow-up teardown fix should confirm that cancellation
  after broad slot rejection no longer produces `state manager must be running`
  panics.
- The next run should sample RSS before broad fanout, after each slot
  completion, and before successor handoff, and should confirm that no
  `Indexer::run` records are emitted by the sparse headless adapter.
- Raw tuple eval/replay helpers in `runner.rs` still consume `TestRuntime` through `into_app_with_state_pwd` and should be audited separately if they are used in long-running parallel loops.

## Fix Direction

The fix belongs at the runtime ownership boundary:

- retain spawned actor `JoinHandle`s in `TestRuntime`;
- expose an opt-in `TestRuntimeActorGuard` for long-lived harnesses;
- store that guard inside `WorkspaceTuiRuntime`;
- on drop, send `CancelChatToken::Close` and abort all retained actor tasks;
- abort retained actor tasks in reverse spawn order so the LLM manager is
  stopped before the state manager;
- treat chat-session UI message emission to a closed state-manager channel as a
  warning, not a panic, during teardown.
- keep the eval-specific sparse constructor free of the dense `IndexerTask`;
  its separate `RagService` remains responsible for BM25 readiness and
  post-apply freshness.

Do not solve this by lowering Prototype 1 parallelism or weakening run
validation. Lower caps reduce pressure but do not restore the runtime lifecycle
contract, and the digest/history checks are unrelated correctness guards.

## Related Bugs

- [`2026-06-04-prototype1-headless-timeout-after-apply.md`](./2026-06-04-prototype1-headless-timeout-after-apply.md)
  Separate terminal-classification issue exposed by broad headless-TUI attempts.
- [`2026-06-04-prototype1-post-apply-stale-snippet-indexing.md`](./2026-06-04-prototype1-post-apply-stale-snippet-indexing.md)
  Separate post-apply stale-index issue exposed by broad headless-TUI replay.
