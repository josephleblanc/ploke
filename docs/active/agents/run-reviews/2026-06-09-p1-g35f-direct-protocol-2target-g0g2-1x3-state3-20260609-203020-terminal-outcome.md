# Terminal Campaign Outcome: p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020

Date: 2026-06-09
Campaign: `p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`

This is a terminal status and fan-in synthesis for the completed two-target
direct-Google campaign. It is not a full semantic patch-quality review for every
child branch; those live in the per-branch run reviews linked below.

## Verdict

The campaign is no longer live. Process probes found no `ploke-eval loop`,
`prototype1-state`, `prototype1-step`, `prototype1-continue`, or
`prototype1-runner` processes tied to this campaign id (only an operator
`tail -f` on the state log).

Termination is a **clean configured stop**, not a crash or auth failure. After
gen-2 children finished under parent `node-0dca4fe449780e85`, selection chose
kept branch `branch-98d590831a932f41` / `node-f4a0067a0d3d0234` as successor.
That gen-2 node completed its treatment runner successfully, attempted successor
handoff, and hit the expected hard stop:

```text
prototype1 hard stop before child planning: parent generation 2 has reached max_generations 2
```

Evidence:

- Transition journal `successor` completion at `2026-06-10T05:07:47.484Z` on
  `node-f4a0067a0d3d0234` with the detail above.
- Channel record:
  `prototype1/nodes/node-f4a0067a0d3d0234/channels/ed09a6b6-8c19-43ed-a529-fbed80532d72/child-to-parent.jsonl`
  → `successor_completion.status=failed` with the same max-generations message.
- Gen-1 parent channel:
  `prototype1/nodes/node-0dca4fe449780e85/channels/377cc8c8-8f30-4f79-8ec7-5c41393e1389/child-to-parent.jsonl`
  → `successor_completion.status=succeeded` at `2026-06-10T05:07:47.498Z`.

The gen-2 successor failure is **expected** under `max_generations=2`; the gen-1
parent still completed cleanly with acknowledged handoff
(`successor_handoff=acknowledged` in parent stdout).

**No HTTP_401** appeared anywhere in this campaign. Gen-2 treatment failures on
`branch-4631ad4187027c68` and `branch-fba911f62c712d5a` were
`HTTP_429 RESOURCE_EXHAUSTED` aborts on `BurntSushi__ripgrep-2209`, surfaced to
the parent runner as `treatment_failed` / batch-selection invalid — provider
capacity, not OAuth expiry (contrast with the sibling `state3-184302` run).

**Risk level:** low for loop-control termination; medium for interpreting gen-2
branch merit because two of three gen-2 treatments failed on provider 429 while
the selected branch succeeded mechanically.

## Evidence Roots

- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020`
- Baseline closure:
  `closure-state.json` (eval+protocol 2/2 complete, updated `2026-06-10T03:53:55Z`)
- Run profile:
  `prototype1/run-profile.toml`, `prototype1/scheduler.json`
- Authority spine:
  `prototype1/transition-journal.jsonl`, `prototype1/branches.json`
- Terminal channels:
  - gen-1 parent:
    `prototype1/nodes/node-0dca4fe449780e85/channels/377cc8c8-8f30-4f79-8ec7-5c41393e1389/child-to-parent.jsonl`
  - gen-2 successor attempt:
    `prototype1/nodes/node-f4a0067a0d3d0234/channels/ed09a6b6-8c19-43ed-a529-fbed80532d72/child-to-parent.jsonl`

Checked: closure state, transition journal tail, runner results for all seven
nodes, branch registry, treatment closure for failed gen-2 branches. Did not
open sqlite databases.

## Profile

From `prototype1/run-profile.toml` / `prototype1/scheduler.json`:

| setting | value |
| --- | --- |
| Parent model | `google/gemini-3.5-flash`, `direct_google` |
| Protocol model | same |
| Targets | `BurntSushi__ripgrep-2209`, `BurntSushi__ripgrep-2295` |
| `max_generations` | **2** (zero-based indexing → gen 0, 1, 2) |
| `parallel_targets` | 3 |
| `stop_on_first_keep` | false |
| `require_keep_for_continuation` | false |
| `explore_from_rejected` | true |
| `stop_after` | complete |

Generation indexing in node records is zero-based. The run reached generation-2
children, then stopped when the selected gen-2 successor tried to plan generation-3
children.

## Execution Path

```text
baseline eval+protocol (gen-0 parent node-e41b4e1ef747bb15)
  -> broad-harness child-plan (3 gen-1 slots)
  -> treatment eval per kept/rejected branch
  -> successor selection (history-traversal)
  -> gen-1 parent child-plan + broad-harness (3 gen-2 slots)
  -> treatment eval per gen-2 branch
  -> successor selection -> node-f4a0067a0d3d0234
  -> successor handoff -> max_generations hard stop
  -> gen-1 parent_complete + successor_completion succeeded
