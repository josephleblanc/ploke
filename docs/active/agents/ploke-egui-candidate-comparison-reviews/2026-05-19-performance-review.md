# Candidate Comparison Performance Review

Reviewer: `cc-perf-reviewer`  
Date: 2026-05-19  
Scope: recent Candidate Comparison UI path for `score_child_prop` formula and side-by-side candidate fields.

## Findings

### High: Candidate Comparison is not covered by the current native benchmark section matrix

Evidence:

- The renderer has a native-benchmark tracing span, `inspector_candidate_comparison`, at `crates/ploke-egui/src/ui/app/shell.rs:1609-1613`.
- That span is not registered in the allocation scope list. `SCOPE_NAMES` includes `selection_inspector`, `inspector_run_records`, `inspector_patch_debug`, and related child spans, but no `inspector_candidate_comparison` at `crates/ploke-egui/src/allocation.rs:983-1077`.
- The benchmark section enum omits Candidate Comparison: `BenchmarkInspectorSection` only covers `LlmCalls`, `RunRecords`, `GraphEdges`, `ArtifactEdges`, `PatchDebug`, `SourceRefs`, and `ArtifactIds` at `crates/ploke-egui/src/benchmark.rs:506-516`, and its sequence omits Candidate Comparison at `crates/ploke-egui/src/benchmark.rs:519-528`.
- The standard benchmark scenarios likewise omit a candidate-comparison expanded or phase-sequence scenario at `crates/ploke-egui/src/benchmark.rs:80-100` and `crates/ploke-egui/src/benchmark.rs:128-145`.
- The change's benchmark note explicitly says allocation/performance was not measured at `crates/ploke-egui/docs/profiling/benchmarks/20260519-score-child-prop-comparison-benchmark-note.md:18-24`.

Risk:

The heavy side-by-side table is currently only visible through broad `selection_inspector`/root buckets. That makes regressions easy to miss, especially because previous right-panel phase reports show selection-related work can materially affect frame allocations and frame time. For example, an existing standard report records right-panel phase medians and growing heap slopes for section-expanded scenarios at `crates/ploke-egui/docs/profiling/benchmarks/20260519-0be892aba8fc-standard/README.md:53-68`.

Suggested fix:

- Add `CandidateComparison` to `BenchmarkInspectorSection`, `from_benchmark`, `as_str`, `parse`, `action_label`, `sequence`, and the standard scenario list.
- Add `inspector_candidate_comparison` to `SCOPE_NAMES`.
- Add a phase-sequence scenario for candidate comparison using the same nine 30-frame windows as the other inspector sections.
- Compare expanded and collapsed phases before treating the UI as performance-reviewed.

### Medium: f64 and range rendering allocate transient strings every frame before cache lookup

Evidence:

- `cached_kv_f64` formats with `format_f64(value).as_str()` at `crates/ploke-egui/src/ui/app/shell.rs:1042-1049`.
- `format_f64` returns an owned `String` via `format!("{value:.6}")` or `value.to_string()` at `crates/ploke-egui/src/ui/app/shell.rs:1964-1970`.
- Candidate Comparison renders many f64 fields per row: oracle rate, alpha, alpha midpoint, exploitation, exploration, weight, cumulative range, sample, sample threshold, lambda, and total weight across `crates/ploke-egui/src/ui/app/shell.rs:1753-1789` and `crates/ploke-egui/src/ui/app/shell.rs:1862-1872`.
- `render_optional_f64_range` allocates at least three `String`s per rendered range cell: two `format_f64` calls plus `format!("{}..{}")` at `crates/ploke-egui/src/ui/app/shell.rs:2129-2139`.
- The galley cache happens after these strings have already been allocated, because `cached_monospace_label` receives the temporary `&str` at `crates/ploke-egui/src/ui/app/shell.rs:902-908`.

Risk:

The cache prevents repeated galley layout for stable text, but it does not prevent per-frame numeric string allocation. A selected candidate set with `N` rows pays this cost for each open frame. This is exactly the kind of transient allocation the right-panel allocation work was trying to avoid.

