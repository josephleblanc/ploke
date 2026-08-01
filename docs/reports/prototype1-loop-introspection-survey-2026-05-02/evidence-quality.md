# Evidence Quality

## Domain

Evaluation evidence, score comparability, evaluator/policy/oracle identity, artifact/runtime provenance, payload refs, and degraded evidence boundaries.

## Questions To Answer

- Can an operator tell why a child was kept/rejected, which evidence files were
  used, and whether the decision came from the parent selection path or from a
  later dashboard heuristic?
- Are baseline and treatment scores comparable: same instance set, same eval and
  protocol policy, same model/provider assumptions, same metric derivation code,
  same oracle eligibility definition, and source records present for both arms?
- Which actor/procedure produced the judgment: child runtime, parent observer,
  branch evaluation procedure, scheduler policy, dashboard metrics projection, or
  an external oracle/protocol artifact?
- Which Artifact/Runtime pair produced each evidence item: active parent
  checkout, temporary child checkout, child runtime id, successor runtime id,
  parent identity, node id, branch id, patch id, and surface commitment?
- Does every selection-critical evidence item have a source ref and payload hash,
  or is it only a mutable projection such as `scheduler.json`, `branches.json`,
  latest `runner-result.json`, monitor output, or a CLI-derived table?
- Where is the degraded boundary: missing baseline/treatment record, treatment
  failure, compile failure, latest-copy result, mutable registry summary,
  heuristic dashboard score, provisional preview block, or late successor
  evidence that may belong to ingress?
- For a 5-10 generation run, can an LLM reconstruct a compact trust bundle for a
  selected node: selected branch, selection source strength, evaluation artifact,
  compared run records, metric values, reasons, runner result, child completion
  journal line, artifact/surface refs, and known gaps?
- For longer runs, can we compare scores across protocol upgrades, metric
  derivation changes, evaluator code changes, oracle availability changes,
  branch merges, retries, and lineage forks without treating generation number
  as History height?

## Already Answered By Persisted Data

- Branch evaluation reports are persisted at
  `prototype1/evaluations/<branch-id>.json`; they carry baseline campaign,
  treatment campaign, branch id, branch registry path, evaluation artifact path,
  treatment manifest/closure paths, overall disposition, reasons, compared
  instances, per-instance baseline/treatment run-record paths, metrics snapshots,
  optional evaluation result, and a string status.
- Compared operational metrics are copied from `record.json.gz` through
  `RunRecord::operational_metrics()`: tool-call totals/failures, patch state,
  submission artifact state, partial patch failures, retry/streak counts,
  aborted/repair-loop flags, convergence, and oracle eligibility.
- The current branch evaluator is deterministic and explicit: it compares
  selected metric fields baseline-vs-treatment and rejects on regressions. The
  evaluator identity is implicit in code, not persisted as a versioned
  procedure/evaluator ref.
- Attempt-scoped child results are written at
  `nodes/*/results/<runtime-id>.json`, while `nodes/*/runner-result.json` is
  still a mutable latest copy. Both include node/generation/branch/status,
  disposition, optional treatment campaign id, optional evaluation artifact path,
  failure details, exit code, and recorded time.
- The transition journal records completion evidence for observed children:
  runtime id, transition id, generation, refs, paths, world, runner result path,
  and either evaluation artifact path plus overall disposition or failure
  disposition/detail/exit code.
- Scheduler and node records persist search policy, frontier/completed/failed
  ids, latest continuation decision, node id, parent node id, generation,
  instance/source/branch/candidate ids, target path, workspace root, binary path,
  runner request/result paths, status, and timestamps. This is useful for
  operator context but remains mutable projection.
- Branch registry persists source content/proposed content hashes, patch/artifact
  ids where available, branch status, selected branch id, active target, and a
  small latest evaluation summary. The summary currently omits a source ref or
  digest for the full evaluation artifact.
- History preview already preserves source refs and payload hashes for imported
  journal/document evidence and labels preview entries as degraded pre-History or
  projection evidence. Scheduler and branch registry documents are deferred as
  mutable projection/catalog records rather than admitted entries.
- History core already has the intended provenance slots for admitted entries:
  executor, input/output refs, observer, recorder, operational environment,
  payload ref/hash, proposer, procedure/policy, admitting authority, ruling
  authority, lineage, block id, and block height. Live evaluation logging does
  not yet feed this surface as authority.

