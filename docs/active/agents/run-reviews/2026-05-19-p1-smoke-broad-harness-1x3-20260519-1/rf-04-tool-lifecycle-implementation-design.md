Verified surface: implementation design from current `ploke-records::agent_turn`, `ploke-records::tool_contracts`, `ploke-eval::runner`, `ploke-eval` replay tests, and `ploke-tui` session/tool-loop/edit-proposal code; no runtime code changed and no tests executed.

# RF-04 Implementation Design: Tool Lifecycle Replay

Taxonomy source: `replay-regression-failure-taxonomy.md`, RF-04.

Assumption: "this one" means RF-04, Tool Proposal Lifecycle Ambiguity. This is
the highest-leverage first slice because it gives later RF families a typed way
to say whether a tool call was requested, malformed, preflight-rejected, staged,
settled, applied, failed, or replayed back to the model.

## Architecture Readout

The larger semantic object is a tool-call lifecycle, not a
requested/completed/failed pair.

Current code has three partly overlapping surfaces:

- `ploke-tui` live loop: `SystemEvent::{ToolCallRequested, ToolCallCompleted,
  ToolCallFailed}` plus proposal state in `AppState::proposals`.
- `ploke-eval` runner artifact: observed events and terminal `PatchArtifact`
  snapshots.
- `ploke-records` passive schema: `AgentTurnArtifactRecord`,
  `ToolRequestRecord`, `ToolCompletedRecord`, `ToolFailedRecord`, and decoded
  tool argument/result helpers.

The lossy reduction to refuse is "just add another completed status." A call can
have multiple relevant events with the same `call_id`: an edit tool can complete
with a staged proposal, then later complete with an applied or failed settlement.
The replay answer is not the first completion; it is the tool reply actually
sent to the next model request, scoped by tool-loop mode.

The implementation should preserve:

- provider request identity: request id, parent id, call id, tool name,
  arguments;
- execution events: preflight rejection, completed result, failed result;
- edit proposal effect: staged proposal id, files, preview mode, pending state;
- settlement effect: applied, denied, failed, stale, or no-op;
- model replay payload: the exact tool message content and call id sent in the
  next provider request;
- authority boundary: live mutation stays in `ploke-tui`; passive persisted
  schema lives in `ploke-records`; run artifact assembly stays in
  `ploke-eval::runner`.

The natural extensions are RF-05 same-file composition, RF-06 semantic edit
addressing, RF-07 protected-surface drift, RF-09 provider aborts, and RF-13
baseline/treatment comparison. All of those need the same base fact: what
happened to this tool call, and what did the model see next?

## Carrier Map

Role(s):

- Provider-emitted call.
- Live tool executor.
- Edit proposal.
- Model replay reply.
- Passive record reader.

State(s):

- `Requested`: provider emitted a tool call and it passed into the local tool
  loop.
- `Rejected`: provider/tool arguments failed preflight before execution or
  before proposal creation.
- `Completed`: the tool emitted a normal result.
- `Failed`: the tool emitted a structured error.
- `Staged`: an edit tool created a pending proposal but has not mutated the
  workspace.
- `Settled`: an edit proposal reached applied, denied, failed, stale, or no-op.
- `Replayed`: a tool reply was appended to the next provider request.

Transition(s):

- provider response -> `Call<Requested>`
- preflight/tool argument rejection -> `Call<Rejected>`
- tool execution result -> `Call<Completed>` or `Call<Failed>`
- edit result with staged proposal -> `Edit<Staged>`
- approval/denial/apply outcome -> `Edit<Settled>`
- next provider request construction -> `Reply<Replayed>`

Durable record projection(s):

- Add passive lifecycle projections under `ploke-records::agent_turn` or a
  small child module such as `agent_turn::tool_lifecycle`.
- Keep current event records intact for compatibility.
- Add an optional, additive lifecycle vector to `AgentTurnArtifactRecord` only
  after the derived projection is green.

Module boundary that should carry repeated context:

- `ploke-tui::llm::manager::session`: live gating and replay-ready decision.
- `ploke-records::agent_turn`: passive persisted schema and derived lifecycle
  records.
