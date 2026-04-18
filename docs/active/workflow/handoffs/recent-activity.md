# Recent Activity

- last_updated: 2026-04-17
- ready_for: restart into a design-oriented eval/protocol pass using the current protocol harvest as evidence about scheduler shape, artifact schema compatibility, and how local analysis outputs should improve tools rather than only fill coverage cells
- owning_branch: refactor/tool-calls
- review_cadence: update after meaningful workflow-doc changes or handoffs
- update_trigger: update after touching workflow structure, review rules, or active artifact layout

## 2026-04-16

## 2026-04-17

- **PROTOCOL DIAGNOSIS WORKFLOW IS NOW EXPLICIT, BUT ITS EXPERIMENT RECORD REMAINS ACTIVE AND INCONCLUSIVE**
  - Added a formal protocol-driven diagnosis workflow and sub-agent launch template:
    - [2026-04-17_protocol-diagnosis-workflow.md](../../agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_protocol-diagnosis-workflow.md)
    - [2026-04-17_protocol-diagnosis-subagent-template.md](../../agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_protocol-diagnosis-subagent-template.md)
  - Ran a bounded parallel workflow trial across issue, tool, combined, and status slices and captured the synthesis in:
    - [2026-04-17_workflow-trial-synthesis.md](../../agents/2026-04-17_eval-failure-and-protocol-audit/2026-04-17_workflow-trial-synthesis.md)
  - The governing experiment record is:
    - [EDR-0003-protocol-diagnosis-workflow-experiment.md](../edr/EDR-0003-protocol-diagnosis-workflow-experiment.md)
  - Important status:
    - the workflow is runnable and structurally useful
    - it is **not yet validated** as a high-quality recommendation workflow
    - the current EDR status is `active` / `inconclusive`, not adopted
  - Restart consequence:
    - treat the workflow as experimental scaffolding
    - the next step is to design a validation experiment for recommendation quality rather than assuming the workflow is already good

- **EVAL CLOSURE FINISHED FOR THE CURRENT RUST SLICE, BUT PROTOCOL IS NOW THE EXPENSIVE FRONTIER**
  - Fresh closure recompute reached:
    - eval: `221` success, `18` fail, `0` missing
    - protocol: `72` full, `21` partial, `8` fail, `120` missing
  - Operational consequence:
    - the restart story is no longer “finish missing evals”
    - eval is effectively closed for this slice
    - protocol is now the active lane

- **THE CAMPAIGN-BACKED PROTOCOL PASS IS FINITE, BUT MUCH MORE EXPENSIVE THAN THE CLI SURFACE SUGGESTS**
  - `closure advance all` is not a fixed-point loop.
  - It does:
    - one eval pass
    - then one large protocol frontier walk
  - The expensive part is not just segmentation plus one review per unit.
  - The protocol review procedures themselves fan out into three adjudication branches:
    - usefulness
    - redundancy
    - recoverability
  - Operational consequence:
    - long protocol runtimes can be real even without transport retries or credit exhaustion
    - the operator problem is partly a scheduler/modeling problem, not only a command-choice problem

- **PROTOCOL FAILURES NOW INCLUDE A REAL ARTIFACT SCHEMA MISMATCH**
  - Some protocol rows failed with:
    - `missing field label`
  - This is not just “still missing coverage”.
  - It indicates a reader/writer incompatibility around stored intent-segmentation artifacts.
  - Operational consequence:
    - some failed rows will not converge by simply letting a frontier walk continue longer
    - restart work should separate:
      - missing coverage
      - partial coverage
      - hard schema/read-model failures

- **LOCAL `PLOKE-EVAL` WORK NOW INCLUDES A BOUNDED-CONCURRENCY PROTOCOL QUEUE**
  - The local worktree includes a scheduler change in `crates/ploke-eval/` that moves protocol advancement from a fully serial per-run walk toward a bounded concurrent worker queue with `max_concurrency`.
  - Important architectural choice:
    - keep `ploke-protocol` as the typed procedure library
    - keep campaign scheduling and queueing in `ploke-eval`
  - Restart consequence:
    - the next thread should treat this as implementation progress toward a better scheduler, not as the whole design answer

