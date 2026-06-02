# Persisted Surface Survey

Date: 2026-05-11

Repository commit: `3a2ea733`

Purpose: remove ambiguity around files currently persisted under
`/home/brasides/.ploke-eval` and how they should relate to
`ploke-tree::RunRecordSet` and `ploke-tree::Graph`.

Policy update: every persisted loop-relevant surface should eventually be
reachable through `ploke-tree::Graph`. Rows below that say "not current graph
input" describe current implementation state, not an exclusion decision. Heavy
or low-authority surfaces may be represented as metadata, evidence locators,
digests, summaries, or explicit ambiguity instead of recursively loaded
content.

Legacy policy: this survey records historical drift to prevent accidental
rediscovery, but implementation targets the current emitted run shape only.
Do not add migrations or compatibility readers for older run shapes. If an
older run contains a file absent from the current emitted shape, count that
older file as unsupported unless the user explicitly asks for legacy support.

This survey is metadata-first. Agents used `find`, `du`, `stat`-style output,
bounded source `rg`, and targeted line references. They did not read large
JSON, JSONL, logs, or run artifacts.

## Scope

Covered:

- top-level `/home/brasides/.ploke-eval`;
- `/home/brasides/.ploke-eval/worktrees`;
- campaign `prototype1` directories outside `worktree` and `target`;
- bounded source lookup for `ploke-eval` persisted-file writers;
- follow-up static audit of workspace eval/prototype/run write sites;
- follow-up type-owner cross-check against `ploke-records` and current
  `ploke-tree` loaders;
- follow-up historical path-shape audit across P1 campaign roots;
- controlled runtime-diff procedure design for a future tiny run.

Not covered as content:

- full JSON / JSONL records;
- log contents;
- binary contents;
- cargo build products;
- source checkouts inside worktrees.

## Top-Level Size Signals

Observed `/home/brasides/.ploke-eval` total: about `88G`.

| Top-level area | Approx size | Graph ingestion posture |
|---|---:|---|
| `campaigns` | `50G` | primary Prototype 1 persisted run area |
| `worktrees` | `19G` | source/runtime checkouts; represent by metadata/locator, do not recursively load |
| `instances` | `9.7G` | broader eval artifacts; future metadata/evidence input if loop-relevant |
| `repos` | `6.7G` | repo/cache material; represent by metadata/locator, do not recursively load |
| `logs` | `1.6G` | log artifacts; future typed log metadata/evidence input |
| `cache` | `1.2G` | cache material; metadata only if loop-relevant |
| `cargo-home-shared` | `510M` | cargo cache; metadata only if needed for reproducibility |

## Worktree Areas

`/home/brasides/.ploke-eval/worktrees` exists and currently contains 19
top-level `p1-*` roots.

Observed worktree families:

| Worktree family | Count |
|---|---:|
| `p1-bounded-surface-long` | 1 |
| `p1-bounded-surface-mini` | 2 |
| `p1-edit-surface-history` | 2 |
| `p1-edit-surface-history-long` | 1 |
| `p1-history-traversal` | 2 |
| `p1-occurrence-selection-long` | 1 |
| `p1-occurrence-selection-overnight` | 1 |
| `p1-overnight-edit-surface` | 4 |
| `p1-smoke-3x4-edit-surface` | 4 |
| `p1-smoke-3x4-workspace-except-eval` | 1 |

Worktrees contain normal source-layout directories whose names can look like
run nodes, such as `nodes`, `assoc_nodes`, `fixture_nodes`,
`fixture_nodes_copy`, `node-reference`, and `ws_fixture_nodes`. These are not
Prototype 1 run records.

Cleanup-sensitive build artifacts:

| Worktree target dir | Approx size |
|---|---:|
| `worktrees/p1-history-traversal-20260511-2/target` | `5.6G` |
| `worktrees/p1-smoke-3x4-edit-surface-20260510-4/target` | `6.3G` |
| `worktrees/p1-smoke-3x4-workspace-except-eval-20260511-2/target` | `5.6G` |