- `ploke-eval::runner`: live artifact to records-owned schema projection.
- `ploke-eval::tests::replay`: historical replay fixtures through the real
  session/tool loop.

Flattened identifiers refused:

- `ToolCallCompletedApplied`
- `ToolCallCompletedStaged`
- `ToolCallReplayStatus`
- `AppliedToolResultRecord`
- `PendingToolCallProjection`

Use module/type context instead: `tool_lifecycle::Call`, `tool_lifecycle::Edit`,
`tool_lifecycle::Reply`, with `Record` suffixes only on passive persisted
projections.

Axes before naming Rust items:

| Axis | Carrier |
| --- | --- |
| role | call, edit, reply |
| state/phase | requested, rejected, completed, failed, staged, settled, replayed |
| authority source | provider response, local tool execution, proposal registry, next-request builder |
| provenance | request id, parent id, call id, event index, proposal id |
| persisted projection | `ploke-records::agent_turn` additive record |
| active-loop consumer | `execute_tools_via_event_bus` and request-message construction |
| renderer/operator projection | run record / archive graph / future replay summaries |

Compile-time constraint to aim for:

- The function that appends a tool result to the next provider request should
  consume only `Reply<Replayed>` or an equivalent `ReplayReady` carrier. An edit
  `Staged` carrier should not satisfy that input in gated mode.

## Current Gap

The live gated path has a partial behavior fix:

- `ToolLoopMode::Gated` exists.
- `execute_tools_via_event_bus` ignores pending edit completions when
  `ui_payload` has `status=pending`.
- The unit test `execute_tools_via_event_bus_gated_waits_for_settled_edit_result`
  covers waiting for a later settled result.
- The headless replay test
  `gated_replay_sends_applied_ns_patch_instead_of_staged_success` covers a
  happy-path replay fixture.

The record/replay projection is still weaker:

- `AgentTurnArtifactRecord.events` can record multiple `ToolCompleted` events for
  one `call_id`, but `extract_tool_calls_from_events` collapses a call to the
  first matched completion/failure.
- `PatchArtifactRecord` snapshots proposal terminal state, but does not say
  which tool event became the next provider request.
- `ToolCompletedRecord.content` can be a staged edit result or a settled apply
  result, and readers must decode/interpret it themselves.
- `ToolUiPayloadRecord.fields` contains useful hints, but generic UI fields
  should not become the semantic authority for replay.

## Implementation Direction

### Slice 1: Derived Lifecycle Projection

Add a derived projection first, without changing the wire shape.

Target crate: `ploke-records`.

Add a module, likely `ploke_records::agent_turn::tool_lifecycle`, with passive
record/projection types:

```rust
pub struct CallRecord {
    pub request: ToolRequestRecord,
    pub events: Vec<EventRecord>,
    pub effect: EffectRecord,
    pub reply: Option<ReplyRecord>,
}

pub enum EventRecord {
    Completed { event_index: usize, record: ToolCompletedRecord, effect: EffectRecord },
    Failed { event_index: usize, record: ToolFailedRecord },
}

pub enum EffectRecord {
    None,
    ReadOnly,
    Edit(EditRecord),
    ParseFailure(ToolResultParseFailure),
}

pub enum EditRecord {
    Staged(ProposalRecord),
    Settled(SettlementRecord),
}

pub enum SettlementRecord {
    Applied { proposal_id: Option<String>, applied: usize, files: Vec<String> },
    Denied { proposal_id: Option<String>, reason: Option<String> },
    Failed { proposal_id: Option<String>, reason: Option<String> },
    Stale { proposal_id: Option<String>, reason: Option<String> },
    NoOp { reason: Option<String> },
}

pub struct ReplyRecord {
    pub call_id: String,
    pub source_event_index: Option<usize>,
    pub content: String,
    pub state: ReplyStateRecord,
}

pub enum ReplyStateRecord {
    Replayed,
    Missing,
    Ambiguous,
}
```

Exact names can be refined during implementation, but the important point is
that `CallRecord` groups all events for one `call_id`. It does not stop at the
first completion.

Add:

```rust
impl AgentTurnArtifactRecord {
    pub fn derive_tool_lifecycle(&self) -> Vec<tool_lifecycle::CallRecord>;
}
```

