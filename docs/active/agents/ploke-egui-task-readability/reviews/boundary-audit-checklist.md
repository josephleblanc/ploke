# Boundary Audit Checklist

Use this after each vertical slice.

## Graph Authority

- `ploke-egui` imports run data through `ploke_tree::Graph`.
- No code in `ploke-egui` parses Prototype 1 files directly.
- No code in `ploke-egui` reads owned persisted JSON through `serde_json::Value`
  or ad hoc field walking.
- New egui types are view state, geometry, render-only labels, diagnostics, or
  true derived values.
- No allocated semantic value is cloned out of `ploke_tree::Graph`.
- No copied id, ref, record, partial record, report, info, status, summary, or
  payload type is used as a semantic stand-in for a graph reference.
- No Artifact, Runtime, role, History, candidate, evidence, or source-status
  fact is carried through `InspectorRow` or any other row-shaped semantic type.
- No mirror type reconstructs graph meaning to avoid borrowing from
  `ploke_tree::Graph`.
- Any new semantic relation was added to `ploke-tree::graph::Graph`, not mirrored
  in egui.

Searches:

```bash
rg "serde_json::Value|from_reader|read_to_string|load_record|RunRecordSet" crates/ploke-egui/src
rg "struct .*Artifact|struct .*Runtime|struct .*Patch|struct .*Candidate|struct .*Selection|struct .*History" crates/ploke-egui/src
rg "struct .*Report|struct .*Info|struct .*Status|struct .*Summary|struct .*Payload|clone\\(|to_owned\\(|to_string\\(" crates/ploke-egui/src
rg "ploke_records::" crates/ploke-egui/src
```

Findings from these searches are not automatically bugs. They are prompts to
verify whether the type/import/allocation is presentation-only, truly derived,
or authority-bearing. Any authority-bearing clone is a hard blocker.

## Diagnostics

- Diagnostics operate on node positions, label bounds, edge paths, ranks,
  subtree spans, and references into the graph.
- Findings are ranked and actionable.
- Diagnostics do not claim causal improvement or History authority.
- Snapshot fields are clearly diagnostic/view fields.
- Snapshot fields do not serialize graph facts, graph ids, or wrapper reports
  copied from `ploke_tree::Graph`.

## Task Readability

- Labels are near-zero collision targets.
- Selected/promoted lineage has explicit visibility metrics.
- Sibling subtree overlap is measured before layout tuning claims success.
- Rank spacing is measured and stable.
- Long edges and crossings are weighted by relevance, not just raw count.

## Failure Policy

A phase that passes tests but leaves an allocated semantic clone in the accepted
path has failed. Stop downstream work, repair the reference boundary, and rerun
the review before continuing.

## Verification

Use bounded output:

```bash
cargo check -p ploke-egui 2>&1 | tail -n 80
cargo test -p ploke-egui <test-filter> 2>&1 | tail -n 80
```

For graph semantic changes:

```bash
cargo test -p ploke-tree <test-filter> 2>&1 | tail -n 80
```