Graph/run loaders should hard-exclude these directories from recursive content
loading:

- `**/worktree/**`
- `/home/brasides/.ploke-eval/worktrees/**`
- `/home/brasides/.ploke-eval/repos/**`
- `/home/brasides/.ploke-eval/msb/repos/**`
- `/home/brasides/.ploke-eval/cargo-home-shared/**`
- `**/target/**`

## Campaign Prototype1 File Patterns

Survey scope here excluded `worktree`, `worktrees`, and `target`.

Observed Prototype 1 campaign roots: `85`.

Observed files under campaign `prototype1`: `24,960`; excluding `target`:
`5,281`. Build byproduct files inside node dirs accounted for `19,679`.

Historical follow-up audited 566 P1-like campaign roots, including treatment
branches. Main observed shapes:

- 477 roots had only `campaign.json` and `closure-state.json`; this is the
  dominant newer treatment-branch form.
- 23 roots had full older bundles including `slice.jsonl`,
  `prototype1-loop-trace.json`, `prototype1/{scheduler.json,
  transition-journal.jsonl, branches.json, evaluations/*.json}`, and logs.
- 18 roots had `campaign.json`, `slice.jsonl`, and
  `prototype1/scheduler.json`, without `closure-state.json`.
- 9 roots had the same shape plus `closure-state.json`.
- 6 roots had newer full bundles with `prototype1/{scheduler,
  transition-journal, branches, evaluations}` and logs, without trace.

Legacy/drift notes:

- `prototype1-loop-trace.json` is stale: 43 roots total, latest observed on
  `p1-occurrence-selection-long-20260509-2`. Treat as unsupported for new
  implementation unless explicitly reauthorized.
- `closure-state.json` is missing on 21 older roots; latest missing-closure
  root observed was `p1-edit-surface-history-20260507-1`. Do not add fallback
  behavior for this older shape.
- Evaluation artifacts are rooted at `prototype1/evaluations/branch-*.json`
  relative to the campaign root. Tables below use paths relative to the
  `prototype1` run root unless explicitly stated.

Common path patterns:

| Pattern | Count | Current `ploke-tree` status |
|---|---:|---|
| `nodes/node-*/runner-request.json` | 695 | loaded into `PassiveEvidence.run_attempts`; graph evidence only |
| `nodes/node-*/node.json` | 695 | loaded into `RunForestInput.node_records` |
| `nodes/node-*/invocations/<runtime-id>.json` | 582 | loaded into `PassiveEvidence.run_attempts`; runtime/operation evidence |
| `nodes/node-*/streams/<runtime-id>/stdout.log` | 568 | not loaded; log artifact |
| `nodes/node-*/streams/<runtime-id>/stderr.log` | 568 | not loaded; log artifact |
| `nodes/node-*/results/<runtime-id>.json` | 430 | loaded into `PassiveEvidence.attempt_runner_results`; attempt-scoped evidence |
| `nodes/node-*/channels/<runtime-id>/child-to-parent.jsonl` | 405 | counted/parsed as passive channel evidence |
| `evaluations/branch-*.json` | 369 | loaded as evaluation evidence |
| `nodes/node-*/runner-result.json` | 351 | loaded into `PassiveEvidence.run_attempts`; graph evidence only |
| `messages/child-plan/node-*.json` | 155 | loaded as passive child-plan evidence; graph summary evidence only |
| `scheduler.json` | 81 | loaded into `RunForestInput.scheduler` |
| `nodes/node-*/successor-ready/<runtime-id>.json` | 73 | loaded into `RunForestInput.successor_ready` |
| `nodes/node-*/successor-completion/<runtime-id>.json` | 71 | loaded into `RunForestInput.successor_completion` |
| `transition-journal.jsonl` | 51 | loaded into `RunRecordSet.transition_journal` |
| `nodes/node-*/bin/ploke-eval` | 46 | binary artifact; future metadata/digest/provenance input |
| `branches.json` | 45 | loaded as branch registry/log summary |
| `history/index/heads.json` | 18 | projection/index; future rebuildability/check metadata |
| `history/index/by-lineage-height.jsonl` | 18 | projection/index; future rebuildability/check metadata |
| `history/index/by-hash.jsonl` | 18 | projection/index; future rebuildability/check metadata |
| `history/blocks/segment-000000.jsonl` | 18 | loaded as sealed History blocks |
| `run-profile.toml` | 12 | loaded as passive run-profile metadata evidence |
| `run-profile.commitment.json` | 12 | loaded as passive run-profile commitment evidence |

