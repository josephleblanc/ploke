Date: 20-05-26

**1. Rewind**
Use recorded provider responses through `RecordedResponseTape`, but only up to a cursor:

- run the real session loop;
- execute real tools;
- preserve real tool messages;
- apply/stage/deny exactly through current code;
- stop at a named breakpoint.

Breakpoints should be semantic, not just “turn 7”:

- before first protected-path attempt;
- before repeated `request_code_context` churn;
- before first same-file repair;
- before first malformed patch;
- after N reads with no edit;
- after first tool miss for `code_item_lookup`;
- after first staged-but-not-applied edit;
- after validation starts failing.

That lets the old nondeterministic trace become a way to reconstruct a plausible bad state.

**2. Inspect**
At the breakpoint, capture the exact thing the model is about to see next:

- full next request messages, bounded/summarized;
- tool definitions and descriptions;
- available read/write roots;
- protected paths;
- current proposal state;
- changed files;
- current workspace/index freshness;
- last K tool replies;
- a compact “why this breakpoint fired” explanation.

This is the key. If the model is about to fail, we want to know whether the environment is making failure likely.

**3. Advance**
Then continue one step at a time in one of two modes:

- `recorded-tail`: play the next historical provider response, but through current tools.
- `live-tail`: ask the provider for exactly one next assistant turn from the reconstructed state.

For each step, record not just pass/fail, but the loop quality signals:

- Did the model call an expected class of tool?
- Did a tool reply contain actionable recovery info?
- Did path policy deny early and clearly?
- Did tool descriptions match actual validator behavior?
- Did the model repeat a denied action?
- Did it switch from search to edit once enough evidence was available?
- Did a staged edit get represented as staged, not applied?
- Did validation failure become visible as candidate state?

This is closer to a debugger than a unit test.

I think the right artifact is an operator command plus a small deterministic harness test, not a normal broad live test. Something like:

```text
prototype1 replay-probe \
  --trace <headless-tui.json or typed record> \
  --stop-before search-thrash \
  --tail live \
  --steps 3 \
  --tool-budget 20 \
  --out <probe-dir>
```

The output should be a compact probe bundle:

```text
probe-summary.json
request-before-breakpoint.md
step-001-request.md
step-001-tool-events.json
step-001-assessment.md
workspace-diff.patch
```

The assessment can be mostly mechanical at first:

- `prompt_mentions_unreadable_path`
- `protected_path_not_predeclared`
- `tool_description_schema_mismatch`
- `repeated_same_denial`
- `search_without_new_information`
- `staged_result_replayed_as_success`
- `validation_failure_hidden_by_applied_terminal`
- `tool_reply_missing_recovery_hint`

That gives you short live tests that answer the real question: “are we presenting a good environment to the model from this state?” The model may still make a bad choice, and that is fine. What we want to eliminate is the framework quietly pushing it toward bad choices.
