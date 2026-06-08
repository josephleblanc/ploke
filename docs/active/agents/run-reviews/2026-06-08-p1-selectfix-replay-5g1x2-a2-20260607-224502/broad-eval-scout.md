# Broad/Eval Scout: p1-selectfix-replay-5g1x2-a2-20260607-224502

Scout frozen at `2026-06-08T01:22:33-07:00`.

## Verdict

Final host-process observation found no live `ploke-eval loop`, `prototype1-state`, `prototype1-step`, `prototype1-continue`, or `prototype1-runner` process for this campaign. This changed during the scout:

- At `2026-06-08T01:17:34-07:00`, the parent `prototype1-state` process and gen4 child runner `node-eb38c0cf2ab7b58c` were still live.
- At `2026-06-08T01:20:56-07:00`, `node-eb38c0cf2ab7b58c` was still live and had no runner result yet.
- At `2026-06-08T01:22:33-07:00`, no matching process remained; `node-eb38c0cf2ab7b58c` had written `runner-result.json`, `results/22ee2807-a392-4720-acfc-085e53766429.json`, and `evaluations/branch-0b3c3b943489e60b.json` at about `01:22:04-07:00`.

The broad/eval pipeline is mechanically progressing: broad request/result records, child plans, child runner records, run roots, LLM full responses, validation audits, patch projections, and evaluation artifacts are present for both gen4 children by final observation. The semantic/protocol surface is not evenly complete: `branch-0b3c3b943489e60b` had 54 tool-call reviews and 8 segment reviews, while `branch-4fa5078a62ae685e` still had only intent segmentation.

## Evidence Roots

Read-only roots inspected:

- Worktree: `/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Instance root: `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Protocol root: `/home/brasides/.ploke-eval/protocol/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`

Node directories inspected under the campaign root:

`node-8cf286015fb78261`, `node-10b418a02e91f3f1`, `node-f21de5ba2e927ab0`, `node-c00551625661424e`, `node-7d41954c057f3002`, `node-655e9397c9273545`, `node-e0215ac6e2825a28`, `node-666e03c70c944722`, `node-d245f3418c712226`, `node-44b3f97965446494`, `node-86b020f1dfdcf8af`, `node-72b640e0a78cafcc`, and `node-eb38c0cf2ab7b58c`.

Broad edit workspaces observed under `prototype1/workspaces/edit-harness/`:

`node-10b418a02e91f3f1`, `node-f21de5ba2e927ab0`, `node-e0215ac6e2825a28`, `node-e0215ac6e2825a28-r2`, `node-7d41954c057f3002-r2`, `node-86b020f1dfdcf8af-r2`, and `node-86b020f1dfdcf8af`.

No sqlite or db file was opened. DB files observed by `find` were treated only as path witnesses.

## Profile And Run Shape

Profile evidence came from `campaign.json` and `prototype1/run-profile.toml`:

- Broad/eval model: `google/gemini-3.5-flash`, route `direct_google`
- Protocol model: `google/gemini-2.5-flash`, route `direct_google`, `max_tokens=8000`
- `max_generations=5`
- `parallel_targets=2`
- `max_attempts=2`
- `fresh_slots_per_child=2`
- `observe_child_stale_after_secs=1200`
- `stop_on_first_keep=false`
- `require_keep_for_continuation=false`
- `explore_from_rejected=true`

## Broad Harness Surface

Each broad parent published four request slots, but the child-plan/admission surface selected two children per parent. The unselected slots are not treated as missing results here.

| parent | request mtimes | submitted result mtimes | child-plan mtime | child generation |
| --- | --- | --- | --- | --- |
| `node-8cf286015fb78261` | `2026-06-07 22:57:52` for base/r2/r3/r4 | `r2` at `23:05:20`, base at `23:08:36` | `23:08:39` | gen1 |
| `node-10b418a02e91f3f1` | `23:24:06` for base/r2/r3/r4 | base at `23:31:15`, `r2` at `23:32:35` | `23:32:38` | gen2 |
| `node-f21de5ba2e927ab0` | `23:43:18` for base/r2/r3/r4 | base at `23:48:36`, `r2` at `23:55:28` | `23:55:31` | gen2 |
| `node-e0215ac6e2825a28` | `2026-06-08 00:09:57` for base/r2/r3/r4 | `r2` at `00:18:37`, base at `00:18:46` | `00:18:49` | gen3 |
| `node-7d41954c057f3002` | `00:33:20` for base/r2/r3/r4 | base at `00:39:38`, `r2` at `00:41:20` | `00:41:23` | gen3 |
| `node-86b020f1dfdcf8af` | `00:57:16` for base/r2/r3/r4 | `r2` at `01:04:55`, base at `01:07:06` | `01:07:10` | gen4 |