## Partially Derivable

- Score comparability can be reconstructed for current runs by joining the
  evaluation report, baseline/treatment `record.json.gz` paths, run metrics, and
  scheduler/registry context. The derivation is degraded because there is no
  evaluation-report schema version, metric derivation version, evaluator code
  digest, source-record digest bundle, or explicit comparable-input-set hash.
- Evaluator identity is inferable from the current `evaluate_branch` function and
  from the branch evaluation report shape, but the persisted evidence does not
  name a stable evaluator/procedure version, policy ref, or oracle contract.
- Policy identity is split across scheduler search policy, campaign eval/protocol
  policies, the branch evaluator, and History `policy_ref`/surface commitments.
  Operators can inspect pieces, but there is no single evidence-quality record
  saying "this selection used policy P over evidence set E with evaluator V".
- Oracle identity is only represented as an operational metric
  `oracle_eligible` and as protocol/run artifacts elsewhere. The current branch
  evaluator compares eligibility booleans; it does not cite an oracle procedure,
  oracle model, oracle artifact, adjudication prompt, or oracle output hash.
- Artifact/runtime provenance is mostly joinable from node records, runner
  requests, invocations, transition journal refs, parent identity, child artifact
  committed records, active checkout advancement, and handoff records. It is not
  yet one compact trust bundle with stable whole-artifact refs, runtime binary
  digest, build recipe digest, and payload hashes.
- Selection evidence can be separated by source strength in projections:
  transition-journal selection is stronger than scheduler/registry latest state,
  but still degraded until sealed by History. Dashboard score/rank are local
  analysis heuristics, not the parent policy or an oracle.
- Degraded boundaries can be inferred from missing per-instance records, status
  strings such as `missing_treatment_record`, runner dispositions, latest-result
  copies, preview authority labels, and audit diagnostics. They are not emitted
  as one normalized `evidence_strength` / `degraded_reason` field at the
  operational event boundary.
- Payload hashes exist in History preview and History core, and content hashes
  exist for branch/source/proposed content. Most live persisted JSON sidecars do
  not carry their own payload hash when written, so later consumers must hash
  files after the fact and trust path stability.

## Requires New Logging

- A uniform operational event should record an evaluation comparison boundary:
  `event_kind=evaluation.result`, campaign/node/generation/branch/runtime ids,
  baseline/treatment campaign ids, evaluation artifact ref/hash, compared
  instance set hash, baseline/treatment record refs/hashes, metric derivation id,
  evaluator/procedure ref, eval/protocol policy refs, oracle policy/ref if used,
  disposition, reasons, and degraded fields for missing or incomplete arms.
- A selection boundary event should record `event_kind=selection.decision` with
  selected branch/node/artifact refs, decision policy ref, evaluator result refs,
  source evidence refs/hashes, whether the source is journal-backed,
  mutable-projection-backed, dashboard-derived, or sealed-History-backed, plus
  stop/continuation reason.
- A child observation/result event should record the attempt-scoped result path
  and hash, runner latest path only as projection, runtime id, binary/artifact
  refs, stream refs, exit status, evaluation artifact ref/hash, and whether the
  child result is terminal, failed-before-eval, or failed-after-eval.
- A runtime/artifact provenance event should record active parent artifact,
  child artifact, runtime id, parent identity ref/hash, binary path/hash if
  known, source/proposed/applied content hashes, patch id, surface commitment,
  and clean tree key when available.
- A degraded-evidence event or fields on every event should record
  `evidence_strength`, `degraded_reason`, `missing_refs`, `late_or_ingress`,
  `projection_only`, and `authority_status`, so LLMs can filter trust bundles
  without parsing prose diagnostics.
- A projection/digest event should record dashboard or report input refs and a
  source digest root when a metrics/report surface emits scores, ranks, selected
  trajectory, or generation summaries. Without this, projections are useful but
  not repeatably citeable.

## Natural Recording Surface

- The natural implementation surface is a small shared operational event helper
  used at transition/procedure boundaries, ideally backed by `tracing` fields and
  persisted as one JSONL stream under the campaign `prototype1/` directory. It
  should emit source refs/hashes and authority/degraded labels, not construct
  new domain-specific sidecar files.
