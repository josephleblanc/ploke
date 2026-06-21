# Prototype 1 Loop Operator Map

This is the practical map for operating and debugging the Prototype 1 loop.
It is descriptive, not an authority source. History blocks, typed transitions,
and admitted profile files are the authority-bearing surfaces; CLI output and
monitor-style projections are read-only views over those records.

The source-side appendix at
`src/cli/prototype1_state/PROTOTYPE1_LOOP_OPERATOR.md` is retained only as a
redirect to this page.

## The Two Roots

Keep the campaign root and active parent checkout separate:

- Campaign state lives under
  `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
- The campaign manifest is
  `~/.ploke-eval/campaigns/<campaign-id>/campaign.json`.
- The active parent checkout is the `--repo-root` used by the live parent
  command. It carries `.ploke/prototype1/parent_identity.json`.
- Child worktrees usually live below
  `prototype1/nodes/<node-id>/worktree/`. They are temporary evaluation
  surfaces, not the long-lived parent home after handoff.

For live parent execution, use the `ploke-eval` binary built inside the active
parent checkout. Do not run a binary from one checkout against another
checkout's `--repo-root`.

## Direct Google Manifest Warning

Current setup uses an asymmetric route/provider encoding for direct Google.
The run profile may contain:

```toml
[model]
id = "google/gemini-3.5-flash"
route_source = "direct-google"
provider = "google"
```

That `provider = "google"` value is a setup-time sentinel only. A correctly
admitted direct Google campaign manifest should serialize the route as
`"route_source": "direct_google"` and should omit `provider_slug` or serialize
it as null. Do not add `"provider_slug": "google"` to `campaign.json` to make
the fields look symmetric; OpenRouter provider slugs belong only to
`route_source = "openrouter"` campaigns.

## Command Surfaces

The main operator commands are in `src/cli.rs` and dispatch into
`src/cli/prototype1_state/`.

The table is the operator index; source excerpts for trust order and walk-step
admission are below it.

| Command | Current role |
| --- | --- |
| `./target/debug/ploke-eval loop prototype1-setup --profile "${P1_PROFILE:?set P1_PROFILE to a profile name or TOML path}"` | Creates or adopts the campaign, admits `run-profile.toml`, registers the generation-0 parent node, creates the parent branch, writes `.ploke/prototype1/parent_identity.json`, and commits that identity into the active checkout. |
| `./target/debug/ploke-eval loop prototype1-doctor --repo-root "${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"` | Read-only diagnosis of the active parent checkout. It loads parent identity, admitted run profile, prompt preflight, child-plan state, node status, and successor markers, then prints allowed next actions. Add `--live-protocol-preflight` only when you explicitly want a tiny live JSON request against the admitted protocol model/provider/reasoning tuple. |
| `./target/debug/ploke-eval loop prototype1-prompt --repo-root "${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"` | Prints the current broad-harness prompt for the active parent when the admitted run profile uses the broad-harness request generator. |
| `./target/debug/ploke-eval loop prototype1-step --repo-root "${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"` | Advances exactly one diagnosed parent phase. Child phases run at cap 1. |
| `./target/debug/ploke-eval loop prototype1-continue --repo-root "${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"` | Repeatedly advances diagnosed phases until the current turn is complete, blocked, or hands off. It has a 256-advance guard. |
| `./target/debug/ploke-eval loop prototype1-state --help` | Typed parent runtime path. It can initialize parent identity, run one parent turn through `driver::advance::run_to_terminal`, or acknowledge a successor handoff with `--handoff-invocation`. |
| `./target/debug/ploke-eval loop walk --help` | Local operator/debugger surface over the same typestate edges. Use `walk summary -v` for completed-run discovery, `walk replay/back/forward` for read-only historical cursor movement, and `walk step` for live edges. Long live edges require `--watch`; selected-successor handoff requires `--watch --allow git-changes`. |
| `./target/debug/ploke-eval loop prototype1-runner --invocation "${P1_INVOCATION:?set P1_INVOCATION to an invocation JSON path}" --execute` | Hidden child/successor runner seam for one persisted invocation. It still exists; do not treat it as removed. |
| `./target/debug/ploke-eval loop prototype1-harness attempt --help` | Hidden broad headless-TUI probe surface. It replays published harness requests outside the full self-propagating parent loop. |
| `./target/debug/ploke-eval loop prototype1 --help` | Older high-level controller over eval, protocol, intervention, treatment, and compare stages. It still exists, but the typed control path is `doctor` / `step` / `continue` / `prototype1-state`. |

History and metrics inspection live under `ploke-eval history ...`; those
commands are read-only projections, not active loop authority.

Source excerpts for the trust-order help, walk-step admission guard, and
projection-read capability:

```rust,ignore
{{#include ../../src/cli/args/root.rs:ploke_eval_cli_trust_order_help}}
{{#include ../../src/cli/args/loop_args.rs:prototype1_walk_step_live_edge_admission}}
{{#include ../../src/projection.rs:ploke_eval_operator_projection_read}}
```

## Parent Phase Map

`src/cli/prototype1_state/run/core.rs` owns the current diagnosis and control
loop. `doctor` reports one of these phases:

| Phase | What it means | Primary files touched |
| --- | --- | --- |
| `baseline_eval` | Generation-0 parent still needs baseline eval closure. | `closure-state.json`, instance run records under `~/.ploke-eval/instances/prototype1/<campaign-id>/...` |
| `baseline_protocol` | Generation-0 parent still needs baseline protocol closure. | `closure-state.json`, protocol artifacts under instance run records |
| `child_plan` | Parent is ready, but no child-plan message exists for this parent node. | `prototype1/messages/child-plan/<parent-node-id>.json`, `scheduler.json`, `branches.json`, `nodes/*` |
| `materialize` | At least one planned child is still `Planned`; the next action stages its workspace/artifact surface. | `nodes/<node-id>/worktree/`, `node.json`, `transition-journal.jsonl` |
| `build` | At least one child is `WorkspaceStaged`; the next action builds the child runtime. | `nodes/<node-id>/bin/`, `nodes/<node-id>/target/`, `node.json`, `transition-journal.jsonl` |
| `spawn` | At least one child is `BinaryBuilt`; the next action writes an invocation and spawns the runtime. | `nodes/<node-id>/invocations/<runtime-id>.json`, `nodes/<node-id>/channels/<runtime-id>/`, `transition-journal.jsonl` |
| `observe` | At least one child is `Running`; the next action observes terminal channel result and compares treatment evidence. | `nodes/<node-id>/channels/<runtime-id>/child-to-parent.jsonl`, `nodes/<node-id>/results/<runtime-id>.json`, `evaluations/<branch-id>.json`, `transition-journal.jsonl` |
| `select` | All planned children are terminal and selection can be recorded. | `transition-journal.jsonl`, History preview inputs, child evaluation reports |
| `handoff` | A selected successor can be installed, sealed into History, spawned from the active checkout, and acknowledged. | active checkout, `.ploke/prototype1/parent_identity.json`, `history/`, `successor-ready/`, `successor-completion/`, successor streams |
| `complete` | No next active phase is available for this parent turn. | Read-only state summary |
| `blocked` | Required evidence or consistency checks failed. | The blocker text names the missing or mismatched file. |

`prototype1-step` advances one matching phase. `prototype1-continue` runs the
same transition loop repeatedly and uses the admitted profile's
`[control].parallel_cap` for child phase fanout.

## Loop Arc

The intended single-successor loop described in
`src/cli/prototype1_state/mod.rs` is:

1. Parent runs from the active checkout.
2. Parent publishes or receives a child plan.
3. Parent materializes temporary child worktrees.
4. Parent builds child binaries from those worktrees.
5. Parent spawns child runtimes through persisted invocation files.
6. Children self-evaluate and write result evidence.
7. Parent observes results and compares treatment evidence against baseline.
8. Parent selects a successor under the admitted selection policy.
9. Parent installs the selected Artifact into the stable active checkout.
10. Parent seals the History block and appends it through `FsBlockStore`.
11. Parent spawns the successor from the active checkout and waits for ready.
12. Successor validates the predecessor handoff and becomes the next Parent.
13. Temporary child worktrees and build products may be cleaned up.

Source excerpts for the handoff authority path:

```rust,ignore
{{#include ../../src/cli/prototype1_process.rs:prototype1_handoff_block_fields}}
{{#include ../../src/cli/prototype1_state/history/stored/mod.rs:prototype1_fs_block_store_append}}
{{#include ../../src/cli/prototype1_process.rs:prototype1_successor_startup_validation}}
```

The stable parent checkout should advance to the selected Artifact. A temporary
child worktree should not become the next long-lived parent checkout.

## Persisted Files

Campaign-local files under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`:

| Path | Meaning |
| --- | --- |
| `run-profile.toml` | Admitted operator profile. It owns target, search, generation, selection, execution, and control policy. |
| `run-profile.commitment.json` | Digest and path commitment for the admitted profile. |
| `scheduler.json` | Mutable scheduler projection: policy, frontier, node ids, and continuation decision. Useful for path/index context, not History authority. |
| `branches.json` | Branch registry record stream for synthesized, applied, selected, and compared treatment branches. |
| `transition-journal.jsonl` | Append-only typed transition journal for parent start, child materialize/build/spawn/observe, successor selection, and handoff events. Query narrowly; do not read wholesale during routine checks. |
| `history/blocks/segment-000000.jsonl` | Sealed History block stream. This is the authority-bearing handoff substrate. |
| `history/index/by-hash.jsonl` | Rebuildable index over sealed blocks. |
| `history/index/by-lineage-height.jsonl` | Rebuildable lineage/height index over sealed blocks. |
| `history/index/heads.json` | Projection of accepted sealed blocks. It is not an independent authority field. |
| `messages/child-plan/<parent-node-id>.json` | Typed parent-owned child-plan box. It binds planned children to the parent node and expected child generation. |
| `messages/edit-harness-request/*.json` | Published broad-harness request slots when broad harness generation is used. |
| `messages/edit-harness-request/*.md` | Rendered prompt text for the published broad-harness request. |
| `messages/edit-harness-result/*.json` | Submitted broad-harness child-plan/result payloads. |
| `workspaces/edit-harness/<parent-node-id>/` | Candidate workspace used by broad harness generation. |
| `evaluations/<branch-id>.json` | Treatment-vs-baseline branch evaluation report. |
| `nodes/<node-id>/node.json` | Scheduler-owned node record. |
| `nodes/<node-id>/runner-request.json` | Runner request payload for one node. |
| `nodes/<node-id>/runner-result.json` | Latest runner outcome projection for reconstruction and operator display. |
| `nodes/<node-id>/invocations/<runtime-id>.json` | Attempt-scoped child or successor bootstrap contract. |
| `nodes/<node-id>/results/<runtime-id>.json` | Attempt-scoped runtime result mirror. |
| `nodes/<node-id>/channels/<runtime-id>/parent-to-child.jsonl` | Parent-to-child channel messages for the invocation. |
| `nodes/<node-id>/channels/<runtime-id>/child-to-parent.jsonl` | Child-to-parent channel messages, including ready/terminal messages when used. |
| `nodes/<node-id>/streams/<runtime-id>/stdout.log` | Detached successor or child stdout stream. |
| `nodes/<node-id>/streams/<runtime-id>/stderr.log` | Detached successor or child stderr stream. |
| `nodes/<node-id>/successor-ready/<runtime-id>.json` | Successor acknowledgement file. |
| `nodes/<node-id>/successor-completion/<runtime-id>.json` | Terminal record for the successor's bounded parent turn. |
| `nodes/<node-id>/worktree/` | Backend-managed temporary child workspace. |
| `nodes/<node-id>/bin/` and `nodes/<node-id>/target/` | Child build products, not durable identity. |

Files outside the `prototype1/` subtree:

| Path | Meaning |
| --- | --- |
| `~/.ploke-eval/campaigns/<campaign-id>/campaign.json` | Campaign manifest and path anchor. |
| `~/.ploke-eval/campaigns/<campaign-id>/closure-state.json` | Baseline eval/protocol closure state used before generation-0 child planning. |
| `~/.ploke-eval/campaigns/<campaign-id>/prototype1-loop-trace.json` | Legacy loop-controller trace from `ploke-eval loop prototype1 ...`. |
| `~/.ploke-eval/instances/prototype1/<campaign-id>/...` | Baseline and treatment run artifacts referenced by evaluation reports. |
| `<repo-root>/.ploke/prototype1/parent_identity.json` | Artifact-carried active parent identity. This is committed into the parent checkout. |
| `<repo-root>/target/debug/ploke-eval` | Runtime binary that should be used for live parent control from that checkout. |

## Implementation Map

| File | Operator relevance |
| --- | --- |
| `src/cli.rs` | Clap command definitions and top-level dispatch. |
| `src/cli/prototype1_state/mod.rs` | Conceptual model: Artifact, Runtime, Parent, Crown, History, child boxes, and intended loop ordering. |
| `src/cli/prototype1_state/run/core.rs` | `doctor`, `prompt`, `step`, and `continue`; phase diagnosis and phase-by-phase advancement. |
| `src/cli/prototype1_state/cli_facing.rs` | Setup, broad harness probes, projections, path helpers, selection/handoff helpers, and report rendering. The live parent turn now delegates to the typed driver. |
| `src/cli/prototype1_state/typestate/` | Structural `Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>` aliases and transition combinators for R0-R14b. |
| `src/cli/prototype1_state/driver/advance.rs` | Canonical typed batch driver for one parent turn. |
| `src/cli/prototype1_state/driver/reconstruct.rs` | Read-only durable reconstruction for `walk` from parent identity, journal, child-plan, channel, evaluation, and handoff evidence. |
| `src/cli/prototype1_state/driver/replay.rs` | Read-only historical replay cursor used by `walk replay/back/forward`. |
| `src/cli/prototype1_state/walk/` | Local debug server/client/operator surface over typed driver edges and reconstruction. |
| `src/cli/prototype1_process.rs` | Child/successor process seam, child execution, successor install, History seal/append, spawn, and ready wait. |
| `src/cli/prototype1_state/parent.rs` | Parent role states and the child-plan message box. |
| `src/cli/prototype1_state/identity.rs` | Parent identity file schema and checkout path helpers. |
| `src/cli/prototype1_state/profile.rs` | Admitted run-profile schema, validation, digest, and load paths. |
| `src/cli/prototype1_state/history/` | History block model, projections, sealing, and filesystem `FsBlockStore`. |
| `src/cli/prototype1_state/journal.rs` | Transition journal schema and replay helpers. |
| `src/cli/prototype1_state/invocation.rs` | Child/successor invocation records, ready/completion records, and launch argv. |
| `src/cli/prototype1_state/backend/` | Workspace backend, currently git worktree based. |
| `src/cli/prototype1_state/c1.rs` through `c4.rs` | Typed materialize/build/spawn/observe transition carriers. |
| `src/intervention/scheduler.rs` | Scheduler and node records, node paths, runner request/result paths. |
| `src/intervention/branch_registry.rs` | Branch registry records and branch registry path. |
| `src/cli/prototype1_state/edit_surface/` | Broad harness and bounded edit surface request/result machinery. |

## Authority And Debugging Rules

- Treat `history/blocks/*` plus `FsBlockStore::append` as the sealed handoff
  authority boundary.
- Treat `transition-journal.jsonl` as the append-only transition replay stream,
  but query it narrowly.
- Treat `scheduler.json`, `branches.json`, node JSON, runner request/result
  files, and CLI tables as operational projections. Parent/child lifecycle
  observation should be driven by the per-runtime channel, not by these
  projections.
- Treat invocation, ready, completion, channel, and stream files as
  attempt-scoped transport/debug evidence.
- Do not infer live egui or live runtime behavior from a CLI projection.
- During routine health checks, start with metadata: node counts, status
  counts, mtimes, byte sizes, and disk free. Read JSONL content only for a
  specific anomaly, with record and width caps.

## Quick Operator Sequence

After setup and build, the usual active-parent loop is:

```bash
P1_PARENT_ROOT="${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"
cd "$P1_PARENT_ROOT"
./target/debug/ploke-eval loop prototype1-doctor --repo-root .
./target/debug/ploke-eval loop prototype1-step --repo-root .
./target/debug/ploke-eval loop prototype1-continue --repo-root .
```

Use `doctor` when deciding what surface to inspect next. Use `step` when you
want one diagnosed phase and a fresh status. Use `continue` only when the current
parent checkout and binary provenance are clear.

For typestate/debugger work, prefer the `walk` surface:

```bash
P1_PARENT_ROOT="${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"
./target/debug/ploke-eval loop walk use "$P1_PARENT_ROOT"
./target/debug/ploke-eval loop walk summary -v
./target/debug/ploke-eval loop walk replay --index 0
./target/debug/ploke-eval loop walk step --until r6
```

Use `walk replay`, `walk back`, and `walk forward` for read-only historical inspection. Use `walk step` only when you intend to drive live typestate edges. Use `doctor --live-protocol-preflight` when provider request shape is the suspected blocker and a live call is acceptable.