Size signals outside `target`:

| Area | Count | Approx size |
|---|---:|---:|
| `nodes/*/bin/ploke-eval` | 46 | `37.8G` |
| `history/blocks` | 18 | `64.4M` |
| `transition-journal.jsonl` | 51 | `10.7M` |
| `messages/child-plan` | 155 | `10.0M` |
| `nodes/*/invocations` | 582 | `7.6M` |
| `nodes/*/streams` | 1136 | `5.3M` |
| `branches.json` | 45 | `2.9M` |
| `nodes/*/channels` | 405 | `1.4M` |
| `evaluations` | 369 | `1.3M` |

## Other `.ploke-eval` Files

Observed common top-level or non-Prototype1 persisted files:

| Pattern | Count | Current graph posture |
|---|---:|---|
| `campaign.json` | 567 | setup/closure metadata; future campaign metadata input |
| `closure-state.json` | 546 | setup/closure metadata; future campaign metadata input |
| `slice.jsonl` | 88 | projection/debug surface; future weak evidence/diagnostic input |
| `prototype1-loop-trace.json` | 43 | projection/legacy trace; future weak evidence/diagnostic input |
| `multi-swe-bench-submission*.jsonl` | 2 | submission artifact; future export/provenance metadata |
| `provider-calibration.json` | 1 | calibration artifact; future provider metadata |
| `last-run.json` | 1 observed singleton | run-history pointer; future discovery metadata |
| `p1-loop-run10.console.log` | 1 observed singleton | console log; future log locator/metadata |
| `prototype1-monitor-target.json` | 1 observed singleton | monitor target; future operator metadata, not authority |
| `selection.json` | 1 observed singleton | selection artifact; future metadata if still relevant |

Large catchall areas outside `campaigns`:

| Area | File count | Approx size | Posture |
|---|---:|---:|---|
| `instances` | 11,687 | `9.66G` | broader eval artifacts |
| `logs` | 7,175 | `1.53G` | logs |
| `cargo-home-shared` | 29,221 | `417M` | cargo cache |
| `cache` | 542 | `1.17G` | cache |
| `repos` | 4,627 | `170M` | repo cache |
| `msb` | 2,252 | `90.8M` | SWE-bench / repo material |
| `batches` | 1,388 | `52.0M` | eval batch material |
| `datasets` | 11 | `39.4M` | dataset material |
| `registries` | 459 | `3.1M` | registry material |

## Source Writer Survey

The source-side survey was read-only and bounded to persisted-file writers and
path constants.

