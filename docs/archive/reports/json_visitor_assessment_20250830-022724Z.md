# OpenRouter JSON Visitor Assessment and Desired Capabilities

Linkage:
- Source under review: `crates/ploke-tui/src/llm/openrouter/json_visitor.rs`
- Latest run output: `crates/ploke-tui/data/models/all_raw_stats.txt`
- Plan for improvements: `docs/plans/agentic-system-plan/impl-plan/plan_20250830-022724Z.md`

## Current Functionality (Summary)
- Traverses `data` array of models from a large OpenRouter JSON (single endpoint) and aggregates:
  - Field usage counts: counts of encountered paths (object keys), with a percentage over model count.
  - Null counts per field.
  - Field values set (unique values) across dataset, as strings.
  - Array length distributions per path.
  - Special union of `supported_parameters` (array of strings).
- Produces a human-readable text report with sections for usage counts, nulls, constant fields, candidate enum-like fields (<= 30 distinct values, excluding pure booleans/numerics), array length stats, and union of `supported_parameters`.
- Includes an unused `ValueVisitor` framework and `ProfilingVisitor` stub; the active logic is a bespoke traversal function.

## Strengths
- Simple, fast pass to spotlight structure: presence, nulls, candidate enums, and array length distributions.
- Generates an actionable union for `supported_parameters` that directly informs enum design.
- Output is lightweight and easy to scan.

## Gaps and Desired Functionality
1. Per-model presence vs raw encounter counts
   - Today’s counts may reflect multiple occurrences (e.g., within arrays) and do not distinguish “missing” vs “present but null”.
   - Desired: per top-level item presence, missing, and null counts with percentages.

2. Type fidelity and mixed-type detection
   - Values are stringified; no summary of type distributions (null/bool/int/float/string/object/array).
   - Desired: per-field type histogram; flag mixed-type fields; expose numeric min/max and integer vs float split.

3. Value distributions and cardinality
   - Candidate enums list values but not frequencies; thresholding is rudimentary and global.
   - Desired: frequency tables (top-K), total cardinality, and ratio heuristics to recommend enums.

4. Arrays: element typing and unions (generalized)
   - Special-case only `supported_parameters`.
   - Desired: for any array-of-scalars, compute union and frequency; for arrays-of-objects, union of keys and field presence within elements.

5. Professional serialization guidance
   - No guidance for numeric-like strings (e.g., "0.000005"), date-like strings, or coercions.
   - Desired: detectors and recommendations (e.g., parse as `f64`, use deserializers, avoid `#[serde(untagged)]` unless justified).

6. Output clarity and structure
   - Lacks missing counts, type histograms, and value frequencies; ordering is mixed.
   - Desired: grouped, sorted sections; explicit thresholds; JSON export for tooling.

7. Efficiency and configurability
   - Stores full unique value sets unbounded; risk for memory bloat.
   - Desired: caps, top-K, sampling, and knobs via a config struct checked in.

## What Else We Want (Feature Wishlist)
- Per-field schema synopsis:
  - presence% / null% / missing% over top-level items
  - type histogram (Null/Bool/Int/Float/String/Object/Array)
  - numeric: min/max; integer vs float counts; decimal precision examples
  - strings: numeric-like detection; date/uuid-like heuristics
- Arrays:
  - length distribution; element type histogram
  - for arrays-of-strings: union + frequency; enum recommendation
  - for arrays-of-objects: union of keys; per-key presence within elements
- Mixed-type and coercion warnings:
  - fields that vary between number and string; propose a tagged union or coercion strategy
  - string-coded numbers; recommend robust parsing patterns
- Enum recommendation heuristics:
  - thresholds on cardinality (absolute and relative), stability across dataset, and value character sets
- Structured artifacts:
  - machine-readable JSON stats alongside text report
  - stable ordering for diff-friendly reviews

## Professional Serialization Considerations
- Strong typing at boundaries:
  - Prefer enums for bounded string sets; use tagged unions to represent variant shapes.
  - Disallow ad-hoc maps unless constrained and validated.
- Early validation and actionable errors:
  - Surface missing and invalid states; avoid silent coercions.
- Numeric handling:
  - Detect and parse numeric-like strings; record min/max and precision to select `i64` vs `f64` and decimal features.
- Optionality and defaults:
  - Distinguish truly missing vs present-null; compute required vs optional fields for structs.
- Backward-compatibility strategy:
  - Avoid `#[serde(untagged)]` except as a documented bridge; add deprecation notes and migration plan.

## Critique and Scoring (7 criteria)
1. Schema coverage fidelity (presence/missing/null, types, arrays)
   - Score: 4/10 — lacks per-model presence, type histograms, and element typing.
2. Generality across endpoints
   - Score: 5/10 — tailored to a single endpoint; `supported_parameters` special-case only.
3. Actionable enum suggestions
   - Score: 6/10 — basic candidate listing without frequencies or rigor.
4. Professional serialization guidance
   - Score: 3/10 — no detection of numeric-like strings, mixed types, or struct recommendations.
5. Output clarity and usability
   - Score: 6/10 — readable but omits key summaries and JSON artifact.
6. Efficiency and scalability
   - Score: 5/10 — unbounded unique value sets and no caps/sampling.
7. Testability and iteration
   - Score: 4/10 — smoke test only; no structured output to assert on.

Overall: 4.7/10 (needs targeted upgrades for generality and actionable guidance).

## Desired End-State (high level)
- General visitor capable of profiling arbitrary OpenRouter responses:
  - Accurate per-model presence/null/missing counts
  - Type and element-type histograms
  - Value frequencies and unions with caps
  - Enum and struct suggestions with thresholds
  - Mixed-type and coercion detection (numeric-like strings)
  - Clean text report + JSON stats for tooling
  - Configurable limits and thresholds for scalability

## Next Steps
- Execute the plan in `docs/plans/agentic-system-plan/impl-plan/plan_20250830-022724Z.md`.
- Iterate with at least 3 improvements, logging each as `impl_*` files linked to the plan.

