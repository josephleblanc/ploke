# 2026-04-17 Campaign Protocol Triage Implementation Report

- task_id: `AUDIT-IMPL-P0`
- packet: [packet-P0-campaign-protocol-triage.md](./packet-P0-campaign-protocol-triage.md)
- date: 2026-04-17
- owner_role: orchestrator
- layer_workstream: `A4`

## Claims

1. Acceptance criteria `1` and `2` are met by a new campaign-scoped command path:
   - `ploke-eval inspect protocol-overview --campaign <id>`
   - it produces a compact triage dashboard without requiring JSON/JQ reconstruction
   - it separates `error`, `partial`, `missing`, `full`, and `ineligible` protocol states
2. Acceptance criteria `3` and `4` are met:
   - the dashboard now ranks issue kinds and issue tools
   - it includes representative exemplar runs and grouped problem families
3. Acceptance criteria `5` and `6` are partially met:
   - each dashboard includes bottom-of-step follow-up command hints
   - the operator can now move from campaign aggregate to issue/tool/status drilldown and then to a concrete exemplar run in a bounded command chain

## Evidence

- Code surface:
  - [`crates/ploke-eval/src/cli.rs`](../../../../crates/ploke-eval/src/cli.rs)
  - [`crates/ploke-eval/src/protocol_triage_report.rs`](../../../../crates/ploke-eval/src/protocol_triage_report.rs)
  - [`crates/ploke-eval/src/lib.rs`](../../../../crates/ploke-eval/src/lib.rs)
- Build verification:
  - `cargo check -p ploke-eval` passed
- Live operator-flow checks:
  - `./target/debug/ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai`
  - `./target/debug/ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --status error`
  - `./target/debug/ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --issue search_thrash`
  - `./target/debug/ploke-eval inspect protocol-overview --campaign rust-baseline-grok4-xai --tool read_file`
  - `./target/debug/ploke-eval inspect protocol-overview --all-runs --limit 5`
- Observed campaign-level triage output now includes:
  - protocol status counts across the campaign
  - reviewed-call / segment-evidence bars
  - ranked issue kinds and issue tools
  - grouped problem families
  - representative exemplars
  - next-step command suggestions

## Unsupported Claims

- No claim is made that the dashboard is fully polished.
- No claim is made that `cargo test -p ploke-eval` passes end-to-end.

## Not Checked

- A full `cargo test -p ploke-eval` pass.
- JSON consumers of the new campaign triage report beyond manual inspection.
- Whether the unfiltered dashboard should suppress the “Family context” section by default.

## Risks

- The test build is still blocked by unrelated existing compile errors outside this slice:
  - `ProtocolReportRenderOptions.color_profile`
  - `PreparedSingleRun.campaign`
- The unfiltered dashboard currently shows nearby segment-label/status context in addition to the top-level triage sections; that may still be denser than ideal.
- Narrow table widths still truncate long follow-up commands, although the command intent is visible.
