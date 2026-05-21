# Latest Run Emission Worksheet

Status: latest-run validation pass, 2026-05-21.

This worksheet checks the current inventory against the newest run found under
`~/.ploke-eval` during this pass:

- instance run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-smoke-broad-harness-1x3-20260519-2/BurntSushi__ripgrep-2209/runs/run-1779210410211-structured-current-policy-d7e36c52`
- protocol root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-smoke-broad-harness-1x3-20260519-2/BurntSushi__ripgrep-2209/runs/run-1779210410211-structured-current-policy-d7e36c52`
- run id: `run-1779210410211-structured-current-policy-d7e36c52`

Validation was metadata-first. The pass listed file names and sizes/mtimes for
related anchors, but did not read JSONL, compressed records, DB files, protocol
artifacts, or traces wholesale.

## Verdict

The inventory categories are broadly right, but the per-record worksheet was
not exhaustive for the latest run. These observed families were missing or too
vague in the previous inventory:

- `benchmark-patch-projection.json`
- `multi-swe-bench-submission.jsonl`
- `config/ploke/proposals.json`
- protocol-root `*_tool_call_intent_segmentation_*.json`
- protocol-root `*_tool_call_review_*.json`
- protocol-root `*_tool_call_segment_review_*.json`
- related anchors outside the run root: `last-run.json`,
  `registries/runs/<run-id>.json`, campaign `campaign.json`,
  `closure-state.json`, `slice.jsonl`, batch `batch.json`,
  `batch-run-summary.json`, and batch aggregate
  `multi-swe-bench-submission.jsonl`

The important design result is that some of these are real playback inputs, but
some are operator conveniences, batch/campaign inputs, or benchmark-facing
exports. `RuntimePlayback` should record their relation to the run without
treating every file as a History authority source.

## Instance Run Root Rows

| File family | Owner type | Writer / reader boundary | Graph status | Join and sequence | Worksheet verdict |
| --- | --- | --- | --- | --- | --- |
| `execution-log.json` | `ploke_eval::runner::ExecutionLog` | writer: `crates/ploke-eval/src/runner.rs`; reader: run registration/closure/report consumers | referenced/diagnostic | joins by `run_id` and run root; sequence is run lifecycle | documented as lifecycle, but should stay diagnostic beside registration and `record.json.gz` |
| `repo-state.json` | `ploke_eval::runner::RepoStateArtifact` and read-side `ploke_records::run_record::RepoStateRecord` | writer: `runner.rs`; reader: run records and registration/report consumers | referenced/evidence | joins by run root, prepared checkout, commit/head fields | covered, but duplicate with setup phase in compressed run record |
| `indexing-status.json` | `ploke_eval::runner::IndexingStatusArtifact` | writer: `runner.rs`; reader: run record refs and operator reports | referenced-only | joins by run root and index/reindex phase | covered as DB/index witness; missing typed playback step |
| `snapshot-status.json` | `ploke_eval::runner::SnapshotStatusArtifact` | writer: `runner.rs`; reader: run record refs and snapshot helpers | referenced-only | joins by run root and final snapshot path | covered as final snapshot locator; missing DB-state drilldown step |
| `indexing-checkpoint.db` | Cozo backup file, referenced by run record/registration paths | writer: eval runner snapshot path; reader: explicit DB drilldown only | referenced-only | joins by run root and index phase | covered as witness, not a graph fact |
| `final-snapshot.db` | Cozo backup file, referenced by run record/registration paths | writer: eval runner terminal snapshot path; reader: explicit DB drilldown only | referenced-only | joins by run root and time-travel marker | covered as witness, not a graph fact |
| `agent-turn-trace.json` | `ploke_records::agent_turn::AgentTurnTraceRecord` | writer: `ploke_eval::runner::AgentTurnArtifact` projection; reader: `FsRunStore` agent-turn loader | loaded-side-payload | joins by task/user/assistant ids, response index, tool call ids | covered; needs iterator placement under agent-turn/model/tool families |
| `agent-turn-summary.json` | `ploke_records::agent_turn::AgentTurnSummaryRecord` | writer: `ploke_eval::runner::AgentTurnArtifact` projection; reader: `FsRunStore` agent-turn loader | loaded-side-payload and compact evidence | joins to trace by task/user ids and path | covered; duplicate pressure with trace and `record.json.gz` summaries |
| `llm-full-responses.jsonl` | `ploke_records::llm_response::RawFullResponseRecord` | writer: TUI/session response sidecar through eval runner path; reader: replay/probe code today | missing-loader for graph, typed sidecar for replay | joins by `(assistant_message_id, response_index)` | covered as a known gap; promote through graph load path |
| `record.json.gz` | current writer `ploke_eval::record::RunRecord`; read-side `ploke_records::run_record::RunRecord` | writer/reader: `crates/ploke-eval/src/record.rs`; graph load reads compared records through passive evidence | evidence / compared-run payload | joins by run id, record key, manifest id, instance id, branch/evaluation refs | covered; needs authority choice against `agent-turn-*` for turn/tool playback |
| `multi-swe-bench-submission.jsonl` | `ploke_eval::runner::MultiSweBenchSubmissionRecord` | writer: `write_msb_submission_artifact`; reader: MBE/campaign export and batch aggregation | missing-loader for graph | joins by instance id, org/repo/number, run root, submission path | newly added row; benchmark-facing patch export, not assistant text authority |
| `benchmark-patch-projection.json` | `ploke_records::evaluation::BenchmarkPatchProjectionRecord` | writer: `write_benchmark_patch_projection`; reader: evaluation/selection metrics and run-record refs | missing-loader or referenced-only | joins by run root, `record.json.gz`, submission path, benchmark target, candidate refs when present | newly added row; should become evaluation/benchmark witness |
| `config/ploke/proposals.json` | `Vec<ploke_tui::app_state::core::EditProposal>` | writer/reader: `ploke-tui` proposal persistence, with `PLOKE_PROPOSALS_PATH` override | outside-run-root unless explicitly redirected into run root | joins only if proposal ids are copied into turn/edit evidence | newly added row; useful edit evidence but not reliable playback source by path alone |

