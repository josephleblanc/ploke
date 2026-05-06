# Status

Prototype 1 has a substantial partial implementation of durable child result and evaluation evidence. A child runtime writes an attempt-scoped `Prototype1RunnerResult`, the parent observes it through typed `Child<State>` and `ObserveChild` journal records, and a successful attempt points to a branch evaluation report that compares baseline and treatment run records. This is enough for generation-local successor continuation.

It is not yet a complete archive-admission record. The current durable pieces do not form one authoritative object that says "this child artifact/evaluation set is admitted to the archive under policy P, evaluator E, evidence root R, by Crown/History authority H." Existing records are mostly attempt evidence, comparison evidence, mutable projections, or successor-selection decisions.

The closest live spine is:

```text
campaign_id/node_id/runtime_id
  -> nodes/<node-id>/results/<runtime-id>.json
  -> evaluation_artifact_path
  -> prototype1/evaluations/<branch-id>.json
  -> compared_instances[*].{baseline_record_path,treatment_record_path}
  -> RunRecord/tool/protocol/provider evidence
  -> SuccessorDecision / continuation decision
```

# Existing Pieces

## Child Runtime Result

- `Prototype1RunnerResult` records `campaign_id`, `node_id`, `generation`, `branch_id`, node status, runner disposition, optional `treatment_campaign_id`, optional `evaluation_artifact_path`, detail, exit code, stdout/stderr excerpts, and `recorded_at` (`crates/ploke-eval/src/intervention/scheduler.rs:148`-`171`).
- There are two persisted result surfaces:
  - latest projection: `prototype1/nodes/<node-id>/runner-result.json` (`crates/ploke-eval/src/intervention/scheduler.rs:260`-`262`);
  - attempt-scoped result: `prototype1/nodes/<node-id>/results/<runtime-id>.json` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:350`-`357`).
- The child writes both through `record_attempt_runner_result`: first the attempt result, then the latest projection via `record_runner_result` (`crates/ploke-eval/src/cli/prototype1_process.rs:1651`-`1662`).
- Success results carry the treatment campaign id and evaluation report path (`crates/ploke-eval/src/cli/prototype1_process.rs:1628`-`1648`). Treatment failures carry structured failure detail/excerpts but no evaluation path (`crates/ploke-eval/src/cli/prototype1_process.rs:1600`-`1623`).
- The parent-side `ObserveChild` transition loads the attempt result, classifies success/failure, and on success loads the evaluation report from `runner_result.evaluation_artifact_path` (`crates/ploke-eval/src/cli/prototype1_state/c4.rs:373`-`443`).

## Child State/Journaling

- Child lifecycle is typed as `Child<Starting> -> Child<Ready> -> Child<Evaluating> -> Child<ResultWritten>` and projects durable `JournalEntry::Child` records (`crates/ploke-eval/src/cli/prototype1_state/child.rs:30`-`40`, `156`-`178`, `195`-`207`).
- The child invocation path records ready/evaluating/result-written and optionally sends a file-channel `ResultWritten` message (`crates/ploke-eval/src/cli/prototype1_process.rs:2122`-`2160`, `2200`-`2217`).
- `ObserveChild` appends `CompletionEntry` before/after records with `runtime_id`, refs, paths, `runner_result_path`, and either succeeded `{evaluation_artifact_path, overall_disposition}` or failed `{disposition, detail, exit_code}` (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:165`-`195`; writer at `crates/ploke-eval/src/cli/prototype1_state/c4.rs:328`-`339`, `414`-`425`, `471`-`482`).
- The live `prototype1-state` path does use this observation transition when completing a planned child, then derives selection input from the successful evaluation (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5369`-`5403`).

## Branch Evaluation

- Branch evaluation artifacts are written to `prototype1/evaluations/<branch-id>.json` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6280`-`6287`).
- `Prototype1BranchEvaluationReport` stores baseline campaign id, branch id, treatment campaign id, branch registry path, self path, treatment campaign manifest, treatment closure state, overall disposition, reasons, and compared instance rows (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6995`-`7007`).
- Each compared row stores instance id, baseline/treatment `record.json.gz` paths, baseline/treatment `OperationalRunMetrics`, per-instance `BranchEvaluationResult`, and a status string (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7009`-`7018`).
- The report builder reads baseline/treatment `RunRecord`s, derives operational metrics, evaluates per-instance regressions/improvements, and stores missing-record failure statuses (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6306`-`6380`).
- The child evaluation path writes the branch evaluation report and also writes a compact branch-registry evaluation summary (`crates/ploke-eval/src/cli/prototype1_process.rs:1928`-`1953`, `1973`-`1985`).

## Metrics And Telemetry Evidence

- `OperationalRunMetrics` captures tool call totals/failures, patch attempt/apply state, submission artifact state, partial patch failures, repeated same-file patch behavior, abort/repair-loop flags, nonempty-valid-patch proxy, convergence, and oracle eligibility (`crates/ploke-eval/src/operational_metrics.rs:36`-`69`).
- Metrics are derived from `RunRecord` tool calls, agent turn artifacts, packaging state, and outcomes (`crates/ploke-eval/src/operational_metrics.rs:71`-`135`).
- `RunRecord` is the main per-run evidence container: manifest id, metadata, phases, DB time-travel markers, conversation, timing (`crates/ploke-eval/src/record.rs:149`-`170`).
- Run metadata captures benchmark instance/repo/base SHA, run arm, agent model/provider/endpoint/system prompt/tool schema fields, runtime limits, and budget (`crates/ploke-eval/src/record.rs:575`-`663`).
- Turn/tool telemetry includes LLM request/response, tool calls, turn outcome, and `AgentTurnArtifact` events (`crates/ploke-eval/src/record.rs:807`-`840`; tool result schema at `crates/ploke-eval/src/record.rs:1238`-`1260`; event schema at `crates/ploke-eval/src/runner.rs:731`-`846`).
- Campaign/closure records preserve eval-set context: campaign manifest contains dataset sources, model/provider, required procedures, eval/protocol policy, framework (`crates/ploke-eval/src/campaign.rs:25`-`49`, `51`-`83`); closure rows carry dataset label, eval/protocol class, eval/protocol failure strings, and artifact refs (`crates/ploke-eval/src/closure.rs:154`-`200`).

## Successor Continuation

- Successor selection is explicitly generation-local evidence for choosing the next successor (`crates/ploke-eval/src/successor_selection/mod.rs:1`-`6`, `28`-`33`).
- `SelectionInput` contains candidate node/branch/generation, branch disposition, evaluation artifact path, and parent-vs-child metric comparisons (`crates/ploke-eval/src/successor_selection/evidence.rs:9`-`41`).
- The only active selection domain is operational (`crates/ploke-eval/src/successor_selection/registry.rs:8`-`24`), and it compares operational metric deltas such as failed tools, partial patches, aborts, convergence, and oracle eligibility (`crates/ploke-eval/src/successor_selection/domains/operational.rs:75`-`128`).
- `SuccessorDecision` records procedure id, candidate node, selected branch, branch disposition, outcome, findings, and rationale (`crates/ploke-eval/src/successor_selection/decision.rs:8`-`19`). Procedure id is currently `successor-selection:v1` (`crates/ploke-eval/src/successor_selection/mod.rs:21`).
- The parent appends the successor selection decision into the transition journal and records a continuation decision before optional successor handoff (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5978`-`6015`). The journal successor record can embed the `SuccessorDecision` (`crates/ploke-eval/src/cli/prototype1_state/successor.rs:20`-`29`, `90`-`105`).