Gen4 child-plan from `node-86b020f1dfdcf8af` selected:

| child node | branch | candidate | target relpath | broad result |
| --- | --- | --- | --- | --- |
| `node-eb38c0cf2ab7b58c` | `branch-0b3c3b943489e60b` | `broad-harness-g4-01` | `crates/ploke-db/src/database.rs` | `node-86b020f1dfdcf8af.json` |
| `node-72b640e0a78cafcc` | `branch-4fa5078a62ae685e` | `broad-harness-g4-02` | `crates/ploke-tui/src/app_state/database.rs` | `node-86b020f1dfdcf8af-r2.json` |

Status labels:

- Published edit requests: `present`.
- Submitted edit results and headless traces for admitted children: `present`.
- Unselected request slots (`r3`/`r4` style slots): `not applicable` as child outputs because the child plan admitted two children.
- Broad workspaces: `present`, but only currently materialized workspace directories were checked. Earlier treatment target checkouts are not all still present.

## Child Runner And Channel Surface

Runner request/result coverage at final observation:

| node | gen | branch | runner request | invocation | result | channel | stream logs |
| --- | ---: | --- | --- | --- | --- | --- | --- |
| `node-10b418a02e91f3f1` | 1 | `branch-52a318aef7a42b68` | present | 2 | present | 2 JSONL | 4 logs |
| `node-f21de5ba2e927ab0` | 1 | `branch-657235b177dc4597` | present | 2 | present | 2 JSONL | 4 logs |
| `node-c00551625661424e` | 2 | `branch-80116f6702fafe10` | present | 1 | present | 1 JSONL | 2 logs |
| `node-7d41954c057f3002` | 2 | `branch-a21b6ff8dee21a03` | present | 2 | present | 2 JSONL | 4 logs |
| `node-655e9397c9273545` | 2 | `branch-e304e47deb78395e` | present | 1 | present | 1 JSONL | 2 logs |
| `node-e0215ac6e2825a28` | 2 | `branch-01ade840a0a918b6` | present | 2 | present | 2 JSONL | 4 logs |
| `node-666e03c70c944722` | 3 | `branch-c17c4cda5764dbb7` | present | 1 | present | 1 JSONL | 2 logs |
| `node-d245f3418c712226` | 3 | `branch-0e230b45dab588be` | present | 1 | present | 1 JSONL | 2 logs |
| `node-44b3f97965446494` | 3 | `branch-e21d8f841040888f` | present | 1 | present | 1 JSONL | 2 logs |
| `node-86b020f1dfdcf8af` | 3 | `branch-564844dfd9b65469` | present | 2 | present | 2 JSONL | 4 logs |
| `node-72b640e0a78cafcc` | 4 | `branch-4fa5078a62ae685e` | present | 1 | present at `01:15:26` | 1 JSONL | 2 logs |
| `node-eb38c0cf2ab7b58c` | 4 | `branch-0b3c3b943489e60b` | present | 1 | present at `01:22:04` | 1 JSONL | 2 logs |

The gen4 live/incomplete window was real:

- `node-72b640e0a78cafcc` wrote `runner-result.json` at `01:15:26`, after the first process probe still showed both gen4 runners live.
- `node-eb38c0cf2ab7b58c` still lacked `runner-result.json` and a `results/*.json` file at `01:20:56`; both appeared at `01:22:04`.
- By final observation, both gen4 children had `status=succeeded`, `disposition=succeeded`, `exit_code=0`.

Runtime channels are `record present, playback gap`: the `child-to-parent.jsonl` files exist, but this scout joined them manually by runtime id and mtime rather than through an ordered playback view.

## Run Roots And LLM Timing

All baseline/treatment run roots had `agent-turn-trace.json`, `agent-turn-summary.json`, `llm-full-responses.jsonl`, `validation-audit.json`, `benchmark-patch-projection.json`, `multi-swe-bench-submission.jsonl`, and `record.json.gz`. Full-provider responses are `record present, manual join needed` because the scout joined them by branch/run path.

