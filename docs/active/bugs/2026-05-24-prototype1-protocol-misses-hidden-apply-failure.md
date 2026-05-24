# 2026-05-24 Prototype 1 Protocol Misses Hidden Apply Failure

## Summary

Prototype 1 protocol adjudication can mark an edit segment as successful when
the model-facing and protocol-facing tool summary reports only proposal staging,
even though the recorded trace later contains an apply failure for the same edit
intent. In the observed run, this made protocol output look green while the
exported benchmark patch was behaviorally incomplete.

This is downstream of the staged proposal lifecycle bug documented in
`2026-05-17-headless-tui-staged-proposal-tool-result-lifecycle.md`, but it has a
separate scoring/adjudication impact: the protocol layer currently lacks enough
final edit/admission evidence to distinguish "staged" from "actually changed
the intended behavior."

## Concrete Run

- Campaign: `p1-gemini35-flash-direct-fresh-20260524-163447`
- Instance: `BurntSushi__ripgrep-2209`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-163447/BurntSushi__ripgrep-2209/runs/run-1779665713181-structured-current-policy-efa0a063`
- Review:
  `docs/active/agents/run-reviews/2026-05-24-p1-gemini35-flash-direct-fresh-20260524-163447-eval.md`

Closure and protocol both completed:

```json
"eval": { "complete_total": 1, "status": "complete" },
"protocol": { "full_total": 1, "status": "complete" }
```

The protocol status CLI also reported complete coverage:

```json
{
  "tool_calls_total": 95,
  "call_review_count": 95,
  "segment_review_count": 7,
  "next_step": { "kind": "complete" }
}
```

## Failure Shape

The model inserted a new helper:

```text
replace_with_captures_at_limited
```

Then it attempted to update `crate::util::Replacer::replace_all` to call that
helper. The recorded trace contains the terminal apply failure:

```json
{
  "applied": 0,
  "ok": false,
  "results": [
    {
      "error": "Content changed for \"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs\""
    }
  ]
}
```

However, the model-facing and protocol-facing summary for the same edit path
showed only the staged result:

```json
{
  "ok": true,
  "staged": 1,
  "applied": 0,
  "files": ["crates/printer/src/util.rs"],
  "preview_mode": "codeblock",
  "auto_confirmed": true
}
```

The final submitted patch added the helper and a passing test, but did not wire
`Replacer::replace_all` to call the helper. Direct checkout search found the
new helper definition and the old existing call to `replace_with_captures_at`,
not a call to the new limited helper.

## Protocol Blind Spot

Segment review for the edit segment reported:

```text
segment=3 label=edit_attempt overall=focused_progress
usefulness=key_progress redundancy=distinct recoverability=no_recovery_needed
```

Its rationale said both edit tools completed successfully and that there were
no failed calls or signs of struggle. That was false relative to the recorded
trace and final diff.

The protocol artifact appears to have evaluated the summarized tool packet,
where the `apply_code_edit` entry was:

```text
tool=apply_code_edit status=completed ... result={"ok":true,"staged":1,"applied":0,...}
```

It did not see or reason over the later terminal apply failure, nor did it
validate the final patch semantics.

## Validity Impact

This run is a false positive for loop evidence:

- closure marks eval complete;
- protocol marks all required procedures complete;
- `benchmark-patch-projection.json` exports a non-empty patch;
- the target checkout is modified;
- but the intended behavioral edit is absent.

If successor selection treats this as strong candidate evidence, the loop can
reward a candidate that only added an unused helper and a weak passing test.

## Expected Behavior

Protocol/adjudication should be able to answer:

- Did each edit proposal merely stage, or did it apply?
- Did a later admission/apply failure supersede an earlier staged result?
- Did the final diff wire newly added helper code into the changed behavior?
- Did final validation target the changed package/surface rather than an
  unrelated focused manifest?

A segment containing a hidden failed apply should not be classified as
`no_recovery_needed` unless the model saw the failure and demonstrably repaired
it.

## Fix Direction

1. Feed protocol procedures final proposal/admission/apply lifecycle evidence,
   not just first-stage tool summaries.
2. Treat `staged:1, applied:0` as non-terminal or at least ambiguous for
   headless eval adjudication.
3. Add an adjudication field for final patch semantic wiring, especially when a
   new helper/function is introduced.
4. Add a regression/replay for this run that asserts the protocol does not mark
   the edit segment as fully recovered when the final apply failure was hidden
   and the final patch omits the intended behavioral call-site change.

