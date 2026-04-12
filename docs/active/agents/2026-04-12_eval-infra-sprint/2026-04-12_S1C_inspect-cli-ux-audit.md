# S1C - Inspect CLI UX Audit

- Date: 2026-04-12
- Owner role: worker
- Layer/workstream: A5
- Related hypothesis: The eval inspection CLI is only useful as a frequent internal touch-point if developers and agents can bootstrap themselves into the commands quickly, extract answers without digging through implementation code, and use the interface without friction
- Design intent: Evaluate the `ploke-eval` inspect/CLI surface as a real user entry-point for quick eval triage and inspection work
- Scope: Audit the inspect-oriented CLI commands in `ploke-eval` for discoverability, ergonomics, and usefulness when answering common eval questions from artifacts and prior runs
- Non-goals: Do not redesign the entire CLI in one packet, do not broaden into unrelated product UX, do not require changing the underlying data model just to complete the audit
- Owned files: `crates/ploke-eval/src/cli.rs`, `crates/ploke-eval/src/lib.rs`, `crates/ploke-eval/tests/**`, related docs as needed
- Dependencies: accepted `P0A`-`P0F`, accepted `P0D/P0E`
- Acceptance criteria:
  1. The audit identifies the main inspect/CLI entry-points and evaluates whether they are sufficient for quick internal eval checks without implementation spelunking.
  2. The output distinguishes missing information, weak ergonomics, and missing command coverage.
  3. The output frames the CLI as both a developer and agent bootstrap surface and proposes concrete improvements or follow-up packets.
  4. The output includes at least a small set of representative inspection questions/tasks and whether the current CLI can answer them cleanly.
- Required evidence:
  - sampled command list and representative invocation paths
  - concise UX findings with file or command references
  - explicit note on what still requires dropping to code or raw artifacts
  - recommended follow-up packet(s) for CLI tightening or docs/help improvements
- Report-back location: `docs/active/agents/2026-04-12_eval-infra-sprint/`
- Status: ready

## Permission Gate

No additional permission required if work stays inside `crates/ploke-eval/` and docs.
