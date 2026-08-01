# 2026-05-09 Prototype 1 Run Profile TOML Plan

## Causal Design Block

Surface Request: Replace the current pile of Prototype 1 launch flags with a
TOML profile stored under `~/.ploke-eval/`.

Causal Chain: operator profile -> campaign setup admission -> scheduler/runtime
policy -> parent turn -> child generation -> successor selection -> successor
parent handoff.

Concern: Parent-local CLI flags currently affect generation 0 only. Generation
1 can silently fall back to defaults because successor argv is reconstructed
without the initial flags.

Evidence Surface: profile file, admitted campaign copy, scheduler policy,
successor invocation, History selection entry.

Existing Algebra: `Prototype1SearchPolicy`, campaign manifest, scheduler state,
parent identity, successor invocation, History selection records.

Missing Structure: a durable Prototype 1 run-shape carrier that includes search
topology, candidate generation, edit surface, successor-selection strategy,
selection evidence inputs, and seed.

Transformation: parse operator TOML once, normalize it into a typed `RunProfile`,
copy/hash/admit it into the campaign, and project parent/successor argv from the
admitted profile.

Projection: CLI accepts `--profile <name-or-path>` for setup; state runs from
campaign-admitted profile. Rendered commands are projections, not authority.

Preservation Check: every successor parent observes the same run shape as the
parent that selected it unless a policy-change transition is explicitly
recorded.

## Refused Reduction

Do not fix this by appending more flags to `successor_parent_argv`.

That would make generation 1 work while preserving the bad structure: the CLI
would still be the authority, and run shape would still be reconstructed from
argv instead of admitted campaign policy.

## Proposed Storage

Operator-authored profiles live under:

```text
~/.ploke-eval/profiles/prototype1/<profile>.toml
```

Campaign setup copies the resolved profile into:

```text
~/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.toml
```

The campaign-owned copy is the runtime authority. The source profile path is
provenance only. A later edit to the source profile must not change an active
campaign unless an explicit profile update/admission path is added.

## Draft TOML Shape

Use section structure instead of encoding subsystem and phase into long flag
names.

```toml
schema_version = "prototype1-run-profile.v1"
name = "overnight-edit-surface"

[storage]
worktree_root = "~/.ploke-eval/worktrees"

[target]
dataset_key = "ripgrep"
instance = "BurntSushi__ripgrep-2209"

[search]
max_generations = 15
max_total_nodes = 96
children = { min = 6, max = 6 }
schedule = "full-batch"
stop_on_first_keep = false
require_keep_for_continuation = false
explore_from_rejected = true

[generation]
source = "edit-surface"
surface = "ploke-tui-tools"

[selection]
strategy = "history-score-child-prop"
evidence = "operational-and-protocol"
seed = 0

[execution]
stop_after = "complete"
trace_jsonl = "auto"
debug_tools = true
```

Names in the TOML should describe the domain role:

- `generation.source`, not `candidate_generator`;
- `generation.surface`, not `edit_surface`;
- `selection.evidence`, not `successor_selection_metrics`;
- `search.children`, not separate min/max child flags;
- `storage.worktree_root`, not an undocumented operator convention.

The Rust model can still map to existing enums internally, but the public
profile should not expose those implementation names.

## Typed Carrier

Introduce a local domain object, not a CLI view:

```rust
Prototype1RunProfile {
    storage: Storage,
    target: Target,
    search: Prototype1SearchPolicy,
    generation: Generation,
    selection: Selection,
    execution: Execution,
}
```

Suggested nested carriers:

- `Generation { source: GenerationSource, surface: Option<EditSurface> }`
- `Selection { strategy: Strategy, evidence: Evidence, seed: u64 }`
- `Execution { stop_after: StopAfter, trace_jsonl: TraceJsonl, debug_tools: bool }`
- `Storage { worktree_root: PathBuf }`

Keep CLI enum names at the adapter edge only. The runtime should consume the
profile carriers.

## Implementation Plan

1. Add `prototype1_state::profile` with serde TOML parsing, defaults, validation,
   and profile-path resolution:
   - `<name>` resolves to `~/.ploke-eval/profiles/prototype1/<name>.toml`;
   - explicit paths are accepted;
   - validation rejects inconsistent generation source/surface combinations.

2. Add `--profile <NAME_OR_PATH>` to `prototype1-setup`.
   Setup should merge profile values before constructing the batch, campaign,
   scheduler, parent identity, and setup report.

3. During setup, copy the resolved TOML into the campaign prototype root and
   record a digest:
   - preferred path: `<campaign>/prototype1/run-profile.toml`;
   - digest field: either in scheduler state or a small
     `run-profile.commitment.json`;
   - source path preserved as provenance, not authority.

4. Extend scheduler or campaign prototype state with a `RunProfileCommitment`.
   The search policy can remain in scheduler for compatibility, but generation,
   selection, and execution policy must become campaign-admitted facts too.

5. Change `Prototype1StateCommand::run_turn` to resolve run shape from the
   admitted profile when a campaign profile exists.
   CLI flags may remain as debug overrides only if the override is explicit and
   recorded. Default behavior should be campaign profile first, command defaults
   second.

6. Change successor invocation construction so `SuccessorInvocation` stores a
   profile commitment or profile path/hash. `successor_parent_argv` should not
   materialize generation/selection flags. It should launch the next parent with
   enough identity to load the admitted campaign profile.

7. Add tests before relying on the overnight run:
   - profile TOML parses into the intended typed carrier;
   - setup writes campaign-owned `run-profile.toml` and digest;
   - state resolves generation/selection policy from campaign profile;
   - successor parent argv or invocation carries the profile commitment;
   - a parent launched with `generation.source = edit-surface` and
     `selection.evidence = operational-and-protocol` produces a successor parent
     that uses the same values.

8. After tests pass, regenerate the overnight campaign under
   `~/.ploke-eval/worktrees/` using only `--profile overnight-edit-surface`
   plus campaign identity.

## CLI End State

The normal operator flow should become:

```bash
ploke-eval loop prototype1-setup \
  --profile overnight-edit-surface \
  --campaign p1-occurrence-selection-overnight-20260509-4
```

Then:

```bash
cd ~/.ploke-eval/worktrees/p1-occurrence-selection-overnight-20260509-4
PLOKE_PROTOTYPE1_TRACE_JSONL=auto \
./target/debug/ploke-eval loop prototype1-state --campaign p1-occurrence-selection-overnight-20260509-4 --repo-root .
```

Longer term, even `PLOKE_PROTOTYPE1_TRACE_JSONL=auto` should probably come from
the admitted profile. Until tracing setup is cleanly integrated, the env var can
remain the one explicit operational escape hatch.

## Open Decisions

- Whether the profile commitment belongs inside `Prototype1SchedulerState` or a
  sibling `run-profile.commitment.json`.
- Whether command-line overrides after setup are rejected outright or admitted
  as explicit profile-delta transitions.
- Whether model/provider settings should move into the same profile immediately
  or remain in campaign manifest for this pass.
- Whether `prototype1` and `prototype1-setup` should both accept `--profile`, or
  whether only setup owns run profile admission.