Derivation rules:

1. Group `ToolRequested`, `ToolCompleted`, and `ToolFailed` by `call_id`.
2. Preserve event order with source event indexes.
3. Decode request arguments through `ToolArgumentsJson::decode_for_tool`.
4. Decode completed result content through
   `tool_contracts::decode_tool_result_content`.
5. Classify edit-tool results by decoded content first:
   - staged result: `ok=true`, `staged>0`, `applied=0`;
   - settled result: `applied>0`, failed result, stale result, or denied result.
6. Use `PatchArtifactRecord` only as corroborating terminal proposal state, not
   as a substitute for event order.
7. Join `llm_prompt` tool-role messages by `tool_call_id` to identify what was
   actually replayed to the next provider request.
8. If more than one candidate reply exists for a call, classify it as
   `Ambiguous` until the request-message join disambiguates it.

Why this slice first:

- It is passive and testable.
- It improves replay semantics without changing the live loop.
- It gives all later RF families one common language for tool-call state.

### Slice 2: Normalize Edit Tool Result Content

Target crate: `ploke-tui`.

Introduce a single edit-tool result schema for both staged and settled outcomes.
Current staged results use `ApplyCodeEditResult` / `ApplyNsPatchResult`, while
settled apply events from approval currently emit a different JSON shape such as
`{"ok":true,"applied":1,"results":[...]}`.

Add a tool-contract DTO behind the existing `tool_contracts` feature, for
example:

```rust
pub enum EditReply {
    Staged {
        proposal_id: Uuid,
        staged: usize,
        files: Vec<String>,
        preview_mode: String,
        auto_confirmed: bool,
    },
    Applied {
        proposal_id: Uuid,
        applied: usize,
        files: Vec<String>,
        results: Vec<EditFileResult>,
    },
    Failed {
        proposal_id: Option<Uuid>,
        reason: String,
    },
    Stale {
        proposal_id: Uuid,
        reason: String,
    },
    Denied {
        proposal_id: Uuid,
        reason: Option<String>,
    },
}
```

The exact type should live beside the TUI tools that emit it, and
`ploke-records::tool_contracts` should re-export it rather than mirror it.

Migration rule:

- Keep decoding legacy staged and settled shapes in `ploke-records`.
- Emit the normalized shape for new events.
- Do not make UI fields the canonical parser for replay semantics.

### Slice 3: Make Replay-Ready State Explicit in the Live Loop

Target crate: `ploke-tui`.

Replace the boolean `should_wait_for_settled_edit` check with a typed classifier
used by `execute_tools_via_event_bus`.

Suggested live-only shape:

```rust
enum ReplyDecision {
    Ready(ToolCallUiResult),
    Awaiting(EditPending),
    Failed(ToolCallUiError),
}

struct EditPending {
    call_id: ArcStr,
    proposal_id: Option<Uuid>,
}
```

The dispatcher should not send a waiter result for `Awaiting`. It should wait
for `Ready` or `Failed`.

The next-message construction path should consume only ready results. This is
the active-loop compile-time guard against replaying a staged proposal as if it
were the final settled tool answer.

This does not require changing interactive behavior immediately. In `Auto` mode,
the classifier can mark staged edit replies as ready with an explicit
`pending_approval` state. In `Gated` mode, `pending_approval` is not ready.

### Slice 4: Persist Optional Lifecycle Records

Target crates: `ploke-records`, `ploke-eval`.