- **REPO CACHE LAYOUT WAS NORMALIZED FOR THE FULL LOCAL RUST SLICE**
  - The newly added dataset families initially had repo clones in flat cache paths such as:
    - `~/.ploke-eval/repos/rayon`
    - `~/.ploke-eval/repos/bat`
    - `~/.ploke-eval/repos/fd`
    - `~/.ploke-eval/repos/bytes`
    - `~/.ploke-eval/repos/tracing`
  - `ploke-eval prepare-msb-batch` expects the cache at:
    - `~/.ploke-eval/repos/<org>/<repo>`
  - The cache was normalized into the expected layout:
    - `rayon-rs/rayon`
    - `serde-rs/serde`
    - `sharkdp/bat`
    - `sharkdp/fd`
    - `tokio-rs/bytes`
    - `tokio-rs/tracing`
  - After normalization, prep probes succeeded for all seven newly targeted families:
    - `nushell`
    - `rayon`
    - `serde`
    - `bat`
    - `fd`
    - `bytes`
    - `tracing`

- **EARLIER EVAL-CLOSURE ADVANCE CONCENTRATED THE NEW FAILURE MASS IN `NUSHELL`**
  - Closure moved from the older `171/239` eval-progress state to:
    - `186/239` progressed
    - `169` success
    - `16` fail
    - `1` partial
    - `53` missing
  - The new failures are dominated by `nushell` parser/indexing problems, including:
    - duplicate `crate::commands` module-tree path
    - `generic_lifetime` relation failure
    - `indexing_completed` timeouts
  - Operational consequence:
    - `nushell` should be treated as a low-yield failure family for now
    - the next eval continuation should focus on:
      - `rayon`
      - `serde`
      - `bat`
      - `fd`
      - `bytes`
      - `tracing`

- **SUB-AGENT ORCHESTRATION FAILED TO ADVANCE THE PIPELINE RELIABLY**
  - Multiple workers were launched for eval expansion and protocol follow-through.
  - The first pair moved some state but underperformed:
    - eval moved mostly through `nushell`
    - protocol advanced `clap-rs__clap-3521` locally without changing campaign totals
  - The second, narrower pair failed their role more clearly:
    - they mostly inspected closure state and protocol artifacts
    - they did not actually run the producer commands required to move closure
  - Operational consequence:
    - the session ended with a reversion to direct operator mode
    - the next thread should not assume sub-agent execution is paying off unless the first checkpoint shows real producer-driven deltas

- **EARLIER PROTOCOL FOLLOW-THROUGH DID NOT CHANGE CAMPAIGN TOTALS, BUT THAT IS NO LONGER THE CURRENT STATE**
  - Earlier in the closure rollout, protocol totals were still flat at:
    - `40/169` progressed
    - `13` full
    - `27` partial
    - `129` missing
  - Since then, protocol has moved materially.
  - The current restart-critical totals are the fresh recompute at the top of this file, not this earlier plateau.
  - Historical significance:
    - this section explains why the old operator instinct was “protocol is not moving”
    - it should not be mistaken for the current campaign state

- **THE LIVE `RAYON` BATCH IS CURRENTLY PROVIDER-SUSPECT**
  - Direct operator mode resumed late in the session:
    - `rayon-missing` was prepared
    - `serde-missing` was prepared
    - `run-msb-agent-batch --batch-id rayon-missing --provider xai` was started directly
  - The run is real enough to have created artifacts for `rayon-rs__rayon-986`, including:
    - `agent-turn-trace.json`
    - indexing artifacts
  - But during the live model turn, the runtime emitted a warning referencing:
    - `https://openrouter.ai/api/v1/chat/completions`
  - This matters because the batch was launched with:
    - `--provider xai`
  - Restart consequence:
    - before trusting any new `rayon` eval artifacts, verify whether this is:
      - a true provider mismatch
      - or only a misleading internal logging path in the provider/router layer
    - until that is resolved, treat the current `rayon` run as suspect rather than baseline-trustworthy

