# Protocol Cold-Start Reference

- date: 2026-04-15
- task title: protocol cold-start reference
- task description: compact reference for the evalnomicon-to-`ploke-protocol` thread, covering persisted eval artifacts, access/query surfaces, and the current conceptual framing for typed mixed-mode protocols
- related planning files: `docs/active/CURRENT_FOCUS.md`, `docs/active/workflow/README.md`, `docs/active/workflow/handoffs/recent-activity.md`, `docs/active/agents/2026-04-12_eval-infra-sprint/2026-04-14_ploke-protocol-bootstrap-handoff.md`, `docs/workflow/evalnomicon/src/core/conceptual-framework.md`, `docs/workflow/evalnomicon/protocol-typing-scratch.md`

## Why This Note Exists

This note is a compact recovery and working-reference surface for the current
`evalnomicon` and `ploke-protocol` thread.

The immediate architectural intent is:

1. treat `ploke-eval` as the concrete observation and persistence substrate
2. treat `ploke-protocol` as the typed derivation layer for mixed mechanized and
   adjudicated procedures
3. ground the protocol abstractions in real run artifacts before generalizing

## Current Readback

The conceptual center is the separation between:

- metric
- method or protocol specification
- admissible input or evidence contract
- executor
- produced value

The current framework is explicitly not:

- "one opaque review blob"
- "LLM call equals metric"
- "whole-run hindsight first"

The active design direction is:

- bounded local protocols first
- typed step composition
- step-local executors
- explicit admissible evidence
- later aggregation into larger NOM-style metrics

## Persisted Eval Artifact Map

### Canonical run record

- `record.json.gz` is the canonical persisted run record
- main type: `RunRecord`
- primary schema file: `crates/ploke-eval/src/record.rs`
- read/write path: gzip + `serde_json` via `read_compressed_record()` and
  `write_compressed_record()`

Important nested record types include:

- `RunMetadata`
- `RunPhases`
- `SetupPhase`
- `TurnRecord`
- `TimeTravelMarker`
- `RunTimingSummary`
- `LlmResponseRecord`
- `ToolExecutionRecord`
- `TurnOutcome`
- `ToolResult`
- `PatchPhase`
- `ValidationPhase`

Important operational note:

- the run record is synthesized from runtime artifacts rather than being a raw
  append-only event dump
- `RunRecordBuilder::add_turn_from_artifact()` reconstructs persisted turn data
  from `AgentTurnArtifact`
- `build_setup_phase()` derives `SetupPhase` from DB state plus
  `parse-failure.json`

### Trace and turn artifacts

- `agent-turn-trace.json`
- `agent-turn-summary.json`

Primary type:

- `AgentTurnArtifact`

Contains or references:

- `ObservedTurnEvent`
- `PatchArtifact`
- `RequestMessage`
- `llm_prompt`
- `llm_response`

Operational note:

- the trace is rewritten during the loop and can be partial if interrupted

### JSON sidecars

Primary persisted JSON sidecars include:

- `execution-log.json`
- `repo-state.json`
- `indexing-status.json`
- `snapshot-status.json`
- `parse-failure.json`

Associated types include:

- `ExecutionLog`
- `RepoStateArtifact`
- `IndexingStatusArtifact`
- `SnapshotStatusArtifact`
- `ParseFailureArtifact`

### DB artifacts

Primary DB files include:

- `indexing-checkpoint.db`
- `indexing-failure.db`
- `final-snapshot.db`
- `~/.ploke-eval/cache/starting-dbs/*.sqlite`

Operational note:

- DB persistence is snapshot/copy based, not serde based

### JSONL sidecars

Primary JSONL artifacts include:

- `multi-swe-bench-submission.jsonl`
- `llm-full-responses.jsonl`

Associated types and behavior:

- `MultiSweBenchSubmissionRecord` is written linewise
- `llm-full-responses.jsonl` is a copied sidecar, not part of `RunRecord`
- raw-response inspection depends on this sidecar being present

### Additional run pointers and summaries

- `batch-run-summary.json`
- `replay-batch-###.json`
- `last-run.json`

Associated types include:

- `BatchRunSummary`
- `ReplayBatchArtifact`
- `LastRunRecord`

Operational note:

- `last-run.json` is the default recovery pointer when `inspect` is invoked
  without `--record` or `--instance`

## Persistence Shape Notes

The persistence model is mostly nested serde structs and enums:

- widespread direct `Serialize` / `Deserialize` derives
- selective `#[serde(default)]`
- selective `#[serde(skip_serializing_if = ...)]`
- tagged enums for some result and source surfaces

Current finding:

- no local `#[serde(flatten)]` surfaced in the inspected `ploke-eval`
  persistence files

Important caveat:

- some embedded types come from `ploke-llm` or `ploke-tui`, so their serde
  behavior is inherited rather than defined locally in `ploke-eval`

## Library Access Surfaces

The main library query surface is already present in `crates/ploke-eval` and is
stronger than the current protocol bootstrap.

### `RunRecord` accessors

The main run-level accessors include:

- `timestamp_for_turn()`
- `tool_calls_in_turn()`
- `turn_record()`
- `llm_response_at_turn()`
- `conversations()`
- `tool_calls()`
- `db_snapshots()`
- `failures()`
- `config()`
- `was_tool_used()`
- `turns_with_tool()`
- `replay_state_at_turn()`
- `replay_query()`

### `TurnRecord` accessors

Turn-local accessors include:

- `db_state()`
- `messages()`
- `tool_calls()`
- `tool_call()`
- `tool_result()`

### `DbState` accessors

Historical DB access includes:

- `lookup()`
- `query()`

### Re-exports

`crates/ploke-eval/src/lib.rs` already re-exports the main read/query types,
including:

- `RunRecord`
- `TurnRecord`
- `DbState`
- `ReplayState`
- `RawFullResponseRecord`
- `RunRecordBuilder`

## CLI Access And Inspection Surface

The previous CLI-first introspection workflow is still the main concrete bridge
between artifacts and analysis.

Primary subcommands:

- `transcript`
- `conversations`
- `inspect conversations` / `inspect turns`
- `inspect tool-calls`
- `inspect db-snapshots`
- `inspect failures`
- `inspect config`
- `inspect turn`
- `inspect query`
- `protocol tool-call-review`

Important behavior:

- `inspect` defaults to the most recent completed run if `--record` and
  `--instance` are omitted
- `inspect turn --show responses` is the main raw-LLM-response inspection path
- `inspect turn --show db-state` feeds naturally into `inspect query`
- many inspect commands print `Next:` hints intended to support agent or LLM
  inspection workflows

Important caveats:

- top-level `conversations` and `inspect conversations` are similar but not
  identical
- `conversations` can fall back to raw-response sidecar token totals
- `inspect turn --show responses` depends on `llm-full-responses.jsonl`

## Conceptual Framework To Preserve In Code

### Metric classes

The current framework distinguishes:

- `O`: obvious metrics
- `N`: non-obvious metrics
- `C`: conceptual metrics
- `D`: uncaptured dimensions

Current maturation path:

- `D -> C -> N -> O`

Operationalized metrics are:

- `O ∪ N`

### Method and execution split

The current conceptual split is:

- `m`: metric
- `x`: method or protocol specification
- `I_x`: admissible input domain
- `e`: executor
- `v`: produced value

The key rule is that the executor should follow the method using only
admissible inputs from `I_x`.

### Reliability is not just "LLM gave an answer"

The conceptual notes keep these concerns separate:

- boundedness of the method
- explicitness of the method
- admissible evidence contract
- repeatability and reliability of the executor on the method
- calibration against a stronger reference

### Typed step composition

The strongest stable idea from the scratch docs is:

- a larger protocol is a composition of steps `x_1, x_2, ..., x_n`
- each step has its own input domain, output domain, and executor
- some steps are mechanized
- some steps are LLM adjudicated
- some steps could later be human-reviewed

This is the main reason `ploke-protocol` should lean into typed compositions
rather than one-shot review functions.

## Current `ploke-protocol` Bootstrap

The crate currently exposes the right seed concepts:

- `Measurement`
- `Protocol`
- `ProtocolStep`
- `Executor`
- `ExecutorKind`
- `Confidence`
- `ProtocolArtifact`
- `JsonChatPrompt`
- `JsonLlmConfig`
- `JsonLlmResult<T>`
- `ProtocolLlmError`

Current implementation status:

- the first concrete protocol is local and bounded
- it reviews one indexed tool call
- the LLM path is one-shot JSON adjudication through `ploke-llm`
- the result is typed
- persistence of protocol artifacts is still thin or absent

## Design Constraints Emerging From The Codebase

1. `ploke-eval` already has rich persisted artifacts and query helpers, so
   `ploke-protocol` should not re-invent artifact storage or historical lookup.
2. The protocol layer should consume typed observation subjects derived from
   `RunRecord` and adjacent sidecars rather than reading arbitrary files ad hoc.
3. The bounded protocol path is the correct starting scale. Whole-run NOMs
   should come from composition and aggregation, not from jumping directly to a
   giant retrospective prompt.
4. Protocol persistence is the clearest next substrate gap:
   protocol input packet, parsed output, raw response, executor identity,
   validation status, and provenance should become first-class artifacts.
5. Terminology is still moving. Keep traits and module structure semantically
   strong enough that local type names can stay short without becoming vague.

## Near-Term Architectural Recommendation

Use this division of responsibility:

- `ploke-eval`
  - owns run artifacts
  - owns replay and historical DB access
  - owns run-record schema and loading
  - can build concrete protocol subjects from eval artifacts
- `ploke-protocol`
  - owns protocol specs
  - owns typed step composition
  - owns executor abstractions
  - owns adjudication result typing and protocol-artifact typing
  - should become the home for protocol execution state and aggregation rules

## Open Risks

- terminology drift between `protocol`, `procedure`, `method`, and `metric`
- hindsight leakage if admissible evidence is not encoded explicitly
- sidecar incompleteness, especially `llm-full-responses.jsonl`
- overfitting the framework to one bootstrap protocol before the second bounded
  protocol tests the abstraction