```

Baseline eval turns used `prototype1-step` / `run_benchmark_turn`. Broad-harness
slots used `tui_adapter::run_headless_with_model`. Treatment children used
spawned treatment campaigns built from derived git commits.

## Handoff And Generation Path

| Phase | Selected successor | Branch | Disposition | Evidence |
| --- | --- | --- | --- | --- |
| Gen-0 → gen-1 | `node-0dca4fe449780e85` | `branch-f7e43aba97c12538` | keep | Transition journal successor selection `2026-06-10T04:26:05Z`; branch registry |
| Gen-1 → gen-2 | `node-f4a0067a0d3d0234` | `branch-98d590831a932f41` | keep | Transition journal selection `2026-06-10T05:07:29Z`; parent stdout `successor=node-f4a0067a0d3d0234` |
| Gen-2 → gen-3 | — | — | **hard stop** | `max_generations 2` at `2026-06-10T05:07:47Z` |

### Gen-0 baseline and broad harness

- Baseline eval+protocol: both instances complete (`closure-state.json`).
- Gen-1 broad slots: base, r2, r3 (reviews:
  [`broad-base`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-base.md),
  [`broad-r2`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r2.md),
  [`broad-r3`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-broad-r3.md)).
- Gen-1 treatment outcomes: `branch-f7e43aba` **keep**; `branch-3dce62110` and
  `branch-bc17b460` **reject** (2295 tool-failure regressions).

### Gen-1 parent and broad harness

Parent `node-0dca4fe449780e85` published three gen-2 slots from kept treatment
artifact `b5d2c10a` (gen-1 treatment binary). Broad-harness reviews:
[`gen1-broad-base`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-base.md),
[`gen1-broad-r3`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r3.md),
[`gen1-broad-r4`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-gen1-broad-r4.md).

### Gen-2 children (final generation)

| Node | Branch | Broad origin | Runner | Treatment 2209 | Treatment 2295 |
| --- | --- | --- | --- | --- | --- |
| `node-f4a0067a0d3d0234` | `branch-98d590831a932f41` | gen-1 r3 `code_item_lookup` guard | **succeeded** | complete | complete |
| `node-2b876d18e53c7bfa` | `branch-4631ad4187027c68` | gen-1 r4 `request_code_context` | **failed** | **429 aborted** | complete |
| `node-07fa4f4ab22217dd` | `branch-fba911f62c712d5a` | gen-1 base `error.rs` | **failed** | **429 aborted** | complete |

Runner failure detail (both rejected branches):

```text
batch selection is invalid: treatment '...' instance 'BurntSushi__ripgrep-2209'
did not produce complete run metrics (status=failed)
```

Treatment closure confirms underlying cause: terminal turn
`HTTP_429 RESOURCE_EXHAUSTED` on Vertex for 2209; 2295 completed on both.

## Concrete Trace Chain (terminal stop)

Verified chain for the configured stop (not model patch quality):

1. Gen-1 parent stdout records
   `completed:Keep;children_ran=3;selection=Accepted;successor=node-f4a0067a0d3d0234`
   after all three gen-2 treatment observers finish.
2. `node-f4a0067a0d3d0234` treatment runner exits 0 (`runner-result.json`,
   `2026-06-10T05:07:29Z`); branch evaluation `overall_disposition=keep`.
3. Successor runtime `ed09a6b6-8c19-43ed-a529-fbed80532d72` starts on gen-2 node;
   stderr logs the max-generations message immediately.
4. Channel writes `successor_completion.status=failed` on gen-2 node; gen-1
   parent channel writes `successor_completion.status=succeeded` 14 ms later.
5. Transition journal records `parent_complete` cargo measurement on gen-1 parent.

This proves loop control reached the budget boundary with persisted handoff
evidence; it does not prove the selected patches are oracle-correct.

## Mechanical Completion Versus Benchmark Usefulness

| layer | state |
| --- | --- |
| Baseline closure | complete (2/2 eval, 2/2 protocol) |
| Gen-1 kept treatment | mechanically complete; mixed benchmark usefulness ([`treatment-f7e43aba`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md)) |
| Gen-2 selected treatment | mechanically complete; operational `keep` only ([`treatment-98d590`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-98d590.md)) |
| Gen-2 rejected treatments | provider 429 on 2209; not model/patch rejection |
| Oracle / MBE | record-only mode; no joined proof |
| Loop termination | clean `max_generations` stop |

## What Is Working

- Baseline and both successful treatment campaigns reached full eval+protocol closure.
- Provider tool-call ledger parity held on completed eval turns (no missing call ids
  in trace audits for selected gen-2 branch).
- Successor selection, treatment spawn, and parent completion records are
  internally consistent across journal, channels, and runner results.
- No OAuth/auth regression on this run (unlike `state3-184302`).

## What Is Not Working Yet

- Gen-2 parallel treatment fan-out hit Vertex `HTTP_429` on two of three 2209
  turns, leaving incomplete batch metrics and `treatment_failed` runner rows despite
  partial 2295 success.
- Branch evaluation and selection still treat provider-aborted 2209 as a batch
  invalidity on the parent runner, without separating provider vs merit failure.
- Benchmark patch quality on both kept generations remains uncertain on 2209
  (test-expectation edits, alternate algorithm shapes) per child reviews.
- Scheduler snapshot (`scheduler.json`) was never updated past gen-0 planning;
  liveness authority is channel/journal based, not stale node files.

## Action Items

### Non-blockers (alive bugs / follow-up)

- Classify `HTTP_429` treatment aborts separately from patch-merit
  `treatment_failed` in branch-evaluation batch validity (artifact gap observed
  on `branch-4631ad4187027c68`, `branch-fba911f62c712d5a`).
- Refresh or deprecate stale `scheduler.json` frontier after campaign completion
  so post-hoc status queries do not contradict journal authority.

### Blockers

- None for recording this campaign as cleanly terminated. No
  `ploke-blocker-repair-loop` handoff required before using these artifacts in
  comparisons.

## Follow-Up Reviews

Completed or indexed elsewhere:

- [`baseline-eval-protocol`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-baseline-eval-protocol.md)
- [`treatment-f7e43aba`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-f7e43aba.md) (gen-1 selected)
- [`treatment-98d590`](2026-06-09-p1-g35f-direct-protocol-2target-g0g2-1x3-state3-20260609-203020-treatment-98d590.md) (gen-2 selected)
- Gen-2 failed branches: provider-429 RCA only unless rerunning; no durable
  trace-bearing patch review warranted for aborted 2209 turns.