| Persisted file or pattern | Writer reference | Owner / shape | Authority class |
|---|---|---|---|
| `transition-journal.jsonl` | `crates/ploke-eval/src/cli/prototype1_state/journal.rs:652-688`; `prototype1_process.rs:164-185` | `JournalEntry` | active-loop append-only transition state |
| `run-profile.toml`, `run-profile.commitment.json` | `crates/ploke-eval/src/cli/prototype1_state/profile.rs:349-362,456-462` | `AdmittedRunProfile` / `RunProfileCommitment` | run profile / commitment |
| `scheduler.json` | `crates/ploke-eval/src/intervention/scheduler.rs:374-395`; `prototype1_process.rs:1498-1503` | `Prototype1SchedulerState` | active-loop scheduler authority |
| `nodes/<node-id>/node.json` | `crates/ploke-eval/src/intervention/scheduler.rs:548-583` | `Prototype1NodeRecord` | projection/passive node evidence |
| `nodes/<node-id>/runner-request.json` | `crates/ploke-eval/src/intervention/scheduler.rs:735-747` | `Prototype1RunnerRequest` | passive runner evidence |
| `nodes/<node-id>/runner-result.json` | `crates/ploke-eval/src/intervention/scheduler.rs:818-827` | `Prototype1RunnerResult` | passive runner evidence |
| `.ploke/prototype1/parent_identity.json` | `crates/ploke-eval/src/cli/prototype1_state/identity.rs:203-219` | `ParentIdentity` | checkout identity authority |
| `nodes/<node-id>/invocations/<runtime-id>.json` | `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:512-519` | `Invocation` / `ChildInvocation` / `SuccessorInvocation` | executable handoff boundary |
| `nodes/<node-id>/successor-ready/<runtime-id>.json` | `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:547-680` | `SuccessorReadyRecord` | passive handoff evidence |
| `nodes/<node-id>/successor-completion/<runtime-id>.json` | `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:547-680` | `SuccessorCompletionRecord` | passive handoff evidence |
| `nodes/<node-id>/channels/<runtime-id>/parent-to-child.jsonl`, `child-to-parent.jsonl` | `crates/ploke-eval/src/cli/prototype1_state/channel.rs:245-253,773-779` | `Envelope` | passive channel evidence / transport |
| `branches.json` | `crates/ploke-eval/src/intervention/branch_registry.rs:72-90,250-251` | `branch_log::Record` / `Prototype1BranchRegistry` | append-only passive comparison stream |
| `record.json.gz` | `crates/ploke-eval/src/record.rs:1649-1672` | `RunRecord` | passive replay artifact |
| `last-run.json` | `crates/ploke-eval/src/run_history.rs:133-153` | `LastRunRecord` | run-history pointer |
| `repo-state.json`, `indexing-status.json`, `snapshot-status.json`, `execution-log.json`, `agent-turn-summary.json`, `agent-turn-trace.json` | `crates/ploke-eval/src/runner.rs:1714-2005,2154,2317,2382,2444,2513,3279-3446,5542` | `RunArtifactPaths` / `AgentRunArtifactPaths` / `ObservedTurnArtifact` | run telemetry / observability projection |
| `batch-run-summary.json`, `multi-swe-bench-submission.jsonl`, `replay-batch-<nnn>.json` | `crates/ploke-eval/src/runner.rs:2630-2791,2841` | `BatchRunArtifactPaths` / `ReplayBatchArtifact` | batch packaging/export / replay projection |
| `execution-log.json`, `indexing-status.json`, `parse-failure.json`, `snapshot-status.json`, `repo-state.json`, `final-snapshot.db`, `indexing-checkpoint.db`, `indexing-failure.db` | `crates/ploke-eval/src/inner/registry.rs:305-324,385-396` | `RunRegistration` artifact layout | run registry / discovery |
| `stdout.log`, `stderr.log` | `crates/ploke-eval/src/cli/prototype1_process.rs:920-945` | stream logs | tracing/log artifact |
| `llm-full-responses.jsonl` | read/projection refs at `cli_facing.rs:4467-4480` | writer not located | provider/log surface, unresolved |
| `prototype1_observation_*.jsonl` | read/projection refs at `cli_facing.rs:5046-5055,5333-5385,5614-5689` | writer not located | observation/projection surface, unresolved |
| `prototype1-loop-trace.json` | read/projection refs at `cli_facing.rs:2479-2488,5797-5798` | writer not located; appears legacy-only | trace/projection surface |
| `campaign.json` | `crates/ploke-eval/src/campaign.rs:288-300` | campaign metadata | setup authority |
| `closure-state.json` | `crates/ploke-eval/src/closure.rs:281-289` | closure metadata | setup authority |
| `protocol-artifacts/<name>.json` | `crates/ploke-eval/src/protocol_artifacts.rs:317-347` | `StoredProtocolArtifact` | protocol evidence attachment |
| `registry.json`, `active-model.json` | `crates/ploke-eval/src/model_registry.rs:121-135,171-184` | `ModelRegistry` / `ActiveModelSelection` | model cache / operator selection |
| `provider-preferences.json` | `crates/ploke-eval/src/provider_prefs.rs:50-60` | `ProviderPrefs` | provider preference cache |
| `registries/<family>.json` | `crates/ploke-eval/src/target_registry.rs:108-160` | `TargetRegistry` | benchmark target inventory |
| `datasets/<filename>.jsonl` | `crates/ploke-eval/src/msb.rs:335-378` | `DatasetRecord` cache entry | eval input cache |

