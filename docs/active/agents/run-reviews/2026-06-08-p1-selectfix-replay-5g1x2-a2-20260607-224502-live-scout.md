# Live scout review: p1-selectfix-replay-5g1x2-a2-20260607-224502

Status: live/incomplete scout report. Inspection cutoff: `2026-06-08T00:14:47-07:00`.

This is a run-scoped review for the active campaign. I did not run
`prototype1-state`, `prototype1-step`, or `prototype1-continue`; inspection used
read-only process and artifact commands plus the read-only run-summary and trace
audit helpers.

## Short verdict

The campaign was live at the cutoff. Host process evidence showed PID `1585253`
running:

```text
/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502/target/debug/ploke-eval loop prototype1-state
  --campaign p1-selectfix-replay-5g1x2-a2-20260607-224502
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502
  --handoff-invocation /home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-e0215ac6e2825a28/invocations/a720628b-478f-45df-8838-9e76dd838388.json
  --stop-after complete --format json
```

Persisted state proves that the loop selected `node-e0215ac6e2825a28` as the
next parent from rejected branch `branch-01ade840a0a918b6`, installed commit
`06390ff4d01022272d78a690599a4eef5247c0fa` into the active worktree, spawned a
new successor process, and published request files for the next broad slots.
It does not yet prove that the live parent produced a child plan, submitted
results, or descendant evaluations.

Mechanically, the selected predecessor branch is trace-bearing and evaluable:
its treatment run wrote `record.json.gz`, `llm-full-responses.jsonl`,
`validation-audit.json`, `benchmark-patch-projection.json`,
`multi-swe-bench-submission.jsonl`, and full protocol review artifacts.
Benchmark usefulness is weaker: the branch was rejected by operational metrics
because same-file patch retry count and max streak regressed, even though it
reduced failed tool calls.