- **CLOSURE NOW CONSUMES THE EXPLICIT TARGET REGISTRY `T` DIRECTLY**
  - The earlier closure slice has been retargeted so `ploke-eval closure recompute` now resolves or refreshes the persisted target registry and builds campaign rows from `RegistryEntry` rather than reconstructing registry state from dataset rows plus `run.json`.
  - Important semantic consequence:
    - registry closure is now genuinely `E_r -> T`
    - eval closure is now genuinely `T -> A`
    - protocol closure remains `A -> M`
  - Verified earlier on the full local Rust dataset slice as:
    - registry: `239/239` complete
    - eval: `171/239` progressed, `169` success, `2` fail, `68` missing
    - protocol: `40/169` progressed, `13` full, `27` partial, `129` missing
  - That historical snapshot is no longer the current campaign state; see the fresh totals at the top of this file.
  - Operational consequence:
    - the control plane no longer treats `run.json` presence as the registry surrogate
    - next work can choose explicitly between expanding eval coverage over the missing `68` targets or continuing protocol follow-through over the completed `169`

- **`T` NOW EXISTS AS AN EXPLICIT PERSISTED TARGET REGISTRY IN `PLOKE-EVAL`**
  - Implemented a new typed local target-registry surface in `crates/ploke-eval/` plus CLI entrypoints:
    - `ploke-eval registry recompute`
    - `ploke-eval registry status`
  - Important semantic change:
    - `T` is no longer only an inferred mapping surface from dataset rows plus `run.json`
    - there is now a persisted local registry under:
      - `~/.ploke-eval/registries/multi-swe-bench-rust.json`
  - Verified on the current local Rust dataset slice as:
    - `10` dataset families
    - `10` repo families
    - `239` active entries
    - `0` ineligible
  - Current represented dataset families:
    - `BurntSushi__ripgrep`
    - `clap-rs__clap`
    - `nushell__nushell`
    - `rayon-rs__rayon`
    - `serde-rs__serde`
    - `sharkdp__bat`
    - `sharkdp__fd`
    - `tokio-rs__bytes`
    - `tokio-rs__tokio`
    - `tokio-rs__tracing`
  - Historical note:
    - this was the intermediate state before the next slice retargeted closure to consume the registry directly

- **`PLOKE-EVAL` NOW HAS THE FIRST WORKING `CLOSURE RECOMPUTE` / `CLOSURE STATUS` SLICE**
  - Implemented a new campaign-level closure module in `crates/ploke-eval/` plus CLI entrypoints:
    - `ploke-eval closure recompute`
    - `ploke-eval closure status`
  - Important semantic choice preserved from the sketch:
    - `closure-state.json` is a reduced campaign snapshot under `~/.ploke-eval/campaigns/<campaign>/`
    - it points at existing run/protocol artifacts instead of duplicating `record.json.gz` or persisted protocol outputs
  - The reducer currently uses:
    - dataset JSONL files for the expected benchmark universe
    - `run.json` as the current local registry carrier
    - run-local artifacts plus batch summaries to classify eval completeness vs explicit failure
    - persisted protocol artifacts plus protocol aggregate reduction to classify per-procedure follow-through
  - Verified against the current Rust baseline as:
    - registry: `171/171` mapped
    - eval: `169` complete, `2` failed
    - protocol: `169` expected, `11` full, `29` partial, `129` missing
  - Important interpretive note:
    - the protocol layer is stricter than the old “protocol-artifact directory exists” count
    - many previously “covered” runs now show as `partial` because segment-review follow-through is still missing or mismatched under the current anchor
  - Restart consequence:
    - use `closure status` as the compact control-plane readout before rescanning the filesystem manually
    - the next implementation slice is optional sparse event emission, but the immediate operational move is still to resume the protocol coverage queue