## Current `ploke-tree` Loader Coverage

Current `ploke-tree::FsRunStore` loads:

| Pattern | Loader target | Graph status |
|---|---|---|
| `scheduler.json` | `RunForestInput.scheduler` | scheduler/status evidence only |
| `nodes/*/node.json` | `RunForestInput.node_records` | node/runtime metadata evidence |
| configured `parent_identity.json` path | `RunForestInput.parent_identity` | parent identity evidence |
| `nodes/*/successor-ready/*.json` | `RunForestInput.successor_ready` | handoff/runtime evidence |
| `nodes/*/successor-completion/*.json` | `RunForestInput.successor_completion` | completion evidence |
| `history/blocks/segment-*.jsonl` | `RunRecordSet.history_blocks` | core History spine |
| `transition-journal.jsonl` | `RunRecordSet.transition_journal` plus passive counts | append-only transition evidence, secondary to History |
| `branches.json` | `PassiveEvidence.branch_registry` summary | branch/comparison summary evidence |
| nested `channels/**/*.jsonl` | `PassiveEvidence.channel_envelopes` summary | communication summary evidence |
| `messages/child-plan/*.json` | `PassiveEvidence.child_plans` | summary-only evidence attachment |
| `run-profile.toml`, `run-profile.commitment.json` | `PassiveEvidence.run_profile` | metadata evidence attachment |
| `evaluations/*.json` | `PassiveEvidence.evaluations` | evidence attachment |
| configured protocol artifacts dir `*.json` | `PassiveEvidence.protocol_artifacts` | evidence attachment |
| `nodes/*/runner-request.json` | `PassiveEvidence.run_attempts` | run-attempt evidence |
| `nodes/*/runner-result.json` | `PassiveEvidence.run_attempts` | run-attempt evidence |
| `nodes/*/invocations/*.json` | `PassiveEvidence.run_attempts` | runtime/operation evidence |
| `nodes/*/results/*.json` | `PassiveEvidence.attempt_runner_results` | attempt-scoped result evidence |

The protocol-artifacts row is intentionally not a run-root pattern. It is a
caller-configured directory consumed by `FsRunStore::with_protocol_artifacts_dir`.

## Type Ownership Cross-Check

This cross-check treats "owner found" as a named passive type in
`ploke-records`. Adjacent `ploke-eval` live/runtime types are noted elsewhere
in the writer survey, but they are not counted here as passive ownership.

