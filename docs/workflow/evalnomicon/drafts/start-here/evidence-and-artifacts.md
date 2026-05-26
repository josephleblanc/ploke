# Evidence And Artifacts

Prototype 1 writes several evidence families. They are not equally
authoritative. This page names the current read discipline for the loop stages.

## Authority Order

1. Sealed History blocks and the `FsBlockStore` append path.
   These are the intended durable authority surface for admitted lineage facts.
2. Parent identity plus admitted run-profile commitment.
   These bind the active checkout and current command to the campaign and
   admitted policy.
3. Typed transition journal entries.
   These are append-only transition evidence and replay input, but not sealed
   History authority by themselves.
4. Typed message boxes and invocation contracts.
   Child plans, child invocations, successor invocations, ready records, and
   completion records are attempt-scoped contracts or transport evidence.
5. Closure, run, protocol, oracle, and branch-evaluation artifacts.
   These are benchmark and adjudication evidence. They must be checked for
   consistency before selection or review claims.
6. Scheduler, branch, node, monitor, and dashboard projections.
   These are operational views. They are useful for navigation and diagnosis,
   but they are not Crown authority.

## Campaign-Local Files

Most loop state lives under:

```text
~/.ploke-eval/campaigns/<campaign>/
```

Key files outside `prototype1/`:

| Path | Role |
| --- | --- |
| `campaign.json` | Campaign manifest and path anchor. |
| `closure-state.json` | Baseline eval/protocol closure state. |

Key files inside `prototype1/`:

| Path | Role |
| --- | --- |
| `run-profile.toml` | Admitted loop configuration. |
| `run-profile.commitment.json` | Digest/path commitment for the admitted profile. |
| `scheduler.json` | Mutable scheduler projection and policy/path context. |
| `branches.json` | Branch and candidate evidence stream. |
| `transition-journal.jsonl` | Append-only typed transition journal. |
| `history/blocks/segment-000000.jsonl` | Sealed History block stream. |
| `history/index/*` | Rebuildable projections over sealed blocks. |
| `messages/child-plan/<parent-node-id>.json` | Parent-owned child-plan message. |
| `messages/edit-harness-request/*.json` | Typed broad-harness request slots. |
| `messages/edit-harness-request/*.md` | Rendered model-facing prompt text for a request. |
| `messages/edit-harness-result/*.json` | Submitted and admitted broad-harness result payloads. |
| `workspaces/edit-harness/<parent-node-id>/` | Candidate workspace for broad-harness generation. |
| `evaluations/<branch-id>.json` | Treatment-vs-baseline branch evaluation report. |
| `nodes/<node-id>/node.json` | Scheduler-owned node record. |
| `nodes/<node-id>/runner-request.json` | Runner request projection for a node. |
| `nodes/<node-id>/runner-result.json` | Latest node-level runner-result projection; not full treatment evidence. |
| `nodes/<node-id>/invocations/<runtime-id>.json` | Child or successor bootstrap contract. |
| `nodes/<node-id>/channels/<runtime-id>/*.jsonl` | Parent-child or successor transport messages. |
| `nodes/<node-id>/streams/<runtime-id>/*.log` | Detached runtime stdout/stderr streams. |
| `nodes/<node-id>/successor-ready/<runtime-id>.json` | Successor ready acknowledgement. |
| `nodes/<node-id>/successor-completion/<runtime-id>.json` | Successor bounded-turn completion record. |
| `nodes/<node-id>/worktree/` | Temporary child Artifact workspace. |
| `nodes/<node-id>/bin/`, `nodes/<node-id>/target/` | Child build products and cleanup targets. |

Artifact-carried active parent identity lives in the active checkout:

```text
<repo-root>/.ploke/prototype1/parent_identity.json
```

Run artifacts usually live under:

```text
~/.ploke-eval/instances/prototype1/<campaign>/...
```

## Stage Evidence

| Stage | Evidence to check first | Common false positive |
| --- | --- | --- |
| Setup | `campaign.json`, admitted profile, parent identity commit | assuming a branch name proves setup authority |
| Baseline eval | closure state plus run records | treating closure completion as benchmark usefulness |
| Baseline protocol | protocol artifacts plus closure state | treating protocol completion as model progress |
| Child plan | child-plan message and broad-harness receipts | trusting prompt markdown over typed request JSON |
| Materialize | node status, worktree, transition journal | treating a temp worktree as successor home |
| Build | child binary path, node status, build result | treating failed cargo output as absent evidence |
| Spawn | invocation, channel, runtime id, streams | treating process spawn as child acknowledgement |
| Observe | terminal channel result, treatment evidence, branch evaluation | treating a non-empty patch as clean success despite aborted turn |
| Select | selection decision and seal material | treating child self-report as promotion |
| Handoff | active checkout update, sealed History block, successor ready record | treating successor invocation as authority without sealed-head validation |
| Blocked | doctor blockers, transition journal, failing artifact | continuing and producing more ambiguous evidence |

## Evidence Receipt

For every diagnostic loop step, preserve a compact receipt:

- campaign id
- active parent worktree and branch
- parent node id and generation
- phase before and after
- exact command
- admitted model route and provider
- admitted protocol policy
- closure delta
- artifacts created
- patch/submission state
- oracle/MBE state
- worktree state
- failure class, if any
- blocker disposition
- next safe command or repair task

This receipt can live in chat for small checks. Durable discoveries should move
to run reviews, bug reports, or operator docs.

## Projection Rules

Do not let useful projections become authority by accident.

- `scheduler.json` can help find nodes and policy context, but it is mutable.
- `branches.json` can preserve branch/candidate evidence, but it is not Crown
  authority.
- `transition-journal.jsonl` is append-only transition evidence, but it must be
  admitted into History or imported by policy before it becomes authority.
- History indexes and heads are rebuildable projections over sealed blocks.
- CLI tables and dashboards are operator views.

## Blocker Classes

Use narrow blocker labels. The next action depends on the class.

- `provider_env`: missing auth, quota, project, network, or provider outage.
- `provider_request_shape`: rejected route, request body, reasoning control, or
  endpoint mismatch.
- `protocol_config`: admitted protocol policy missing, stale, too small, or not
  threaded to request construction.
- `loop_progress`: controller retries or stays in one phase without required
  artifacts.
- `model_behavior`: model ran but failed to localize, edit, validate, or use
  evidence.
- `tool_contract`: tools completed mechanically but returned stale, truncated,
  misleading, or low-information payloads.
- `artifact_accounting`: trace, closure, patch, submission, terminal outcome,
  or protocol artifacts disagree.
- `invalid_transition_evidence`: required eval, protocol, oracle, History, or
  selection evidence is malformed, contradictory, or unverifiable.
- `validation_surface`: validation was weak, missing, unrelated, or not
  model-visible.
- `repo_state`: wrong branch, dirty worktree, stale checkout, missing path, or
  path-sensitive fixture issue.
