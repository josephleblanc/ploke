# Scope
- Campaign slice: `rust-baseline-grok4-xai` filtered to `partial_next_step`.
- Goal: identify why this slice matters, sample concrete runs, inspect the implicated `ploke-eval` reporting code, and rank interventions.

# Claims
- `target_family`: `partial_next_step` in the campaign protocol triage surface.
- `why_this_family`: this slice is large enough to matter operationally: 74 selected runs and 157 matching calls, with `read_file` and `request_code_context` dominating the issue surface.
- `observed_pattern`: many runs show short exploration loops, repeated context reads, and `non_semantic_patch` retries; the clearest mismatch is that short clean traces can still be summarized as `mixed` / `partial_next_step`.
- `suspected_root_cause`: run-level projection is too coarse. `primary_call_issue` and the aggregate reporting path promote `partial_next_step`/`mixed` from low-evidence traces instead of preserving the cleaner `focused_progress` signal.
- `code_surface`: `crates/ploke-eval/src/cli.rs` around `collect_protocol_campaign_triage_report`, `protocol_summary_row_from_aggregate_with_report`, `primary_call_issue`, and `call_review_severity`; `crates/ploke-eval/src/protocol_report.rs` render path for call-level detail.
- `small_slice`: tighten run-summary labeling for short, mostly-successful traces so they do not default to `mixed`/`partial_next_step`, while preserving call-level detail.
- `medium_slice`: add a clean-trace guard or low-evidence bucket using call count, issue density, and segment evidence so triage separates real detours from minimal-progress runs.
- `long_term`: split call-level issue taxonomy from run-level recovery taxonomy and validate both against the blind-review set.
- `recommended_next_step`: implement the small slice in `cli.rs`, then re-run the same campaign triage and expect false-positive `mixed` / `partial_next_step` labels on short clean traces to drop.
- `confidence`: medium-high.
- `exemplar runs reviewed`: `clap-rs__clap-5084`, `tokio-rs__tokio-5200`, `BurntSushi__ripgrep-2626`, `tokio-rs__tokio-4789`, `tokio-rs__tokio-6252`.

# Evidence
- `./target/debug/ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --issue partial_next_step` reports 74 selected runs, 157 issue calls, and near-fanout across `read_file` and `request_code_context`.
- `clap-rs__clap-5084`: 23 calls, 5 usable segments, issue mix led by `partial_next_step`; tool calls show repeated context reads, failed `code_item_lookup`, and a `non_semantic_patch` retry loop.
- `tokio-rs__tokio-5200`: 45 calls, 5 segments, with heavy `search_thrash` / `partial_next_step` overlap and duplicated artifacts.
- `BurntSushi__ripgrep-2626`: 24 calls, 3 segments, with `mixed` and `partial_next_step` both prominent and repeated `non_semantic_patch` / `read_file` activity.
- `tokio-rs__tokio-4789`: 9 calls, 3 usable segments; protocol labels it `search_thrash`-heavy, but the blind summary says both reviewers saw a clean trace, which points to over-labeling.
- `tokio-rs__tokio-6252`: 3/3 call and segment reviews usable; protocol-overview still labels the run `mixed` / `partial_next_step` despite only three successful calls, matching the blind-review mismatch note.
- `crates/ploke-eval/src/cli.rs:5478-5513`: run status is reduced to `full` vs `partial` based only on missing or mismatched coverage.
- `crates/ploke-eval/src/cli.rs:4781-4865`: campaign triage builds family counts from matching call issues and selected entries.
- `crates/ploke-eval/src/cli.rs:5911-5935`: `primary_call_issue` prefers `search_thrash`, then `partial_next_step`, then `overall`; severity also gives `partial_next_step` and `mixed` special treatment.
- `crates/ploke-eval/src/protocol_report.rs:788-799`: call-level rows are rendered directly, so the detailed signal is already available even when run-level summary is too coarse.

# Unsupported Claims
- I did not verify a before/after implementation change for the proposed label calibration.
- I did not inspect every run in the campaign, only the aggregate and five exemplars.
- I did not prove that every `partial_next_step` label is false-positive; some are likely real recovery opportunities.

# Not Checked
- I did not run code changes or tests.
- I did not inspect other `inspect` subcommands beyond `protocol-overview` and `tool-calls` for the sampled runs.
- I did not compare against a regenerated aggregate after any code edit, because no edit was made.

# Risks
- Calibrating run-level labels too aggressively could hide genuine short detours.
- If the summary threshold is changed without preserving call-level detail, the triage surface becomes less useful for downstream debugging.
- The safest intervention is one that reduces false positives on short clean traces while leaving the call-level evidence intact.