- **EVAL-CLOSURE FORMAL SKETCH LANDED AS THE NEW COMPACT PLANNING SURFACE**
  - Added [2026-04-16_eval-closure-formal-sketch.md](../../agents/2026-04-16_eval-closure-formal-sketch.md)
  - The note reframes the active problem as layered closure over:
    - benchmark-instance mapping
    - eval-artifact completion
    - protocol-artifact completion
  - Important implementation clarification captured there:
    - `closure-state.json` should be reduced campaign state, not a duplicate of `record.json.gz` or persisted protocol outputs
    - `closure-events.jsonl` should stay sparse and semantic rather than becoming another tracing stream
    - the first useful implementation slice is:
      - `closure recompute`
      - `closure status`
      - then optional event emission via existing producer commands
  - Restart consequence:
    - if the next thread starts on implementation rather than more orchestration, use the eval-closure sketch as the planning entrypoint instead of reconstructing the state model from chat

- **`CLAP-RS-ALL` BASELINE EFFECTIVELY COMPLETED AT `130/132` ON `GROK-4-FAST` / `XAI`**
  - Direct batch state from `~/.ploke-eval/batches/clap-rs-all/batch-run-summary.json` is now:
    - `132` attempted
    - `130` succeeded
    - `2` failed
    - `stopped_early: false`
    - `selected_model: x-ai/grok-4-fast`
    - `selected_provider: xai`
  - The two failed instances are:
    - `clap-rs__clap-1624`
    - `clap-rs__clap-941`
  - Failure class:
    - both are parse/indexing failures in the local `clap-rs/clap` checkout, not provider-routing failures
  - Important interpretive note:
    - the reused batch summary still carries stale-looking `run_arm` metadata, so per-run directories and run-local artifacts should be treated as the source of truth over that batch header field
  - Restart consequence:
    - treat the Rust baseline as established except for the two explicit `clap` failures
    - do not spend the next restart rediscovering whether the `clap` batch ran; it did

- **PROTOCOL COVERAGE PROGRESSED, THEN STOPPED AT THE FIRST `CLAP` FRONTIER**
  - Protocol-artifact coverage moved from the older legacy gap set into the first completed `clap` run, then stopped.
  - Confirmed on disk:
    - `BurntSushi__ripgrep-727` now has:
      - `1` intent segmentation
      - `11` tool-call reviews
      - `4` segment reviews
    - `tokio-rs__tokio-5520` now has:
      - `1` intent segmentation
      - `11` tool-call reviews
      - `3` segment reviews
    - `clap-rs__clap-3521` now has partial protocol coverage:
      - `1` intent segmentation
      - `2` tool-call reviews
      - `0` segment reviews
  - Aggregate state:
    - protocol-artifact run directories increased to `40`
    - last observed protocol-artifact write was on `2026-04-15 22:53` for `clap-rs__clap-3521`
  - Operational consequence:
    - progress did not stop because evals were still running; it stopped because orchestration of the protocol queue stopped after `clap-rs__clap-3521`
  - Restart consequence:
    - resume protocol coverage exactly from that frontier rather than rescanning from zero

- **COZO-FRAMED BUILDER-REVIVAL DESIGN NOTE WAS LANDED**
  - Added [2026-04-15_ploke-db-builder-revival-note.md](../../agents/2026-04-15_ploke-db-builder-revival-note.md)
  - The note now:
    - frames Cozo as `an algebra of relations`
    - treats the builder revival as a basis-aware query-intent surface
    - includes formal/symbolic specification plus five explicit refinement/evaluation cycles
  - Operational consequence:
    - the note should be treated as a semantic design input for later `ploke-db` work, not as implementation permission

## 2026-04-15

