# Scope
- Campaign slice: `rust-baseline-grok4-xai`, tool filter `request_code_context`.
- Reviewed exemplars: `clap-rs__clap-4408`, `clap-rs__clap-4159`, `clap-rs__clap-4101`.
- Sources used: `inspect protocol-overview --campaign ... --tool request_code_context`, `inspect protocol-overview --instance ...`, `inspect tool-calls --instance ...`, and the owning `ploke-tui` tool code.

# Claims
- `target_family`: `tool=request_code_context`.
- `why_this_family`: this is a high-volume friction slice, with `574` request_code_context calls across `106` affected runs in the filtered view, and `search_thrash` is the dominant issue kind.
- `observed_pattern`: runs repeatedly issue near-duplicate `request_code_context` searches, often with broad terms, then drift into `read_file`, `list_dir`, or `code_item_lookup` after the search loop has already thrashed.
- `suspected_root_cause`: the tool is a generic retrieval wrapper with weak routing guidance; it accepts missing search terms by falling back to the last user message, returns only generic snippet output, and lacks structured retry hints, so the model keeps searching instead of switching tools sooner.
- `code_surface`: `crates/ploke-tui/src/tools/request_code_context.rs`, `crates/ploke-tui/src/tools/code_item_lookup.rs`, and the tool description surface in `crates/ploke-tui/src/app/view/rendering/highlight.rs`.
- `small_slice`: add retry/routing guidance for `request_code_context` so repeated broad searches point to `code_item_lookup` or `read_file` sooner.
- `medium_slice`: add structured `retry_hint` / `adapt_error` coverage and a clearer tool-selection contract for fuzzy-vs-exact lookup.
- `long_term`: redesign the retrieval-to-lookup ladder so the assistant can detect search stagnation and switch from fuzzy retrieval to exact path-based tools automatically.
- `recommended_next_step`: implement the small slice first; expected payoff is lower `search_thrash` and fewer repeated `request_code_context` calls per run.
- `confidence`: medium-high.
- `exemplar runs reviewed`: `clap-rs__clap-4408`, `clap-rs__clap-4159`, `clap-rs__clap-4101`.

# Evidence
- Campaign aggregate for the tool slice: `selected_runs=106`, `full_runs=106`, `partial_runs=0`, `error_runs=0`, `total_tool_calls=1597`, `reviewed_tool_calls=1597`, `issue_kinds.search_thrash=471` across `89` runs, `issue_tools.request_code_context=574` across `106` runs.
- Full campaign context: `selected_runs=221`, `search_thrash=593` across `99` runs, `request_code_context=574` across `106` runs, `read_file=297` across `100` runs, `list_dir=122` across `58` runs, `artifact_failure_runs=9`.
- `clap-rs__clap-4408`: `36/36` call reviews and `4/4` usable segment reviews; `search_thrash=23`; the tool trace shows 23 request_code_context calls before the run reaches `list_dir`, `read_file`, and one `code_item_lookup` failure.
- `clap-rs__clap-4159`: `42/42` call reviews and `4/4` usable segment reviews; `search_thrash=24`; the trace alternates request_code_context with failed `read_file`/`list_dir` path attempts, which is consistent with a search loop that has not converged.
- `clap-rs__clap-4101`: `26/26` call reviews and `3/3` usable segment reviews; `search_thrash=19`; the trace mixes repeated request_code_context calls with failed `read_file`, `list_dir`, and one `code_item_lookup` failure.
- Code surface evidence: `request_code_context` schema and fallback live at [request_code_context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/request_code_context.rs:21), with last-user fallback and generic retrieval at [request_code_context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/request_code_context.rs:113) and [request_code_context.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/request_code_context.rs:149); `code_item_lookup` explicitly tells the user to try request_code_context when file path resolution is wrong at [code_item_lookup.rs](/home/brasides/code/ploke/crates/ploke-tui/src/tools/code_item_lookup.rs:154); the UI tool table presents request_code_context as a generic snippet fetcher at [highlight.rs](/home/brasides/code/ploke/crates/ploke-tui/src/app/view/rendering/highlight.rs:1067).
- Supporting gap evidence: the tool error matrix records `request_code_context` with no `retry_hint` coverage at [tool_call_error_matrix.md](/home/brasides/code/ploke/crates/ploke-tui/src/tools/tool_call_error_matrix.md:14).

# Unsupported Claims
- I did not verify the same pattern across other tool families.
- I did not prove the root cause is only missing retry hints; the retrieval/routing policy itself may also be too permissive.
- I did not run or modify code, so I did not measure the post-change effect.

# Not Checked
- `ploke-core` RAG internals and ranking quality.
- Whether the last-user-message fallback is the main trigger in these runs versus a secondary amplifier.
- Other campaign slices or non-`clap` exemplars for this family.
- Whether a prompt-only change is enough without tool-selection policy changes.

# Risks
- A narrow fix may only reduce surface churn without improving underlying lookup accuracy.
- If the model keeps using `request_code_context` as a default search primitive, guidance alone may not be enough.
- This analysis is dominated by `clap` exemplars, so it may overstate how uniform the family behaves elsewhere.
