# Packet Templates

These are prompt fragments for sub-agents. The main thread should combine the
fragment with the generated `xtask orchestrate packet`.

## Retainer

Role: read-only locator and context retainer.

Task:

- Locate the exact code ranges for the requested question.
- Report files, line ranges, and the smallest verification command.
- Do not edit files.
- Do not read large docs or logs wholesale.
- Identify whether the question is view-only, projection-only, or semantic
  graph authority.

## Projection Worker

Role: implementation worker for artifact projection identity.

Task:

- Work only in the board-allowed `egui-projection` surfaces.
- Add borrowed structure needed by diagnostics.
- Do not introduce egui-owned semantic domain carriers.
- Do not clone ids, refs, records, or semantic payloads out of
  `ploke_tree::Graph`.
- Keep projection derived from `&ploke_tree::Graph`.
- Add focused tests for projection shape.

Stop if a needed relation is missing from `ploke_tree::Graph`.

## Diagnostics Worker

Role: implementation worker for geometry diagnostics.

Task:

- Work only in `egui-diagnostics` surfaces.
- Add metrics over existing widget geometry.
- Prefer rank spacing, selected-path straightness/visibility, subtree overlap,
  and weighted crossings before local edge-density heatmaps.
- Add synthetic tests that fail on known bad geometry.

Diagnostics are view facts, not loop authority.

## Layout Worker

Role: implementation worker for layout tuning.

Task:

- Work only in `egui-layout` surfaces.
- Use diagnostics to tune layout.
- Do not change projection semantics to make layout easier.
- Preserve stable mental-map behavior where practical.

Start this lane after measurement exists.

## Snapshot/Docs Worker

Role: implementation and documentation worker.

Task:

- Persist new diagnostic fields and findings in snapshot output.
- Update app summary text if useful.
- Keep docs indexed.
- Do not turn diagnostic snapshots into semantic run records.

## Graph-Gap Worker

Role: implementation worker only after a proven semantic gap.

Task:

- Work in `ploke-tree::graph` or inventory docs only.
- Add semantic relation to `ploke-tree::Graph` deliberately.
- State source facts, join keys, ambiguity, and tests.
- Do not add an egui-local mirror.

## Reviewer

Role: independent review.

Task:

- Check graph-boundary discipline.
- Check that diagnostics are geometry/view facts.
- Check that tests catch task-readability failures.
- Check for cloned semantic values, copied ids/refs/records, wrapper reports,
  mirror types, direct record parsing, or authority drift.
- Report findings first, with file/line references.