- **`INSPECT PROTO` SHIFTED TO AN EVIDENCE-RELIABILITY SLICE**
  - The human-facing aggregate CLI in `ploke-eval` now frames the first useful surface as evidence reliability rather than mixed generic coverage.
  - Concrete changes in `crates/ploke-eval/src/cli.rs` and `crates/ploke-eval/src/protocol_report.rs`:
    - single-run report now emphasizes `Call reviews`, `Usable seg reviews`, and explicit segment evidence states (`usable`, `mismatched`, `missing`)
    - added provenance lines for the current anchor and derivation path
    - removed the older `Coverage shape` / `Signal histograms` sections
    - added larger issue-surface charts for issue kinds and issue tools
    - added hardcoded semantic color profiles:
      - `tokio-night`
      - `gruvbox`
      - `mono-dark`
  - Operational consequence:
    - the current CLI is now more clearly an admissibility / evidence-trust surface
    - it is still not yet the intervention-ranking view
  - Restart references:
    - [2026-04-15_protocol-aggregate-cli.md](../../agents/2026-04-15_protocol-aggregate-cli.md)

- **CURRENT PASS SHIFTED FROM PROTOCOL DEVELOPMENT TO ORCHESTRATED HYGIENE + ARTIFACT COVERAGE**
  - New `ploke-protocol` development is intentionally shelved for now.
  - The live working surface is:
    - restart-critical doc cleanup
    - README and folder-index hygiene
    - doc comment / README coverage auditing
    - testing-surface auditing
    - persisted protocol-artifact generation across finished eval runs
  - Active orchestration note:
    - [2026-04-15_orchestration-hygiene-and-artifact-monitor.md](../../agents/2026-04-15_orchestration-hygiene-and-artifact-monitor.md)
  - Operational consequence:
    - treat this as a documentation, monitoring, and artifact-generation pass
    - prefer reports and tracking docs over fresh implementation work

- **PROTOCOL-ARTIFACT COVERAGE IS NOW THE HIGHEST-PRIORITY OPERATIONAL LANE**
  - `ploke-eval` already exposes:
    - `protocol tool-call-intent-segments`
    - `protocol tool-call-review`
    - `protocol tool-call-segment-review`
    - `inspect protocol-artifacts`
  - Current local inventory at start of pass:
    - `39` runs with `record.json.gz`
    - `2` runs with `protocol-artifacts/`
    - `37` runs still missing protocol-artifact coverage
  - Immediate next move:
    - generate protocol artifacts across the finished-run set using the existing CLI
    - once at least half coverage is present, run sampled sanity-check review passes comparing protocol outputs against `inspect`-based qualitative reads

- **DOC HYGIENE TRACKING SURFACES WERE ADDED**
  - Added [2026-04-15_docs-hygiene-tracker.md](../../agents/2026-04-15_docs-hygiene-tracker.md)
  - Current tracked concerns include:
    - stale restart pointers
    - under-indexed README surfaces
    - stale-but-keep docs for later review
    - README/doc-comment drift
    - testing-surface follow-up findings

- **EARLY TESTING AUDIT FOUND A REAL BUG WORTH TRACKING**
  - Added [2026-04-15-observability-test-todo-panic.md](../../bugs/2026-04-15-observability-test-todo-panic.md)
  - Why it matters:
    - an active `ploke-db` observability test still contains `todo!()` placeholders and should not be treated as meaningful coverage until reviewed

## 2026-04-13

- **EVALNOMICON CONCEPTUAL FRAMEWORK WORK DEEPENED INTO A PROTOCOL ARCHITECTURE**
  - The active design conversation moved from high-level NOM discussion into a sharper framework for:
    - metric maturity (`D -> C -> N -> O`)
    - metric vs value-domain distinction
    - method specification vs executor vs admissible input domain
    - typed protocol-step composition for mixed mechanized/adjudicative procedures
  - To avoid losing that nuance on restart, the conceptual thread is now externalized in:
    - [protocol operationalization memory](../../../workflow/evalnomicon/src/meta-experiments/protocol-operationalization-memory.md)
    - [notation scratch](../../../workflow/evalnomicon/notation-scratch.md)
    - [protocol typing scratch](../../../workflow/evalnomicon/protocol-typing-scratch.md)
  - Operational consequence:
    - the next method-design work should treat `evalnomicon` as the conceptual heart
    - do not rely on chat history alone to recover the rationale behind protocol design choices

