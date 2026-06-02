# 2026-05-23 Prototype 1 Closure Observability

Status: active design note.

Purpose: preserve the current understanding of how initial eval and closure/protocol state should be surfaced to operators, the GUI, and future agents.

Related files:

- `crates/ploke-eval/src/closure.rs`
- `crates/ploke-eval/src/cli.rs`
- `crates/ploke-tree/src/store/fs.rs`
- `crates/ploke-tree/src/graph/build/passive.rs`
- `crates/ploke-egui/src/ui/app/shell.rs`
- `docs/workflow/skills/ploke-eval-operator/SKILL.md`

## Problem

Prototype 1 setup and closure commands already write enough durable evidence to explain a run, but that evidence is split across several surfaces:

- `closure-state.json` summarizes registry, eval, and protocol coverage.
- Eval run roots contain `record.json.gz`, `execution-log.json`, `agent-turn-summary.json`, and LLM response traces.
- Protocol review artifacts live under the eval-home protocol tree.
- `ploke-eval inspect ...` can render the details if the operator knows the right command sequence.
- `ploke-egui` currently has run-record and selection-time protocol displays, but no first-class baseline closure/protocol panel that is visible before sealed History blocks exist.

The tempting reduction is to treat `closure status` as the full answer. That loses the protocol issue surface, tool-call trace, model behavior, and drilldown path.

## Current CLI Surface

The current useful command sequence is:

```bash
./target/debug/ploke-eval closure status --campaign <campaign>
./target/debug/ploke-eval closure status --campaign <campaign> --format json
./target/debug/ploke-eval inspect operational --record <record.json.gz> --format json
./target/debug/ploke-eval inspect protocol-overview --campaign <campaign> --format json
./target/debug/ploke-eval inspect protocol-overview --record <record.json.gz> --view overview
./target/debug/ploke-eval inspect protocol-overview --record <record.json.gz> --view calls
./target/debug/ploke-eval inspect protocol-overview --record <record.json.gz> --view segments
./target/debug/ploke-eval inspect protocol-artifacts --record <record.json.gz>
./target/debug/ploke-eval inspect tool-calls --record <record.json.gz>
./target/debug/ploke-eval inspect failures --record <record.json.gz>
```

For the `p1-google-live-multigen-2g3x3-20260523-192017` campaign, these commands showed:

- closure status: registry complete, eval complete, protocol complete.
- operational metrics: 17 tool calls, 0 failed tool calls, no patch attempted, empty submission artifact, patch projection check passed.
- protocol overview: 17/17 call reviews, 6/6 usable segment reviews, no missing protocol coverage, but issue families still present.
- protocol issue families: `recoverable_detour`, `mixed`, `partial_next_step`, `redundant_thrash`, `search_thrash`.
- protocol issue tools: mostly `read_file` and `request_code_context`.
- protocol artifacts: 24 artifacts, including 1 intent segmentation artifact, 17 tool-call review artifacts, and 6 segment-review artifacts.

That is enough to say both:

- The harness/eval path succeeded mechanically.
- The model still showed protocol-level friction, especially repeated reading and context-search behavior.

## CLI Gap

The CLI has the data, but the narrative is not one command.

Add or refine one operator-facing command that joins the existing surfaces without requiring log digging. Candidate shape:

```bash
./target/debug/ploke-eval inspect run-narrative --campaign <campaign> [--instance <id>] [--format table|json]
```

The command should:

1. Load closure state.
2. List each instance row and its registry/eval/protocol status.
3. Resolve the eval `record_path` for complete or partial runs.
4. Attach operational metrics.
5. Attach protocol coverage and issue summaries.
6. Attach artifact locators for drilldown.
7. Render next commands for selected detail paths.

JSON output should be typed and stable enough for agents and GUI import. Table output should be concise and causal: state what completed, what did not, and what evidence explains the result.

## GUI Gap

The GUI should not wait for sealed History before surfacing baseline closure/protocol state.

Add a first-class evidence panel that can render from passive evidence alone:

- campaign closure summary
- instance status table
- eval run record summary
- tool-call summary
- protocol coverage summary
- protocol issue-family summary
- segment review table
- call review drilldown
- artifact locators and source record links

The UI should continue to use the established path:

```text
ploke-records -> ploke-tree::Graph -> borrowed projection -> ploke-egui renderer
```

Do not make `ploke-egui` parse raw protocol JSON or CLI report text directly. If a typed witness is missing, add it in `ploke-records` or `ploke-tree` first.

The natural drilldown shape is:

```text
Campaign
  Instance
    Closure row
    Eval run
      Run record
      Tool calls
      Operational metrics
    Protocol aggregate
      Intent segmentation
      Segment reviews
      Tool-call reviews
      Issue families
      Raw artifact locator
```

## Graph/Record Direction

Current typed loading already covers part of this:

- `FsRunStore::load_protocol_artifacts_evidence` loads protocol artifacts as passive evidence.
- `GraphBuilder::ingest_protocol_artifacts` attaches protocol summaries and artifact evidence.
- `Graph::protocol_artifacts()` exposes the loaded evidence.
- `Graph::run_records()` exposes compressed run-record evidence.

The missing piece is a projection that joins closure row, run record, protocol aggregate, and artifact refs into one GUI-ready witness that is not selection- or History-dependent.

Candidate type direction:

```text
ClosureRunEvidence
  campaign_id
  instance_id
  registry_status
  eval_status
  protocol_status
  record_ref
  operational_metrics
  protocol_coverage
  protocol_issue_summary
  protocol_artifact_refs
```

This should be a typed read-side witness, not a mirror of every raw artifact field.

## Agent Skill Direction

The repo-local `ploke-eval-operator` skill should teach future agents the closure/protocol narrative path:

1. Start from `closure status`.
2. Use `closure status --format json` to get record refs and protocol artifact dirs.
3. Use `inspect operational` for run-level success/failure.
4. Use `inspect protocol-overview` for protocol coverage and issue families.
5. Use `inspect protocol-artifacts` for artifact inventory and selective full detail.
6. Use `inspect tool-calls` and `inspect failures` to explain the local model/tool behavior.
7. Avoid reading raw logs unless typed artifacts are missing or contradictory.

This makes the skill a retrieval guide rather than a stale bundle of one-off paths.

## Implementation Slices

1. Update `ploke-eval-operator` with the current command sequence and narrative rules.
2. Add a CLI narrative command or JSON report that joins closure, operational, and protocol evidence.
3. Add typed `ploke-tree` witness/projection for baseline closure evidence independent of sealed History.
4. Add a `ploke-egui` panel for baseline closure/protocol state with drilldown to run records, tool calls, protocol segments, and artifact locators.
5. Add tests using a fixture with no sealed History blocks and complete baseline closure evidence.

## Guardrails

- Do not use logs as the primary source when typed artifacts exist.
- Do not flatten mechanical eval success and protocol friction into one verdict.
- Do not make GUI state depend on sealed History for baseline eval/protocol visibility.
- Do not have `ploke-egui` parse raw CLI text.
- Do not require every optional artifact ref path to exist. For example, a successful run may expose a candidate `parse_failure` path in closure refs without a `parse-failure.json` file existing.