# Gaps

- No single archive-admission record joins `campaign_id`, `node_id`, `branch_id`, `runtime_id`, treatment campaign id, evaluation path, compared run record paths, selection/admission policy, evaluator identity, evidence hashes, and admission outcome. A prior persistence synthesis calls out this exact missing object (`docs/workflow/evalnomicon/drafts/prototype1-persistence-map-2026-05-03/08-synthesis.md:102`-`105`).
- Runtime id is in the attempt-result filename and child journal, but not inside `Prototype1RunnerResult`; an archive consumer has to recover it from path/journal context.
- Branch evaluation copies derived metrics and record paths, but does not commit record digests, evaluator version, evaluator code/surface digest, or an explicit eval-set identity beyond campaign/treatment manifest refs.
- The active selection policy is only successor-local. `SuccessorDecision` says whether to continue/explore/stop, not whether to admit the child/evaluation bundle into an archive with longer-lived authority.
- Only the operational domain is active. `DomainName` lists protocol, patch, oracle, and adjudication domains, but registry default evaluates only `Operational` (`crates/ploke-eval/src/successor_selection/domains/mod.rs:12`-`20`; `crates/ploke-eval/src/successor_selection/registry.rs:19`-`24`).
- Failure classes are scattered: runner dispositions distinguish compile/treatment failure; closure rows carry eval/protocol failure strings; tool failures and turn errors/timeouts live inside `RunRecord`; branch evaluation uses string statuses like `missing_treatment_record`. There is no normalized archive-facing failure taxonomy.
- Tool/process/provider telemetry is available, but mostly through run records, sidecars, streams, and tracing. It is not folded into a typed child admission/evaluation summary.
- History has the intended authority model, but child evaluation evidence is not yet admitted into sealed blocks. The History docs say artifact-local provenance manifests for self-evaluations/build/runtime records are intended but not implemented (`crates/ploke-eval/src/cli/prototype1_state/history.rs:248`-`253`), and stored sealed block loading currently rejects nonempty entries (`crates/ploke-eval/src/cli/prototype1_state/history.rs:950`-`961`).
- The History block design explicitly names stochastic evidence/eval refs/risk summaries as future block commitments when policy uses them for admission (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2708`-`2711`), but current live handoff blocks are minimal and not child-evaluation admission records.

# Recommended Next Slice

Implement a small typed `ChildEvaluationRecord` or `ChildAdmissionEvidence` record as an evidence join, not as final History authority yet.

Minimum fields:

- schema/version and `recorded_at`;
- `campaign_id`, `node_id`, `generation`, `branch_id`, `runtime_id`;
- child invocation path, attempt result path, latest runner-result path;
- runner disposition/status/detail/exit code;
- `evaluation_artifact_path`, `treatment_campaign_id`, treatment campaign manifest, treatment closure state;
- compared instance refs: instance id, baseline/treatment record paths, optional record digests, per-instance status/disposition;
- summarized metrics already used by `SelectionInput`;
- policy/procedure refs: `successor-selection:v1`, branch evaluator id/version, campaign eval/protocol policy refs, dataset source refs;
- explicit `archive_admission: not_admitted | candidate_evidence | admitted` or equivalent, initially `candidate_evidence`.

Smallest implementation path:

1. Build this record from the existing successful `ObserveChild` path, immediately after loading `Prototype1RunnerResult` and `Prototype1BranchEvaluationReport`.
2. Persist it under an attempt-scoped path such as `prototype1/nodes/<node-id>/evaluations/<runtime-id>.json`, and optionally mirror a latest projection if needed for monitor UX.
3. Add a reference to that path in `SelectionInput`/`SuccessorDecision` or the successor journal record, without changing selection behavior.
4. Add one monitor/history-preview row that prefers this record for child evaluation evidence.
5. Defer sealed History admission until the record is stable; then admit only digest/reference roots into History or an artifact-local provenance manifest, not copied run-record payloads.

This slice turns the existing evidence spine into one durable child-evaluation bundle while preserving the distinction between archive admission and generation-local successor continuation.
