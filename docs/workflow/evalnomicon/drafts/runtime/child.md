# Prototype 1 Child Runtime Execution Path

Recorded: 2026-05-03

Status: working draft for understanding Prototype 1 child execution and
timeout behavior. This is intentionally written as a walkthrough first, so it
can later become module-level or command-level documentation.

## Purpose

This document follows the path from a parent running `loop prototype1-state` to
a child runtime finishing evaluation and handing control back to the parent.

The immediate goal is to make timeout behavior understandable. The important
distinction is that the long wall-clock times seen in multi-generation runs are
not caused by one single long timeout. They emerge from a parent wait loop
around a child runtime, an agent loop inside the child, and an HTTP retry loop
inside each chat step.

## Execution Path

### 1. Parent Runtime Enters `prototype1-state`

Operator command:

```bash
ploke-eval --debug-tools loop prototype1-state --repo-root .
```

Relevant entry points:

- `crates/ploke-eval/src/cli.rs`
  - `LoopSubcommand`
  - `Prototype1StateCommand`
  - `LoopCommand::run`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `Prototype1StateCommand::run`
  - `run_turn`

The active checkout is treated as a parent runtime over the current artifact.
The parent turn either acknowledges an existing successor handoff or starts from
the campaign's current parent identity and history state.

For a parent at generation `k`, the state command resolves a runnable direct
child at generation `k + 1`. If no runnable child exists yet, target selection
stages one.

Relevant code:

- `run_parent_target_selection`
- `receive_existing_child_plan`
- `runnable_candidate_nodes`
- `resolve_next_child`
- `SelectedChild::load_c1`

### 2. Parent Advances Child Through C1 -> C4

The typed child path is:

```text
C1 materialize child artifact
-> C2 build child binary
-> C3 spawn child runtime
-> C4 observe child result
-> C5 observed child result
```

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `MaterializeBranch::transition`
- `crates/ploke-eval/src/cli/prototype1_state/c2.rs`
  - `BuildChild::transition`
- `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
  - `SpawnChild::transition`
  - `wait_for_ready`
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
  - `ObserveChild::transition`

Build/check failures mark the node failed. There is no visible build retry loop
in this path.

### 3. Parent Spawns the Child Runtime

`SpawnChild::transition` writes a `ChildInvocation`, launches:

```text
loop prototype1-runner --invocation <path> --execute
```

and redirects child stdout/stderr to the node runtime stream files.

The parent then waits for the child to record readiness in the transition
journal.

Current pre-ready behavior:

- parent polls the transition journal for `Child<Ready>`
- parent also polls child process status with `try_wait`
- child exit before ready rejects the spawn and marks the node failed
- ready timeout rejects the spawn and marks the node failed
- polling interval is short, currently around tens of milliseconds

This is a bounded handshake.

Open concern: the ready-timeout path marks the node failed, but the discovery
pass did not show the same explicit child kill/wait behavior used by successor
ready timeout. That should be checked before changing policy.

### 4. Child Runner Records Ready, Evaluates, Writes Result

Inside the spawned child process, the runner:

```text
record Child<Ready>
record Child<Evaluating>
run treatment branch evaluation
write runner result
record Child<ResultWritten>
exit
```

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_state/child.rs`
  - `Child<Starting>::ready`
  - `Child<Ready>::evaluating`
  - `Child<Evaluating>::result_written`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - `record_prototype1_child_ready`
  - `execute_prototype1_runner_invocation`
  - `run_prototype1_branch_evaluation`
  - runner-result constructors

The child runner currently records one runner result for the attempt. Successful
branch evaluation becomes `Prototype1RunnerDisposition::Succeeded`. Evaluation
failure becomes a non-success runner disposition such as `TreatmentFailed`.

Provider and agent timeouts are not handled by the parent scheduler directly.
They occur inside the child runner's evaluation work and surface back only if
the child writes a runner result.

### 5. Parent Observes the Child Result

After the child has reached ready, `ObserveChild::transition` waits for the
result path recorded in the transition journal:

```text
loop:
  if child result path exists:
    load runner result
    load evaluation report when runner succeeded
    append ObserveChild after-entry with observed result
    return
  sleep RESULT_POLL
```

This is the main parent-side waiting span behind the monitor's child observe
time.

In the current code, the "observed result" is recorded in the parent
`PrototypeJournal`, not directly in the scheduler node record. `c4.rs` builds an
`ObservedChildResult` and appends:

```text
JournalEntry::ObserveChild(
  CompletionEntry {
    phase: CommitPhase::After,
    child_lifecycle: ChildRuntimeLifecycle::Terminated,
    result: Some(ObservedChildResult::...)
  }
)
```

