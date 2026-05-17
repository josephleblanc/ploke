# ploke-egui Allocation Priorities Overclaimed

## Trigger

The user challenged the agent's priority claim that central graph rendering was
the top allocation problem after the agent listed it first in a performance
triage answer.

## User-Visible Failure

The agent ranked `central_graph` as the first area to fix because it appeared as
a top named allocation group in idle benchmark scenarios, then inferred likely
sub-causes in graph shape and edge-label rendering. When asked whether the
allocations actually showed that, the numbers showed `central_graph` was only a
small fraction of total allocated object bytes per frame.

## Touched Surface

- `crates/ploke-egui/docs/profiling/benchmarks/20260517-c7a9bab87e76-standard/report.json`
- `crates/ploke-egui/src/ui/view/{mod.rs,edge.rs,node.rs,label.rs}`
- `crates/ploke-egui/src/ui/app/shell.rs`

## What The Agent Did

- Treated "top named group" as if it meant "dominant allocation source".
- Did not calculate per-frame byte contribution before ranking priorities.
- Mixed measured facts with code-informed guesses about `egui_graphs`, shape
  generation, dotted edges, and edge-label diagnostics.
- Failed to call out that standard benchmark mode had no callsite attribution.

## Skipped Docs / Skills / Instructions

- `ploke-egui-benchmarking` allocation discipline requires allocation churn
  summaries and callsite-attribution status.
- Verification-surface honesty requires not letting one measured surface stand
  in for another.
- The user's standing performance direction is to make decisions from measured
  quantities whenever available.

## Why This Was Risky

The false ranking would have pushed the next optimization slice toward one of
the lower measured byte contributors, while the benchmark still had a large
unattributed `root` allocation body and larger expanded-inspector groups. That
would waste implementation time and make later benchmark movement look
arbitrary.

## Concrete Prevention Rule

Before ranking performance work, calculate per-frame contribution for each
measured group and state attribution limits. A span/group may be called
"observed" only from benchmark data; a code path may be called a "suspect" only
if it is explicitly labeled as inference or backed by callsite/subspan data.

## Memory Hypothesis

Future memory should bias agents to present a numeric table first for
`ploke-egui` allocation triage: scenario, total bytes/frame, named group
bytes/frame, percent of total bytes/frame, callsite attribution status, and only
then proposed priority.
