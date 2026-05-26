# Inspector UX Discipline

This document records the working UI style for `ploke-egui` inspector and
drilldown work. The target user is an operator, data analyst, or data scientist
trying to understand a Prototype 1 run from typed evidence.

The inspector should make the run explain itself. It should expose enough detail
to find data bugs, protocol gaps, and model behavior problems without becoming a
raw artifact browser.

## Principles

- Evidence first, not rows first. A row is useful only when it helps answer what
  happened, why it happened, and where the proof is.
- Progressive disclosure by default. Long prose, payloads, rationales, summaries,
  and traces belong behind obvious dropdowns.
- Headers stay visible and meaningful. Do not replace a useful section header
  with `details`, `not_applicable`, or a clipped preview.
- Typed fields over generic parsing. Use existing `ploke-records`,
  `ploke-protocol`, and `ploke-tree` types and deserializers. Do not build a
  generic JSON field walker in `ploke-egui`.
- Raw evidence remains available. Raw text should be copyable and visible on
  demand, but it should not dominate the primary view.
- Missing, truncated, parse-failed, not-applicable, not-recorded, and zero are
  different states. Preserve the distinction in the UI.
- Provenance is UI data. Runs, artifacts, records, calls, and paths should be
  compact by default, expandable when useful, and copyable.
- The UI should reveal data bugs. If fields cannot render because a protocol
  artifact only persisted a truncated preview, say that directly.
- No per-frame reconstruction. Render from borrowed graph-backed facts or cached
  typed decodes. Do not parse, format, allocate, or rebuild vectors in hot
  render paths.
- Comfortable inspection matters. Indent child content, leave scroll end padding,
  and avoid layout jumps that make expansion/collapse hard to follow.

## Display Patterns

### Copyable Identifiers

Long identifiers, artifact names, run names, and paths should use a compact
visible form plus a copy affordance.

- Left click expands or collapses when there is a shorter display form.
- Right click copies the full value.
- A small copy icon is visible in both collapsed and expanded forms.
- Paths should usually show the tail by default.
- Protocol artifact files may show the subject/file suffix, such as
  `ripgrep-2209.json`, while copying the full artifact key/path.

### Long Text

Long text should render as a collapsible header with a copy button.

- Collapsed state shows only the header, not a clipped one-line preview.
- Expanded state shows wrapped monospace text.
- Use this for synthesis rationales, branch rationales, summaries, scope text,
  LLM excerpts, and other prose-like evidence.

### Raw And Fields Modes

Payload-like values should expose explicit modes when typed fields are available.

- `Raw` shows the persisted raw text, wrapped and copyable.
- `Fields` renders existing typed structures with the local typed renderer.
- `Fields` must be shown only when the existing typed decoder succeeds.
- Do not infer a schema from `serde_json::Value` in the egui layer.
- Indent both `Raw` and `Fields` bodies under the control row so their ownership
  is visually clear.

If `Fields` is unavailable, explain why when the reason is known. For example,
protocol `result_preview` currently stores a truncated preview, not the full
tool result, so the typed result decoder may not be able to render fields.

### Drilldowns

Prefer typed subsections over raw dumps.

- Group related fields under named collapsibles.
- Keep high-signal status/count fields near the top.
- Put raw payloads at the bottom or behind `Raw`.
- Keep sibling sections intact when adding a new drilldown.

### Analyst Views

The UI should help compare and diagnose, not only list facts.

- Use charts or compact summaries for coverage, volume, outcome mix, and patch
  production.
- Preserve drilldown paths from aggregate claims to raw evidence.
- Make missing evidence visible as a state, not an absence of UI.
- Keep dense-table hover lightweight. Hovers may show a short rationale preview
  or field explanation, but full LLM prose belongs in the selected detail pane.

## Boundaries

`ploke-egui` should render from the semantic read side:

```text
ploke-records / ploke-protocol -> ploke-tree::Graph -> borrowed inspector view -> egui renderer
```

Use `ploke-records` for persisted schemas and tool-contract decoders. Use
`ploke-protocol` for protocol data shapes. Use `ploke-tree::Graph` as the
semantic authority for imported run facts. Add or expose a graph witness when a
UI claim cannot be proven from the current graph.

Do not make `ploke-egui` a second data model by reparsing run artifacts,
protocol JSON, History JSON, or report text directly.

## Performance Contract

Inspector work is still render-path work.

- Borrow from the graph or typed records when possible.
- Cache expensive text layouts and typed decodes behind stable keys.
- Avoid per-frame string formatting in repeated rows; use existing buffers or
  cached labels where practical.
- Do not allocate new vectors or mirror DTOs just to make rendering convenient.
- If a UI change adds a large inspector surface, verify with the focused
  `ploke-egui` checks and call out any allocation surface that was not measured.

## Anti-Patterns

- A long value rendered as a single line that scrolls offscreen. Instead:
  render a compact label with an expandable wrapped body and a copy affordance.
- A collapsed field that still displays long prose. Instead: make the
  collapsed state a short header, then show the prose only inside the expanded
  body.
- A generic JSON deserializer or field walker in `ploke-egui`. Instead: call
  the existing typed decoder or graph-backed renderer for that payload shape.
- Raw payloads used as the primary view when typed fields exist. Instead:
  show the typed `Fields` view first and keep `Raw` as an explicit copyable
  mode.
- Silent failure to show fields. Instead: show an unavailable, truncated, or
  parse-failed state with the specific reason when it is known.
- Treating truncated previews as full records. Instead: label them as
  previews, keep raw preview text copyable, and link the UI expectation to the
  full persisted record when available.
- Rebuilding strings, parsed payloads, or row vectors every frame. Instead:
  borrow stable values or cache formatted/decoded projections behind stable
  graph or record keys.
- Walls of absolute paths in ordinary prose. Instead: show the tail by
  default, expand on click, and copy the full path from a visible copy
  affordance or context click.
- Removing existing inspector sections while adding a new one. Instead: keep
  sibling sections present with their headers, popouts, and empty-state behavior
  intact.

## Review Checklist

Before finishing a `ploke-egui` UI or inspector change:

- Does the UI answer an operator question, not just expose a data structure?
- Are long values dropdowns instead of clipped lines?
- Are raw values copyable?
- Are typed fields rendered through existing typed decoders or graph witnesses?
- Are missing/truncated/unavailable states explicit?
- Is child content visually attached to its parent row or section?
- Is the render path borrowed or cached rather than rebuilt per frame?
- Did existing sibling inspector sections keep their headers and behavior?
- Were focused checks run, and was any unmeasured allocation risk reported?