Relevant code:

- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
  - `completion_entry`
  - `ObserveChild::transition`
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs`
  - `ObservedChildResult`
  - `CompletionEntry`

Current post-ready behavior:

- parent waits for `Child<ResultWritten>`
- parent sleeps between polls
- no post-ready observation deadline was visible in the discovery pass
- no child `try_wait` check was visible in this observation loop

Consequence: if the child reaches ready, then exits or stops making progress
before writing a runner result, the parent can wait indefinitely. If the child
is alive but slow because of provider retries, the parent waits for the full
duration.

This explains why long provider waits show up as long parent observe spans.

### 6. Parent Classifies the Observed Child

Once the runner result exists:

- non-success runner disposition becomes an observed failed child
- success loads the evaluation artifact and branch disposition
- the parent records an after-entry for `ObserveChild`

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
  - `ObserveChild::transition`
- `crates/ploke-eval/src/intervention/scheduler.rs`
  - `Prototype1NodeStatus`
  - `Prototype1RunnerDisposition`
  - `update_node_status`
  - `decide_continuation`

Current continuation behavior has a temporary permissive path: completed child
evaluations can be successor-eligible even when the branch evaluation rejects
the candidate. That selection-policy issue is separate from timeout behavior.

### 7. Parent Starts Successor Runtime

If continuation says to proceed, the parent spawns the selected successor
runtime.

Successor handoff has a bounded ready wait. Unlike the child-ready path, the
discovery pass found that successor ready timeout kills and waits the successor
process before returning no successor.

Relevant files:

- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - successor wait and handoff helpers
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
  - `Role`
  - `InvocationAuthority`
  - `successor_parent_argv`

## Timeout Behavior Along This Path

### Parent/Child Runtime Timeouts

Current visible parent-side timeouts:

- child ready timeout before `Child<Ready>`
- successor ready timeout before successor parent handoff

Current missing or unclear parent-side timeouts:

- no visible deadline for `Child<Ready> -> Child<ResultWritten>`
- no visible child process status check while observing child result
- unclear child cleanup on child-ready timeout

The missing post-ready deadline is important for reliability. A long provider
wait should be allowed only up to an explicit child-evaluation budget. A child
runtime that exits or stops making progress should become a durable failed
child result, not an infinite parent wait.

The timeout values should not be scattered as unrelated constants. A policy
that uses hardcoded numbers at each wait point would make it too easy to tune
one layer while accidentally changing the overall campaign budget. The shape we
probably want is a shared timeout policy object that is passed or projected into
the layers that need it.

Possible policy surface:

```text
Prototype1TimeoutPolicy
  child_ready_timeout
  child_result_timeout
  successor_ready_timeout
  eval_run_timeout
  llm_http_attempt_timeout
  llm_http_max_attempts
  chat_step_retry_limit
  chat_step_retry_delay
  tool_call_timeout
  tool_chain_limit
```

The exact struct and ownership boundary are still open. The important point is
that parent runtime waits, child evaluation budgets, TUI chat retries, and LLM
HTTP attempts should be derived from one coherent policy rather than adjusted
independently.

### Agent Loop Timeouts

The child runner's evaluation enters the TUI/agent chat loop.

Relevant files:

- `crates/ploke-tui/src/llm/manager/session.rs`
  - `run_chat_session`
  - `TuiToolPolicy`
  - `TuiTimeoutPolicy`
  - `FinishPolicy`
- `crates/ploke-tui/src/llm/manager/loop_error.rs`
  - `classify_llm_error`
- `crates/ploke-tui/src/user_config.rs`
  - `llm_timeout_secs`
  - `ChatPolicy`

Important current behavior:

- one agent session can call `ploke_llm::chat_step` many times
- tool-call continuations are bounded by `tool_call_chain_limit`
- request errors classified as retryable can retry the whole chat step
- default `error_retry_limit` is currently `2`
- timeout retry delay uses chat timeout policy, defaulting to a fixed retry
  shape with a base duration around `30s`

This means the TUI layer can retry after the LLM layer has already exhausted
its own HTTP attempts.

### LLM HTTP Attempt Timeouts

The lowest layer is `ploke-llm`.

Relevant files:

- `crates/ploke-llm/src/lib.rs`
  - `LLM_TIMEOUT_SECS`
- `crates/ploke-llm/src/manager/session.rs`
  - `ChatHttpConfig`
  - `chat_step`
  - `should_retry_send_error`
  - `should_retry_body_failure`
  - `should_retry_status`
  - `compute_retry_backoff`
- `crates/ploke-llm/src/error.rs`
  - `HttpFailure`
  - `HttpPhase`
  - `HttpSendFailure`
  - `HttpReceiveFailure`
  - `HttpBodyFailure`

Current default `ChatHttpConfig`:

```text
timeout: 120s
max_attempts: 3
initial_backoff: 250ms
max_backoff: 2s
```

`chat_step` applies the timeout to each HTTP request attempt:

```text
for attempt in 1..=max_attempts:
  POST request with reqwest timeout
  read response body
  retry retryable send/body/status failures
