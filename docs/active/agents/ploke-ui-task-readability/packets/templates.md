# Packet Templates

Use these as short additions to the generated board packet. Do not repeat the
whole plan in worker chat when the packet already points here.

## Retainer

Find the minimum code and doc evidence needed for this lane.

- Stay read-only.
- Report exact files and line ranges.
- Name any outputs the main thread should avoid loading wholesale.
- Flag any place where UI code appears to clone semantic ids or records out of
  `ploke-tree::Graph`.

## Inventory Refresh Worker

Refresh only the stale coverage claims for the assigned first-wave rows.

- Work in docs unless a task explicitly opens a graph-gap lane.
- Classify every issue as `graph-semantic-gap`, `typed-loader-gap`,
  `ui-projection-gap`, `view-diagnostic-gap`, or `deferred-drilldown`.
- Do not broaden the inventory to unrelated rows.

## Projection Worker

Keep the projection borrowed and readability-facing.

- Do not add UI-owned semantic mirrors.
- Prefer references and small typed handles that cannot drift into authority.
- If a needed fact is missing, stop and report the exact missing join.

## Diagnostics Or Layout Worker

Work only inside the assigned diagnostics or layout lane.

- Diagnostics first: measure failures before tuning constants.
- Layout second: respond to accepted findings, not intuition alone.
- Any new owned value must be clearly render-only.

## Inspector/Docs Worker

Keep summary and snapshot surfaces aligned with accepted code and the
reference-only gate.

- Do not invent semantic facts or wrapper carriers in the summary layer.
- Keep snapshot text render-only.
- Do not edit governance docs, packet templates, review checklists, lane maps,
  or inventory files unless the board assigns an explicit governance or
  inventory task.

## Reviewer

Audit the patch as if it will become the foundation for a later browser/gui
consumer.

- Look for authority drift, clone drift, naming collapse, and weak typed joins.
- Prefer concrete findings with exact file/line references.
- Check `shared-brief.md`, `lanes.md`, and `orchestration.md` when reviewing
  board or packet setup.
- If the critique is sound and bounded, be ready to convert into a fixer.
