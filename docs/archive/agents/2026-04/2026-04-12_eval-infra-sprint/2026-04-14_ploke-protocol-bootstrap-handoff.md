# Ploke-Protocol Bootstrap Handoff

- date: 2026-04-14
- workstream: `A1` / eval introspection method design
- status: bootstrap implemented; first bounded protocol path compiles; conceptual thread externalized

## Summary

This handoff covers the first concrete implementation pass on the protocol-based
approach for NOM-oriented review procedures.

The main change is that protocol structure now has its own crate:

- [crates/ploke-protocol](/home/brasides/code/ploke/crates/ploke-protocol)

`ploke-eval` now depends on that crate and exposes a first bounded protocol
command:

- `ploke-eval protocol tool-call-review`

This is not yet the full NOM framework. It is the first real implementation of
the architecture discussed during the evalnomicon conceptual work.

## What Exists Now

### Workspace / crate boundary

- Added `ploke-protocol` to the workspace in [Cargo.toml](/home/brasides/code/ploke/Cargo.toml)
- Added `ploke-protocol` as a dependency of [ploke-eval](/home/brasides/code/ploke/crates/ploke-eval/Cargo.toml)
- Replaced the local scratch protocol module in
  [crates/ploke-eval/src/protocol/mod.rs](/home/brasides/code/ploke/crates/ploke-eval/src/protocol/mod.rs)
  with a thin re-export of the new crate

### Core protocol abstractions

- [core.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/core.rs)
  defines:
  - `Metric`
  - `Protocol`
  - `ProtocolStep`
  - `Executor`
  - `ExecutorKind`
  - `Confidence`
  - `ProtocolArtifact` (minimal seed only)

### One-shot LLM execution

- [llm.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/llm.rs)
  defines:
  - `JsonChatPrompt`
  - `JsonLlmConfig`
  - `JsonLlmResult<T>`
  - `ProtocolLlmError`
  - `adjudicate_json<T>()`

This wraps the existing `ploke-llm` one-shot `chat_step()` path and requires a
JSON-object response.

### First bounded protocol

- [tool_call_review.rs](/home/brasides/code/ploke/crates/ploke-protocol/src/tool_call_review.rs)
  defines:
  - `ToolCallTraceSubject`
  - `IndexedToolCall`
  - `SuspicionKind`
  - `SuspiciousToolCallSelection`
  - `ToolCallReviewVerdict`
  - `ToolCallReview`
  - `ToolCallSummary`
  - `ToolCallReviewProtocol`

### First CLI surface

- [crates/ploke-eval/src/cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs)
  now includes:
  - `ploke-eval protocol`
  - `ploke-eval protocol tool-call-review`

Current flow:

1. read a real `record.json.gz`
2. flatten tool calls into a typed subject
3. select one explicit indexed tool call
4. build a compact review prompt
5. call the model once with JSON response required
6. parse the result into `ToolCallReview`
7. print table or JSON output

## Verification

Completed:

- `cargo fmt --all`
- `cargo check -p ploke-protocol -p ploke-eval`
- `cargo run -p ploke-eval -- protocol --help`

Known non-blocking warnings were pre-existing unrelated warnings in `syn_parser`
plus one dead-code warning in `ploke-eval`.

## Conceptual Commitments Preserved In Code

The code now reflects several important design decisions:

1. methods are specifications, not executions
2. executors are step-local, not necessarily method-global
3. NOM procedures should be modeled as typed compositions rather than one opaque
   review blob
4. the first useful implementation should be local and bounded rather than
   whole-run and hindsight-heavy

The conceptual source note for these decisions is:

- [protocol operationalization memory](/home/brasides/code/ploke/docs/workflow/evalnomicon/src/meta-experiments/protocol-operationalization-memory.md)

## What Is Still Thin

This bootstrap intentionally leaves several things incomplete:

- protocol outputs are not yet persisted as run-local protocol artifacts
- tool-call review prompt input is still compact and summary-oriented
- no calibration or agreement-check path exists yet
- no second protocol exists yet to prove the crate shape generalizes well
- no aggregation path yet exists from local protocol outputs into larger NOMs

## Suggested Next Steps

1. Persist a protocol artifact beside the run
   - include protocol input, parsed output, raw response, model/provider, and
     validation status

2. Improve the input packet for `tool-call-review`
   - likely include richer detail from the specific tool call and maybe some
     parent-turn context instead of only one compact summary line

3. Add one second bounded protocol
   - only after the first protocol's artifact and payload shape feel stable

Good second candidates:

- one-call search-result relevance review
- one-call tool misuse review with richer context
- one localized target-set-contact slice

## Source Trail

- [evalnomicon conceptual framework](/home/brasides/code/ploke/docs/workflow/evalnomicon/src/core/conceptual-framework.md)
- [notation scratch](/home/brasides/code/ploke/docs/workflow/evalnomicon/notation-scratch.md)
- [protocol typing scratch](/home/brasides/code/ploke/docs/workflow/evalnomicon/protocol-typing-scratch.md)
- [protocol operationalization memory](/home/brasides/code/ploke/docs/workflow/evalnomicon/src/meta-experiments/protocol-operationalization-memory.md)
- [cleaned source conversation](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/2026-04-13_codex-session-019d8a3a_cleaned.md)
