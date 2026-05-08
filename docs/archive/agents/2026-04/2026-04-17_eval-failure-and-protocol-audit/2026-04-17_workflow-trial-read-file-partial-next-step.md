# Scope
Campaign slice: `ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --tool read_file --issue partial_next_step`.
Workstream/gate: eval failure audit, `A1`.

# Claims
- `target_family`: `issue=partial_next_step` on `read_file`.
- `why_this_family`: aggregate triage shows `43 / 221` runs selected, `66` issue calls across `43` runs, and the slice is entirely concentrated in `read_file`.
- `observed_pattern`: exemplar traces start with file/context discovery, then drift into repeated `read_file` / `request_code_context` / `code_item_lookup` calls; several runs end in `search_thrash` or only partial recovery.
- `suspected_root_cause`: the agent is getting local context but not turning it into a decisive next step, so the trace stalls in incremental inspection instead of converging on an action.
- `code_surface`: `crates/ploke-eval/src/cli.rs` issue classification (`primary_call_issue`, `call_review_severity`) and triage next-step generation (`build_triage_next_steps`); `crates/ploke-eval/src/protocol_triage_report.rs` renders the family/report surface.
- `small_slice`: make `read_file` follow-ups more concrete by emitting the next expected action or target artifact after a successful read, instead of another generic inspect prompt.
- `medium_slice`: add a recovery heuristic for `read_file` chains that detects repeated context-only loops and suggests a bounded action plan or file-specific decision point.
- `long_term`: align protocol review labels and guidance with the trace state machine so `partial_next_step` points to an actionable recovery primitive, not just a reporting category.
- `recommended_next_step`: implement the smallest `read_file` follow-up heuristic and measure a drop in `partial_next_step` calls within this slice, plus fewer repeated context requests per run.
- `confidence`: medium-high; the pattern is consistent across the sampled runs, but I only reviewed three exemplars.
- `exemplar runs reviewed`: `clap-rs__clap-5084`, `clap-rs__clap-5075`, `tokio-rs__tokio-5520`.

# Evidence
- Aggregate triage: `43` eligible runs, `66` `partial_next_step` calls, all on `read_file`, with `730/730` call reviews and `165/165` usable segment reviews.
- `clap-rs__clap-5084`: `partial_next_step` on `read_file` appears after a mixed inspection span; tool trace shows repeated `read_file`, `request_code_context`, `code_item_lookup`, then a failed patch attempt.
- `clap-rs__clap-5075`: six `partial_next_step` and six `search_thrash` issues, with early repeated `request_code_context` calls and a later `read_file`-to-lookup recovery attempt.
- `tokio-rs__tokio-5520`: `partial_next_step` is attached to `read_file` and `cargo`, with a mixed/partial trajectory that never reaches a strong recovery signal.
- `crates/ploke-eval/src/cli.rs:5911-5928`: `primary_call_issue` prefers `search_thrash` over `partial_next_step`, and `call_review_severity` assigns `0.7` to `partial_next_step`.
- `crates/ploke-eval/src/cli.rs:5269-5315`: `build_triage_next_steps` only suggests another inspection command or exemplar follow-up; it does not propose a concrete corrective action.
- `crates/ploke-eval/src/protocol_triage_report.rs:1-220`: the report surface is presentation-only, so the missing behavior is in the upstream classification/guidance logic rather than rendering.

# Unsupported Claims
- I did not inspect every run in the campaign.
- I did not run a fix or verify a post-change metric.
- I did not inspect non-`ploke-eval` production crates.

# Not Checked
- Whether the same pattern appears under other tools besides `read_file`.
- Whether the issue is driven more by prompt shape, trace packaging, or downstream protocol labels.
- Whether a concrete heuristic would need changes outside `crates/ploke-eval/`.

# Risks
- A narrow heuristic may improve this slice while leaving other `partial_next_step` variants unchanged.
- Overfitting to `read_file` could hide adjacent failures in `request_code_context` or `code_item_lookup`.
- If the next-step logic is changed too aggressively, it could suppress useful exploratory behavior instead of reducing thrash.