## Evidence roots checked

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Active worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Instance root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Run profile:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/run-profile.toml`
- Transition journal:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/transition-journal.jsonl`
- Branch registry and evaluations:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/branches.json`
  and `prototype1/evaluations/*.json`
- Selected branch run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/treatments/branch-01ade840a0a918b6/instances/BurntSushi__ripgrep-2209/runs/run-1780901885639-structured-current-policy-9f8a8db1`
- Selected branch protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/treatments/branch-01ade840a0a918b6/instances/BurntSushi__ripgrep-2209/runs/run-1780901885639-structured-current-policy-9f8a8db1`
- Latest live parent node:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-e0215ac6e2825a28`
- Latest live parent broad requests:
  `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/messages/edit-harness-request/node-e0215ac6e2825a28*.json`

Read-only helpers used:

```text
python3 .codex/skills/prototype1-loop-run-status/scripts/prototype1_run_summary.py --campaign p1-selectfix-replay-5g1x2-a2-20260607-224502
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/treatments/branch-01ade840a0a918b6/instances/BurntSushi__ripgrep-2209/runs/run-1780901885639-structured-current-policy-9f8a8db1 \
  --markdown
```

## Exact execution paths

### Active Prototype 1 path

```text
host process PID 1585253
-> ploke-eval loop prototype1-state --handoff-invocation .../node-e0215ac6e2825a28/invocations/a720628b-478f-45df-8838-9e76dd838388.json
-> transition-journal successor_handoff
-> node-e0215ac6e2825a28 child-to-parent ready record
-> edit-harness-request/node-e0215ac6e2825a28{,-r2,-r3,-r4}.json
```

The transition journal mtime was `2026-06-08 00:09:57 -0700`; the live parent
stdout stream mtime was `2026-06-08 00:11:58 -0700`. At the cutoff, no
`prototype1/messages/edit-harness-result/node-e0215ac6e2825a28*.json` files
existed and `prototype1/messages/child-plan/` had no
`node-e0215ac6e2825a28.json`.

### Selected predecessor path

```text
broad-harness request node-f21de5ba2e927ab0-r2
-> headless ploke-tui adapter result applies proposal 95625960-39a0-5158-ba35-a70457228c61
-> active worktree installs git commit 06390ff4d01022272d78a690599a4eef5247c0fa
-> treatment eval run run-1780901885639-structured-current-policy-9f8a8db1
-> run_benchmark_turn / agent-single-turn
-> validation-audit.json + benchmark-patch-projection.json + multi-swe-bench-submission.jsonl
-> protocol tool-call-review and segment-review artifacts
-> branch evaluation rejects branch-01ade840a0a918b6
-> selection samples rejected branch as next parent under explore_from_rejected
```

The selected treatment run's `execution-log.json` records
`selected_model = google/gemini-3.5-flash`, `selected_provider = google`, and
steps through `benchmark_turn_completed`, `write_validation_audit`,
`write_msb_submission`, and `write_benchmark_patch_projection`.

## Concrete trace chain

The selected predecessor branch has a full model/tool/protocol chain:

```text
headless broad attempt node-f21de5ba2e927ab0-r2
  -> direct_google route applies proposal 95625960-39a0-5158-ba35-a70457228c61
  -> changed Ploke files:
       crates/ploke-tui/src/rag/utils.rs
       crates/ploke-tui/src/tools/code_item_lookup.rs
       crates/ploke-tui/src/tools/get_code_edges.rs
  -> active worktree commit 06390ff4 verifies the same three-file diff
  -> treatment run branch-01ade840a0a918b6 emits 39 LLM responses / 38 provider tool calls
  -> trace audit reports 38 recorded tool calls and no missing provider call ids
  -> validation audit records final focused cargo test against grep-printer as ok
  -> protocol call review 1780902338431 marks call 37 cargo test as focused_progress/key_progress
  -> branch evaluation rejects the branch because same_file_patch_retry_count regressed 1 -> 2 and same_file_patch_max_streak regressed 2 -> 3
  -> selection still chooses it as next parent under score_child_prop/explore_from_rejected
```

Verification against the checkout: the original broad candidate workspace
`prototype1/workspaces/edit-harness/node-f21de5ba2e927ab0-r2` no longer exists,
but `git show --stat 06390ff4d01022272d78a690599a4eef5247c0fa` in the active
worktree verifies the committed broad result:

```text
crates/ploke-tui/src/rag/utils.rs              | 5 ++---
crates/ploke-tui/src/tools/code_item_lookup.rs | 6 ++++++
crates/ploke-tui/src/tools/get_code_edges.rs   | 6 ++++++
3 files changed, 14 insertions(+), 3 deletions(-)
```

This is a record present/manual join case: the submitted result still names the
now-removed candidate workspace, so durable verification has to join the
submitted result to the installed commit.

## Mechanical completion versus benchmark usefulness

Mechanical completion that is real:

- The campaign is configured for `google/gemini-3.5-flash` on `direct_google`;
  protocol uses `google/gemini-2.5-flash` on `direct_google` with
  `max_tokens = 8000`.
- The selected treatment run has `llm-full-responses.jsonl` with 39 responses.
- Trace audit over the selected run root reported 38 provider-emitted tool calls,
  38 recorded tool calls, zero missing recorded provider call ids, and one final
  stop response.
- `benchmark-patch-projection.json` has `check.status = passed`,
  `line_count = 120`, and a non-empty Multi-SWE-bench submission.
- `validation-audit.json` records final `cargo test` ok with
  `covers_changed_files = true` for the focused `grep-printer` manifest.
- Protocol for `branch-01ade840a0a918b6` wrote 51 artifacts: 1 intent
  segmentation, 38 tool-call reviews, and 12 segment reviews.

Benchmark usefulness that remains limited:

- The treatment run is mechanically trace-bearing but branch evaluation rejects
  it. It improved `tool_calls_failed` from 3 to 1, but regressed
  `same_file_patch_retry_count` from 1 to 2 and `same_file_patch_max_streak`
  from 2 to 3.
- Protocol review calls can identify locally useful actions, but they do not
  overturn the operational branch rejection.
- MBE is disabled in the run profile, so there is no oracle proof that the
  selected predecessor or current live parent is benchmark-useful.
- The current live parent has only request files so far. It should not be
  counted as having produced descendants, results, or validation output yet.

## Selection and live continuation

The helper summary found three sealed selection rows. The latest selected:

```text
selected=candidate:node-e0215ac6e2825a28:plan_index=1
total_weight=0.6177700595386348
sample=0.8591638986461331
selected_index=3
```

Candidate weights in that row:

```text
node-c00551625661424e branch-80116f6702fafe10 disposition=reject performance=1825 weight=0.0008682926383120847
node-7d41954c057f3002 branch-a21b6ff8dee21a03 disposition=reject performance=7055 weight=0.28836268352752814
node-655e9397c9273545 branch-e304e47deb78395e disposition=reject performance=1850 weight=0.0009012144146791731
node-e0215ac6e2825a28 branch-01ade840a0a918b6 disposition=reject performance=8525 weight=0.3276378689581154 selected=true
```

The selection outcome is consistent with the profile:
`stop_on_first_keep = false`, `require_keep_for_continuation = false`, and
`explore_from_rejected = true`.

## Record surface status

| Surface | Observed state | Label |
| --- | --- | --- |
| Campaign manifest/profile | `campaign.json` and `prototype1/run-profile.toml` present | present |
| Active process | PID `1585253` running at cutoff | operator/convenience record |
| Transition journal | present; latest durable transition is successor handoff / parent start for `node-e0215ac6e2825a28` | present |
| Selected branch run root | run root present with record, trace, validation, submission, projection | record present, manual join needed |
| Selected branch protocol | protocol artifacts present for all tool-call reviews and segment reviews | record present, playback gap |
| Selected broad submitted result | `node-f21de5ba2e927ab0-r2.json` and `.headless-tui.json` present | present |
| Selected broad candidate workspace | submitted-result path no longer exists | record absent for workspace path; record present via installed commit |
| Current live parent ready channel | `channels/a720.../child-to-parent.jsonl` has successor_ready | present |
| Current live parent requests | `node-e0215ac6e2825a28{,-r2,-r3,-r4}.json` present | present |
| Current live parent results | no matching edit-harness result files at cutoff | record absent |
| Current live parent child plan | no `child-plan/node-e0215ac6e2825a28.json` at cutoff | record absent |

## Current incompleteness boundary

The last point where the live loop had enough information to act was after
successor handoff and request publication for `node-e0215ac6e2825a28`. The
trace had not yet reached a persisted result, child-plan, child admission, or
descendant eval boundary for that new parent at the cutoff.

Follow-up review should start at:

```text
/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/messages/edit-harness-result/node-e0215ac6e2825a28*.json
/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/messages/child-plan/node-e0215ac6e2825a28.json
/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/transition-journal.jsonl
```

Do not treat this scout as a completed campaign review. It is a live snapshot
with one verified selected-predecessor trace chain and a request-only boundary
for the current parent.
