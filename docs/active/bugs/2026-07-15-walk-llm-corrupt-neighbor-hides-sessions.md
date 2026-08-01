# Walk LLM Inventory Aborted on a Corrupt Neighbor

- Date: 2026-07-15
- Status: fixed and live verified; durable imported historical fixture pending
- Related plan:
  [`2026-07-13_prototype1-loop-operator-control-plan.md`](../agents/2026-07-13_prototype1-loop-operator-control-plan.md)

## Broken Contract

A campaign-scoped, read-only LLM inventory must return every independently
valid debugger session and report each malformed or unreadable sibling as its
own issue. One damaged session must not abort CLI or UI access to healthy
sessions.

## Evidence

The preserved campaign is:

```text
p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824
```

The read-only probe used its preserved parent worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824
```

Its tool-loop evidence root is:

```text
/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824/prototype1/debug/tool-loop
```

Disk exhaustion during the stopped r13-r15 attempt left this exact zero-byte
manifest:

```text
360fcc4c-416c-45f2-996b-31e2393dc453/session.json
size = 0
sha256 = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
```

The same root contains five valid session manifests, including the accepted
Direct Google r16-r18 sweep:

| Lane | Session | Debug state | Published steps |
|---|---|---:|---:|
| r16 | `0cf7c223-7b45-4e6e-81bb-5078b6fa00b8` | terminal | 46 |
| r17 | `b548822d-0966-4cd8-8125-595de4ac8132` | paused | 9 |
| r18 | `596aebf9-a621-4d66-9282-7dfe069e6754` | terminal | 39 |

The earlier controller-backed LLM list path called the strict session loader
with `?`. Parsing the r15 empty file therefore propagated one neighbor's error
out of the whole request before a client could inspect the healthy sessions.
This was a read-model failure, not evidence that r16-r18 were invalid.

Final protocol-v10 validation with the rebuilt `bbb468116` source ran:

```text
target/debug/ploke-eval loop walk serve \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824 \
  --socket /home/brasides/.ploke-eval/tmp/typed-llm-v10-final.sock \
  --ttl-secs 120
target/debug/ploke-eval loop walk llm \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824 \
  --socket /home/brasides/.ploke-eval/tmp/typed-llm-v10-final.sock \
  --format json sessions
target/debug/ploke-eval loop walk llm \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824 \
  --socket /home/brasides/.ploke-eval/tmp/typed-llm-v10-final.sock \
  --format json observe \
  --session-id b548822d-0966-4cd8-8125-595de4ac8132 --step 8
target/debug/ploke-eval loop walk llm \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-15g2x3-fixed-20260525-123824 \
  --socket /home/brasides/.ploke-eval/tmp/typed-llm-v10-final.sock \
  --format json observe \
  --session-id 0cf7c223-7b45-4e6e-81bb-5078b6fa00b8 --step 45
```

The index returned protocol 10, five lanes, five healthy sessions, and one
independent `invalid` r15 issue carrying the empty-file digest above. It did not
drop or synthesize a representative session. The temporary server stopped
cleanly and its socket was removed; the separate active successor runtime and
socket remained alive.

The exact r17 response kept the conflicting-looking but independently true
layers separate:

- debugger manifest: `paused`;
- resume frontier: `next_step=9`, `terminal=false`;
- selected checkpoint: response index 8, assistant
  `502f740c-915d-461e-8da5-b90263dbcb52`;
- outer headless terminal: `exhausted`;
- agent-turn terminal: `aborted`, 10 attempts.

The exact sources and digests for that observation were:

| Evidence | SHA-256 |
|---|---|
| `session.json` | `02de409747a78df80754898634eae7d3932785787d0aa7e8c2b85d77c39ec3c8` |
| `resume.json` | `d1480e6cd69f38f98877d08712cc3c8afcd462dce3712bad97b036072b0ab115` |
| `steps/0008.json` | `59e1d386610dd8c3f9f61ed202ccb5d8756c99355996bed35450b0fbc7237e73` |
| published harness request | `7c5232ad29db4ec3674845d726c89ce98be944389cf7cb3c3791109222e44417` |
| headless summary | `a6d635b791ee8265a6e6191eb5762e17dbaa3093a2170c78efc5c8c2a494d162` |
| agent-turn summary | `91d79f02f8ec6b2dae472575b58266fd157585b2a39ddebd492e7651c539257f` |

## Source Trace

The failure and repair cross two ownership boundaries:

```text
tool-loop writer
  -> session/resume/step JSON publication
  -> campaign tool-loop directory
  -> campaign inventory reader
  -> walk protocol response
  -> CLI and UI sibling clients