| branch | run id | responses | first response | last response | finish reasons |
| --- | --- | ---: | --- | --- | --- |
| baseline | `run-1780897832425-structured-current-policy-31f7977c` | 54 | `2026-06-07 22:50:35-07:00` | `22:54:26-07:00` | 53 tool calls, 1 stop |
| `branch-52a318aef7a42b68` | `run-1780899065850-structured-current-policy-08954e06` | 44 | `23:12:15` | `23:15:30` | 43 tool calls, 1 stop |
| `branch-657235b177dc4597` | `run-1780899065822-structured-current-policy-00b50f7c` | 43 | `23:12:15` | `23:15:24` | 42 tool calls, 1 stop |
| `branch-80116f6702fafe10` | `run-1780900503689-structured-current-policy-fb24875c` | 50 | `23:36:11` | `23:40:02` | 49 tool calls, 1 stop |
| `branch-a21b6ff8dee21a03` | `run-1780900501489-structured-current-policy-cfd7f666` | 46 | `23:36:09` | `23:38:49` | 45 tool calls, 1 stop |
| `branch-e304e47deb78395e` | `run-1780901885654-structured-current-policy-7ea8c430` | 46 | `23:59:14` | `00:03:08` | 45 tool calls, 1 stop |
| `branch-01ade840a0a918b6` | `run-1780901885639-structured-current-policy-9f8a8db1` | 39 | `23:59:14` | `00:03:06` | 38 tool calls, 1 stop |
| `branch-c17c4cda5764dbb7` | `run-1780903279470-structured-current-policy-c9a2f895` | 44 | `00:22:28` | `00:25:26` | 43 tool calls, 1 stop |
| `branch-0e230b45dab588be` | `run-1780903281103-structured-current-policy-4be1f9b3` | 46 | `00:22:29` | `00:26:02` | 45 tool calls, 1 stop |
| `branch-e21d8f841040888f` | `run-1780904631644-structured-current-policy-2a5fb499` | 48 | `00:45:00` | `00:48:07` | 47 tool calls, 1 stop |
| `branch-564844dfd9b65469` | `run-1780904631608-structured-current-policy-b8412836` | 54 | `00:45:00` | `00:49:10` | 53 tool calls, 1 stop |
| `branch-4fa5078a62ae685e` | `run-1780906168842-structured-current-policy-f648e09f` | 51 | `01:10:36` | `01:14:04` | 50 tool calls, 1 stop |
| `branch-0b3c3b943489e60b` | `run-1780906170104-structured-current-policy-a8a516ee` | 55 | `01:10:39` | `01:15:01` | 54 tool calls, 1 stop |

All response rows used `google/gemini-3.5-flash`.

## Evaluation And Patch Output

Evaluation artifacts are `present` for every completed child branch by final observation, including both gen4 branches:

- `branch-4fa5078a62ae685e`: evaluation mtime `01:15:26`, `overall_disposition=keep`. Compared against baseline `branch-564844dfd9b65469`; treatment had `tool_calls_failed=1`, `same_file_patch_retry_count=0`, `same_file_patch_max_streak=1`, `patch_apply_state=applied`, nonempty submission, and passed patch projection.
- `branch-0b3c3b943489e60b`: evaluation mtime `01:22:04`, `overall_disposition=reject`. Compared against the same baseline; treatment had `tool_calls_failed=3` versus baseline `1`, so the reason was `tool_calls_failed regressed: 1 -> 3`. It still had `patch_apply_state=applied`, nonempty submission, passed patch projection, `same_file_patch_retry_count=0`, and `same_file_patch_max_streak=1`.

Validation-audit warnings found across run roots:

- Every inspected run warns that no formatting check evidence was recorded. Do not claim `cargo fmt` or `rustfmt` passed.
- Some earlier branches had final cargo checks that did not cover changed files (`branch-80116f6702fafe10`, `branch-c17c4cda5764dbb7`, `branch-e21d8f841040888f`).
- The two gen4 branches both had final cargo calls covering changed files:
  - `branch-4fa5078a62ae685e`: final `cargo check`, workspace manifest, ok, covers `crates/printer/src/util.rs` and `crates/printer/src/standard.rs`.
  - `branch-0b3c3b943489e60b`: final `cargo check`, workspace manifest, ok, covers the same changed files.

