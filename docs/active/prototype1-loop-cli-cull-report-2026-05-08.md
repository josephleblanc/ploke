# Prototype 1 Loop CLI Cull Report

## 1. Scope and verdict legend

Scope: `ploke-eval loop` commands only. This report excludes deep analysis of the `prototype1-state` runner path and treats it as out of cull scope.

Verdicts:

- `KEEP` - useful and currently fit for operator use.
- `FIX` - keep the command shape, but repair behavior or output.
- `CULL` - remove, rename, or replace.
- `RUNNER` - runtime path, not part of CLI cull.
- `LEGACY` - should be moved behind a legacy namespace or retired.

## 2. Recommended cull order

1. `prototype1-monitor peek`: operator-unsafe. File-level limits still explode into ~1700 lines. Replace or delete first.
2. `prototype1-monitor timing` non-watch mode: misleading empty 0ms tree. Split the watch/probe mode from any real timing view.
3. `prototype1-monitor report`: broad provisional summary; the name overstates authority. Demote or replace with a narrower alias.
4. `prototype1-runner`: hidden from top-level help, advertised as inspect, but the tested node returned exit 0 with no output.
5. Legacy `prototype1` / `prototype1-setup`: help is tangled and still exposes old wrapper stages; split current setup from historical wrapper behavior or retire the wrapper surface.
6. `prototype1-branch` commands: manipulative treatment-branch surface. Move behind a legacy namespace unless still actively required.
7. Diagnostics walls in `history-metrics`, `history-scores`, `score-selection-review`, `child-evidence`: keep the projections, but cap diagnostics hard.

## 3. Command inventory

| Command | Verdict | Note |
|---|---|---|
| `prototype1` | LEGACY | Legacy wrapper help; overlaps with setup and old staged flow. |
| `prototype1-setup` | FIX | Only keep if it is the current setup entry; help still exposes old stages/options. |
| `prototype1-state` | RUNNER | Current typed parent runtime path; out of cull scope. |
| `prototype1-branch status` | LEGACY | Branch registry view; large and treatment-branch oriented. |
| `prototype1-branch show` | LEGACY | Stored synthesized branch content/metadata. |
| `prototype1-branch apply` | LEGACY | Applies synthesized treatment branch. |
| `prototype1-branch evaluate` | LEGACY | Runs treatment arm and compares metrics. |
| `prototype1-branch select` | LEGACY | Selects synthesized treatment branch active. |
| `prototype1-branch restore` | LEGACY | Restores source content and clears active state. |
| `prototype1-runner` | CULL | Hidden inspect path; unusable in the tested case. |
| `prototype1-monitor list` | KEEP | Location map only; intentionally not History authority. |
| `prototype1-monitor status` | FIX | Useful, but current liveness classification is wrong in some cases. |
| `prototype1-monitor child-evidence` | FIX | Read-only projection, but output is too large by default. |
| `prototype1-monitor history-metrics` | FIX | Read-only projection, but diagnostics are not bounded. |
| `prototype1-monitor history-scores` | FIX | Read-only projection, but diagnostics need a wall. |
| `prototype1-monitor score-selection-review` | FIX | Read-only projection, but diagnostics swamp the rows. |
| `prototype1-monitor selection-show` | KEEP | Bounded and useful. |
| `prototype1-monitor history-preview` | KEEP | Bounded and useful. |
| `prototype1-monitor peek` | CULL | Emits excerpts across many files and ignores the practical line/byte limits. |
| `prototype1-monitor report` | CULL | Functional but broad and provisional; the name overstates authority. |
| `prototype1-monitor timing` | CULL | Non-watch mode is misleading; should be split into real timing vs watch/probe behavior. |
| `prototype1-monitor watch` | KEEP | Polls snapshots; useful as a live probe. |

## 4. Empirical usability notes

- `loop --help` only exposes `prototype1`, `prototype1-setup`, `prototype1-state`, `prototype1-branch`, and `prototype1-monitor`. It omits `prototype1-runner`, which is a discoverability bug.
- `prototype1-monitor --help` is clear and complete for the monitor family.
- `prototype1-branch --help` reads as a legacy treatment-branch surface, not a live-loop operator surface.
- `prototype1-runner --help` says inspect, but it also has `--execute`. The tested command exited 0 with no output, so the inspect path is not operator-safe in practice.
- `prototype1` and `prototype1-setup` help are nearly identical and both still expose old wrapper stages/options like `--stop-after baseline-eval|baseline-protocol|target-selection|intervention-apply|compare`.
- `prototype1-state --help` is the only help text that reads like the current runner contract: campaign, node-id, repo-root, handoff-invocation, stop-after, successor-selection, candidate-generator, edit-surface.
- `prototype1-monitor list` is bounded and useful as a path map.
- `prototype1-monitor status` is readable, but it can misclassify active or recently active runs as dead because it mixes journal, scheduler, and stale PID evidence.
- `prototype1-monitor history-metrics --rows 5` still printed 302 lines and 193 diagnostics. That is not operator-safe.
- `prototype1-monitor history-scores --rows 5` bounded the child rows, but diagnostics still ran long.
- `prototype1-monitor score-selection-review --rows 5` behaved similarly: useful failure signal, poor default output control.
- `prototype1-monitor child-evidence` has no practical row limit and is too large by default.
- `prototype1-monitor history-preview --entries 3 --diagnostics 3` behaves well and should survive.
- `prototype1-monitor selection-show --row 0` is bounded, concrete, and useful.
- `prototype1-monitor report` gives a broad summary, but it is provisional and too wide for common operator use.
- `prototype1-monitor timing --depth node` printed an empty 0ms timing tree even though watch mode showed provider activity.
- `prototype1-monitor peek --lines 5 --bytes 2048` still dumped about 1700 lines because the limits apply per file across many files.

## 5. Proposed surviving minimal operator surface

- `prototype1-state` as the runner.
- `prototype1-setup` only if it is confirmed as the current setup entry, not the legacy wrapper.
- `prototype1-monitor status`.
- `prototype1-monitor selection-show`.
- `prototype1-monitor history-preview`.
- `prototype1-monitor list` if a location map is still useful.
- A future `prototype1-monitor activity` command should replace the current timing/watch confusion and separate real timing from provider-attempt probing.