- **`PLOKE-PROTOCOL` BOOTSTRAP LANDED AS THE FIRST REAL NOM-PROTOCOL IMPLEMENTATION**
  - Added a new workspace crate:
    - [ploke-protocol](/home/brasides/code/ploke/crates/ploke-protocol)
  - `ploke-eval` now exposes:
    - `ploke-eval protocol tool-call-review`
  - Current implementation proves the first end-to-end path:
    - build a typed subject from real run artifacts
    - send one bounded JSON-output adjudication request through `ploke-llm`
    - parse typed output back into a protocol result
  - Current implementation note:
    - this is still a bootstrap, not a full protocol framework
    - persistence, richer input packets, calibration, and a second protocol remain next-step work
  - Active handoff:
    - [ploke-protocol bootstrap handoff](../../agents/2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md)

- **TOKIO-RS FULL BATCH COMPLETED 25/25**
  - The fresh `tokio-rs-all` second-target batch completed with `25` attempted, `25` succeeded, `0` failed, and `stopped_early: false`
  - Trusted batch artifacts:
    - [tokio-rs-all batch summary](/home/brasides/.ploke-eval/batches/tokio-rs-all/batch-run-summary.json)
    - [tokio-rs-all submissions](/home/brasides/.ploke-eval/batches/tokio-rs-all/multi-swe-bench-submission.jsonl)
  - Operational interpretation:
    - the infra path held up end-to-end under a materially broader repo family than ripgrep
    - transient OpenRouter response-body decode timeouts were retried successfully and did not prevent full completion
    - the main remaining issues are no longer batch-startup or parser-collapse problems; they are concentrated in tool/patch-loop execution quality and in tighter evaluation of retrieval relevance and context bloat
  - Restart consequence:
    - the next move is a cleaner post-batch evaluation pass over the tokio artifacts rather than another immediate broad batch
    - keep `tokio-rs__tokio` as `watch` / `default_run` until that tighter read is done

- **TOKIO-RS SELECTED AS THE NEXT RUST REPO EXPANSION TARGET**
  - Added a provisional `tokio-rs__tokio` row to [target-capability-registry.md](../target-capability-registry.md) as the live run-policy gate for second-target expansion
  - Current rationale:
    - the upstream Multi-SWE-Bench Rust harness exposes five visible `tokio-rs` instances
    - the family is medium-small and mostly uses `rust:latest`, so it is a cleaner second probe than higher-variance options such as `clap_rs` or `sharkdp`
    - we do not yet have repo-specific interpretability evidence in `ploke-eval`, so the row remains provisional at `watch`
  - Immediate next move:
    - fetch the `tokio-rs` dataset/repo into the normal `~/.ploke-eval` layout
    - run one reviewed `tokio-rs` probe
    - promote or tighten the registry row from that evidence before preparing the fresh batch id
  - Constraint:
    - do not force a generic benchmark-family row into the target-capability registry unless a real benchmark-wide limitation appears; the registry remains a run-policy/interpretability surface, not a benchmark inventory

- **TOKIO-RS PROBE COMPLETED CLEANLY AND OPENS THE SECOND-REPO BATCH PATH**
  - Added [tokio-rs probe and batch entry](../../agents/2026-04-12_eval-infra-sprint/2026-04-13_tokio-rs-probe-and-batch-entry.md)
  - Ran a reviewed single-instance probe on `tokio-rs__tokio-6618` using the upstream `tokio-rs__tokio_dataset.jsonl` file and a normal `~/.ploke-eval/repos/tokio-rs/tokio` checkout
  - Probe result:
    - completed successfully
    - `1` turn
    - `5` successful tool calls
    - full artifact set written under `~/.ploke-eval/runs/tokio-rs__tokio-6618/`
  - Operational interpretation:
    - no hard parser, modeling, or runner blocker surfaced
    - `tokio-rs` is materially broader than ripgrep, so the target stays at `watch`
    - the watch concern is scaling/breadth and legacy-mode non-primary target skipping, not fairness collapse
  - Registry consequence:
    - `tokio-rs__tokio` now moves to `run_policy = default_run` with `graph_valid` and a `scaling_constraint` watch note
  - Immediate next move:
    - prepare and launch a fresh `tokio-rs` batch id across the visible `tokio-rs` instance family

