**Parent Create Attempt**

A selected parent-to-child edge, or selected child node, should expose the bounded edit attempt that produced that child artifact.

That attempt has this shape:

```text
Parent<Ruling>
  -> diagnose / choose surface
  -> grant readable/writable/forbidden surface
  -> ask harness / LLM to produce proposal
  -> record LLM turns, tool calls, provider attempts
  -> check proposal against grant and hashes
  -> apply or reject
  -> produce derived Artifact
  -> hydrate Child Runtime
  -> child self-evaluates
  -> selection later accepts/rejects/promotes
```

The UI should make that chain visible without turning it into a wall of JSON.

**What It Should Look Like**
In the right Inspector, selecting a child node or patch edge should show a top-level **Patch Generation** section:

- **Outcome:** applied, rejected, placeholder, missing, failed, or unchecked.
- **Objective:** what the parent asked the harness/LLM to do.
- **Surface:** readable / writable / forbidden summary, with full grant expandable.
- **Proposal:** generated patch/proposal summary, touched files, semantic vs broad patch mode.
- **Check:** containment, hash match, protected path status, partial apply status.
- **LLM Run:** model/provider, wall time, provider attempts, tokens, finish/error status.
- **Tools:** count, success/failure count, total latency, key retrieved context.
- **Child Eval:** whether the derived child ran, evaluated, and contributed evidence.
- **Source Status:** which rows are authority, typed evidence, projection, telemetry, or missing.

The first view should be compact. No giant hashes. No full prompt dumps. No raw provider envelopes. Full IDs and raw-ish payloads belong behind explicit expand/debug affordances.

**Progressive Drilldown**
The ladder I would use:

1. **Edge/Node Badge**
   On the canvas: small visual status for “LLM-backed”, “placeholder patch”, “checked”, “failed check”, “selected”, “rejected”, or “missing evidence”.

2. **Inspector Summary**
   Human-readable rows answering: what was attempted, did it produce a real artifact delta, was it accepted, and what evidence supports that.

3. **Attempt Timeline**
   A compact sequence:
   `surface chosen -> LLM request -> tool calls -> proposal -> check -> apply -> child plan -> child eval`.
   Hovering a segment highlights related inspector rows and graph node/edge.

4. **LLM Conversation Outline**
   Not raw chat by default. Show message roles, short summaries, tool-call anchors, response status, token usage, and duration. Expand a turn to show message text with truncation and “show full” only for debug.

5. **Tool Calls**
   Start with tool name, purpose/intent if typed, status, latency, retrieved item counts, and result class. Expand to typed args and typed returns. Only show raw arguments/returns in debug when useful.

6. **Evidence / Debug**
   Full record refs, hashes, paths, provider response ids, raw response sidecar refs, and exact source files. This is where long SHA values belong.

**Bottom Timeline**
The bottom lane should show the parent’s generation as ordered spans, not just a selected-node detail table:

```text
Parent epoch
  surface/objective
  LLM/provider attempt span
    tool calls
  proposal/check/apply
  child runtime/eval
  selection/handoff
```

History order remains the strongest ordering spine. Provider timings, observation JSONL, and trace timings should be labeled as telemetry/projection unless promoted through a typed graph projection.

**Data Boundary**
The sources should join through `ploke-tree`/`Graph`, then `ploke-egui` should borrow:

```text
typed records / RunRecord / trace projections / edit-surface records
  -> ploke-tree RunRecordSet / execution graph evidence
  -> Graph evidence attachments + InspectorRef + TimelineSpanRef
  -> borrowed Inspector projection
  -> egui render
```

No semantic `String`/`Arc<str>` diagnostic model. No UI-owned mirror of chat/tool records. No parsing owned JSON in `ploke-egui`.

**For `p1-broad-harness-retry-20260514-1`**
This is a good fixture. Since it used placeholder patch material while apparently asking the LLM to produce patches, the UI should explicitly separate:

- “LLM/harness request happened”
- “proposal evidence exists or is missing”
- “material artifact delta came from placeholder path”
- “surface check/apply evidence exists or is missing”
- “child/evaluation/selection proceeded”

That distinction is exactly the kind of thing the Inspector should make obvious without requiring CLI spelunking.

**End Goal**
We are done when selecting a child/patch edge in that run lets an operator answer:

- What did the parent ask the LLM/harness to do?
- What tools ran, what did they retrieve, and did they fail?
- How long did provider/network/model time take?
- Was a real proposal produced, checked, and applied?
- Was the resulting patch placeholder or LLM-derived?
- What typed records prove each claim?
- What is still missing or only telemetry?

The next implementation slice aligns with the docs: `run-execution-graph.browser-spine` first, then attach already-typed tool/provider facts as graph evidence, while `edit-surface.source-records` and later timeline/concurrency work fill in the deeper patch-generation proof.
