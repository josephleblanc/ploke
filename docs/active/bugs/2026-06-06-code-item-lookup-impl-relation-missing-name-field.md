# 2026-06-06 `code_item_lookup` Impl Queries Assume `name` Field

## Status

**Open.** Protocol adjudication marked recoverability blocked for the failing
lookup; no regression yet.

## Broken Contract

`code_item_lookup` with `node_kind: impl` must resolve impl blocks through the
stored `impl` relation schema, or return a model-actionable lookup error. It must
not issue Cozo queries that reference non-existent relation fields.

## Evidence

| Field | Value |
|-------|-------|
| Campaign | `p1-admissionfix-g35flash-p25flash-20260606-053302` |
| Treatment branch | `branch-38f8c2eeb85c5e3f` |
| Run | `run-1780754942323-structured-current-policy-7261887d` |
| Protocol artifact | `1780755875905_tool_call_review_BurntSushi__ripgrep-2209.json` |
| Blocked tool call | `[36] code_item_lookup` |
| Recoverability verdict | `no_clear_recovery` (UI: blocked), confidence high |
| File | `crates/printer/src/util.rs` |
| Args | `module_path: crate::util`, `node_kind: impl`, item lookup for `Replacer` impl block |

Observed tool error:

```text
code_item_lookup: Internal compiler error: Database lookup failed: Database error: stored relation 'impl' does not have field 'name'. This indicates an issue with the ploke application iteself, not an error in the search input. Please consider filing an issue at the ploke github.
```

Neighborhood context from protocol packet: call `[35]` successfully resolved the
`struct` `crate::util::Replacer`; call `[36]` then attempted the `impl` lookup
and failed with the schema error; call `[37]` later succeeded on a `method`
lookup for the same file.

Instance trace:
`/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-053302/treatments/branch-38f8c2eeb85c5e3f/instances/BurntSushi__ripgrep-2209/runs/run-1780754942323-structured-current-policy-7261887d/agent-turn-trace.json`

## 2026-06-06 Recurrence: 090815 Baseline

The same impl-relation schema failure recurred in the latest 090815 baseline/protocol run.

| Field | Value |
|-------|-------|
| Campaign | `p1-admissionfix-g35flash-p25flash-20260606-090815` |
| Run | `run-1780762798969-structured-current-policy-b8dc71f0` |
| Run root | `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0` |
| Source report | `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-baseline-protocol-tool-correlation.md` |
| Blocked tool call | `[11] code_item_lookup` |
| Args | `file_path=crates/printer/src/util.rs`, `item_name=Replacer`, `module_path=crate::util`, `node_kind=impl` |

Trace audit over the run root reported 52 provider-emitted tool calls and 52 recorded calls. Call `[10]` resolved the `struct` `crate::util::Replacer`; call `[11]` then tried the impl lookup and failed with the same internal schema shape, `stored relation 'impl' does not have field 'name'`; call `[15]` later recovered by looking up the `replace_all` method directly. This recurrence confirms the issue is not isolated to the earlier 053302 treatment run.

## Source Trace

`code_item_lookup` maps `node_kind: impl` to relation `"impl"` via
`NodeKind::as_relation()` (`crates/ploke-tui/src/rag/utils.rs`), then calls
`graph_resolve_exact()` (`crates/ploke-db/src/helpers.rs`).

`graph_resolve_exact` always projects `name` from the target relation:

```cozo
*{rel}{ id, name, tracking_hash: hash, span @ 'NOW' },
  ...
  name == {item_name_lit},
```

If the stored `impl` relation has no `name` column, Cozo rejects the query
before any lookup logic runs. The tool surfaces this as an internal compiler
error rather than a recoverable user-input error.

## Docs/Policy Expectation

`NodeKind::Impl` is an allowed `code_item_lookup` `node_kind` in the tool schema
(`crates/ploke-tui/src/tools/code_item_lookup.rs`). Agents are expected to use
impl lookups after struct discovery to gather method context.

## Current Repro Coverage

None for impl-schema mismatch. Existing `graph_resolve_exact` fixture tests cover
struct/method/function/module paths but not `node_kind: impl`.

## Missing Repro / Validation

- Unit/integration test: `code_item_lookup` with `node_kind: impl` against a
  fixture DB that contains impl rows, asserting either a successful resolve or a
  deliberate unsupported-kind error — not a Cozo schema failure.
- Replay probe on the treatment run call `[36]` after fixing the query shape or
  explicitly rejecting impl lookups with a typed error.

## Fix Direction

- Align `graph_resolve_exact` (or a dedicated impl resolver) with the actual
  `impl` relation schema from `ploke-transform` / ingest.
- If impl rows are not name-keyed, define the correct lookup key (trait/type id,
  span, or owner association) and document it in the tool contract.
- Do not weaken the query by dropping validation; either support impl lookup
  correctly or reject `node_kind: impl` at parameter validation with a clear
  retry hint.

## Related Bugs

- [`2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`](./2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md)
  — same treatment run also hit `code_item_lookup` snippet `Content changed`
  errors later in the turn (distinct stale-snippet class).