## Protocol Root Rows

The latest protocol root contained:

- 1 `*_tool_call_intent_segmentation_*.json`
- 20 `*_tool_call_review_*.json`
- 9 `*_tool_call_segment_review_*.json`

| File family | Owner type | Writer / reader boundary | Graph status | Join and sequence | Worksheet verdict |
| --- | --- | --- | --- | --- | --- |
| `*_tool_call_intent_segmentation_*.json` | `ploke_records::protocol::Artifact` with `ArtifactBody::ToolCallIntentSegmentation` | writer: `ploke-protocol` tool-call segmentation procedure; reader: protocol artifact loader / passive evidence | evidence summary, not procedure steps | joins by `procedure_name`, `subject_id`, `run_id`, `created_at_ms`, path | documented too vaguely before; needs explicit protocol procedure playback family |
| `*_tool_call_review_*.json` | `ploke_records::protocol::Artifact` with `ArtifactBody::ToolCallReview` | writer: `ploke-protocol` tool-call review procedure; reader: protocol artifact loader / passive evidence | evidence summary, not procedure steps | joins by `procedure_name`, `subject_id`, `run_id`, `created_at_ms`, path | documented too vaguely before; count and per-artifact sequence should be playable |
| `*_tool_call_segment_review_*.json` | `ploke_records::protocol::Artifact` with `ArtifactBody::ToolCallSegmentReview` | writer: `ploke-protocol` segment review procedure; reader: protocol artifact loader / passive evidence | evidence summary, not procedure steps | joins by `procedure_name`, `subject_id`, `run_id`, `created_at_ms`, path and segment id | documented too vaguely before; should drill down from segmentation to segment review |

Protocol artifacts are analysis/evaluation evidence over the run. They should
not replace the underlying agent-turn/tool-call records. Playback should expose
them as a protocol procedure family joined to the run and turn/tool sequence.

## Related Files Outside The Run Root