| Pattern | Passive owner | Loader | Current graph ingestion | Confidence | Gap |
|---|---|---|---|---|---|
| `nodes/node-*/runner-request.json` | `scheduler::RunnerRequestRecord` | `PassiveEvidence.run_attempts` | evidence | high | passive evidence only; not History authority |
| `nodes/node-*/node.json` | `scheduler::NodeRecord` | `RunForestInput.node_records` | evidence | high | scheduler/node metadata only |
| `nodes/node-*/invocations/<runtime-id>.json` | `invocation::InvocationRecord` | `PassiveEvidence.run_attempts` | evidence | high | runtime/operation evidence only |
| `nodes/node-*/results/<runtime-id>.json` | `scheduler::RunnerResultRecord` | `PassiveEvidence.attempt_runner_results` | evidence | high | attempt-scoped result evidence only |
| `nodes/node-*/streams/<runtime-id>/{stdout,stderr}.log` | missing | missing | no | high | log artifact; needs metadata/locator shape |
| `nodes/node-*/channels/<runtime-id>/child-to-parent.jsonl` | `channel::Envelope<ToParent>` | `PassiveEvidence.channel_envelopes` | no | high | summary evidence only |
| `evaluations/branch-*.json` | `evaluation::Artifact` | `PassiveEvidence.evaluations` | evidence | high | evidence attachment only |
| `nodes/node-*/runner-result.json` | `scheduler::RunnerResultRecord` | `PassiveEvidence.run_attempts` | evidence | high | passive evidence only; not History authority |
| `messages/child-plan/node-*.json` | `child_plan::ChildPlanRecord` | `PassiveEvidence.child_plans` | evidence | high | summary-only evidence; no child branch/runtime authority |
| `scheduler.json` | `scheduler::SchedulerStateRecord` | `RunForestInput.scheduler` | evidence | high | scheduler/status metadata only |
| `nodes/node-*/successor-ready/<runtime-id>.json` | `invocation::SuccessorReadyRecord` | `RunForestInput.successor_ready` | evidence | high | handoff evidence only |
| `nodes/node-*/successor-completion/<runtime-id>.json` | `invocation::SuccessorCompletionRecord` | `RunForestInput.successor_completion` | evidence | high | completion evidence only |
| `transition-journal.jsonl` | `journal::JournalEntry` | `RunRecordSet.transition_journal` | evidence | high | append-only transition evidence only |
| `history/blocks/segment-*.jsonl` | `history::SealedBlockRecord` | `RunRecordSet.history_blocks` | yes | high | none |
| `branches.json` | `branch::BranchLogRecord` / `Prototype1BranchRegistry` | `PassiveEvidence.branch_registry` | evidence | high | summary only |
| `parent_identity.json` | `identity::ParentIdentityRecord` | `RunForestInput.parent_identity` | evidence | high | parent identity evidence only |
| `protocol-artifacts/*.json` | `protocol::Artifact` | `PassiveEvidence.protocol_artifacts` | evidence | high | caller-configured directory only |
| `history/index/{heads.json,by-lineage-height.jsonl,by-hash.jsonl}` | missing | missing | no | high | projection/index; needs metadata/check shape if used |
| `record.json.gz` | missing exact; adjacent eval `RunRecord` | missing | no | high | tree does not read it |
| `run-profile.toml` / `run-profile.commitment.json` | `run_profile::RunProfileRecord` / `run_profile::RunProfileCommitmentRecord` | `PassiveEvidence.run_profile` | evidence | high | metadata evidence only; no lineage/runtime/artifact authority |
| `execution-log.json` / `indexing-status.json` / `parse-failure.json` / `snapshot-status.json` / `repo-state.json` / `final-snapshot.db` | missing exact; adjacent eval-side run-history bundle | missing | no | high | eval owns bundle, tree does not |
| `campaign.json` / `closure-state.json` | missing exact | missing | no | medium | setup/closure only; passive metadata shape needed if graph uses them |
| `last-run.json` / `selection.json` / `prototype1-monitor-target.json` | missing exact; adjacent eval `LastRunRecord` / `ActiveSelection` / `ActivePrototype1MonitorTarget` | missing | no | high | active/projection metadata only |
| `slice.jsonl` / `prototype1-loop-trace.json` / `multi-swe-bench-submission*.jsonl` / `provider-calibration.json` / `p1-loop-run10.console.log` | missing | missing | no | high | projection/debug/artifact surfaces |
| `llm-full-responses.jsonl` / `prototype1_observation_*.jsonl` | missing | missing | no | high | no passive owner; only eval-side references found |
| `nodes/node-*/bin/ploke-eval` | missing | missing | no | high | binary artifact; needs digest/provenance metadata if used |