Suggested fix:

- Move f64/range formatted text into a render-boundary cache keyed by `(style, field, value_bits)` or by `(selection_entry_id, metric_set_id, payload_index, field)`.
- For rows, prefer a candidate-comparison row render cache invalidated by graph revision plus selection key. That keeps the semantic witness borrowed while caching only display strings/galleys at the egui boundary.
- Keep `itoa::Buffer` use for integers; do not replace integer stack formatting with owned strings.

### Medium: row resolution repeats linear graph searches and key-cloning lookups during each open frame

Evidence:

- The renderer calls `slot.resolve_child(graph, child)` for every child row every frame at `crates/ploke-egui/src/ui/app/shell.rs:1709-1712`.
- `resolve_child` calls `graph.child_plans.child_for_node_id(...)` at `crates/ploke-egui/src/ui/inspector.rs:512-520`; that graph helper scans all plans and all children at `crates/ploke-tree/src/graph/types/child_plan.rs:20-25`.
- If the cached child slot cannot identify the candidate directly, `candidate_for_child` scans `graph.candidates.candidates` by node id, then scans again by branch id, then scans again by derived artifact at `crates/ploke-egui/src/ui/inspector.rs:1999-2030`.
- Metric lookups clone `selection.metric_set_id` into a fresh `MetricCandidateKey` at `crates/ploke-egui/src/ui/inspector.rs:2032-2041`.
- Formula lookups clone both `selection.entry_id` and `selection.metric_set_id` into a fresh `SelectionFormulaKey` at `crates/ploke-egui/src/ui/inspector.rs:2046-2057`.
- `ScoreChildPropNode::row_for_payload_index` linearly scans formula rows at `crates/ploke-tree/src/graph/types/selection.rs:222-228`.

Risk:

The existing `InspectorCache` does prevent `InspectorSections` from rebuilding every frame for the same graph revision and selection at `crates/ploke-egui/src/ui/inspector.rs:32-60`. However, the open table still does repeated per-row resolution work after that cache boundary. The likely steady-state cost is:

- `O(children * all_child_plans)` for child resolution.
- Up to three `O(all_candidates)` scans on fallback candidate matching.
- `O(children * formula_rows)` for formula row lookup.
- Several `String`-backed id clones for map keys.

Suggested fix:

- Resolve the parent plan once in `render_candidate_comparison_for_inspector`, or store a parent-local child ordinal in `CandidateComparisonChildSlot` so `resolve_child` does not scan all child plans.
- Add graph-side indexes for common lookup shapes: child by node id, candidate by `(selection_entry_id, payload_index)`, and formula row by payload index.
- Avoid cloning `EntryId`/`HistoryHash` just to read from maps. Either add borrowed lookup helpers on `MetricIndex` or store direct metric/formula handles in the cached section slot.

### Medium: metric aggregate columns refold the same compared-run vector multiple times per candidate per frame

Evidence:

- Each row renders tool failure delta, patch failure delta, valid patch, convergence, oracle eligibility, protocol reviewed delta, and protocol missing delta separately at `crates/ploke-egui/src/ui/app/shell.rs:1906-1940`.
- Each helper walks `metric.compared_runs` independently: `metric_tool_failures_delta` at `crates/ploke-egui/src/ui/app/shell.rs:1992-2002`, `metric_patch_failures_delta` at `crates/ploke-egui/src/ui/app/shell.rs:2004-2014`, valid patch/converged/oracle eligibility at `crates/ploke-egui/src/ui/app/shell.rs:2016-2058`, and protocol deltas at `crates/ploke-egui/src/ui/app/shell.rs:2060-2082`.

Risk:

For a candidate with multiple compared runs, the renderer repeatedly performs small folds over stable data. The absolute cost may be modest today, but the table already has 32 columns and this is an avoidable per-frame multiplier.

Suggested fix:

- Fold `compared_runs` once into a typed render-boundary aggregate for the row.
- Keep that aggregate cache keyed by metric set id plus payload index, or promote it into a graph-owned aggregate only if other consumers need the same semantic fold.
- Do not compute these aggregate values inside each cell renderer.

### Medium: the 32-column `egui::Grid` has no virtualization or row limiting

Evidence:

- Candidate Comparison creates a horizontal `ScrollArea` around an `egui::Grid` with `num_columns(32)` at `crates/ploke-egui/src/ui/app/shell.rs:1670-1674`.
- It emits all 32 headers at `crates/ploke-egui/src/ui/app/shell.rs:1675-1707`.
- It then iterates every child and renders all cells for every resolved candidate at `crates/ploke-egui/src/ui/app/shell.rs:1709-1715` and `crates/ploke-egui/src/ui/app/shell.rs:1819-1942`.

Risk:

The current fanout may be small, but the implementation scales widget count as `32 * candidate_count` every open frame. With larger candidate sets or chart/popout reuse, frame time can grow quickly because egui still lays out the grid contents.

Suggested fix:

- Add a focused benchmark before deciding whether virtualization is needed.
- If large candidate sets are expected, use a vertically virtualized table/list pattern for rows and reserve the full 32-field view for selected rows or a popout.
- If the product requirement is full side-by-side display, cache formatted cell text and keep the visible row count bounded by viewport.

### Low: the galley cache removes layout churn but uses linear lookup and unbounded retention

Evidence:

- `InspectorRenderCache` stores `text_galleys` and `id_galleys` as `Vec`s at `crates/ploke-egui/src/ui/app/shell.rs:33-47`.
- `text_galley` linearly scans the vector, then stores `text: text.into()` on misses at `crates/ploke-egui/src/ui/app/shell.rs:67-88`.
- Candidate Comparison adds many distinct numeric and id values to the shared inspector cache through `cached_label`, `cached_monospace_label`, and `cached_expandable_id` at `crates/ploke-egui/src/ui/app/shell.rs:893-923`.

Risk:

This is probably acceptable for a small fanout and one selected node. Across many selections, the cache can retain stale candidate-specific strings and each lookup becomes a scan over a larger cache. The immediate cost is CPU more than allocation, but it becomes coupled to how much unique inspector text the user has viewed.

Suggested fix:

- Consider splitting the cache by selection/revision or adding a bounded map for candidate-comparison cell text.
- Prefer keyed cache invalidation over a global vector that accumulates old row values.

### Low: graph ingestion clones complete formula records into the graph

Evidence:

- Formula ingestion clones the persisted formula before converting it into the graph node at `crates/ploke-tree/src/graph/build/selection.rs:353-362`.
- `SelectionFormulaNode` owns `SelectionFormulaKind`, and `ScoreChildPropNode` owns the full `ScoreChildPropRecord` at `crates/ploke-tree/src/graph/types/selection.rs:202-220`.

Risk:

This is not a per-frame cost, and it may be the right tradeoff if `Graph` must own an independent typed projection. It does increase import-time memory and duplicates rows that are already persisted in History records.

Suggested fix:

- Do not optimize this before measuring startup/import or graph memory impact.
- If it becomes measurable, consider storing formula rows in a graph-owned indexed arena once, rather than cloning full records into multiple nodes.

## Positive Notes

- The main inspector-section projection is not rebuilt every frame for a stable graph revision and selection. `InspectorCache::sections` only rebuilds when `(revision, selection)` changes at `crates/ploke-egui/src/ui/inspector.rs:32-60`.
- The Candidate Comparison semantic projection mostly preserves borrowed graph data at render time: `CandidateComparisonCandidate<'g>` carries borrowed child/candidate/metric/formula row references at `crates/ploke-egui/src/ui/inspector.rs:576-592`.
- Integer cells use `itoa::Buffer`, avoiding owned integer formatting strings at `crates/ploke-egui/src/ui/app/shell.rs:1002-1039` and `crates/ploke-egui/src/ui/app/shell.rs:2091-2115`.

## Verification Gaps