```

- `crates/ploke-eval/src/replay/tool_loop.rs` owns the stored session, resume,
  and checkpoint schemas. Its JSON writes now use
  `durable_io::write_atomic`, so a failed rewrite cannot truncate an already
  published file.
- `crates/ploke-eval/src/cli/prototype1_state/walk/llm_trace.rs` owns the new
  campaign inventory and exact-session projection. It catches manifest and
  resume failures per session, preserves them as typed `Invalid`, `Unreadable`,
  or `Missing` evidence, and continues collecting healthy siblings.
- That reader bounds the tool-loop root plus every session, resume, checkpoint,
  harness request, headless summary, and agent-turn source before reading.
  Parent traversal, symlink escape, cross-session identity, request-path,
  workspace, and response-index mismatches fail closed.
- `crates/ploke-eval/src/cli/prototype1_state/walk/server.rs` serves the typed
  index/detail outside the mutation-controller lock. Protocol v10 exposes the
  same carriers through public `WalkClient` methods.
- `llm sessions`, `llm observe`, and the `Live LLM` UI tab consume those typed
  carriers. The older cursor-oriented LLM debugger commands remain a separate
  legacy inspection surface and are not the canonical corrupt-neighbor-safe
  inventory.

## Docs/Policy Expectation

The operator-control plan requires CLI and UI to be sibling clients over one
source of truth, to preserve source identity, and to distinguish incomplete,
missing, invalid, and independently terminal evidence. It also requires the UI
to show the debugger trace and outer executor evidence without converting one
layer into another.

Silently skipping r15 would violate that policy because the corruption would
disappear. Aborting on r15 also violates it because healthy r16-r18 evidence
becomes inaccessible. The typed index must show both.

## Current Repro Coverage

Production-reader regressions now cover:

- `index_retains_valid_session_and_reports_invalid_neighbor`;
- `index_retains_unreadable_resume_and_healthy_sibling`;
- `index_reports_unreadable_manifest_without_hiding_sibling`;
- manifest and resume symlink escapes beside healthy siblings;
- exact checkpoint symlink escape;
- an exact trace with no published frontier returning no selected checkpoint;
- exact public-client response-coordinate mismatch;
- `egui_kittest` rendering of healthy and corrupt siblings plus exact
  session/checkpoint click coordinates.

Verification on the final source:

```text
cargo test -p ploke-eval llm_trace --lib -- --test-threads=1
18 passed; 0 failed

cargo test -p ploke-eval --lib -- --test-threads=1
1348 passed; 0 failed; 28 ignored

cargo test -p ploke-walk-ui
25 passed; 0 failed

cargo clippy -p ploke-walk-ui --all-targets --no-deps -- -D warnings
passed
```

## Missing Repro / Validation

The real r15-r18 files remain machine-local evidence. A bounded copy of that
incident has not yet been imported into repository test data and replayed
through the production reader. The current regressions reproduce each damaged
shape and the final live probe exercises the actual artifacts, but a durable
historical replay fixture is still needed so CI can join the zero-byte r15
neighbor with the real healthy r16-r18 sessions.

The native UI has production-carrier and `egui_kittest` coverage, but a manual
UI-only session connected to a live server, with screenshots, remains a Stage 5
acceptance task. Neither gap reopens the CLI/service contract fixed here.

## Fix Direction

Keep the repair at the writer and typed read-model boundaries:

- publish mutable JSON artifacts atomically;
- enumerate sessions independently and preserve per-artifact failures;
- select exact sessions and checkpoints by typed coordinates;
- bind every present artifact to its path and digest;
- bound all derived and primary evidence paths before reads;
- keep debugger, resume, headless, and agent-turn lifecycle states separate.

Do not make the decoder permissive, silently skip malformed sessions, infer a
latest representative, reinterpret absence as in flight, or weaken source and
identity validation to make the inventory appear healthy.

## Related Bugs

- [`2026-07-15-prototype1-parallel-trace-capture-cross-talk.md`](./2026-07-15-prototype1-parallel-trace-capture-cross-talk.md)
  records the session-ownership repair and live r16-r18 provenance sweep.
- [`2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md`](./2026-07-15-prototype1-timeout-cancellation-drops-trace-evidence.md)
  records preservation of partial trace evidence during cancellation.
- [`2026-07-14-walk-llm-inspection-controller-lock.md`](./2026-07-14-walk-llm-inspection-controller-lock.md)
  records why persisted LLM reads must not wait behind the mutation controller.