Bottom line: `history/blocks/segment-*.jsonl` is currently the graph-spine
input. Evaluation artifacts, configured protocol artifacts, child-plan
messages, run-profile records, run-attempt records, attempt-scoped runner
results, scheduler/node metadata, handoff records, transition journal entries,
branch summaries, and channel summaries attach as typed evidence. Remaining
rows are passive-owner-present-but-not-loaded, projection/debug-only, or not
yet owned by `ploke-records`.

## Missing Or Unresolved Loader Rows

These persisted files exist or are referenced by source, but are not currently
first-class `RunRecordSet` inputs:

| Pattern | Why it matters | Suggested next owner |
|---|---|---|
| `record.json.gz` | replay artifact | graph replay/provenance metadata input |
| `history/index/*` | rebuildable History indexes | rebuildability/check metadata input |
| `stdout.log`, `stderr.log` | stream logs | log locator/metadata input, not History authority |
| `llm-full-responses.jsonl` | provider response evidence | locate writer/type before loader |
| `prototype1_observation_*.jsonl` | observation/retry/timeout projections | locate writer/type before loader |
| `prototype1-loop-trace.json` | legacy trace projection | do not use as source unless reauthorized |
| `agent-turn-summary.json`, `agent-turn-trace.json` | turn-level observability | done for typed owner, direct run-root loader, and passive graph evidence; remaining issue is canonical passive tool transport ownership |
| `protocol-artifacts/<name>.json` | protocol evidence | already has writer; align configured directory with run/campaign discovery |
| model/provider/target registry and dataset cache files | run environment context | graph metadata only when tied to a concrete run |

## Runtime Diff Procedure

A future controlled run can remove remaining uncertainty about runtime-emitted
paths. The safe procedure is metadata-only:

Proxy verdict from the audit:

- There is no recent completed small run in `/home/brasides/.ploke-eval` that
  is both recent and small.
- Closest small completed proxy: `p1-3gen-run3`, with three nodes and one
  completed node. Its run-root shape is `branches.json`, one
  `evaluations/branch-*.json`, `scheduler.json`, and
  `transition-journal.jsonl`.
- Recent 2026-05-11 roots are mostly one-node planned setups, so they are not
  completed proxies.
- Closest recent completed run: `p1-3gen-15nodes-run12`, with ten nodes and
  three completed nodes, but it is not small.

```bash
root="$HOME/.ploke-eval/campaigns/<campaign>/prototype1"
before="/tmp/<campaign>.before.tsv"
after="/tmp/<campaign>.after.tsv"

find "$root" -type f -printf '%P\t%s\t%TY-%Tm-%TdT%TH:%TM:%TS\n' | sort > "$before"
# Run the tiny campaign outside this procedure.
find "$root" -type f -printf '%P\t%s\t%TY-%Tm-%TdT%TH:%TM:%TS\n' | sort > "$after"
comm -13 <(cut -f1 "$before" | sort) <(cut -f1 "$after" | sort)
```

The diff itself is read-only and needs no escalation, but starting a new
Prototype 1 campaign or `prototype1-setup` requires explicit user approval
because it mutates run state and git refs.

## Practical Loader Rule

For `ploke-tree::Graph` work, treat this survey as the file representation map:

- include explicit typed rows through `ploke-tree::store`;
- represent worktrees, source repos, cargo caches, build products, binaries,
  and logs through metadata, locators, digests, summaries, or drilldown records
  rather than recursive content ingestion;
- make every newly discovered persisted file a row in this document or the
  graph ingestion inventory before adding a loader;
- do not deserialize run files in graph builders, browser code, or `ploke-egui`.
