# Boundary Audit Checklist

Use this for review lanes and for main-thread acceptance of worker patches.

## Semantic Authority

- Does any UI-owned type now stand in for `Artifact`, `Runtime`, `Patch`,
  `Candidate`, `Selection`, `History`, or equivalent graph meaning?
- Was any semantic id, record, or partial record copied out of
  `ploke-tree::Graph` for later semantic use?
- Did the patch introduce a mirror type, wrapper carrier, or convenience report
  that merely reconstructs graph meaning under a new name?
- If a fact was missing, was the fix pushed to the right layer instead of being
  mirrored in UI state?
- Do the snapshot and app-summary surfaces stay reference-derived, with no
  semantic ids or records promoted into summary text?

If any answer above is yes, the phase fails until repaired.

## Typed Persistence

- Did the patch introduce any `serde_json::Value` walking or stringly recovery
  of owned persisted data?
- Are new joins backed by named typed records or domain-owned answer objects
  below the UI boundary?
- If coverage claims changed, were the relevant inventory rows updated?

## Readability Discipline

- Does the patch measure or improve a real readability failure rather than only
  changing constants?
- If layout tuning happened, is there diagnostic evidence explaining why?
- Are new owned values clearly geometry, styling, diagnostics, or render-only
  text rather than latent semantic carriers?
- Do snapshot fields and summary labels remain render-only projections of
  accepted diagnostics?

## Audit Cadence

- Was the reference-only audit performed for this lane completion?
- If the patch touches projection, diagnostics, snapshot, app, layout,
  inventory, or governance docs, is there an explicit audit result?
- Does the review explicitly state that no semantic clone, wrapper, or mirror
  workaround remains?
- Does the review explicitly state that summary and snapshot text are
  reference-only projections?

## Naming And Structure

- Do names preserve role/state structure instead of collapsing it into long
  compounds?
- Did the patch add report/info/status/output blobs where a smaller structural
  carrier or module boundary would have been clearer?
- Are shared-pressure files staying narrow, or did the patch spread across
  module-export hotspots without need?

## Worker Hygiene

- Did the worker stay within its lane-owned files?
- Are commands/tests focused and bounded?
- Does the final report name exact files, lines, commands, and deferred risks?