After the derived projection is green, add an additive field:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub tool_lifecycle: Vec<tool_lifecycle::CallRecord>,
```

to `AgentTurnArtifactRecord`.

Then update `ploke-eval::runner` projection so new `agent-turn-trace.json` and
`agent-turn-summary.json` files include the lifecycle projection. Existing
v1 artifacts continue to parse because the field is optional and derivable.

Schema decision:

- If record schema constants are treated as external compatibility promises,
  introduce `agent-turn-trace.v2` / `agent-turn-summary.v2`.
- If the constants are crate-local labels and optional fields are accepted as
  v1-compatible, keep v1 and document the additive field in the handoff.

Given the current `Record` trait uses a single schema constant, the safer design
is to add v2 constants and loaders that accept both v1 and v2.

### Slice 5: Update Run-Record Tool Extraction

Target crates: `ploke-records`, `ploke-eval`.

The current `extract_tool_calls_from_events` shape reconstructs
`ToolExecutionRecord` by removing the pending request on first completion or
failure. That loses later settlement completions.

Do not extend that helper with more booleans. Replace the consumer path with:

- `turn.lifecycle_calls()` for full replay/projection.
- legacy `turn.tool_calls()` remains as a compatibility summary, but when a
  lifecycle record exists it should choose the replayed/settled reply, not the
  first completion.

This keeps old CLI surfaces working while giving replay and future UI code the
right carrier.

## Minimal Verification Plan

No expected-red tests are needed for the first implementation if we land it as
fixed-contract coverage. The tracker should only be updated if we intentionally
check in a failing test.

Suggested test order:

1. `ploke-records` unit: parse a synthetic trace with one request, a staged
   completion, an applied completion, and an `llm_prompt` tool message. Assert
   one lifecycle call, two events, staged then settled effects, and replay source
   points at the applied completion.

2. `ploke-records` real-shape parse: current `AgentTurnTraceRecord` fixture
   still deserializes and `derive_tool_lifecycle()` returns legacy-compatible
   results.

3. `ploke-tui` unit: extend
   `execute_tools_via_event_bus_gated_waits_for_settled_edit_result` to assert
   the classifier returns `Awaiting` for staged edit output and `Ready` for
   settled output.

4. `ploke-eval` replay: extend
   `gated_replay_sends_applied_ns_patch_instead_of_staged_success` to assert the
   persisted/derived lifecycle says the replayed message was applied, not
   staged.

5. `ploke-eval` historical replay: use the existing protected `ns_patch`
   historical tests to assert protected preflight failures produce no staged
   edit effect and no completed success lifecycle event.

Smallest likely commands:

```bash
cargo test -p ploke-records --features tool-contracts tool_lifecycle
cargo test -p ploke-tui execute_tools_via_event_bus_gated_waits_for_settled_edit_result
cargo test -p ploke-eval gated_replay_sends_applied_ns_patch_instead_of_staged_success
cargo test -p ploke-eval historical_trace_replay_marks_repeated_protected_ns_patch_before_staging
```

Use bounded output when running these under AGENTS.md.

## Implementation Boundaries

Do:

- Use typed tool DTO decoding through `ToolArgumentsJson::decode_for_tool` and
  `decode_tool_result_content`.
- Keep raw provider output replay through `RawFullResponseRecord` and
  `RecordedResponseTape`.
- Preserve old event records for compatibility.
- Make lifecycle derivation deterministic and source-indexed.
- Treat UI payload fields as supporting evidence only.

Do not:

- Read persisted agent-turn JSON through `serde_json::Value` field walking.
- Add active mutation authority to `ploke-records`.
- Hide the staged/applied split behind `ToolCallCompleted`.
- Infer applied workspace state from proposal existence.
- Treat `PatchArtifactRecord.applied=true` as proof of which event was replayed
  to the model.

## Open Decisions

1. Schema versioning: add v2 schema constants now, or keep v1 with an optional
   additive field.
2. Normalized edit result naming: whether the shared DTO should be `EditReply`,
   `EditResult`, or a module-scoped `edit::Reply`.
3. Settlement source: whether denial/stale/failed proposal outcomes should be
   emitted as explicit `SystemEvent` variants, or reconstructed from proposal
   snapshots until the edit-surface source-records slice lands.
4. Legacy `ToolExecutionRecord`: whether to preserve first-completion behavior
   behind an explicit legacy method, or switch it to settled/replayed behavior
   once lifecycle records exist.

## Recommended First Patch

Start with Slice 1 only:

- Add `agent_turn::tool_lifecycle` passive projection types.
- Add `AgentTurnArtifactRecord::derive_tool_lifecycle()`.
- Add tests for staged-then-applied and failed-before-staging cases.

That patch has the smallest blast radius and creates the semantic basis for the
later live-loop and persisted-record changes. Once it is green, the next patch
can normalize edit result content and wire the projection into `ploke-eval`.