This is mechanical eval evidence, not benchmark success evidence. No oracle or MBE verdict was found in this scout surface.

## Protocol Surface

Protocol artifacts are `present`, but finality is uneven:

| branch | intent segmentation | tool-call review | segment review | status |
| --- | ---: | ---: | ---: | --- |
| baseline | 1 | 0 | 0 | segmentation only |
| `branch-52a318aef7a42b68` | 1 | 43 | 12 | reviewed |
| `branch-657235b177dc4597` | 1 | 42 | 13 | reviewed |
| `branch-80116f6702fafe10` | 1 | 0 | 0 | segmentation only |
| `branch-a21b6ff8dee21a03` | 1 | 45 | 4 | partial segment review |
| `branch-e304e47deb78395e` | 1 | 0 | 0 | segmentation only |
| `branch-01ade840a0a918b6` | 1 | 38 | 12 | reviewed |
| `branch-c17c4cda5764dbb7` | 1 | 0 | 0 | segmentation only |
| `branch-0e230b45dab588be` | 1 | 45 | 12 | reviewed |
| `branch-e21d8f841040888f` | 1 | 47 | 10 | reviewed |
| `branch-564844dfd9b65469` | 1 | 53 | 12 | reviewed |
| `branch-4fa5078a62ae685e` | 1 | 0 | 0 | segmentation only as of final check |
| `branch-0b3c3b943489e60b` | 1 | 54 | 8 | reviewed calls, partial segment review |

Protocol warning surfaces:

- `node-72b640e0a78cafcc` stderr logged malformed adjudication JSON retries for `tool_call_review[19]` and `[10]`, with the invalid enum spelling `helpful_but_non-essential`.
- `node-eb38c0cf2ab7b58c` stderr logged malformed adjudication JSON retries for `tool_call_review[8]` with `helpful_but_non-essential`, and for `tool_call_review[48]` with missing field `rationale`.
- These retries did not prevent `branch-0b3c3b943489e60b` from writing all 54 tool-call-review artifacts by final observation, but they are protocol robustness warnings.

## Concrete Trace Chain

Completed-child chain from gen4 `node-72b640e0a78cafcc`, branch `branch-4fa5078a62ae685e`:

1. `prototype1-state` spawned `node-72b640e0a78cafcc` from parent `node-86b020f1dfdcf8af`; runner request mtime was `01:07:11`, invocation mtime was `01:09:28`.
2. Run root `run-1780906168842-structured-current-policy-f648e09f` emitted 51 provider responses from `01:10:36` to `01:14:04`, with 50 provider tool calls and 50 recorded tool calls. The trace audit found no missing provider call IDs.
3. Early retrieval was low-information: `request_code_context` calls for `printer replacement multiline`, `crates/printer replace`, `replacer`, `replace_with_captures_at`, `standard.rs replace_all`, and `replace_all` all returned empty context.
4. The model recovered by listing directories and reading actual files: `crates/printer/src/util.rs`, `crates/matcher/src/lib.rs`, and `crates/printer/src/standard.rs`.
5. The first precise lookup failed: `code_item_lookup` for `replace_all` as a `crate::util::Replacer` method failed. The model then read `util.rs` directly and applied a `non_semantic_patch` to `crates/printer/src/util.rs`.
6. It ran `cargo check` and `cargo test -p grep-printer` after the first patch, then attempted late reads near the end of `standard.rs`. The trace audit classified three of those as empty completed reads.
7. It read a lower line range with content, applied a second `non_semantic_patch` to `crates/printer/src/standard.rs`, then ran `cargo test -p grep-printer`, a workspace `cargo test`, and a final `cargo check -p grep-printer`.
8. Runner result and branch evaluation were written at `01:15:26`; the evaluation kept the branch because same-file retry metrics improved against the parent baseline.

Important caveat: by the time this scout tried to inspect the treatment checkout recorded in `run.json`, the `node-72b640e0a78cafcc/instance-targets/.../BurntSushi/ripgrep` path was no longer present. That makes direct checkout verification for this completed child a `record present, manual join needed` gap. The run-root trace and validation artifacts remain present.

Currently-present checkout verification from gen4 `node-eb38c0cf2ab7b58c`, branch `branch-0b3c3b943489e60b`:

- Trace audit found 55 provider responses, 54 provider tool calls, and 54 recorded tool calls, again with no missing provider call IDs.
- It followed a similar pattern: empty broad retrieval, direct file reads, successful `code_item_lookup`, `insert_rust_item`, failed semantic edit attempts, fallback reads, `non_semantic_patch`, cargo checks/tests, and final validation.
- Direct read against the still-present treatment checkout verified that `crates/printer/src/util.rs` contains `replace_with_captures_at_limit` at lines 87 and 465.
- Direct read also verified `crates/printer/src/standard.rs` has 3702 lines and includes the added `replacement_multi_line_lookaround` test region near lines 3290-3330.
- Validation audit recorded final workspace `cargo check` as ok and covering the changed files, but still recorded no formatting check.
- Despite the applied patch and validation, branch evaluation rejected this child because failed tool calls regressed from 1 to 3 versus the parent baseline.

## Failure And Warning Surfaces

Broad/headless warnings:

- Several parent broad harness stream logs recorded `WorkspacePathMismatch` on stderr. Treat these as artifact-prep or workspace-path evidence, not provider failure evidence.
- Multiple broad headless runs logged `INVALID_MODEL_RESPONSE` warnings, including parents `node-f21de5ba2e927ab0`, `node-e0215ac6e2825a28`, `node-7d41954c057f3002`, and `node-86b020f1dfdcf8af`.

Eval runner warnings:

- Gen4 runners logged typed type-context expansion disabled because active databases were missing typed graph relations.
- Gen4 runners logged skipped non-primary targets in legacy parse mode.
- Gen4 runners logged repeated embedding context-length fallback warnings and at least one truncated embedding snippet.
- `node-eb38c0cf2ab7b58c` logged `Cannot send shutdown message, other side dropped` during embedding shutdown and a `ploke_io::actor` file/database mismatch diagnostic before continuing to completion.
- Both gen4 traces ended with terminal summaries that include `TOOL_EXECUTION_FAILED` tool errors, even though the overall runner result was `succeeded`.

## Record Status Checklist

| surface | status | note |
| --- | --- | --- |
| Transition journal | present | Joined by timestamp and node/branch ids. |
| Sealed History blocks | present | Helper found selection formula rows; this scout did not deep-review selection payloads. |
| Run profile | present | `run-profile.toml` and campaign model routes inspected. |
| Edit requests | present | Four slots per broad parent. |
| Edit results and headless traces | present | Two admitted broad results per parent. |
| Child plans | present | Six child-plan files, two children each. |
| Runner requests | present | Present for every non-root child node. |
| Invocations/results | present | Final gen4 results appeared during scout. |
| Channels | record present, playback gap | `child-to-parent.jsonl` present, manual join by runtime id. |
| Stream logs | operator/convenience record | Used for liveness and warnings. |
| Run roots | present | Baseline plus all treatment branches. |
| `llm-full-responses.jsonl` | record present, manual join needed | Joined manually by branch/run root. |
| Validation audits | present | Strong enough for cargo coverage warnings, not semantic benchmark proof. |
| Patch/submission projections | present | Nonempty/passed per evaluation metrics for gen4. |
| Protocol artifacts | present, playback gap | Artifact counts available, semantic finality uneven. |
| Earlier treatment checkouts | record absent at inspected path | Most completed treatment `repo_root` paths from `run.json` were no longer present. |
| Gen4 `branch-0b3c3b943489e60b` checkout | present | Used for direct verification. |

## Scout Conclusion

No immediate live-run blocker remained by final observation because the last gen4 child completed and the parent process exited. The main red flags are observability and adjudication quality, not a stuck runner:

- Completed child target checkouts can disappear while run roots remain, forcing manual joins and limiting direct verification after the fact.
- Protocol finality is uneven: one gen4 branch has only segmentation while the other has call reviews plus partial segment reviews.
- Protocol retry handling is working in the sense that malformed adjudication JSON did not stop `branch-0b3c3b943489e60b`, but the malformed enum and missing-field retries should be tracked as protocol robustness issues.
- Mechanical eval success is not benchmark success. Gen4 branches produced applied patches and cargo evidence; one was kept on operational metrics, one was rejected for failed-tool regression. Neither is oracle/MBE proof.

Recommended follow-up: a full run review is warranted for at least `branch-4fa5078a62ae685e` and `branch-0b3c3b943489e60b` because the gen4 branches now have complete runner/evaluation records but different protocol completeness and opposite evaluation dispositions.
