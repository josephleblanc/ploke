# Scope
Campaign slice: `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --tool read_file`.
Goal: explain why this slice matters, sample exemplar runs, inspect the implicated production surface, and rank the intervention ladder.

# Claims
- `target_family`: `tool=read_file`.
- `why_this_family`: this slice covers `100/100` protocol-tracked runs and `297` read calls; it is the only issue tool in the aggregate, so it concentrates the campaign's path-finding friction.
- `observed_pattern`: the dominant shapes are `mixed`, `partial_next_step`, and `search_thrash`; exemplars show repeated `read_file`/`list_dir`/`request_code_context` probes around wrong or missing paths rather than decisive convergence.
- `suspected_root_cause`: `read_file` and sibling navigation tools require exact workspace-root-relative paths, but their recovery affordances are generic; the model keeps retrying path guesses instead of getting a stronger next-step policy.
- `code_surface`: `crates/ploke-tui/src/tools/ns_read.rs`, `list_dir.rs`, `code_item_lookup.rs`, and the tool descriptions shown in `crates/ploke-tui/src/app/view/rendering/highlight.rs`.
- `small_slice`: tighten retry hints and path examples in `NsRead`/`ListDir`, so invalid-path failures point to one concrete recovery path.
- `medium_slice`: add richer follow-up guidance from `code_item_lookup` and tool descriptions so path uncertainty routes toward discovery tools earlier.
- `long_term`: add a guided navigation/recovery layer that turns missing-path loops into a structured "find target, then read" flow.
- `recommended_next_step`: patch `NsRead` and `ListDir` recovery text first, then re-run the same campaign slice and check whether `search_thrash` drops and `useful_exploration` rises.
- `confidence`: medium-high; the evidence is complete for the sampled runs, but the root-cause claim is still inferred from trace shape plus tool contracts.
- `exemplar runs reviewed`: `clap-rs__clap-4159`, `clap-rs__clap-5873`, `clap-rs__clap-5075`.

# Evidence
- Aggregate triage output: `Selection 100 / 221`, `Call reviews 1495/1495`, `Usable seg reviews 380/380`, `Issue tools: read_file 297 calls 100 runs`.
- Aggregate issue mix: `mixed 132 calls / 75 runs`, `partial_next_step 66 / 43`, `search_thrash 56 / 38`, `useful_exploration 38 / 28`.
- `clap-rs__clap-4159`: `42/42` call reviews, `search_thrash` dominates; tool trace shows repeated failed `read_file` attempts plus `request_code_context` and `list_dir` probes before any stable target is found.
- `clap-rs__clap-5873`: `35/35` call reviews, `search_thrash` plus `recoverable_detour`; tool trace shows repeated directory/file probing and a late successful `read_file` after multiple misses.
- `clap-rs__clap-5075`: `27/27` call reviews, `partial_next_step` and `search_thrash`; the trace ends in `read_file`/`code_item_lookup` recovery attempts rather than a clean converge-and-read sequence.
- `ns_read.rs:97-183`: invalid-path and directory failures both return the same broad retry shape: absolute or workspace-root-relative path, file not directory.
- `list_dir.rs:109-219`: invalid paths get a generic absolute/workspace-root-relative hint, but missing directories are treated as success-with-empty-entries, which can keep the model in exploration mode.
- `code_item_lookup.rs:123-166`: file-path failures explicitly punt to `request_code_context` for fuzzy search, which is a useful fallback but not a stronger path-discovery policy.
- `highlight.rs:1067-1075`: the model-facing tool table presents `read_file`, `list_dir`, and `code_item_lookup` as adjacent options, but not as an ordered recovery ladder.

# Unsupported Claims
- I did not prove that the root cause is exclusively in `ploke-tui`; the eval-side report surface could also shape behavior.
- I did not measure before/after impact for any proposed fix.
- I did not inspect every `read_file` run in the campaign; this is a sampled diagnosis from three exemplars plus the aggregate.

# Not Checked
- I did not inspect `protocol-overview --instance` for additional non-`clap-rs` exemplars.
- I did not inspect `tool-calls --full` for the sampled runs.
- I did not run code or tests against the production surface.

# Risks
- Changing retry hints alone may not move the metric if the real failure is upstream prompt selection rather than tool messaging.
- Making `list_dir` more permissive could hide real path errors; keep the current validation semantics.
- If the model is anchored on bad search terms, the right fix may need prompt/tool ordering changes, not just error text.
