# Typed Persistence Survey Orchestration

Updated: 2026-05-10

## Goal

Run the `typed-persistence-spine` survey without overloading the main agent context.

The main thread owns the invariant, taxonomy, validation, and curated inventory. Sub-agents own bounded discovery for one execution-surface family at a time. Their findings are written as structured JSONL reports that can be validated with `jq`, queried with `rg`, and folded into a compact inventory without reading long prose reports.

## Directory Layout

Use this layout for survey artifacts:

```text
docs/active/plans/self-improvement-loop/typed-persistence-spine/
  README.md
  survey-index.md
  inventory.jsonl
  inventory.md
  reports/
    YYYY-MM-DD-<family-slug>.<agent-slug>.jsonl
  families/
    01-protocol-artifacts.md
    02-tool-calls-results.md
    03-monitor-projections.md
    04-edit-surface-patch-evidence.md
    05-llm-attempts.md
    06-database-context.md
    07-evaluation-oracle-targets.md
```

`reports/` contains bounded raw sub-agent output. `families/` contains curated family notes. `inventory.jsonl` is the machine-readable working map. `inventory.md` is the human-facing rollup.

## Naming Convention

Raw report files use:

```text
YYYY-MM-DD-<family-slug>.<agent-slug>.jsonl
```

Examples:

```text
2026-05-10-protocol-artifacts.agent-a.jsonl
2026-05-10-tool-calls-results.agent-b.jsonl
2026-05-10-monitor-projections.agent-c.jsonl
```

Surface ids use stable dotted names:

```text
protocol.artifact.decode
tool.call.arguments
monitor.agent_turn_trace
edit_surface.grant_record
llm.attempt.request
db.context.snippet
eval.oracle_target
```

Later implementation and review work should refer to these ids instead of restating file lists.

## Report Allocation And Collision Safety

The main thread allocates report paths before spawning sub-agents. Sub-agents do not invent their own target path.

Initial survey agents use explicit slugs:

```text
survey-a
survey-b
survey-c
```

Reviewer agents use explicit reviewer slugs:

```text
reviewer-a
reviewer-b
reviewer-c
```

Before writing, every agent must check whether its assigned report path already exists.

- If the path does not exist, write exactly that path.
- If the path exists, do not overwrite it.
- On collision, write to the next `-vN` filename, for example:

```text
2026-05-10-protocol-artifacts.survey-a-v2.jsonl
2026-05-10-protocol-artifacts.survey-a-v3.jsonl
```

The agent final response must report the collision and the actual path written. The main thread treats repeated collisions as an orchestration error and checks whether a previous agent is still running or whether a retry reused the wrong path.

## Taxonomy

Organize by execution-surface family first, not by crate or file path.

Families:

- `protocol-artifacts`
- `tool-calls-results`
- `monitor-projections`
- `edit-surface-patch-evidence`
- `llm-attempts`
- `database-context`
- `evaluation-oracle-targets`

Role tags:

- `source-record`: persisted fact/source shape
- `message`: transmitted parent/child/runtime message
- `artifact`: protocol, edit, or evaluation artifact
- `projection`: derived monitor/debug/operator view
- `ui-playback`: intended UI or replay projection
- `fixture`: test-only JSON fixture or comparison
- `sidecar`: metadata file adjacent to a primary record

Compliance states:

- `typed-compliant`
- `typed-reader-missing`
- `value-field`
- `value-staging`
- `ad-hoc-reader`
- `projection-needs-type`
- `owner-unclear`
- `test-only-ok`

Desired type homes:

- `ploke-records`: shared passive persisted DTOs
- `ploke-eval`: runtime-owned records, typestate mirrors, and local projections
- `ploke-tree`: playback/browser projection types
- `ploke-llm`: provider/request/response attempt records
- `ploke-tui`: edit-surface executor/proposal-facing shapes
- `test-only`: fixtures and comparison helpers only

## JSONL Report Schema

Every agent report is JSONL. Each line is one object with `kind`.

Required `summary` line:

```json
{"kind":"summary","family":"protocol-artifacts","agent":"agent-a","rows":4,"non_compliant":1,"needs_semantic_decision":0,"notes":"Found Value staging in artifact decode; no implementation performed."}
```

Required `surface` lines:

```json
{"kind":"surface","id":"protocol.artifact.decode","family":"protocol-artifacts","roles":["artifact","source-record"],"path":"crates/ploke-records/src/protocol/mod.rs","lines":"120-210","writer":"Artifact serializers","reader":"Artifact Deserialize impl","current_type":"Artifact + RawArtifact with serde_json::Value staging","desired_type_home":"ploke-records","nested_payloads":["procedure input","procedure output","artifact payload"],"replay_joins":["artifact id","protocol kind"],"ui_drilldown":["typed protocol payload"],"non_compliance":["value-staging"],"smallest_verification":"cargo test -p ploke-records protocol"}
```

