# `ploke-protocol`

> Imported from the generated Hermes code wiki at `~/.hermes/wikis/ploke` (generated `2026-06-03T03:55:43Z`, source commit `97b1a101b9363b88b8e41f1447cb36d76a3eb8a2`). Verify implementation details against current source before making changes.


`ploke-protocol` defines typed protocol and procedure abstractions for evaluation workflows that mix mechanized execution, LLM adjudication, and human review. It is a schema/control crate rather than a UI or database crate.

## Responsibilities

- Model procedure states, step artifacts, sequence/fan-out/merge artifacts, and executor provenance.
- Define composable `Procedure` and `Step` abstractions with typed input/output states.
- Encode evidence policies and state disposition (`record`, `forward`, both, or ephemeral).
- Provide optional LLM adjudication types behind the `llm` feature.
- Provide tool-call segmentation/review packet and metric types for local analysis/evaluation artifacts.

## Key Files

- [`crates/ploke-protocol/src/lib.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-protocol/src/lib.rs) — crate docs and public exports.
- [`crates/ploke-protocol/src/core.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-protocol/src/core.rs) — `Measurement`, `ProcedureState`, `EvidencePolicy`, `StateEnvelope`, artifact structs.
- [`crates/ploke-protocol/src/procedure.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-protocol/src/procedure.rs) — `Procedure`, `Sequence`, `FanOut`, `Merge`, debug sink/event helpers.
- [`crates/ploke-protocol/src/step.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-protocol/src/step.rs) — typed step specifications and executor traits.
- [`crates/ploke-protocol/src/tool_calls/mod.rs`](https://github.com/josephleblanc/ploke/blob/97b1a101b9363b88b8e41f1447cb36d76a3eb8a2/crates/ploke-protocol/src/tool_calls/mod.rs) — tool-call review/segment/trace modules.

## Public API

- Core artifacts: `StepArtifact`, `SequenceArtifact`, `FanOutArtifact`, `MergeArtifact`, `ProcedureArtifact`, `ProcedureRun`.
- State/evidence: `EvidencePolicy`, `StateDisposition`, `StateEnvelope`, `ExecutorKind`, `Confidence`.
- Procedure composition: `Procedure`, `ProcedureExt`, `Sequence`, `FanOut`, `Merge`, `NamedProcedure`.
- Step execution: `Step`, `StepSpec`, `StepExecutor`, `MechanizedSpec`, `MechanizedExecutor`, `MechanizedProvenance`.
- Tool-call analysis types: `IntentSegment`, `SegmentedToolCallSequence`, `ToolCallSequence`, review metrics/verdicts.
- Optional `llm` exports: JSON adjudication configs/results and protocol reasoning policy.

## Internal Structure

The crate uses Rust generics to encode procedure input/output state at type level. A `Step<Spec, Exec>` implements `Procedure`; composition types (`Sequence`, `FanOut`, `Merge`) run typed child procedures and produce typed aggregate artifacts. Debug events can be sent to a process-local sink or emitted as JSON lines when `PLOKE_PROTOCOL_DEBUG` is set.

## Dependencies

- **Uses:** `serde`, `async_trait`, `thiserror`, optional LLM feature support.
- **Used by:** `ploke-eval`, `ploke-records` protocol artifacts, `ploke-tree` protocol evidence projections, and review/evaluation tooling.

## Notable Patterns / Gotchas

- `StateDisposition` separates recording from forwarding; do not assume every intermediate state should be both persisted and passed downstream.
- The debug sink is process-global through `OnceLock<Mutex<Option<...>>>`; tests should reset or scope it carefully.
- Optional LLM adjudication is feature-gated; code using those exports must enable `llm`.
