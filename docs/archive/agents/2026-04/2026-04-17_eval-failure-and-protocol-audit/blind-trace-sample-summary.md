# 2026-04-17 Blind Trace Sample Summary

- date: 2026-04-17
- sample source: random bag drawn from protocol-complete runs in the
  `rust-baseline-grok4-xai` campaign
- review mode: two independent blind CLI-only reviews using
  `./target/debug/ploke-eval inspect ...` commands only
- protocol comparison surface: `./target/debug/ploke-eval inspect protocol-overview --format json`

## Sampled Runs

- `clap-rs__clap-3700`
- `clap-rs__clap-5228`
- `clap-rs__clap-5873`
- `BurntSushi__ripgrep-1980`
- `sharkdp__bat-1518`
- `tokio-rs__tokio-4789`
- `tokio-rs__tokio-6252`
- `sharkdp__bat-1402`

## Blind Consensus

| run_id | blind consensus | notes |
|---|---|---|
| `clap-rs__clap-3700` | recoverable path/layout failure plus a later lookup mismatch | both reviewers called out missing-file recovery and the later `function` vs `method` lookup miss |
| `clap-rs__clap-5228` | low-signal / no hard failure | both reviewers saw repeated zero-hit search and shallow exploration, but no trace-visible hard failure |
| `clap-rs__clap-5873` | sustained path/root thrash | both reviewers treated this as the noisiest path-finding trace in the sample |
| `BurntSushi__ripgrep-1980` | recoverable path/layout failure | both reviewers called out the wrong `crates/rg` path and later recovery via `crates/core` |
| `sharkdp__bat-1518` | single recoverable path failure | both reviewers treated the missing `controller.rs` path as the only clear issue |
| `tokio-rs__tokio-4789` | clean short success path | neither reviewer found a trace-visible hard failure |
| `tokio-rs__tokio-6252` | clean minimal success path | neither reviewer found a trace-visible hard failure |
| `sharkdp__bat-1402` | recoverable patch-format failure | both reviewers treated malformed unified diff format as the key issue |

## Agreement Numbers

- blind reviewer exact agreement on the coarse run-level class:
  - `8/8` = `100.0%`
  - Wilson 95% interval: `67.6%` to `100.0%`
- protocol-vs-blind agreement on the same sample:
  - strict run-level reading: `5/8` = `62.5%`
  - Wilson 95% interval: `30.6%` to `86.3%`
  - lenient reading that credits borderline alignment for `clap-rs__clap-5228`
    and call-level patch-format capture on `sharkdp__bat-1402`: `6/8` =
    `75.0%`
  - Wilson 95% interval: `40.9%` to `92.9%`

## Protocol Comparison

| run_id | protocol signal | blind alignment | notes |
|---|---|---|---|
| `clap-rs__clap-3700` | call issues dominated by `search_thrash`; segment mix includes `recoverable_detour` and `focused_progress` | aligned | protocol captures the failed `read_file` path and later lookup friction, though it labels them mostly as `search_thrash` |
| `clap-rs__clap-5228` | top issue `search_thrash`; segments read as `recoverable_detour` + `focused_progress` | borderline | blind reviewers saw a low-signal trace without a hard failure; protocol is harsher |
| `clap-rs__clap-5873` | top issue `search_thrash`; segment mix strongly detour-heavy | aligned | strong agreement that the trace is dominated by path/root thrash |
| `BurntSushi__ripgrep-1980` | top issue `search_thrash`; segment mix `focused_progress` + `mixed` | aligned | protocol and blind review both surface recoverable layout discovery |
| `sharkdp__bat-1518` | top issue `search_thrash`; segments mostly `focused_progress` | aligned | protocol sees the same short recoverable path miss, though again under a `search_thrash` label |
| `tokio-rs__tokio-4789` | top issue `search_thrash`; all segments non-clean | mismatch | both blind reviewers saw a clean trace |
| `tokio-rs__tokio-6252` | run-level protocol summary is `mixed` / `partial_next_step` despite only three successful calls | mismatch | both blind reviewers saw a clean minimal trace |
| `sharkdp__bat-1402` | run-level summary is still `mixed`, but call-level protocol output captures the malformed `non_semantic_patch` failure exactly | aligned at call level | this is the clearest example where run summary is too coarse but the call-level artifact is useful |

## Main Takeaways

- Blind reviewers converged strongly on a path/layout-recovery story:
  `5/8` runs primarily showed path/root/layout friction, `1/8` showed a real
  patch-format failure, and `2/8` looked clean or too short to indict.
- The current protocol outputs are directionally useful on the noisier traces,
  but they appear to over-assign `search_thrash` and `mixed` labels on short or
  otherwise clean traces.
- The strongest false-positive candidates in this sample are:
  - `tokio-rs__tokio-4789`
  - `tokio-rs__tokio-6252`
- `sharkdp__bat-1402` shows that call-level protocol detail can be much more
  faithful than the run-level aggregate.
