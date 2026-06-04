# Incomplete child-state note: p1-gemini35-flash-direct-3g2x3-par2-20260602-131345 post-exit requests

## Short verdict

This is an incomplete-state note, not a successful run review. A read-only process scan after reviewing `node-69dd9bb1de784313-r2` found no live `ploke-eval` / campaign process. The request directory contains 18 broad-harness request JSON files, but only six paired `*.headless-tui.json` results exist. The remaining 12 request slots have no completed headless result, no submitted-result JSON, and no paired turn-live trace.

One abandoned slot is materialized and dirty: `node-69dd9bb1de784313` has a candidate workspace at HEAD `cb2a7136` with a one-file diff in `crates/ploke-io/src/write.rs` (228 insertions, 6 deletions). Because there is no result/trace/submission for that slot, this note cannot reconstruct a model/tool lifecycle or classify the diff as a valid candidate.

## Evidence roots

- Request dir: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-request`
- Result dir: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/messages/edit-harness-result`
- Workspace root: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness`
- Dirty abandoned workspace: `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-3g2x3-par2-20260602-131345/prototype1/workspaces/edit-harness/node-69dd9bb1de784313`
- Campaign run profile / closure: `prototype1/run-profile.toml`, campaign `closure-state.json`

## Post-exit process check

A `/proc` command-line scan excluding the review process reported `matches=0` for `ploke-eval` and the campaign id. This is the condition that turns request-only slots from “still possibly live” into incomplete/abandoned state for review purposes.

## Completed results now accounted for

Completed `*.headless-tui.json` result slots:

- `node-26f01da56959fd47`
- `node-26f01da56959fd47-r2`
- `node-26f01da56959fd47-r3`
- `node-26f01da56959fd47-r4`
- `node-26f01da56959fd47-r5`
- `node-69dd9bb1de784313-r2`

After the `node-69dd9bb1de784313-r2` review, all completed result files observed in the result directory have a corresponding run-review report.

## Request slots with no completed headless result

The following request JSON slots have no paired `*.headless-tui.json`, no submitted-result JSON, and no `*.turn-live/agent-turn-trace.json`:

- `node-26f01da56959fd47-r6`
- `node-26f01da56959fd47-r7`
- `node-26f01da56959fd47-r8`
- `node-26f01da56959fd47-r9`
- `node-69dd9bb1de784313`
- `node-69dd9bb1de784313-r3`
- `node-69dd9bb1de784313-r4`
- `node-69dd9bb1de784313-r5`
- `node-69dd9bb1de784313-r6`
- `node-69dd9bb1de784313-r7`
- `node-69dd9bb1de784313-r8`
- `node-69dd9bb1de784313-r9`

Workspace inventory for those incomplete slots is almost empty. The four `node-26f01da56959fd47-r6` through `r9` slots and `node-69dd9bb1de784313-r3` through `r9` have no materialized workspace under `workspaces/edit-harness/` at review time.

## Materialized dirty workspace without result

`node-69dd9bb1de784313` is the only incomplete slot with a materialized workspace. Git evidence:

```text
HEAD: cb2a7136
status: M crates/ploke-io/src/write.rs
diff stat: crates/ploke-io/src/write.rs | 234 +++++++++++++++++++++++++++++++++++++++++--
          1 file changed, 228 insertions(+), 6 deletions(-)
```

The diff rewrites `write_snippets_batch` to group write requests by normalized path, verify all expected file hashes against the initial content, apply grouped splices from back to front, write through a temporary file, rename, best-effort fsync the parent directory, and fill each result slot. That may be an attempted same-file batch-write repair, but without `agent-turn-trace.json`, submitted result, terminal record, or validation rows it is not possible to prove:

- which model/tool calls produced the diff;
- whether the diff passed or failed the broad-harness protected-surface policy;
- whether the requested `ploke-eval` validation commands ran;
- whether a result was submitted or admitted; or
- whether the diff improved descendant benchmark behavior.

## Missing artifact joins

For each incomplete slot, the missing first-class artifacts are the same core set:

- no `messages/edit-harness-result/<slot>.headless-tui.json`;
- no `messages/edit-harness-result/<slot>.json` submitted result;
- no `messages/edit-harness-result/<slot>.turn-live/agent-turn-trace.json` or summary;
- no result-to-commit/submission/admission join;
- no branch evaluation attributable to the slot.

For `node-69dd9bb1de784313`, the dirty workspace adds one more gap: there is a checkout diff but no trace or terminal record that can explain whether it is a stale partial edit, a timed-out attempt, or an abandoned unpublished candidate.

## Action items

1. **Preserve or quarantine dirty workspace before cleanup:** `workspaces/edit-harness/node-69dd9bb1de784313` contains a substantial `ploke-io/src/write.rs` diff with no result record. Do not admit it as a candidate, but do not delete it without deciding whether the diff is useful forensic evidence.
2. **Mark request-only slots abandoned:** downstream status should distinguish these 12 request slots from completed no-candidate turns. They were published requests, but no headless result or trace exists after the live process exited.
3. **Require workspace/result join for materialized attempts:** if a workspace is created and mutated, the harness should persist at least a terminal/incomplete record naming the slot, workspace HEAD, changed paths, and reason no submitted result exists.
4. **Avoid fan-in overclaiming:** this note proves incomplete state only. It does not evaluate the abandoned `ploke-io` diff or synthesize child-campaign performance.
