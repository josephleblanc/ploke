# 2026-05-11 MBE Oracle Calibration Handoff

Short restart packet for the Multi-SWE-bench oracle track attached to
Prototype 1 loop outputs.

## Related Tracks

- [`../plans/self-improvement-loop/handoffs.md`](../plans/self-improvement-loop/handoffs.md)
  Shared track index for records, playback, frontend observability, and loop
  evaluation.
- [`2026-05-09_egui-wasm-observability-handoff.md`](2026-05-09_egui-wasm-observability-handoff.md)
  Frontend observability track that should eventually display oracle evidence,
  patch validity, and benchmark outcome diagnostics.
- [`../plans/self-improvement-loop/frontend-questions.md`](../plans/self-improvement-loop/frontend-questions.md)
  UI-facing questions for multi-generation loop runs.

## Current Evidence

Campaign under analysis:

```text
p1-history-traversal-20260511-2
```

MBE artifact batch:

```text
/tmp/ploke-mbe-campaign-p1-history-traversal-20260511-2-v2
```

Observed facts:

- `ploke-eval mbe` can discover Prototype 1 campaign candidates and run MBE for
  selected nodes.
- The MBE gold patch for `BurntSushi/ripgrep:pr-2209` resolves.
- The empty patch produces no fix-stage test results.
- The sampled candidate `node-e73f3c82afd7f4c3` fails before tests because the
  submitted patch calls missing Rust code:

```text
crate::util::replace_all_clipped
```

- All 23 non-empty candidates in the batch have `could not compile` in
  `fix-patch-run.log`.
- The 23 candidates contain 12 distinct non-empty patch hashes, so the failure
  is not only one duplicated patch.

Current diagnostic improvement:

```text
missing_fix_results
```

was split so compile-collapse cases can report:

```text
fix_compile_failed
```

Verification:

```text
cargo fmt --all
cargo test -p ploke-eval mbe::
```

The MBE test filter passed with 10 tests after the diagnostic change.

## Interpretation

MBE is not proven perfect, but the basic harness/config path is valid for this
instance because the dataset gold patch resolves.

The loop-produced candidates tested so far are not oracle-near misses. They are
compile-broken under MBE, so they are usable only as negative evidence for
selection until the source of the compile break is understood.

The active suspicion moved upstream from "MBE cannot judge the candidates" to
"the candidate artifact, patch export, or benchmark-base relation is wrong."

## Questions To Answer Next

1. Are the admitted Prototype 1 artifacts themselves cargo-check valid?
   - If yes, the bug is likely in submission patch projection or base
     selection.
   - If no, the admission path or check surface is broken.

2. Is the MBE submission patch complete?
   - Example: does the admitted artifact for `node-e73f3c82afd7f4c3` contain
     `replace_all_clipped` somewhere that the exported `fix_patch` omitted?

3. Is the patch diffed against the correct benchmark base?
   - MBE applies `fix_patch` against the original benchmark base.
   - A child artifact may be valid relative to its parent but invalid against
     the benchmark base.

4. Are descendant patches being exported as relative parent deltas instead of
   full benchmark-base patches?
   - MBE needs a standalone patch against the benchmark base unless the exporter
     intentionally composes ancestry.

5. Did self-evaluation check the same object MBE checks?
   - Cargo-checking a hydrated artifact is not equivalent to checking the
     exported `fix_patch` against MBE's base plus test patch.

## Next Concrete Check

Use one candidate first:

```text
node-e73f3c82afd7f4c3
```

Compare:

```text
admitted Artifact tree
submitted MBE fix_patch
MBE base checkout after applying fix_patch
```

Decision rule:

- If the admitted Artifact contains the missing helper but `fix_patch` does not,
  the exporter is wrong or ancestry was not composed.
- If the admitted Artifact also lacks the helper, the loop admitted or evaluated
  a compile-broken artifact, or checked the wrong surface.
- If the helper exists only in a parent Artifact, descendant patch export is
  relative when MBE requires benchmark-base-composed output.

## Task Stack

Related task-stack ids:

```text
mbe-oracle-calibration
mbe-candidate-artifact-vs-submission
mbe-descendant-patch-composition
mbe-self-eval-object-parity
```