```

Retryable cases include:

- send timeout
- connect/request/body send errors
- response body timeout
- response body read failure
- statuses `408`, `409`, `425`, `429`, `500`, `502`, `503`, `504`

Non-retryable body case:

- response body decode failure

The important multiplication:

```text
one chat_step ~= up to 3 HTTP attempts * 120s
TUI may retry the whole chat_step up to 2 more times
one agent session may run many chat_steps because of tool-call continuation
parent observe waits for the whole child evaluation
```

So there does not need to be a single configured 30-minute timeout for a node
to take 30 minutes. The wall time can emerge from nested bounded retries.

### Inefficient Timeout Examples

Example 1: repeated body timeouts during tool-driven evaluation.

Suppose the child process reaches `Child<Evaluating>` and starts
self-evaluation. The agent asks the model for a patch plan, receives tool calls,
executes them, appends tool results, and asks the model again. If this happens
across `X` chat steps, each step can pay its own HTTP attempt budget.

For one `chat_step`:

```text
attempt 1: response headers arrive, body read stalls until 120s timeout
attempt 2: response headers arrive, body read stalls until 120s timeout
attempt 3: response succeeds after 80s
```

That single chat step costs about `320s` even though it eventually succeeds.
Because it succeeds, the TUI outer request-error retry does not see a terminal
error. The agent continues to the next tool cycle. If this pattern happens five
times in one child evaluation, provider waiting alone can exceed `25min`.

Example 2: exhausted HTTP attempts followed by TUI retry.

For one `chat_step`:

```text
ploke-llm attempt 1: 120s timeout
ploke-llm attempt 2: 120s timeout
ploke-llm attempt 3: 120s timeout
ploke-tui classifies HTTP_BODY_TIMEOUT as retryable
ploke-tui sleeps according to chat retry policy
ploke-tui retries the whole chat_step
```

With the current defaults, the TUI layer can do this whole-chat-step retry up
to `error_retry_limit = 2` times. That can turn one logical model turn into
roughly:

```text
3 HTTP attempts * 120s
+ retry delay
+ 3 HTTP attempts * 120s
+ retry delay
+ 3 HTTP attempts * 120s
```

This is inefficient but bounded at the LLM/TUI layers. The parent observe loop
then waits for the whole duration because it has no separate
`Child<ResultWritten>` deadline.

Example 3: parent wait with no result record.

The parent sees `Child<Ready>` and enters `ObserveChild::transition`. If the
child process exits after ready, loses the ability to append
`Child<ResultWritten>`, or blocks forever in evaluation, the current parent loop
keeps polling the journal for a result path. Because that loop does not visibly
check child process status or a result deadline, this is a reliability risk even
when lower-level HTTP and chat-step retries are bounded.

## Timeout Questions To Decide

These questions should be answered before changing defaults:

1. Is `llm_timeout_secs` meant to be per HTTP attempt, per chat step, or a
   user-visible wall-clock budget?
2. Should `ploke-tui` retry a chat step after `ploke-llm` already exhausted
   HTTP attempts?
3. Should body/read timeout be retried the same way as send/connect timeout?
4. Should timeout exhaustion produce a distinct terminal reason from semantic
   branch rejection?
5. What deadline should bound `Child<Ready> -> Child<ResultWritten>`?
6. Should child-ready timeout kill and wait the child process, matching
   successor-ready timeout behavior?
7. Which timeout facts must be stored as durable run evidence rather than
   reconstructed from trace or stderr logs?

## Working Policy Direction

This section is provisional.

The likely target behavior is:

- keep provider HTTP attempt retries small and explicit
- keep TUI whole-chat-step retries separate from provider attempt retries
- add a hard child-evaluation budget around `Child<Evaluating>`
- make timeout exhaustion a typed failed-evaluation reason
- keep timeout failure separate from branch rejection
- record timeout class, retry count, elapsed time, and final terminal reason
  in durable run evidence
- make monitor timing report those records instead of inferring them from log
  text

That would let long runs continue reliably without hiding inefficient provider
waits inside broad `agent`, `run`, or `observe` spans.