| File family | Owner type | Writer / reader boundary | RuntimePlayback role | Join key | Worksheet verdict |
| --- | --- | --- | --- | --- | --- |
| instance `run.json` | `ploke_eval::spec::PreparedSingleRun` | writer: prepared run setup; reader: runner and registration freezing | attempt setup input | task id, instance id, run root, campaign context | add as setup input, not run result |
| `registries/runs/<run-id>.json` | `ploke_eval::inner::registry::RunRegistration` | writer/reader: registry persist/load/discover | canonical run identity, lifecycle, artifact refs | run id, task id, run root, artifact refs | add as main discovery/lifecycle authority |
| `last-run.json` | `ploke_eval::run_history::LastRunRecord` | writer/reader: `record_last_run_at` / `load_last_run_at` | operator convenience pointer | run dir, completed timestamp | add as outside-run-root convenience, not authority |
| campaign `campaign.json` | `ploke_eval::campaign::CampaignManifest` | writer/reader: campaign save/load | campaign configuration and root locations | campaign id, dataset roots, eval/protocol policy | add as campaign scope input |
| campaign `closure-state.json` | `ploke_eval::closure::ClosureState` | writer/reader: recompute/load closure | reduced campaign status projection | campaign id, instance id, run refs, protocol refs | add as projection/cache, not History authority |
| campaign `slice.jsonl` | selected benchmark dataset rows | writer: Prototype 1 setup slice filter; reader: campaign setup/load | campaign input evidence | instance id | legacy gap: currently not a named project-owned row type if treated as persisted data |
| batch `batch.json` | `ploke_eval::spec::PreparedMsbBatch` | writer/reader: batch setup/runner | batch input | batch id, instances root, instance ids | add as batch scope input |
| `batch-run-summary.json` | `ploke_eval::runner::BatchRunSummary` | writer: batch runner; reader: closure failure collection and reports | batch rollup projection | batch id, instance task ids, run refs | add as historical report, not live registry authority |
| batch `multi-swe-bench-submission.jsonl` | aggregate `MultiSweBenchSubmissionRecord` rows | writer: batch runner aggregation; reader: submission export/tooling | benchmark export aggregate | instance id, org/repo/number | add as aggregate export; prefer per-run row for playback drilldown |
| eval log file | plain log text | writer: process logging | diagnostic only | timestamp/run id by convention | keep out of default playback; link only as diagnostic evidence |

## Coverage By User-Requested Category

| Category | Current worksheet status | Main gap |
| --- | --- | --- |
| `Graph` and `RunRecordSet` contents | mapped in `record-surface-map.md` | needs a completed row for every emitted latest-run family |
| emitted records not in `Graph` | partial | `multi-swe-bench-submission.jsonl`, `benchmark-patch-projection.json`, and protocol procedure artifacts need explicit rows |
| related crate records | partial | TUI proposal persistence and protocol procedure payloads need clearer run relation |
| `Channel` files | partial | no channel files appeared in this latest run root, but the graph still only counts envelopes where present |
| `History` items | partial-to-covered | History is loaded and typed, but the concrete playback iterator family is still design-level |
| Index/ReIndex DBs | partial | DB snapshots are path witnesses; no typed DB snapshot step family yet |
| selection-derived data | covered for sealed decision material | playback iterator placement and duplicate suppression are still to implement |
| benchmark/evaluation output | partial | patch projection and MBE submission rows were missing from the latest-run worksheet |
| operational metrics | partial | cost/time/token data needs shared cursor attribution rather than report-only summaries |

## Next Inventory Closure Tasks

1. Add `multi-swe-bench-submission.jsonl` and
   `benchmark-patch-projection.json` to the main record surface map as
   benchmark/evaluation witnesses.
2. Make protocol artifact rows procedure-specific instead of one broad
   `protocol-artifacts/*.json` bucket.
3. Record `config/ploke/proposals.json` as a TUI edit-proposal persistence
   surface that is only replay evidence when the run explicitly redirects or
   records the path.
4. Add campaign, registry, batch, and last-run rows as outside-run-root anchors.
   The registry row should be the discovery/lifecycle authority; `last-run.json`
   should stay an operator convenience.
5. Turn this worksheet into an implementation checklist by adding graph
   iterator names once the implementation thread defines them.
