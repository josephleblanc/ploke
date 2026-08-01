# `run_prototype1_state_turn` smell clusters

Date: 2026-06-15
Scope: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`, `run_prototype1_state_turn` only.
Status: observation only; no refactor decisions.

## Why this pass exists

The function is about 555 lines. A large fraction is not algorithmic work; it is repeated argument plumbing, repeated observability fields, and final report assembly over values created hundreds of lines earlier.

This note marks the repeated clusters before proposing any code changes.

## High-argument call blocks

Current high-argument call sites inside the function:

| Lines | Call | Args | Apparent cluster |
|---|---:|---:|---|
| 7232-7237 | `initialize_prototype1_parent_identity` | 5 | command + turn coordinate |
| 7327-7333 | `acknowledge_prototype1_state_handoff` | 6 | command + turn coordinate + parent |
| 7362-7371 | `append_parent_target_sample` | 7 | journal/evidence + parent context + phase |
| 7382-7387 | `establish_parent_baseline` | 5 | campaign/config + parent identity |
| 7420-7430 | `resolve_child_plan_for_id` | 10 | campaign/checkout + parent + run-policy child planning inputs |
| 7459-7464 | `active_strategy` | 5 | successor-selection policy pieces |
| 7469-7474 | `ParentSelection::new` | 5 | selection context over parent + outcomes/rejections |
| 7480-7493 | `run_adaptive_child_fanout` | 13 | child runtime context + adaptive selection inputs |
| 7497-7510 | `run_child_fanout` | 13 | child runtime context |
| 7512-7517 | `ParentSelection::new` | 5 | selection context over parent + outcomes/rejections |
| 7576-7583 | `live_successor_continuation_decision` | 7 | successor decision context |
| 7606-7611 | `SuccessorRecord::selected_with_decision` | 5 | successor journal record construction |
| 7621-7628 | `spawn_and_handoff_prototype1_successor` | 7 | successor handoff context |
| 7641-7646 | `SuccessorRecord::stopped` | 5 | successor journal record construction |
| 7662-7671 | `append_parent_target_sample` | 7 | journal/evidence + parent context + phase |
| 7750-7756 | `record_prototype1_successor_completion` | 6 | predecessor/successor completion record |

These calls occupy roughly 131 lines, mostly vertical argument lists.

## Repeated argument clusters

### 1. Turn coordinate cluster

Repeated values:

- `campaign_id`
- `manifest_path`
- `repo_root`

Appears in initialization, parent acknowledgement, child planning, child fanout, successor handoff, reporting.

Likely meaning: one active turn’s campaign + manifest + checkout coordinate.

Smell: this is a concept, but the function carries it as loose locals. Every helper manually chooses a subset.

### 2. Campaign/config cluster

Repeated values:

- `campaign_id`
- `manifest_path`
- `resolved_campaign`
- closure/baseline state implied by `ensure_prototype1_baseline_closure_state`

Appears around baseline setup and child planning.

Likely meaning: resolved campaign scope for this turn.

Smell: campaign identity, manifest path, and resolved config are separated even though most phases treat them as one authority/config scope.

### 3. Parent authority cluster

Repeated values:

- `parent`
- `parent_identity`
- `handoff_invocation`
- `manifest_path`
- `repo_root`

Appears in parent load/check/startup, parent start journaling, baseline, child fanout, selection, successor continuation.

Likely meaning: active parent after identity/handoff/history startup authority is resolved.

Smell: parent identity is cloned/rebound and threaded separately from `Parent<Ready>`, so it is easy to lose which identity is authoritative at each point.

### 4. Evidence/journal cluster

Repeated values:

- `journal`
- `journal_path`
- `parent_identity`
- `handoff_invocation.runtime_id()`
- `repo_root`
- resource phase (`ParentStart` / `ParentComplete`)

Appears in `ParentStarted`, two `append_parent_target_sample` calls, successor journal records, and final report.

Likely meaning: evidence scope for the current parent turn.

Smell: durable journal records, resource samples, tracing logs, observe spans, and CLI report are interleaved without a clear observability taxonomy.

### 5. Run policy cluster

Repeated values:

- `run_shape.stop_after`
- `run_shape.observe_child_stale_after`
- `run_shape.candidate_generation`
- `run_shape.broad_tui`
- `run_shape.successor_selection*`
- `complete_search_policy`
- `plan_child_budget`
- `child_budget`
- `child_schedule_mode`

Appears in child planning, child execution, and successor selection.

Likely meaning: normalized execution policy for this turn.

Smell: `run_shape` helps, but then policy fragments are unpacked into several additional locals and passed positionally.

### 6. Child planning cluster

Repeated values:

- turn coordinate (`campaign_id`, `manifest_path`, `repo_root`)
- `parent`
- `run_shape.candidate_generation`
- `command.node_id.as_deref()`
- `plan_child_budget`
- `run_shape.broad_tui`
- `resolved_campaign.route_source`

Appears in `resolve_child_plan_for_id`.

Likely meaning: child-plan request for this parent turn.

Smell: the 10-arg signature hides which values are policy, which are authority, and which are operator overrides.

### 7. Child runtime cluster

Repeated values:

- turn coordinate (`campaign_id`, `manifest_path`, `repo_root`)
- evidence (`journal_path`)
- parent (`parent_identity`, `parent_baseline`)
- run policy (`stop_after`, stale timeout, schedule mode, budget)
- `children`
- adaptive-only selection inputs (`rejected_surface_attempts`, seed, strategy)

Appears in `run_child_fanout` and `run_adaptive_child_fanout`.

Likely meaning: child execution context.

Smell: the two fanout calls share a large prefix, then diverge in mode-specific values. That suggests the common context is unmodeled.

### 8. Selection cluster

Repeated values:

- `manifest_path`
- `parent_identity`
- child outcomes or empty outcomes
- `rejected_surface_attempts`
- selection seed/strategy

Appears in both `ParentSelection::new` calls and adaptive selection.

Likely meaning: successor-selection context for this parent generation.

Smell: selection state is reconstituted after fanout instead of being carried as an explicit stage.

### 9. Successor handoff cluster

Repeated values:

- `manifest_path`
- `parent_identity`
- `complete_search_policy`
- `selection_decision`
- `selection_material`
- selected `node`
- `repo_root`
- `parent`
- selected artifact/entry
- handoff mode

Appears in continuation decision, successor journal entry, and spawn/handoff.

Likely meaning: successor handoff attempt.

Smell: decision, journaling, and process spawning are adjacent but still wired through several separate loose values.

### 10. Final report cluster

Repeated values:

- `report_child`
- `fallback_node`
- `repo_root`
- `journal_path`
- `run_shape.stop_after`
- accumulated `outcome`
- child/successor runtime fields

Appears near the end.

Likely meaning: operator-facing turn result.

Smell: report construction depends on state accumulated across the entire function. It is easy to break by changing earlier branches.

## Logging / observability clusters

Observed blocks:

| Lines | Block | Lines | Notes |
|---|---|---:|---|
| 7222-7228 | `tracing::info_span!` | 7 | turn-level span with campaign only |
| 7279-7290 | `info!` | 12 | parent identity resolved |
| 7335-7347 | `info!` | 13 | parent ready after History startup |
| 7373-7381 | `debug!` | 9 | parent turn start, repo/journal paths |
| 7591-7603 | `observe::Step::start(observe::span!(...))` | 13 | successor-selection event |

Repeated fields:

- `campaign`
- `parent_id`
- `node_id`
- `generation`
- `branch_id`
- role/authority/transition labels

Smell: the function uses multiple observability channels at once:

- tracing span/logs;
- observe step;
- journal entries;
- resource samples;
- CLI table/JSON report;
- monitor target file.

Some duplication is probably intentional, but the taxonomy is not visible in this function. A reader cannot easily tell which events are durable authority/evidence, which are operational breadcrumbs, and which are operator presentation.

## Initial questions before refactoring

1. Which repeated clusters are real domain concepts versus temporary convenience bags?
2. Which observability events are durable evidence, and which are runtime diagnostics?
3. Should parent identity resolution and parent readiness be one phase boundary or two in this CLI layer?
4. Should child fanout receive a modeled execution context instead of 13 positional args?
5. Should final report assembly be its own projection from a completed turn state?
6. Which logs duplicate journal records enough that they should be deleted, downgraded, or moved behind a narrower helper?