- Evaluation comparison should log where
  `build_prototype1_branch_evaluation_report` finishes and where
  `run_prototype1_branch_evaluation` writes the evaluation report and registry
  summary. That boundary has the complete baseline/treatment state, metrics,
  disposition, reasons, artifact path, and branch context.
- Child result observation should log where the parent records
  attempt-scoped/runner results and journal `ObserveChild` entries. That boundary
  naturally binds runtime id, runner result path, evaluation artifact path,
  status/disposition, and failure detail.
- Selection should log at the parent continuation/selected-successor decision,
  before handoff mutates the active checkout. That is where operators need the
  evidence bundle proving why this child was selected and what policy stopped or
  continued the loop.
- Artifact/runtime provenance should log at materialize/build/spawn/handoff
  transition boundaries, using existing journal refs plus the History surface
  commitment and parent identity where available.
- History preview/report/metrics should consume these events later; they should
  not be the primary writers of operational truth. Their own outputs may get a
  projection event with input digest roots when saved.

## Essential

- Source ref plus payload hash for every selection-critical input: evaluation
  report, compared run records, runner attempt result, child completion journal
  line, parent identity, selected branch/artifact refs, and policy/evaluator refs.
- Explicit evaluator/procedure identity and metric derivation id for branch
  comparison; include eval/protocol policy refs and oracle identity/ref when
  oracle evidence contributes or is expected.
- Evidence strength / authority status: sealed History, transition journal
  evidence, attempt-scoped evidence, mutable projection, dashboard heuristic,
  degraded pre-History, late/ingress candidate, or diagnostic only.
- Baseline/treatment comparability keys: instance set, baseline/treatment
  campaign ids, model/provider or eval environment identity, per-arm record
  hashes, metric schema/derivation version, and missing-arm status.
- Selection trust bundle: selected node/branch/artifact, disposition/reasons,
  selection policy, continuation/stop reason, evidence refs/hashes, runtime id,
  generation as projection, and lineage/block only when real History supplies it.
- Artifact/runtime provenance: parent identity ref/hash, runtime id, node id,
  binary/build ref if available, clean artifact/tree/surface refs, patch/content
  hashes, and active checkout advancement evidence.

## Nice To Have

- Dashboard/report source digest root so metrics projections can be cited
  repeatably without upgrading them to authority.
- Bounded summaries of prompt/model/tool/oracle artifacts alongside refs/hashes,
  so LLMs can triage evidence without opening full `record.json.gz` or protocol
  artifacts.
- Confidence/uncertainty fields for heuristic rankings and oracle eligibility,
  especially when a run is incomplete, an oracle was unavailable, or a metric is
  a proxy.
- Cross-generation comparison keys for evaluator or policy upgrades, so longer
  runs can mark "not directly comparable" instead of silently charting one score
  axis.
- Normalized status enums for compared-instance status and degraded evidence
  reasons, replacing free-form strings where current reports use them.
- Operator-friendly compact evidence bundle view derived from the JSONL stream:
  one row per selected generation plus drill-down refs.

## Too Granular Or Noisy

- Full prompt/response bodies, complete stream logs, full proposed content, and
  entire compressed run records inline in the operational event. Store refs,
  hashes, bounded excerpts, and summaries instead.
- Every filesystem mtime/length change observed by monitor watch. That is useful
  for liveness display, not evaluation trust.
- Repeating whole scheduler, node, or branch registry snapshots on every event.
  Record stable ids, source refs/hashes, and changed semantic facts.
- Per-tool-call payload duplication in every evaluation or selection event.
  Keep aggregate metrics plus refs to run records/tool traces; add detailed tool
  events only where a tool call itself is the subject.
- Treating dashboard score components as authority fields. They are useful
  projection diagnostics unless and until the parent selection policy records the
  same procedure as its decision basis.
