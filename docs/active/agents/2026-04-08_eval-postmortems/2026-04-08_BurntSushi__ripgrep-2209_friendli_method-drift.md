# BurntSushi ripgrep-2209 Friendli Method Drift

- date: 2026-04-08
- task title: BurntSushi__ripgrep-2209 Failed Eval Postmortem
- task description: Document the failed `run-msb-agent-single` investigation where the model correctly diagnosed the multiline replacement issue but drifted into a compile-breaking refactor after tool friction.
- related planning files:
  - [2026-04-08_postmortem-plan.md](/home/brasides/code/ploke/docs/active/agents/2026-04-08_eval-postmortems/2026-04-08_postmortem-plan.md)

## Header

- run id: timestamped log `ploke_eval_20260408_040518_296310.log`
- instance: `BurntSushi__ripgrep-2209`
- model: `minimax/minimax-m2.5`
- provider: `friendli`
- repository: `BurntSushi/ripgrep`
- base sha: `4dc6c73c5a9203c5a8a89ce2161feca542329812`
- stable evidence source: [ploke_eval_20260408_040518_296310.log](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log)

## Outcome Snapshot

- final runner state: run artifacts were written, but the current run directory later became mixed with a newer run and was not trustworthy for this investigation
- final chat outcome: chat loop eventually reported completion despite accumulated tool errors
- primary user-visible failure: compile failure in `grep-printer` after an unnecessary refactor of `Replacer::replace_all`
- did the model produce a patch: yes
- did the target file change: yes

## Failure Classification

- primary category: `model-drift`
- secondary category: `tool-retry-friction`
- confidence: medium-high

## Timeline

1. Initial diagnosis:
   The model correctly recognized that multiline replacement lacked the same `m.start() >= range.end` guard already present in `find_iter_at_in_context`.
2. First meaningful tool failure:
   `code_item_lookup` repeatedly failed to locate `replace_with_captures_at` in `crates/matcher/src/lib.rs`, alternating between `method` and `function` retries without finding the real target.
3. First edit proposal:
   Instead of inserting the missing guard into `replace_all`, the model proposed a new helper method `replace_all_in_range` and changed the algorithm to use `captures_iter_at`.
4. First compile failure:
   The new helper introduced generic shadowing, broke existing callers expecting `replace_all`, and created a closure type-inference problem.
5. End-of-run state:
   The run never recovered to the minimal patch and ended with a compile-broken workspace.

## Evidence

### Correct Local Reasoning

The model was locally coherent at first. It correctly summarized the issue as a missing replacement-path equivalent of the `range.end` filter already present in `find_iter_at_in_context`.

Relevant evidence:

- diagnosis and tool call pivot at [ploke_eval_20260408_040518_296310.log#L17277](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L17277)

### Tool Friction

The biggest repeated friction point was `code_item_lookup` around `replace_with_captures_at`. The recovery hints alternated between "try method" and "try function" without guiding the model toward a better lookup strategy or a direct read of the trait definition.

Relevant evidence:

- first `method` lookup failure at [ploke_eval_20260408_040518_296310.log#L7519](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L7519)
- subsequent `function` lookup failure at [ploke_eval_20260408_040518_296310.log#L7561](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L7561)
- ambiguous semantic target for `crate::util::Replacer::new` at [ploke_eval_20260408_040518_296310.log#L26435](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L26435)
- repeated malformed `non_semantic_patch` payloads with `"test"` instead of a diff at [ploke_eval_20260408_040518_296310.log#L11808](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L11808)

### Model Mistake

The crucial mistake was not misunderstanding the bug. The crucial mistake was changing solution shape.

The correct change was to keep `replace_all` and insert the same range-end rejection logic already used in `find_iter_at_in_context`. Instead, the model invented a new method, changed the internal iteration strategy, and implicitly changed the call contract.

Relevant evidence:

- speculative refactor proposal at [ploke_eval_20260408_040518_296310.log#L17277](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L17277)
- readback showing the inserted `replace_all_in_range` method at [ploke_eval_20260408_040518_296310.log#L31080](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L31080)

### Compile-Broken End State

The compile failure was a direct consequence of that refactor:

- generic parameter shadowing: `E0403`
- caller breakage because `replace_all` no longer existed: `E0599`
- closure type inference failure in the new code path: `E0282`

Relevant evidence:

- cargo test failure at [ploke_eval_20260408_040518_296310.log#L31288](/home/brasides/.ploke-eval/logs/ploke_eval_20260408_040518_296310.log#L31288)

## Minimal Correct Fix

Keep `Replacer::replace_all` in place and add a guard in the existing replacement callback that rejects matches whose overall match starts at or after `range.end`.

This is a local edit inside the existing method, not a new helper, not a call-site migration, and not a change from `replace_with_captures_at` to a new iteration API.

## Open Questions

- Tool-design questions:
  - Are the `method` vs `function` retry hints for `code_item_lookup` creating oscillation instead of helping the model choose a more grounded follow-up action?
  - Should failed lookup hints recommend `read_file` or `request_code_context` on the defining trait when the target is likely provided by a trait?
- semantic editing capability questions:
  - Does `apply_code_edit` have a first-class way to insert a new child node under an existing impl or module without replacing an existing canonical target?
  - If not, are models implicitly forced toward unsafe replacements when they actually want insertion semantics?
  - What are the intended move/delete semantics for semantic edits under auto-approval?
- runner or artifact questions:
  - Should eval postmortems prefer timestamped logs over run-directory artifacts whenever a second run can overwrite shared files?

## Follow-Up Actions

- instrumentation:
  - Capture a clearer distinction between "first recovered tool error" and "first terminal compile/test failure."
- tool UX:
  - Revisit `code_item_lookup` recovery messaging for trait-provided methods.
  - Revisit whether `apply_code_edit` needs explicit insertion affordances.
- runner artifact changes:
  - Consider per-run immutable copies for all conversation artifacts when repeated runs reuse the same instance directory.
- regression tests:
  - Add replay coverage for this exact drift pattern so future tool changes can be measured against it.