Optional `follow_up` lines:

```json
{"kind":"follow_up","family":"protocol-artifacts","priority":1,"item":"Replace RawArtifact Value staging with typed payload decode.","surface_ids":["protocol.artifact.decode"]}
```

## Report Caps

Each sub-agent report must stay bounded:

- at most 20 `surface` rows;
- at most 10 exact line references;
- at most 5 `follow_up` rows;
- no pasted code blocks;
- no raw JSON records, prompts, responses, traces, journals, or logs.

If a family exceeds the cap, split it by semantic child family, for example:

```text
llm-attempts.requests
llm-attempts.responses
llm-attempts.provider-errors
llm-attempts.tool-bridges
```

## Validation Commands

Validate all report files are parseable:

```bash
jq -c . docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/*.jsonl >/dev/null
```

List summaries without reading details:

```bash
jq -r 'select(.kind=="summary") | [.family,.rows,.non_compliant,.needs_semantic_decision] | @tsv' docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/*.jsonl
```

List non-compliant surfaces:

```bash
jq -r 'select(.kind=="surface" and (.non_compliance|length>0)) | [.id,.path,(.non_compliance|join(","))] | @tsv' docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/*.jsonl
```

Check required surface keys:

```bash
jq -e 'select(.kind=="surface") | has("id") and has("family") and has("path") and has("reader") and has("current_type") and has("desired_type_home") and has("non_compliance") and has("smallest_verification")' docs/active/plans/self-improvement-loop/typed-persistence-spine/reports/*.jsonl >/dev/null
```

## Orchestration Loop

1. Pick one family.
2. Allocate one unique report path for the sub-agent.
3. Assign one sub-agent to survey that family only.
4. Require the agent to write one JSONL report under `reports/`, after checking that the assigned path does not already exist.
5. Require the agent final response to include only:
   - report path;
   - whether a collision occurred;
   - row count;
   - top three blockers;
   - suggested next split, if any.
6. Validate report parseability and required keys with `jq`.
7. Query summaries and non-compliant rows without reading the full report.
8. Spot-check only exact file and line ranges cited by the agent.
9. Fold accepted rows into `inventory.jsonl`.
10. Curate the matching `families/*.md` page only after the JSONL inventory is coherent.
11. Assign reviewer agents against the inventory layer, not against the whole codebase.

## Reviewer Passes

After initial discovery, use review agents for bounded consistency checks:

- Coverage reviewer: compare `rg` hits for production JSON/JSONL readers against reported surface ids.
- Type-home reviewer: flag inconsistent `desired_type_home` decisions across families.
- Replay reviewer: check whether `replay_joins` are sufficient to connect parent, child, patch, tool, database context, evaluation, and successor records.
- UI reviewer: check whether `ui_drilldown` fields support the intended tree inspection without anonymous JSON.

Reviewer output should also be JSONL and should reference existing surface ids.

## Main Thread Rules

The main thread does not read broad files, long agent reports, large logs, journals, traces, prompts, responses, or raw JSONL artifacts.

The main thread may read:

- this plan;
- `typed-persistence-spine-plan.md`;
- `typed-data-coverage-report.md`;
- `jq` summaries;
- exact cited source line ranges;
- compact `rg` result sets.

If a report is too large, missing required fields, or written as narrative prose, reject it and ask for a corrected JSONL report before integrating it.

## Sub-Agent Prompt Template

```text
Task handle: typed-persistence-spine
Family: <family-slug>

Goal: survey this family only. Do not implement. Do not read large logs, journals, traces, prompts, responses, or raw JSONL artifacts. Do not paste code or raw JSON into your final answer.

Invariant: owned persisted/transmitted JSON/JSONL must be read through named Rust types. Production serde_json::Value field walking, Value staging, and ad hoc JSON readers are non-compliant.

Write your report to:
<exact report path allocated by the main thread>

Before writing, check whether that path exists. If it exists, do not overwrite it. Write to the next `-vN` filename and report the collision plus the actual path written.

Use JSONL with one required summary object and one surface object per discovered surface. Follow docs/active/plans/self-improvement-loop/typed-persistence-survey-orchestration.md.

Final response must include only:
- report path
- whether a collision occurred
- row count
- top three blockers
- suggested next split, if any
```

## Completion Bar

The survey orchestration is complete when:

- every initial family has at least one valid JSONL report;
- every report has a summary line and bounded surface rows;
- `inventory.jsonl` contains accepted surface ids for every owned Prototype 1 execution surface;
- `inventory.md` gives the high-level view without duplicating every raw detail;
- reviewer passes have checked coverage, type-home consistency, replay joins, and UI drilldown completeness.