- Using generation, branch name, path, pid, or latest JSON path as a durable
  authority key. They can be projection coordinates or operational environment
  details, not the trust root.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:16-24` states the design
  constraints directly: do not store scores without evaluator/eval-set/policy
  identity, and do not let runtime self-report become promotion without
  independent verification.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:71-83` names the History
  evidence fields that matter: subject, transition/procedure/policy, executor,
  environment, observer, recorder, proposer, ruling authority, refs, timestamps,
  and payload hash.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:221-228` says successor
  verification should include selected Artifact, successor identity,
  policy-bearing surface digest, and required evidence refs; late evidence
  belongs to ingress.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:52-65` defines History,
  Entry, Ingress, and Projection; `history.rs:203-208` reiterates that scheduler,
  branch registry, node records, invocation files, ready/completion files, and
  monitor reports are evidence/projections until admitted.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:248-253` says admitted
  Artifacts should eventually carry a provenance manifest and History should
  reference tree key plus manifest digest rather than duplicate large evidence.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:264-270` lists current
  enforcement gaps, including no live `Parent<Ruling>` writer gate, no uniform
  startup admission carrier, no structural surface gate, no ingress import, and
  no signatures/consensus.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2208-2324` shows the
  intended entry payload/provenance shape: input/output refs, observer,
  recorder, environment, payload ref/hash, proposer, policy, admitting authority,
  ruling authority, lineage, block id, and height.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2428-2467` shows block
  header/seal material already has `policy_ref`, `surface`, selected successor,
  active artifact, Crown lock transition ref, and sealed time.
- `docs/reports/prototype1-record-audit/history-admission-map.md:29-48`
  classifies current sources: evaluation reports and attempt results are
  admissible evidence, latest runner result/scheduler/registry are projections or
  degraded evidence, and CLI reports are projection only.
- `docs/reports/prototype1-record-audit/history-admission-map.md:56-77`
  assigns field ownership for campaign/node/runtime refs, artifact/content
  hashes, run record paths, dispositions, timestamps, and statuses.
- `docs/reports/prototype1-record-audit/history-admission-map.md:79-106`
  calls out missing source digests for evaluation reports, missing path refs in
  registry summaries, inconsistent model/prompt/full-response refs, and metric
  derivation/source digests.
- `docs/reports/prototype1-record-audit/history-admission-map.md:224-235`
  notes that metrics selection source is explicit but dashboard rank/score are
  heuristics, not oracle truth or historical selection policy.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3866-3977` builds
  the branch evaluation report from baseline/treatment closure states and
  compressed run records, then stores per-instance metrics, evaluation result,
  status, reasons, and overall disposition.
- `crates/ploke-eval/src/branch_evaluation.rs:25-85` is the current evaluator:
  it compares fixed operational metrics and rejects on regressions.
- `crates/ploke-eval/src/operational_metrics.rs:36-69` defines the metrics
  snapshot, and `operational_metrics.rs:106-119` documents the current proxy
  boundary for valid patch, convergence, and oracle eligibility.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1429-1546` is the child leaf
  evaluation boundary: materialize, prepare treatment campaign, run eval and
  protocol, compare, write the evaluation report, then persist a registry
  summary.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1276-1310` writes both the
  attempt-scoped result and the latest runner-result projection.
- `crates/ploke-eval/src/intervention/scheduler.rs:15-54` defines search policy
  and continuation decision, while `scheduler.rs:75-130` defines node and runner
  result records used as operational projections/evidence.
- `crates/ploke-eval/src/intervention/branch_registry.rs:28-36` shows the latest
  evaluation summary lacks an evaluation artifact ref/hash, and
  `branch_registry.rs:682-710` writes that summary into the mutable registry.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:180-196` records
  observed child completion with runner result path and evaluation artifact path;
  `journal.rs:332-353` shows these are legacy JSONL storage variants that should
  be normalized before History admission.
- `crates/ploke-eval/src/cli/prototype1_state/event.rs:97-137` shows the current
  transition refs/paths/world/hash witnesses available at materialize/build/spawn
  boundaries.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1-6` documents
  that preview preserves source refs and payload hashes without writing sealed
  History; `history_preview.rs:668-707` hashes journal payloads and marks entries
  degraded pre-History.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1237-1331`
  classifies evaluation, invocation, attempt result, and latest runner-result
  documents with degraded/projection authority statuses.
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md:18-33`
  notes that monitor reports omit many source artifacts and remain narrower than
  the read-only History preview contract.
- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md:96-103`
  lists claims that would be too strong today, including treating transition
  journal selection as Crown authority or dashboard rank as parent policy.
