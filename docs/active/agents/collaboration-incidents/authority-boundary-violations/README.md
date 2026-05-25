# Authority Boundary Violations

Incidents where the agent moved from diagnosis or archaeology into edits on
authority-heavy code without first earning that move from docs, user direction,
or the right structural workflow.

## Entries

- [`2026-05-15-history-claims-overreach.md`](2026-05-15-history-claims-overreach.md)
  During artifact-identity archaeology, the agent started editing Prototype 1
  History claim structure in `ploke-eval` without first respecting History as a
  central control/authority surface.
- [`2026-05-17-headless-harness-probe-used-primary-gitdir.md`](2026-05-17-headless-harness-probe-used-primary-gitdir.md)
  The agent ran a hidden broad headless-TUI harness probe against a request
  whose git worktree metadata pointed back to the primary checkout's shared
  `.git`, instead of first isolating the probe checkout and request carrier.
