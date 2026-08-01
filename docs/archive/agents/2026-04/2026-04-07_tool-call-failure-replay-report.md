# Tool Call Failure Replay Report

- date: 2026-04-07
- task title: Diagnose the recorded `apply_code_edit` replay failure
- task description: Reproduce the eval-run failure, inspect the target file and DB resolution path, and determine whether the observed `ToolCallFailed` came from tool-call plumbing or from a mismatch between the request shape and the database schema.
- related planning files:
  - /home/brasides/code/ploke/docs/active/agents/2026-04-07_tool-call-failure-replay-plan.md

## Executive Summary

The replayed failure is real, but it is not caused by the tool-call event bridge.

The recorded request targeted:

- file: `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs`
- canon: `crate::util::Replacer::replace_all`
- recorded node kind: `Function`

The database probe showed:

- the file exists in the snapshot DB
- `replace_all` does not exist as a primary `function` node
- `replace_all` does exist in the DB as a `method`

That means the failure is a schema / request-shape mismatch:

1. the tool request classified the item as `Function`
2. the DB stores it as `method`
3. the resolver correctly returned `No matching node found (strict+fallback)`

## Evidence

Replay test:

- `cargo test -p ploke-eval test_apply_code_edit_failure_path -- --ignored --nocapture`

Run-request fixture:

- [crates/ploke-eval/src/tests/fixtures/BurntSushi__ripgrep-2209_code_item_lookup_context.json](../../../crates/ploke-eval/src/tests/fixtures/BurntSushi__ripgrep-2209_code_item_lookup_context.json)
- sourced from [ploke_eval_20260407_164834_34623.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260407_164834_34623.log)
- `api_request` is stored as parsed JSON and includes the full model-facing request, including the `code_item_lookup` tool definition and its `node_kind` vocabulary

Observed replay output:

- strict resolve hits: `0`
- relaxed canon probes: `0`
- primary-node probe for the target file: no `replace_all` among primary nodes
- coarse name probe across primary+assoc relations: `replace_all` matched `method`

Source references:

- replay test: [crates/ploke-eval/src/tests/replay.rs](../../../crates/ploke-eval/src/tests/replay.rs)
- tool kind docs: [crates/ploke-tui/src/tools/code_item_lookup.rs](../../../crates/ploke-tui/src/tools/code_item_lookup.rs)
- matching tool docs: [crates/ploke-tui/src/tools/get_code_edges.rs](../../../crates/ploke-tui/src/tools/get_code_edges.rs)
- tool list shown to the model: [crates/ploke-tui/src/app/view/rendering/highlight.rs](../../../crates/ploke-tui/src/app/view/rendering/highlight.rs)

## Interpretation

The most likely cause is that the model was not given `method` as a valid node kind when it selected the request, so it fell back to `function`.

That is consistent with the current tool descriptions, which list:

- `function`
- `const`
- `enum`
- `impl`
- `import`
- `macro`
- `module`
- `static`
- `struct`
- `trait`
- `type_alias`
- `union`

but do not mention `method`.

## Conclusion

This failure looks like a tool-definition / classification issue, not a broken replay harness and not a missing file in the DB.

The next likely fix is to add `method` to the tool-facing node-kind vocabulary and then re-test the replay to see whether the model or request generation changes accordingly.