- **RIPGREP BATCH EXECUTION ROLLED FORWARD TO A USABLE 14-RUN ARTIFACT SET**
  - Added [ripgrep batch rollup and next target](../../agents/2026-04-12_eval-infra-sprint/2026-04-13_ripgrep-batch-rollup-and-next-target.md)
  - Under time pressure, the first `ripgrep-all` batch attempt exposed an operational caveat:
    - the original batch id was reused after an earlier failed attempt
    - `run_batch()` only writes `batch-run-summary.json` after the full loop finishes
    - the rerun was interrupted mid-batch, so the old stale `ripgrep-all` summary remained even though several per-run directories had advanced
  - The clean recovery path was:
    - trust per-run directories under `~/.ploke-eval/runs/BurntSushi__ripgrep-*`
    - launch a fresh remainder batch as `ripgrep-remaining-r1`
    - finish the final straggler `BurntSushi__ripgrep-454` with a direct single-run fallback
  - Operational result:
    - all 14 ripgrep instances now have full per-run artifacts
    - the clean trusted batch-level summary is [ripgrep-remaining-r1/batch-run-summary.json](/home/brasides/.ploke-eval/batches/ripgrep-remaining-r1/batch-run-summary.json), which reports `9/9` succeeded for the remainder slice
    - the snapshot DBs needed for downstream website work are available per run as `final-snapshot.db` and `indexing-checkpoint.db`
  - Restart consequence:
    - ripgrep is no longer the active execution problem
    - the next bounded orchestrator move is choosing a second repo, classifying it in the target capability registry, and launching a fresh batch id for that repo

- **RAW LLM FULL-RESPONSE TRACE STOPGAP LANDED FOR EVAL INTROSPECTION**
  - Added [LLM full response trace stopgap](../../agents/2026-04-12_eval-infra-sprint/2026-04-13_llm-full-response-trace-stopgap.md)
  - Landed a narrow production-code slice in `ploke-tui` and `ploke-eval` with explicit user permission:
    - `ploke-tui` now emits a serialized wrapper record to a dedicated `llm-full-response` tracing target just before finish-policy handling
    - the wrapper currently carries `assistant_message_id`, per-turn `response_index`, and the full `OpenAiResponse`
    - `ploke-eval` now writes a dedicated target-filtered full-response trace file under `.ploke-eval/logs/`
  - Restart consequence:
    - raw provider envelopes are now capturable during eval runs
    - the next bounded slice is deciding how to surface that file from run-local artifacts or `inspect`, not redesigning the whole persistence model

- **FULL-RESPONSE TRACE NOW HAS RUN-LOCAL ARTIFACT + MINIMAL INSPECT SURFACE**
  - Extended the same stopgap so agent-mode runs now copy the relevant traced slice into `runs/<instance>/llm-full-responses.jsonl`
  - `execution-log.json` and `RunArtifactPaths` now carry that sidecar path when present
  - Added `ploke-eval inspect turn <n> --show responses` to load the run-local sidecar and filter by the turn's `assistant_message_id`
  - Added a narrow `inspect turns` fallback so run summary token usage now comes from the raw sidecar when normalized `RunRecord` totals are zero
  - Bounded validation against a fresh `BurntSushi__ripgrep-1294` rerun plus the OpenRouter dashboard showed one known remaining gap:
    - the sidecar captured 13 tool-call responses and useful token totals
    - but it still missed the final `stop` response, so current sidecar totals undercount exact prompt/completion/total tokens and should not yet drive displayed cost
  - Restart consequence:
    - raw provider usage is now reachable from the run directory and visible through the normal CLI flow well enough for eval readiness
    - the next bounded fix is the missing final-response capture, not broader schema redesign