- Native interactive window: not tested in this review.
- Native benchmark: not run in this review.
- Allocation/callsite profiling: not run in this review.
- Candidate Comparison section-specific frame timing: not currently available because the benchmark section matrix and allocation scope registry do not include Candidate Comparison.
- Real-run import with a large candidate set: not tested in this review.

## Commands Run

Source and instructions:

- `sed -n '1,220p' .orchestrator/workers/cc-perf-reviewer.md`
- `sed -n '1,220p' .codex/skills/ploke-egui-benchmarking/SKILL.md`
- `sed -n '1,180p' /home/brasides/.codex/skills/rust-review-discipline/SKILL.md`
- `sed -n '1,220p' /home/brasides/.codex/skills/rust-review-discipline/references/ploke-review-policy.md`
- `sed -n '1,160p' /home/brasides/.codex/skills/rust-review-discipline/references/rust-analyzer-style.md`

Bounded source reads and searches:

- `rg -n "ploke-egui|candidate comparison|score_child_prop|allocation|benchmark" /home/brasides/.codex/memories/MEMORY.md`
- `rg -n "CandidateComparison|candidate_comparison|score_child_prop|formula|SelectionFormula|render_candidate|Grid|Table|format!|to_string|clone|collect|sort|Vec<" crates/ploke-egui/src/ui/app/shell.rs crates/ploke-egui/src/ui/inspector.rs crates/ploke-tree/src/graph/types/selection.rs crates/ploke-tree/src/graph/build/selection.rs crates/ploke-egui/src/benchmark.rs crates/ploke-egui/src/allocation.rs`
- `rg -n "fn cached_|struct InspectorRenderCache|impl InspectorRenderCache|cached_kv|format_optional|render_metric|format_f64|candidate" crates/ploke-egui/src/ui/app/shell.rs`
- `rg -n "fn candidate_comparison_slot|selection_metric_for_candidate|candidate_for_child|selection_formula_for_selection|selection_marks_candidate|MetricCandidateKey|SelectionFormulaKey" crates/ploke-egui/src/ui/inspector.rs`
- `rg -n "SCOPE_NAMES|inspector_candidate_comparison|inspector_run_records|inspector_patch_debug|selection_inspector|AllocationScope" crates/ploke-egui/src/allocation.rs crates/ploke-egui/src/ui/app/shell.rs crates/ploke-egui/src/benchmark.rs`
- Targeted `nl -ba ... | sed -n ...` reads for the line ranges cited above.

Benchmark metadata/doc inspection:

- `rg --files crates/ploke-egui/docs/profiling/benchmarks docs/active/agents/ploke-egui-candidate-comparison-reviews`
- `ls -lh crates/ploke-egui/docs/profiling/benchmarks/20260519-score-child-prop-comparison-benchmark-note.md crates/ploke-egui/docs/profiling/benchmarks/20260519-0be892aba8fc-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-bfb055b22276-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-485d15440be0-standard/README.md`
- `wc -l crates/ploke-egui/docs/profiling/benchmarks/20260519-score-child-prop-comparison-benchmark-note.md crates/ploke-egui/docs/profiling/benchmarks/20260519-0be892aba8fc-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-bfb055b22276-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-485d15440be0-standard/README.md`
- `find crates/ploke-egui/docs/profiling/benchmarks -path '*20260519*report.json' -printf '%p %s bytes\n'`
- `rg -n "candidate|comparison|inspector_candidate_comparison|frame|median|p95|allocation|not measured|scenario|select_artifact|right panel" crates/ploke-egui/docs/profiling/benchmarks/20260519-score-child-prop-comparison-benchmark-note.md crates/ploke-egui/docs/profiling/benchmarks/20260519-0be892aba8fc-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-bfb055b22276-standard/README.md crates/ploke-egui/docs/profiling/benchmarks/20260519-485d15440be0-standard/README.md`

## Changed Files

- `docs/active/agents/ploke-egui-candidate-comparison-reviews/2026-05-19-performance-review.md`
