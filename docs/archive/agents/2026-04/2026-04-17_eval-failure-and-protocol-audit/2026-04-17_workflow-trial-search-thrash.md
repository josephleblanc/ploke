# Scope

Campaign slice: `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --issue search_thrash`.

# Claims

- `target_family`: `search_thrash`.
- `why_this_family`: it is the dominant issue family in this filtered slice, affecting `99/99` selected runs and `593` issue calls.
- `observed_pattern`: traces loop through `request_code_context`, `read_file`, and `list_dir` on wrong or incomplete paths, then retry with nearby variants; the top nearby labels are `refine_search`, `locate_target`, and `inspect_candidate`, with `mixed` and `focused_progress` dominating nearby statuses.
- `suspected_root_cause`: the protocol surface is collapsing recoverable path-discovery churn into a single `search_thrash` bucket, so repeated search-and-read retries look like a broad failure family instead of a narrower path-recovery problem.
- `code_surface`: `crates/ploke-eval/src/protocol_triage_report.rs` and `crates/ploke-eval/src/cli.rs`, especially the triage rendering, issue classification, and next-step selection logic.
- `small_slice`: split `search_thrash` reporting from recoverable path-finding loops and rank the top tool slice separately from the top issue kind.
- `medium_slice`: add family views that distinguish true thrash from recoverable detours using segment status, tool mix, and exemplar path-miss patterns.
- `long_term`: make path discovery and code lookup recovery first-class in the tooling/protocol model so repeated retries become guided recovery rather than blind search churn.
- `recommended_next_step`: implement the small slice first; it should make the triage output more actionable and reduce over-broad `search_thrash` labeling.
- `confidence`: medium-high.
- `exemplar runs reviewed`: `clap-rs__clap-4159`, `clap-rs__clap-4408`, `clap-rs__clap-5873`, `clap-rs__clap-4101`.

# Evidence

- Campaign triage output for the slice showed `99 / 221` protocol-tracked runs selected, `1535/1535` call reviews, `389/389` usable segment reviews, and `search_thrash` as the only issue kind in the filtered view.
- Issue tools were concentrated in `request_code_context` (`471` calls, `89` runs), `read_file` (`56` calls, `38` runs), `list_dir` (`46` calls, `27` runs), and `code_item_lookup` (`18` calls, `12` runs).
- Nearby segment labels were dominated by `refine_search` (`390` calls, `76` runs) and `locate_target` (`103` calls, `33` runs); nearby statuses were mostly `mixed` (`384` calls, `71` runs) and `focused_progress` (`115` calls, `51` runs).
- `clap-rs__clap-4159` showed `42/42` reviewed calls and repeated misses on `src/output/help.rs`, `clap/src/output/help.rs`, and `clap/src/lib.rs` before landing on the correct file set.
- `clap-rs__clap-4408` showed `36/36` reviewed calls with repeated `request_code_context` on `help_flag` / help text and a late `code_item_lookup` failure plus `non_semantic_patch` retry.
- `clap-rs__clap-5873` showed `35/35` reviewed calls, heavy `search_thrash` on `list_dir` and `read_file`, and a late `redundant_thrash` segment after repeated path/root churn.
- `clap-rs__clap-4101` showed `26/26` reviewed calls, repeated help-subcommand search terms, and one `code_item_lookup` plus one `non_semantic_patch` failure near the end.
- Relevant implementation code explicitly maps `request_code_context` and related search tools into the search bucket, and the triage renderer emits the ranked issue kinds, issue tools, exemplar runs, and next-step hints from that aggregation.

# Unsupported Claims

- I did not verify whether the same classification issue affects other campaign slices.
- I did not measure any before/after effect of a fix.
- I did not inspect the underlying `ploke-tui` production implementation beyond the `ploke-eval` reporting surface.

# Not Checked

- Other issue families in `rust-baseline-grok4-xai`.
- Any non-CLI protocol artifacts for the reviewed runs.
- Whether a production-side path hint or recovery affordance already exists outside the sampled traces.

# Risks

- Collapsing recoverable detours into `search_thrash` may hide a distinct product problem if the classifier is actually correct and the model behavior is the primary defect.
- A small reporting-only fix may improve triage clarity without changing the underlying tool behavior.
- The sample is concentrated in `clap-rs__clap-*`, so the ladder may need re-ranking if another campaign family shows a different trace shape.
