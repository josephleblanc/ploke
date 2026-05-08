# Protocol Aggregate CLI

- date: 2026-04-15
- task title: protocol aggregate cli
- task description: add a human-facing `ploke-eval` inspection surface that aggregates persisted protocol artifacts into terminal-native reports and run summaries
- related files: `crates/ploke-eval/src/cli.rs`, `crates/ploke-eval/src/protocol_aggregate.rs`, `crates/ploke-eval/src/protocol_report.rs`

## What Landed

- new inspect surface:
  - `./target/debug/ploke-eval inspect protocol-overview ...`
  - short alias:
    - `./target/debug/ploke-eval inspect proto ...`
- supports:
  - single-run aggregate report
  - all-runs summary table via `--all-runs`
  - filtering by `--overall`, `--segment-label`, `--tool`
  - focused views via `--view overview|segments|calls`
  - terminal width control via `--width`
  - color control via `--color auto|always|never`
  - semantic color profiles via `--color-profile tokio-night|gruvbox|mono-dark`

## 2026-04-15 Evidence-Reliability Pass

- the first follow-on slice has now shifted the surface away from generic coverage language and toward an evidence-reliability framing
- the single-run report now emphasizes:
  - `Call reviews`
  - `Usable seg reviews`
  - `Segment evidence` as `usable / mismatch / missing`
  - provenance lines for the active anchor and the derived procedure family
- the old generic `Coverage shape` and `Signal histograms` sections were removed
- the report now includes larger issue-surface bar charts:
  - issue kinds by count
  - issue tools by count
- the segment table now uses explicit evidence labels (`usable`, `mismatched`, `missing`) instead of the earlier overloaded `Cover`
- the all-runs summary now uses the same evidence-reliability language in compact form

## Current Command Examples

- all runs:
  - `./target/debug/ploke-eval inspect proto --all-runs --limit 12`
- one run:
  - `./target/debug/ploke-eval inspect proto --instance tokio-rs__tokio-5200 --width 100`
- one run with an alternate semantic palette:
  - `./target/debug/ploke-eval inspect proto --instance tokio-rs__tokio-5200 --width 100 --color-profile gruvbox`
- filtered segment view:
  - `./target/debug/ploke-eval inspect proto --instance tokio-rs__tokio-5200 --view segments --only-issues --width 100`

## Design Notes

- aggregation is anchored to the latest persisted `tool_call_intent_segmentation` artifact for a run
- segment reviews are only merged when they still match the selected anchor basis
- anchor mismatches are surfaced explicitly instead of silently merged
- older malformed review artifacts are skipped so all-runs inspection remains usable

## Known Gaps

- some large runs still show many `anchor mismatch` segment rows because persisted segment reviews were generated against different segmentation bases
- the all-runs summary is intentionally compact and still uses shorthand (`u/m/x`) that likely wants one more readability pass or a richer alternate mode
- the call-issue detail column still truncates low-value raw summaries and needs a better protocol-aware detail projection
- the report is currently factual/mechanical, not yet a richer intervention-hypothesis surface

## Immediate Next Slice

1. refine all-runs readability so the compact evidence legend and shorthand land more cleanly
2. improve call-issue drilldown detail so issue rows carry more useful context than truncated raw payload summaries
3. add a richer all-runs inspection mode for cross-run ranking and filtering