- **INSPECT TURN-SELECTION UX TIGHTENED; LOOP VIEW SCOPED FOR RESTART**
  - Added [inspect turns and loop UX note](../../agents/2026-04-13_inspect-turns-and-loop-ux-note.md)
  - Accepted the CLI inspection ladder:
    - `inspect conversations` as the compact turn-selection surface
    - `inspect turns` as the clearer alias-compatible mental model
    - `inspect turn` as the focused drilldown surface
  - Landed the narrow `ploke-eval` UX slice in `crates/ploke-eval/src/cli.rs`:
    - `inspect turns` alias
    - positional `inspect turn 1`
    - compact dotted turn summary
    - next-step hints between list and drilldown views
    - role filtering for `inspect turn --show messages`
  - Important caveat captured for restart:
    - `turn.messages()` reflects prompt/response reconstruction, not the full agent-tool loop
    - the next bounded slice is a dedicated `inspect turn --show loop` mid-level view rather than overloading `messages`

- **CLI TRACE REVIEW SKILL META-EXPERIMENT SEEDED AND ROUND 1 COMPLETED**
  - Added [CLI trace review skill meta-experiment](../../agents/2026-04-13_cli-trace-review-skill-meta-experiment.md)
  - Added [EDR-0002](../edr/EDR-0002-cli-trace-review-skill-experiment.md) to track the prompt-comparison experiment before promoting the workflow into a durable repo-local skill
  - Fixed the first-round evidence surface to CLI-only `ploke-eval inspect tool-calls` drill-downs against the latest run and explicitly prohibited `crates/ploke-eval/` source inspection during the comparison
  - Ran three parallel prompt variants over the same latest-run trace:
    - Variant A: narrative-first
    - Variant B: failure-classification-first
    - Variant C: intervention-first
  - Round-1 comparison outcome:
    - Variant A was the cleanest fit for sequence-aware narrative and disciplined avoidance of unsupported model blame
    - Variant B was the strongest on explicit failure bucketing and surfaced the repeated missing-path cluster well
    - Variant C produced the most concrete product-intervention recommendations and correctly elevated `read_file` recoverability as the highest-leverage gap
  - Operational consequence:
    - a longer bake-off was not warranted because all variants converged on the same substantive interpretation
    - promoted the stable workflow into [docs/workflow/skills/cli-trace-review/SKILL.md](../../workflow/skills/cli-trace-review/SKILL.md), using Variant A as the durable base

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

## 2026-04-15 (Protocol Aggregate CLI)

- Added a new human-facing protocol aggregate inspection surface in `ploke-eval`:
  - `inspect protocol-overview`
  - alias:
    - `inspect proto`
- The new command currently supports:
  - all-runs summary table:
    - `./target/debug/ploke-eval inspect proto --all-runs --limit 12`
  - single-run aggregate report:
    - `./target/debug/ploke-eval inspect proto --instance tokio-rs__tokio-5200 --width 100`
  - filtered single-run views:
    - `--view overview|segments|calls`
    - `--only-issues`
    - `--overall`
    - `--segment-label`
    - `--tool`
- Aggregation is anchored to the latest persisted `tool_call_intent_segmentation` artifact for the run.
- Segment reviews are only merged when they still match the selected anchor basis; anchor mismatches are surfaced explicitly rather than silently merged.
- Older malformed persisted review artifacts are now skipped so the all-runs summary remains usable.
- Default console tracing for `ploke-eval` was tightened so the new terminal report is not prefixed by routine info-level tracing noise.
- Current main limitation:
  - large-run segment-review coverage still often collapses into `anchor mismatch` rows because many persisted segment reviews do not match the latest segmentation basis.

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
