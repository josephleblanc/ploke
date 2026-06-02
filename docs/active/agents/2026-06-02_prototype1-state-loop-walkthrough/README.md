# 2026-06-02 Prototype 1 state-loop walkthrough

Status: draft walkthrough / implementation audit
Command: `ploke-eval loop prototype1-state`
Base branch: `feature/ploke-loop`
Audit branch: `docs/prototype1-state-loop-walkthrough`
Scope: `crates/ploke-eval`, with live protocol boundaries into `ploke-protocol`.

## Purpose

This guide explains the current implementation path for `ploke-eval loop prototype1-state` so future work on the Prototype 1 observable self-improving loop can reason about configuration, execution phases, disk writes, persisted data, live API/model routing, and parallelism opportunities.

This document is written for developers who may be new to this part of the project. The glossary below defines the project-specific vocabulary before the walkthrough uses it. The most important early definition is **node**: in this Prototype 1 loop, a node is a persisted candidate state in the self-improvement search tree, represented by `Prototype1NodeRecord`. It is not an AST node, graph node from `ploke-tree`, or generic UI node.

The command is not just a CLI wrapper. It is the typed runtime for one **parent turn**. A parent checkout enters the command with a checkout-carried parent identity or a successor handoff invocation. It plans child nodes, runs them through C1-C4 state transitions, observes terminal child results, compares treatments against the parent baseline, selects a successor, and may launch the successor parent runtime.

## Reading notes

- File/line references are from branch `docs/prototype1-state-loop-walkthrough`, current local merge `09375711` with turn-live replay emission commit `5a7dbe83` included.
- This document intentionally distinguishes setup/admission (`loop prototype1-setup`) from the runtime parent turn (`loop prototype1-state`).
- No secrets are recorded here. Model/provider names and source paths are code/config provenance, not credentials.
- `prototype1-state` is the live typed path in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`; the `prototype1_state/run` module still documents extraction work, but the live parent turn remains in `cli_facing.rs`.
- Companion docs:
  - [`campaign-configs.md`](campaign-configs.md): expanded campaign/run-profile config audit, model/provider/route precedence, and config footguns.
  - [`turn-live-replay.md`](turn-live-replay.md): focused guide to the broad headless-TUI turn-live replay bundle, live observer emissions, replay command, branch tapes, and model routing.
  - [`terminology-conflicts.md`](terminology-conflicts.md): terminology conflicts and proposed conventions.

## 0. Draft glossary and conceptual map

Read this section first if you are new to Prototype 1. These are source-grounded working definitions, not final product terminology. The loop vocabulary is still evolving, so future passes should refine these definitions as the code and architecture stabilize. The words below are used with narrower meanings than their everyday meanings.

### 0.1 Core loop and search terms

| Term | Definition |
|---|---|
| Prototype 1 | The current experimental self-improvement harness in `ploke-eval`. It runs a baseline, generates candidate code changes, evaluates child candidates, records evidence, and may continue from a selected successor. The active parent command is `ploke-eval loop prototype1-state`. |
| Loop | One or more parent turns chained by successor handoffs. A single invocation of `prototype1-state` runs one parent turn; if it selects and launches a successor, the next runtime invokes the same command for the next turn. |
| Parent turn | One execution of `Prototype1StateCommand::run_turn`. It resolves a parent checkout, establishes baseline evidence, plans children, runs/evaluates children, selects a successor, and optionally launches the successor (`cli_facing.rs:6477-7043`). |
| Parent | The current controlling runtime/checkout for one generation of the search. In code, `Parent<S>` carries a `RuntimeId`, `ParentIdentity`, `Prototype1NodeRecord`, and typestate marker (`parent.rs:78-85`). The parent is responsible for planning and selecting children. |
| Child | A leaf evaluation runtime spawned by the parent to evaluate one candidate node. A child uses the `prototype1-runner --execute` path, evaluates one treatment branch, writes a runner result, sends a terminal result to the parent, and exits (`invocation.rs:17-18`, `prototype1_process.rs:2977-3129`). |
| Successor | The selected continuation runtime after a parent turn. Unlike a child, a successor is not a leaf evaluator; it is the next parent runtime, launched with a `Successor` invocation and then entering `prototype1-state` again (`invocation.rs:17-19`, `invocation.rs:65-91`). |
| Node | A persisted candidate state in the Prototype 1 search tree, represented by `Prototype1NodeRecord` (`scheduler.rs:282-312`). A node identifies a campaign, generation, benchmark instance, source state, target file, candidate branch, workspace, binary path, runner request/result paths, and current status. It is not a Rust syntax tree node or `ploke-tree` graph node. |
| Parent node | The node represented by the active parent checkout. For generation 0, setup creates a root parent identity from the initial node. For later generations, the selected child node becomes the next parent identity (`identity.rs:87-135`). |
| Child node | A node planned by the parent for generation `parent.generation + 1`. The child plan code states this direct-child rule explicitly (`parent.rs:126-137`). |
| Generation | Search-tree depth. Generation 0 is the initial parent. Children planned by a parent are generation + 1 (`Prototype1NodeRecord.generation`, `scheduler.rs:286-289`; direct-child rule in `parent.rs:134-136`). |
| Candidate | A proposed treatment change that can become a branch/node. `candidate_id` identifies the generated proposal inside a treatment branch (`TreatmentBranchNode`, `branch_registry.rs:134-155`). |
| Candidate branch / treatment branch | A concrete proposed code state for a target file/source state. `TreatmentBranchNode` stores `branch_id`, `candidate_id`, label, proposed content/hash, patch/artifact identifiers, and status (`branch_registry.rs:134-155`). |
| Branch id | Stable-ish string identity for one treatment branch. For generated treatment branches it is derived from source state id, target path, and candidate id (`treatment_branch_id`, `branch_registry.rs:254-267`). |
| Candidate id | Identity of the proposal inside a candidate set. It is distinct from `branch_id`; the branch id combines candidate id with source state and target path. |
| Source state id | Identity of the source content/state from which candidate branches were generated. It appears in node records, runner requests, resolved branches, and branch registry entries (`scheduler.rs:289-290`, `branch_registry.rs:221-231`). |
| Target relpath | Repo-relative file path that the candidate/treatment is trying to modify (`Prototype1NodeRecord.target_relpath`, `scheduler.rs:301-304`). |
| Instance id | Benchmark instance id for the eval target. It ties nodes/eval records to a dataset instance (`Prototype1NodeRecord.instance_id`, `scheduler.rs:288-289`). |
| Operation target | Graph-level description of what one runtime operates over. Currently the live `OperationTarget` variant is `Artifact { artifact_id }` (`loop_graph.rs:130-134`). |
| Artifact | A recoverable code/content state. `ArtifactId` is backend-neutral: it may later be a git commit/tree id, digest, artifact-manifest id, etc.; dirty worktrees should not get an artifact id until recoverable (`loop_graph.rs:58-66`). |
| Patch | A generated or composed code change. `PatchId` identifies the patch record, not just a candidate branch name (`loop_graph.rs:92-99`). |
| Runtime | One concrete process attempt participating in the loop. Runtime identity is not the same as node identity: a node can have runtime attempts; each attempt gets a `RuntimeId` (`loop_graph.rs:33-41`). |
| Runtime id | UUID-backed durable identity for one runtime attempt (`RuntimeId`, `loop_graph.rs:33-56`). It appears in invocations, channel paths, transition evidence, and runner result paths. |
| Authority | In this doc, authority means “the persisted/typed evidence that allows a runtime to do a role-specific action.” Examples: parent identity lets a checkout claim a parent coordinate, a child invocation allows leaf evaluation only, and a successor invocation allows the next parent bootstrap. |
| Projection | A persisted view/cache of state, not necessarily the authority source. Example: `node.json` is a node projection; the transition journal and invocation/channel evidence often carry stronger runtime evidence. |

### 0.2 Configuration terms

| Term | Definition |
|---|---|
| Campaign | A durable evaluation scope: benchmark family, dataset sources, roots, eval policy, protocol policy, selected model/provider/route, and required procedures. The persisted type is `CampaignManifest` (`campaign.rs:27-53`). |
| Campaign manifest | The JSON manifest for one campaign. `resolve_campaign_config` loads and validates it into `ResolvedCampaignConfig` (`campaign.rs:478-572`). |
| Resolved campaign config | Runtime form of the campaign manifest after defaults and validation. It carries non-optional fields needed by eval/protocol/closure execution (`campaign.rs:123-138`). |
| Run-profile | Operator-approved TOML config for Prototype 1 behavior. The persisted type is `Prototype1RunProfile`, with `storage`, `target`, `model`, `search`, `generation`, `selection`, `protocol`, `execution`, and `control` sections (`profile.rs:37-59`). |
| Operator profile | Source TOML profile selected by path/name before admission. Bare names resolve under `~/.ploke-eval/profiles/prototype1/<name>.toml` (`profile.rs:911-930`). |
| Admitted run-profile | Campaign-local copy of the operator profile plus a digest commitment. After admission, `prototype1-state` prefers this profile over CLI defaults (`profile.rs:837-864`, `cli_facing.rs:990-999`). |
| Run-profile commitment | JSON provenance/integrity record for an admitted profile: schema, profile path, SHA-256 digest, source path, admission timestamp (`profile.rs:800-807`, `profile.rs:948-954`). |
| Run shape | Compact runtime settings for one parent turn: stop-after, stale timeout, candidate generation, successor selection, oracle mode/evidence, and metrics policy (`Prototype1StateRunShape`, `cli_facing.rs:946-999`). |
| Search policy | Limits and scheduling policy for complete parent turns. It controls maximum generations/nodes and child scheduling/budget, either from admitted profile or scheduler state (`cli_facing.rs:6670-6701`). |
| Child budget | The max/min number of child candidates allowed for a parent turn. Complete runs reserve budget from search policy; non-complete/debug runs usually force one child (`cli_facing.rs:6687-6736`). |
| Stop-after | CLI/profile control that stops the state machine at a named point for debugging or runs through `Complete` for the full loop (`Prototype1StateCommand.stop_after`, `cli.rs:640-641`; profile mapping at `profile.rs:658-665`). |
| Observe-child stale timeout | Duration the parent waits for a child terminal result before treating observation as stale/timed out (`Execution.observe_child_stale_after`, `profile.rs:667-669`; C4 timeout at `c4.rs:410-424`). |
| Parallel cap | Profile-level cap that may narrow, but not widen, search-derived fanout. Validation rejects zero or widening caps (`profile.rs:747-767`). |

### 0.3 Planning and candidate-generation terms

| Term | Definition |
|---|---|
| Candidate generator | Strategy used before child-plan publication. Current command variants include `Legacy`, `BroadHarnessRequest`, and `DeterministicTuiTools`; live complete runs reject legacy generation (`cli.rs:655-657`, `cli_facing.rs:893-925`, `cli_facing.rs:6682-6686`). |
| Broad harness request | Candidate-generation mode where the parent publishes a harness request and expects request-bound child-plan responses. The parent typestate includes `AwaitingHarnessPlan` for this path (`parent.rs:44-52`). |
| Deterministic TUI tools | Candidate-generation mode that uses checked edit-surface evidence instead of legacy free-form generation. The code validates that checked edits match expected edit surface and target (`cli_facing.rs:1002-1078`). |
| Edit surface | The specific file/content surface being edited. Candidate-generation validation ensures the checked edit surface and target path match the child node (`cli_facing.rs:1041-1078`). |
| Checked surface edit | Evidence-bearing generated edit used by deterministic candidate generation. It carries patch/artifact ids and surface evidence that can be projected into `ChildFiles` (`cli_facing.rs:1041-1078`). |
| Rejected surface attempt | Candidate-generation evidence for attempts that did not become runnable child nodes. Complete runs can still project these rejected attempts into selection/reporting (`cli_facing.rs:6745-6757`). |
| Child plan | Cross-runtime parent message containing planned children. `ChildPlanFiles` stores parent node id, child generation, `children: Vec<ChildFiles>`, and rejected surface attempts (`parent.rs:110-123`). |
| Child files | Bundle for one planned child: node record, runner request, resolved treatment branch, and optional surface evidence (`parent.rs:200-206`). C1 consumes this bundle to materialize the child. |
| Resolved treatment branch | Fully resolved treatment candidate for one instance/source/target: source content/hash, selected branch, and `TreatmentBranchNode` (`branch_registry.rs:220-231`). |

### 0.4 Persistence and observability terms

| Term | Definition |
|---|---|
| Parent identity | Checkout-local JSON at `.ploke/prototype1/parent_identity.json`. It names campaign, parent/node id, generation, branch/artifact coordinate, optional instance, and predecessor links (`identity.rs:1-12`, `identity.rs:27-35`). |
| Parent checkout / active parent checkout | Git checkout/worktree that carries the active parent identity and can run `prototype1-state`. Child worktrees do not carry parent control state and are rejected by parent-control commands (`identity.rs:8-12`). |
| Worktree | Git working tree used to materialize a parent or child candidate. C1/C2/C3 operate on child worktrees and binaries; successor handoff advances/uses the active parent checkout. |
| Transition journal | Append-only JSONL event stream for parent/C1-C4/successor/resource events. `PrototypeJournal::append` creates parent dirs, writes one JSON line, and syncs (`journal.rs:672-713`). |
| Journal entry | One typed transition/event record appended to the transition journal. Examples include `ParentStarted`, C1-C4 before/after entries, resource samples, and successor records. |
| Resource sample | Journal evidence about process/system resources at a phase such as parent start or parent complete (`cli_facing.rs:6643-6652`, `cli_facing.rs:6946-6955`). |
| MessageBox | File-backed typed message container used for parent/child plan publication. The child-plan message file lives under `prototype1/messages/child-plan/<parent-node-id>.json` (`parent.rs:278-289`). |
| Channel | Role-indexed parent/child runtime transport. It separates protocol authority from transport mechanics; in the live path the transport is file-backed JSONL (`channel.rs:3-8`, `channel.rs:413-454`). |
| ToParent / ToChild | Channel message bodies. `ToParent` includes `Ready`, `Evaluating`, `Result`, `ResultWritten`, `SuccessorReady`, `SuccessorCompletion`, `Failed`, and `Exited`; `ToChild` currently includes `Cancel` (`channel.rs:374-411`). |
| Invocation | Persisted bootstrap record for one runtime attempt. It includes role, campaign, node, runtime id, journal path, channel root, and optionally node/request/resolved branch or active parent root (`invocation.rs:1-11`, `invocation.rs:111-133`). |
| Invocation authority | Classification of an invocation as `Child` or `Successor`; the command rejects using one role where the other is required (`invocation.rs:149-153`, `prototype1_process.rs:2977-2994`, `cli_facing.rs:6543-6556`). |
| Runner request | Durable child executable contract: campaign/node/generation/instance/source, branch id, target/workspace/binary, stop-on-error, and runner args (`Prototype1RunnerRequest`, `scheduler.rs:339-362`). |
| Runner result | Child terminal result projection: campaign/node/branch/status, disposition, optional treatment campaign/evaluation path/detail/exit/stdout/stderr, and timestamp (`Prototype1RunnerResult`, `scheduler.rs:314-337`). |
| Runner disposition | Coarse outcome of a child runtime: `Succeeded`, `CompileFailed`, or `TreatmentFailed` (`scheduler.rs:274-280`). |
| Node status | Lifecycle status of a node projection: `Planned`, `WorkspaceStaged`, `BinaryBuilt`, `Running`, `Succeeded`, or `Failed` (`Prototype1NodeStatus`, `scheduler.rs:263-272`). |
| Node projection | Persisted `Prototype1NodeRecord` file, usually `prototype1/nodes/<node>/node.json`. It is the current view of node metadata/status and is updated by C1/C2/C3 and runner result recording. |
| Child streams | Persisted stdout/stderr files for a spawned child process (`c3.rs:147-168`, `c3.rs:484-503`). |
| Branch registry | Campaign-local record of synthesized/selected/applied/restored/dropped treatment branches and parent comparisons. Snapshot type is `Prototype1BranchRegistry`; append-only branch log records parent comparisons (`branch_registry.rs:18-29`, `branch_registry.rs:46-70`, `branch_registry.rs:193-202`). |
| Branch evaluation report | Parent-created comparison artifact for a child treatment vs baseline. It stores baseline/treatment campaign ids, evaluator/eval-set identity, branch registry/evaluation paths, disposition, reasons, and compared instances (`Prototype1BranchEvaluationReport`, `cli_facing.rs:7902-7920`). |
| Treatment evidence | Child-created evidence that its treatment campaign completed far enough to compare: treatment manifest/closure path, eval policy, benchmark family, dataset sources, and per-instance metrics/oracle status (`Prototype1TreatmentEvidence`, `cli_facing.rs:7875-7900`). |
| Run record | Compressed durable trace of a benchmark/eval run, including agent turns, LLM responses, tool calls, artifacts, and metrics. Protocol closure and replay use these records (`record.rs:37-40`). |
| Turn-live replay bundle | Broad headless-TUI candidate-generation replay/debug bundle written beside the broad-harness submitted result. The directory path is `submitted_result_path.with_extension("turn-live")` and currently contains `agent-turn-trace.json`, `agent-turn-summary.json`, and `llm-full-responses.jsonl` (`cli_facing.rs:2055-2088`). |
| Agent-turn trace/summary | Passive `ploke-records` persisted shapes for ordered agent-turn events. `AgentTurnTraceRecord` and `AgentTurnSummaryRecord` are transparent wrappers over the same `AgentTurnArtifactRecord` wire shape today; they do not grant runtime authority (`agent_turn.rs:1-35`). |
| Full response sidecar | `llm-full-responses.jsonl`, a JSONL sidecar of `RawFullResponseRecord` provider-response envelopes tagged with assistant message id and response index. Replay uses this as the provider-response tape (`llm_response.rs:1-53`). |
| Replay cursor | `TurnCursor` names artifact family/path/event index inside an agent-turn artifact. `TurnEventStepRef` is the borrowed event-level playback view used to resolve replay anchors (`turn.rs:41-78`). |

### 0.5 Evaluation, protocol, and model-routing terms

| Term | Definition |
|---|---|
| Baseline | Evidence for the parent/current branch before applying a child treatment. Generation 0 establishes it by advancing eval/protocol closure for the baseline campaign; later generations promote selected child baseline evidence (`cli_facing.rs:420-487`). |
| Treatment | The child candidate branch evaluated against the baseline. The child creates a treatment campaign, advances eval/protocol closure for it, then returns treatment evidence (`prototype1_process.rs:3131-3333`). |
| Closure state | JSON summary of whether registry, eval, and protocol requirements are complete/missing/partial/etc. `ClosureState` stores config, registry summary, eval summary, protocol summary, and per-instance rows (`closure.rs:54-64`). |
| Eval closure | Part of closure state/evidence that tracks benchmark/eval runs and whether expected run records/submissions are complete. `advance_eval_closure` creates missing eval artifacts before recomputing closure. |
| Protocol closure | Part of closure state/evidence that tracks required protocol procedures over run records: tool-call intent segmentation, tool-call review, and tool-call segment review (`closure.rs:29-37`, `campaign.rs:21-24`). |
| Closure class | Status enum for closure sections: `complete`, `failed`, `missing`, `ineligible`, `incompatible`, or `partial` (`ClosureClass`, `closure.rs:89-98`). |
| Benchmark family | Which benchmark type the campaign targets. Default campaign benchmark family is `MultiSweBenchRust` (`campaign.rs:197-198`). |
| Multi-SWE-Bench / MSB | Benchmark/harness family represented by `MultiSweBenchSource` and submission files named `multi-swe-bench-submission.jsonl`; the MBE module invokes `multi_swe_bench.harness.run_evaluation` (`mbe/mod.rs:21-27`, `mbe/mod.rs:1481-1489`). |
| MBE | Local module name for Multi-SWE-Bench evaluation/harness integration. It writes `mbe-evaluation-config.json`, invokes the external `multi_swe_bench.harness.run_evaluation` module, and reads `final_report.json` / `report.json` (`mbe/mod.rs:21-27`, `mbe/mod.rs:76-121`). |
| Oracle evaluation | Multi-SWE-Bench harness verdict evidence for submitted patches. It appears in treatment instance evidence and compared instance reports (`cli_facing.rs:7896-7899`, `cli_facing.rs:7955-7957`). |
| Operational metrics | Run metrics used by branch evaluation and successor selection. Branch evaluation compares baseline metrics vs treatment metrics and returns keep/reject plus reasons (`branch_evaluation.rs:5-23`, `branch_evaluation.rs:25-80`). |
| Branch disposition | `Keep` or `Reject` result from branch evaluation (`BranchDisposition`, `branch_evaluation.rs:11-16`). It is evidence for successor selection; it is not by itself authority to continue. |
| Successor selection | Procedure that decides whether a child candidate becomes the next parent. `SuccessorDecision` stores candidate node, selected branch, branch disposition, outcome, findings, and rationale (`successor_selection/mod.rs:1-17`, `successor_selection/decision.rs:8-19`). |
| Successor outcome | Selection outcome: `Accepted`, `ExploreFrom`, or `Stop` (`successor_selection/decision.rs:83-89`). |
| History | Durable authority surface for admitted lineage facts: authenticated store over sealed lineage-local blocks. The History docs explicitly distinguish authority from scheduler snapshots, branch registries, CLI reports, metrics dashboards, and projections (`history.rs:52-65`). |
| History traversal | Successor-selection strategy that projects candidate evidence from History and current-generation candidates, then scores/selects a successor (`successor_selection/mod.rs:8-17`, `successor_selection/traversal.rs:64-86`). |
| Lineage | Policy-governed projection over admitted artifact continuity inside History (`history.rs:57-65`). In this loop, successor handoff tries to preserve one lineage of parent authority. |
| Crown | History typestate authority for the current ruler/parent. The History docs use `Crown<Ruling>`/`Crown<Locked>` to describe who may admit handoff facts for a lineage (`history.rs:81-100`, `history.rs:116-129`). |
| Protocol procedure | One structured analysis pass over run records. Default required procedures are `tool-call-intent-segments`, `tool-call-review`, and `tool-call-segment-review` (`campaign.rs:21-24`, `closure.rs:29-37`). |
| Protocol artifact | JSON evidence envelope written for a protocol procedure. It stores schema/procedure/subject/run id/model/provider plus typed input/output/artifact body (`protocol_artifacts.rs:21-35`, `protocol_artifacts.rs:292-352`). |
| Protocol aggregate | Derived view loaded from protocol artifacts to determine coverage/completeness for required protocol procedures (`closure.rs:10-12`). |
| JSON adjudicator | LLM-backed structured JSON reviewer used by protocol procedures. It is created with a `reqwest::Client` and `JsonLlmConfig` for intent segmentation and review calls (`cli.rs:7639-7641`, `cli.rs:7845-7848`, `cli.rs:7879-7884`). |
| Model id | Identifier of the LLM model used for eval/chat or protocol JSON calls. Campaign/profile config supplies it; runtime config stores selected model/provenance in run/protocol artifacts. |
| Provider slug | Provider route selected for a model, such as OpenRouter provider slug or `google` for direct Google route. The run profile validates provider strings and direct-Google compatibility (`profile.rs:149-179`). |
| Route source | How a model call is routed. In this doc/code path, relevant sources are OpenRouter and direct Google. `ModelDefaults.route_source` validates aliases such as `openrouter`, `open-router`, `direct-google`, and `google` (`profile.rs:130-142`, `profile.rs:183-218`). |
| Live API call | Any network call to a model/provider or provider metadata endpoint. Two major surfaces exist here: eval/headless TUI chat calls and protocol JSON adjudication calls. `resolve_route_for_model` also calls OpenRouter endpoint metadata for non-direct-Google routes (`runner.rs:2501-2536`). |
| Headless TUI / eval chat | Non-interactive benchmark execution path that configures `RuntimeConfig` for chat/tool use without a human TUI session. `configure_headless_benchmark_chat` applies benchmark chat policy and model runtime (`runner.rs:126-130`). |

## 1. Command entrypoint and dispatch

### 1.1 Binary entry

`crates/ploke-eval/src/main.rs` parses the Clap CLI, initializes tracing, then calls `Cli::run()` (`main.rs:5-20`). The `Cli` type is re-exported from `crates/ploke-eval/src/lib.rs`.

### 1.2 Top-level loop dispatch

The `loop` command variants include `Prototype1State(Prototype1StateCommand)` (`crates/ploke-eval/src/cli.rs:539-568`). The dispatch arm sends `LoopSubcommand::Prototype1State(cmd)` to `cmd.run().await` (`cli.rs:1445-1457`).

`Prototype1StateCommand` is the concrete CLI shape for the parent turn (`cli.rs:609-661`). Its fields are:

- `--campaign`: explicit campaign id. The comment says default is parent identity, then active selection (`cli.rs:612-614`). The live resolver is stricter than normal active-selection flows for this path.
- `--node-id`: candidate node id. During `--init-parent-identity`, this is the generation-0 parent node (`cli.rs:616-618`).
- `--repo-root`: active parent checkout root; defaults to cwd (`cli.rs:620-622`).
- `--init-parent-identity`, `--identity-branch`, `--identity-instance`: bootstrap path for generation-0 parent checkout identity (`cli.rs:624-634`).
- `--handoff-invocation`: successor handoff token from the previous parent runtime (`cli.rs:636-638`).
- `--stop-after`: state-machine stop point, default `Complete` (`cli.rs:640-641`).
- `--successor-selection`, `--successor-selection-seed`, `--successor-selection-metrics`: successor-selection controls (`cli.rs:643-653`).
- `--candidate-generator`: defaults to `BroadHarnessRequest` (`cli.rs:655-657`).
- `--format`: table/json output (`cli.rs:659-660`).

The sibling `Prototype1RunnerCommand` is the persisted child runtime entrypoint (`cli.rs:699-720`). Parent C3 writes an invocation, then launches the child binary with `loop prototype1-runner --invocation <path> --execute --format json`.

## 2. Two config planes: campaign and run-profile

There are two durable config planes that matter for this loop:

1. The campaign config says what campaign, dataset, storage roots, model route, eval policy, and protocol policy are being evaluated.
2. The admitted run-profile says what operator-approved Prototype 1 behavior is allowed for this campaign: target selection, candidate generation, search limits, selection policy, protocol knobs, execution stop/staleness, and control caps.

### 2.1 Campaign config

`CampaignManifest` is the persisted campaign manifest (`crates/ploke-eval/src/campaign.rs:27-53`). It carries:

- `schema_version`
- `campaign_id`
- `benchmark_family`
- `dataset_sources`
- `model_id`
- `provider_slug`
- `route_source`
- `required_procedures`
- `instances_root`
- `batches_root`
- `eval`
- `protocol`
- `framework`

`ResolvedCampaignConfig` is the materialized runtime config (`campaign.rs:123-138`). It has non-optional model/route/storage fields after defaulting and validation.

The setup path constructs a Prototype 1 campaign in `prepare_prototype1_loop_campaign` (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:7553-7714`). That function:

- resolves profile/CLI/default model choices;
- resolves eval route/provider;
- enforces shared baseline eval/protocol model routing when baseline protocol is enabled;
- chooses campaign id, defaulting to `prototype1-<batch-id-sanitized>`;
- writes a selected-instance slice dataset (`cli_facing.rs:7717-7771`);
- writes the campaign manifest;
- resolves it into `ResolvedCampaignConfig`.

`resolve_campaign_config` loads and validates a campaign manifest (`campaign.rs:478-572`). `ensure_prototype1_baseline_closure_state` either loads the existing closure state or recomputes it (`cli_facing.rs:409-418`). Recompute writes the closure-state JSON (`crates/ploke-eval/src/closure.rs:254-298`).

### 2.2 Run-profile config

The run-profile schema is `prototype1-run-profile.v1` (`crates/ploke-eval/src/cli/prototype1_state/profile.rs:29`). `Prototype1RunProfile` persists these sections (`profile.rs:37-59`):

- `storage`
- `target`
- `model`
- `search`
- `generation`
- `selection`
- `protocol`
- `execution`
- `control`

Validation is centralized in `Prototype1RunProfile::validate` (`profile.rs:61-85`). It validates target/model/search/generation/protocol/execution/control/selection and rejects legacy generation with multiple target instances.

Important profile methods:

- `search_policy()` maps profile search config into `Prototype1SearchPolicy` (`profile.rs:95-104`).
- `protocol_policy()` maps profile protocol config into `ProtocolCampaignPolicy` (`profile.rs:107-113`).
- `default_parallel_cap()` and `patch_generation_parallel_cap()` derive profile-approved fanout caps (`profile.rs:87-93`).

#### 2.2.1 Profile model defaults

`ModelDefaults` persists:

- `id`: model id string
- `route_source`: `openrouter` or `direct-google`
- `provider`: provider slug

Refs: `profile.rs:130-142`.

Validation parses model ids as `ModelId`, validates provider strings as `ProviderKey`, and rejects `direct-google` plus any non-`google` provider (`profile.rs:149-179`). The route-source serde accepts aliases such as `open-router`, `open_router`, `direct_google`, and `google` (`profile.rs:183-218`).

#### 2.2.2 Profile protocol controls

`Protocol` persists:

- `max_tokens`
- `tool_review_parallelism`
- `reasoning`

Refs: `profile.rs:577-585`. Validation rejects zero tokens or zero review parallelism and validates reasoning (`profile.rs:587-600`).

#### 2.2.3 Profile execution controls

`Execution` persists:

- `stop_after`
- `observe_child_stale_after_secs`
- `trace_jsonl`
- `debug_tools`
- `mbe`

Refs: `profile.rs:635-646`. `Execution::state_stop_after` maps profile stop-after to `Prototype1StateStopAfter` (`profile.rs:658-665`), and `observe_child_stale_after` returns the child-observation timeout (`profile.rs:667-669`).

#### 2.2.4 Profile control caps

`Control` persists:

- `mode`
- `parallel_cap`

Refs: `profile.rs:747-753`. Validation rejects zero and rejects any cap that widens the search-derived fanout; `parallel_cap` may only narrow admitted parallelism (`profile.rs:755-767`).

### 2.3 Profile loading and admission

`load_operator_profile` resolves a name/path, reads TOML, parses it, and validates it (`profile.rs:822-835`, `profile.rs:911-919`). Bare names resolve under `~/.ploke-eval/profiles/prototype1/<name>.toml` (`profile.rs:922-930`).

`admit_run_profile` writes two campaign-local artifacts (`profile.rs:837-864`):

1. `prototype1/run-profile.toml`: pretty TOML copy of the operator profile.
2. `prototype1/run-profile.commitment.json`: commitment/provenance JSON.

`RunProfileCommitment` includes (`profile.rs:800-807`):

- `schema_version`
- `profile_path`
- `sha256`
- `source_path`
- `admitted_at`

`write_commitment` serializes it as pretty JSON (`profile.rs:948-954`). `load_admitted_run_profile` reads `run-profile.toml`, loads or creates a commitment, and rejects digest mismatch (`profile.rs:868-903`).

### 2.4 How `prototype1-state` chooses profile vs CLI config

`Prototype1StateRunShape` is the compact runtime shape used by `prototype1-state` (`cli_facing.rs:946-957`). It includes stop-after, stale timeout, candidate-generation config, successor-selection controls, oracle mode/evidence, and metrics policy.

- `from_command` uses CLI flags and defaults (`cli_facing.rs:959-972`).
- `from_profile` uses the admitted run-profile (`cli_facing.rs:974-988`).
- `resolve` first tries `profile::load_admitted_run_profile(manifest_path)` and only falls back to command flags if there is no admitted profile (`cli_facing.rs:990-999`).

This is important for stability: after setup admits a profile, `prototype1-state` should normally be profile-driven, not ad hoc CLI-driven.

## 3. Setup vs runtime turn

### 3.1 Setup/admission path

`loop prototype1-setup` dispatches to `Prototype1LoopCommand::run_setup`, which calls `prepare_prototype1_parent_setup` (`cli_facing.rs:154-170`). Setup:

1. loads optional operator profile;
2. prepares or loads a benchmark batch;
3. resolves the primary instance;
4. prepares the campaign;
5. admits the run profile;
6. ensures baseline closure state;
7. registers the root parent node;
8. checks out the fresh parent branch;
9. writes parent identity;
10. commits parent identity into the active checkout;
11. validates the parent checkout;
12. prints the next `loop prototype1-state --repo-root .` command.

Refs: `cli_facing.rs:195-301`.

### 3.2 Runtime turn path

`loop prototype1-state` is `Prototype1StateCommand::run` -> `run_turn` (`cli_facing.rs:6477-6488`). `run()` only wraps error handling for successor handoff failure recording. The runtime work is in `run_turn`.

## 4. Full parent-turn execution path

The parent turn is easiest to read as a sequence of authority transitions and evidence writes.

### 4.1 Resolve repo, campaign, config, closure, journal

`run_turn` begins by resolving:

1. `repo_root`: `--repo-root` or current directory (`cli_facing.rs:6488-6493`).
2. `campaign_id`: `resolve_prototype1_state_campaign` (`cli_facing.rs:6494`).
3. active monitor target (`cli_facing.rs:6495`).
4. `manifest_path`: `campaign_manifest_path` (`cli_facing.rs:6496`).
5. `run_shape`: admitted profile or command fallback (`cli_facing.rs:6497`).
6. `resolved_campaign`: `resolve_campaign_config` (`cli_facing.rs:6498-6499`).
7. baseline closure state (`cli_facing.rs:6500`).
8. transition journal path and `PrototypeJournal` (`cli_facing.rs:6501-6502`).

Disk write in this phase:

- `ensure_prototype1_baseline_closure_state` may recompute/write closure state JSON (`closure.rs:254-298`).
- Opening `PrototypeJournal` does not write; each later `journal.append` writes JSONL.

### 4.2 Optional parent identity bootstrap

If `--init-parent-identity` is set, `run_turn` calls `initialize_prototype1_parent_identity` and returns after printing identity (`cli_facing.rs:6512-6540`).

Disk writes in this path:

- parent identity JSON at `.ploke/prototype1/parent_identity.json`, via `write_parent_identity` (`crates/ploke-eval/src/cli/prototype1_state/identity.rs:222-239`);
- setup path also persists the active checkout files through git commit (`cli_facing.rs:267-280` and backend commit path below).

### 4.3 Resolve parent identity or successor handoff

If `--handoff-invocation` is present, the invocation is loaded and must be `InvocationAuthority::Successor`; child invocations are rejected (`cli_facing.rs:6543-6556`). The successor invocation is validated into a `ParentIdentity` (`cli_facing.rs:6544-6547`).

If no handoff is present, `resolve_prototype1_parent_identity` loads the active checkout parent identity (`cli_facing.rs:6557-6559`).

The parent is then loaded as `Parent<Unchecked>` with or without a runtime id (`cli_facing.rs:6572-6607`). `acknowledge_prototype1_state_handoff` checks/acknowledges startup authority and yields a ready parent plus optional handoff invocation (`cli_facing.rs:6608-6614`).

### 4.4 Append parent-start evidence

The parent start is persisted immediately:

- `JournalEntry::ParentStarted` append (`cli_facing.rs:6629-6642`).
- parent resource sample at `ParentStart` (`cli_facing.rs:6643-6652`).

The journal path is `<campaign-root>/prototype1/transition-journal.jsonl`; append creates parent dirs, opens append/create, writes a JSON line, and syncs (`crates/ploke-eval/src/cli/prototype1_state/journal.rs:672-713`).

### 4.5 Establish baseline

`establish_parent_baseline` chooses between:

- generation 0: establish initial parent baseline;
- later generation: promote selected child baseline.

Refs: `cli_facing.rs:420-433`.

Generation-0 baseline calls:

- `advance_eval_closure` (`cli_facing.rs:435-441`);
- `advance_protocol_closure` (`cli_facing.rs:443-445`);
- `load_closure_state` and `complete_baseline_from_closure` (`cli_facing.rs:447-449`).

Selected-child baseline promotion reads the child branch evaluation report JSON and validates campaign/branch match (`cli_facing.rs:451-487`).

Disk writes in baseline:

- eval closure writes batch/run manifests and run artifacts through `advance_eval_closure`;
- protocol closure writes protocol artifacts through `advance_protocol_closure`;
- closure recompute writes closure state JSON.

### 4.6 Resolve search policy and child budget

For `Complete` runs, `run_turn` loads search policy from admitted run-profile if present; otherwise from scheduler state (`cli_facing.rs:6670-6679`). It rejects live complete runs with legacy candidate generation (`cli_facing.rs:6682-6686`). It enforces generation and total-node hard stops before planning children (`cli_facing.rs:6687-6701`).

### 4.7 Resolve/generate child plan

`resolve_child_plan` is called with campaign, manifest, repo root, parent, candidate-generation config, optional explicit node id, and child budget (`cli_facing.rs:6702-6711`). It returns `PlannedChildren`: parent, plan, children, and rejected surface attempts (`cli_facing.rs:6712-6717`).

The plan publication path uses the `ChildPlan` message file under `prototype1/messages/child-plan/<parent-node-id>.json` (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:278-289`). The live write is performed while locking the message sender and calling `write_child_plan_file` (`cli_facing.rs:1217-1232`).

Disk write:

- `ChildPlan` JSON message.

### 4.8 Choose fanout mode and run children

`run_turn` chooses among three paths:

1. rejected-only plan: no child runs, selection is computed from rejected surface attempts (`cli_facing.rs:6748-6757`);
2. `Complete` + adaptive batch: `run_adaptive_child_fanout` (`cli_facing.rs:6757-6776`);
3. otherwise: `run_child_fanout` then one successor-selection pass (`cli_facing.rs:6777-6805`).

`run_child_fanout` uses blocking tasks around `run_planned_child` and applies budget/schedule controls (`cli_facing.rs:5123-5245`). `run_adaptive_child_fanout` runs child batches and re-runs selection after each batch, allowing early stop when a successor is accepted (`cli_facing.rs:5248-5314`).

### 4.9 Selection and continuation

After child outcomes, `ParentSelection` computes candidates and optional successor (`cli_facing.rs:6792-6801`). If a successor exists, `live_successor_continuation_decision` enforces hard continuation gates: direct-child, generation, node-count, stop-on-keep, require-keep, and History traversal conditions (`cli_facing.rs:6856-6863`; detailed helper around `cli_facing.rs:5327-5400`).

The chosen successor decision is appended to the journal as `JournalEntry::Successor(selected_with_decision)` (`cli_facing.rs:6885-6899`). If continuation is disallowed, `JournalEntry::Successor(stopped)` is appended (`cli_facing.rs:6923-6940`).

### 4.10 Successor handoff

If continuation is allowed, `spawn_and_handoff_prototype1_successor` installs/starts the successor parent runtime (`cli_facing.rs:6904-6922`). The successor invocation argv is generated in `invocation.rs` as `loop prototype1-state --campaign ... --repo-root ... --handoff-invocation ... --stop-after complete --format json` (`crates/ploke-eval/src/cli/prototype1_state/invocation.rs:65-91`).

Disk writes in handoff include successor invocation JSON and channel messages. Invocation JSON paths are `prototype1/nodes/<node>/invocations/<runtime>.json`; the writer creates parent dirs and writes pretty JSON (`invocation.rs:490-538`).

### 4.11 Parent completion report

Before returning, `run_turn` appends a parent resource sample at `ParentComplete` (`cli_facing.rs:6946-6955`), builds `Prototype1StateReport`, and prints table/json (`cli_facing.rs:6956-7030`). If this turn itself was a successor invocation, `record_prototype1_successor_completion` records completion (`cli_facing.rs:7033-7040`).

## 5. Child execution path: C1-C4 plus child runner

`run_planned_child` executes a planned child through C1-C4 (`cli_facing.rs:4811-5104`). The typed child runner then evaluates the materialized treatment branch and returns a terminal result over the channel.

### 5.1 C1: materialize branch

C1 is implemented in `crates/ploke-eval/src/cli/prototype1_state/c1.rs`.

`C1::from_child_plan` validates the campaign/node/request/resolved branch consistency and verifies parent target content against recorded source (`c1.rs:436-518`).

Normal/single-file C1 transition:

- append before journal entry (`c1.rs:836-843`);
- backend realizes workspace/worktree (`c1.rs:851-866`);
- remove stale child binary if present (`c1.rs:867-873`);
- write updated node projection and runner request (`c1.rs:875-893`);
- append after journal entry (`c1.rs:930-937`).

Broad-harness C1 transition:

- validate admitted workspace/artifact evidence and changed paths (`c1.rs:591-694`);
- append before journal (`c1.rs:696-703`);
- remove stale child binary and write node/request projections (`c1.rs:712-738`);
- append after journal (`c1.rs:768-775`).

Disk writes:

- child worktree target files through backend;
- node projection JSON;
- runner request JSON;
- transition journal JSONL;
- possible stale binary removal.

### 5.2 C2: build child binary

C2 is implemented in `crates/ploke-eval/src/cli/prototype1_state/c2.rs`.

`BuildChild::transition`:

- append before journal (`c2.rs:313-323`);
- create scratch target dir and child binary dir (`c2.rs:332-345`);
- run `cargo check` with `CARGO_TARGET_DIR` (`c2.rs:347-370`);
- on check failure, write failed node projection and after journal (`c2.rs:381-408`);
- run `cargo build` with `CARGO_TARGET_DIR` (`c2.rs:411-434`);
- on build failure, write failed node projection and after journal (`c2.rs:445-472`);
- copy built binary to node bin path, clean scratch dir, write `BinaryBuilt` node projection, append after journal (`c2.rs:481-545`).

Disk writes:

- scratch cargo target dir under the node;
- promoted child binary under `prototype1/nodes/<node>/bin/ploke-eval`;
- node projection JSON;
- transition journal JSONL;
- scratch cleanup via `fs::remove_dir_all` (`c2.rs:75-94`).

### 5.3 C3: spawn child and wait for ready

C3 is implemented in `crates/ploke-eval/src/cli/prototype1_state/c3.rs`.

`SpawnChild::transition`:

- build child invocation and write invocation JSON (`c3.rs:439-461`);
- append spawn-starting journal entry (`c3.rs:464-483`);
- create/open stdout/stderr stream files (`c3.rs:147-168`, `c3.rs:484-503`);
- spawn child process with env vars and current directory set to child artifact root (`c3.rs:484-503`);
- append spawn-spawned journal entry (`c3.rs:515-531`);
- wait for child `Ready` over file channel (`c3.rs:533-537`);
- on ready, write node `Running` projection and append observed/acknowledged journal entry (`c3.rs:540-592`);
- on rejected spawn, write node `Failed` projection and append observed/rejected journal entry (`c3.rs:594-638`);
- readiness loop polls channel and child exit until timeout (`c3.rs:649-700`).

Disk writes:

- child invocation JSON;
- stdout/stderr stream files;
- channel JSONL ready/evaluating/result messages from child;
- node projection JSON;
- transition journal JSONL.

### 5.4 Child runner: `prototype1-runner --execute`

The child process executes `execute_prototype1_runner_invocation` in `crates/ploke-eval/src/cli/prototype1_process.rs` (`prototype1_process.rs:2977-3129`).

Function path:

1. `execute_prototype1_runner_invocation(invocation_path)` loads the executable invocation and requires `InvocationAuthority::Child`; successor invocations are rejected (`prototype1_process.rs:2977-2994`).
2. It resolves campaign manifest, node, runner request, and resolved branch from the invocation (`prototype1_process.rs:2995-2998`).
3. It constructs a typed `Child` with journal path, runtime id, node refs, paths, and process id (`prototype1_process.rs:3009-3031`).
4. It opens a child-side channel from invocation endpoints (`prototype1_process.rs:3032-3034`).
5. It transitions child to ready and sends `Ready` over the channel (`prototype1_process.rs:3035-3048`).
6. It transitions child to evaluating and sends `Evaluating` (`prototype1_process.rs:3051-3068`).
7. It calls `run_prototype1_resolved_branch_treatment` (`prototype1_process.rs:3070-3078`).
8. It converts success into `build_succeeded_runner_result` plus treatment evidence, or failure into `build_treatment_failed_runner_result` with no treatment payload (`prototype1_process.rs:3080-3099`).
9. It writes attempt and node runner-result projections via `record_attempt_runner_result` (`prototype1_process.rs:3100-3110`).
10. It transitions to result-written and sends terminal result over channel (`prototype1_process.rs:3111-3119`).

`record_attempt_runner_result` writes two result projections and updates node projection (`prototype1_process.rs:2961-2974`):

- `prototype1/nodes/<node>/results/<runtime>.json`;
- `node.runner_result_path`;
- node projection with projected status.

### 5.5 Child self-evaluation of the treatment branch

`run_prototype1_resolved_branch_treatment` performs the child-side evaluation (`prototype1_process.rs:3131-3333`). The important phases are:

1. materialize/ensure treatment branch (`prototype1_process.rs:3189-3199`);
2. resolve baseline campaign (`prototype1_process.rs:3201-3205`);
3. prepare treatment campaign (`prototype1_process.rs:3206-3210`);
4. prepare instance target cache (`prototype1_process.rs:3211-3216`);
5. advance eval closure for treatment campaign (`prototype1_process.rs:3217-3234`);
6. advance protocol closure for treatment campaign (`prototype1_process.rs:3236-3247`);
7. load treatment closure state (`prototype1_process.rs:3249-3254`);
8. build treatment evidence (`prototype1_process.rs:3255-3267`);
9. require complete treatment and validate patch projection (`prototype1_process.rs:3268-3279`).

Disk writes:

- treatment campaign manifest;
- treatment eval batches/runs/records;
- treatment protocol artifacts;
- treatment closure state;
- runner-result JSON projections;
- channel terminal result JSONL.

### 5.6 Broad headless-TUI turn-live replay artifacts

Broad harness candidate generation has an additional observability write boundary before the resulting submitted edit is converted into child files. `run_broad_headless_tui_attempt_with_options` prepares the workspace, reads the prompt, selects a model label, and runs `tui_adapter::run_headless_with_model_capture_responses` (`cli_facing.rs:1532-1620`). That wrapper installs a response tap before entering the normal headless TUI runtime (`tui_adapter.rs:152-173`).

After the headless attempt reaches a terminal outcome, the parent writes two observability bundles:

1. compact `.headless-tui.json` diagnostics (`cli_facing.rs:2042-2053`);
2. a `.turn-live/` replay bundle via `write_broad_headless_tui_turn_live_bundle` (`cli_facing.rs:2055-2088`).

The `.turn-live/` directory is `slot.published.submitted_result_path().with_extension("turn-live")`. It contains:

- `agent-turn-trace.json`: `AgentTurnTraceRecord(AgentTurnArtifactRecord)`;
- `agent-turn-summary.json`: `AgentTurnSummaryRecord(AgentTurnArtifactRecord)`;
- `llm-full-responses.jsonl`: one `RawFullResponseRecord` per captured provider response.

The source `HeadlessRun` records prompt/tool/turn observations, proposal/application attempts, prompt diagnostics, cargo-validation observations, debug relay messages, and captured full provider responses (`tui_adapter.rs:2031-2040`). `HeadlessRun::agent_turn_artifact_record` converts the observed event stream into passive `ploke-records` agent-turn records (`tui_adapter.rs:2094-2189`).

This write boundary is replay evidence, not runtime authority. It lets `ploke-eval run replay turn-live` reconstruct a historical provider-response prefix and re-execute the tool loop in a current workspace. The dedicated companion doc [`turn-live-replay.md`](turn-live-replay.md) covers the replay command, branch tapes, live observer emissions, and model routing details.

### 5.7 C4: observe child terminal result

C4 is implemented in `crates/ploke-eval/src/cli/prototype1_state/c4.rs`.

`ObserveChild::transition`:

- append observe-before journal entry (`c4.rs:266-277`);
- read child-to-parent channel for `ToParent::Result` (`c4.rs:194-217`);
- on failed runner result, append observe-after journal (`c4.rs:328-358`);
- on success, require treatment evidence and append observe-after journal (`c4.rs:361-407`);
- time out if no terminal result before stale timeout (`c4.rs:410-424`).

After C4 success, `run_planned_child` calls `compare_observed_child_treatment` to build the branch evaluation report and branch registry entry (`cli_facing.rs:5042-5076`, `cli_facing.rs:4754-4808`).

## 6. Disk-write boundaries

This table lists the main persistent write boundaries in the `prototype1-state` path.

| Boundary | Writer | Path / artifact | Originating runtime value | Meaning |
|---|---|---|---|---|
| Parent identity | `write_parent_identity` (`identity.rs:222-239`) | `.ploke/prototype1/parent_identity.json` | `ParentIdentity` | Checkout-carried authority for the active parent. |
| Admitted profile | `admit_run_profile` (`profile.rs:837-853`) | `<campaign>/prototype1/run-profile.toml` | `Prototype1RunProfile` from `OperatorRunProfile` | Campaign-local execution contract. |
| Profile commitment | `write_commitment` (`profile.rs:948-954`) | `<campaign>/prototype1/run-profile.commitment.json` | `RunProfileCommitment` | Digest/provenance guard for admitted profile. |
| Campaign manifest | `save_campaign_manifest` via `prepare_prototype1_loop_campaign` (`cli_facing.rs:7553-7714`) | `<campaign>/campaign.json` | `CampaignManifest` | Durable campaign config. |
| Slice dataset | `write_prototype1_slice_dataset` (`cli_facing.rs:7717-7771`) | `<campaign>/slice.jsonl` | Selected prepared dataset entries | Dataset subset for this Prototype 1 campaign. |
| Closure state | `recompute_closure_state` (`closure.rs:254-298`) | closure-state JSON | `ClosureState` | Snapshot of registry/eval/protocol completeness. |
| Transition journal | `PrototypeJournal::append` (`journal.rs:672-713`) | `<campaign>/prototype1/transition-journal.jsonl` | `JournalEntry` | Append-only recovery/audit/event stream. |
| Child plan | `write_child_plan_file` via lock (`cli_facing.rs:1217-1232`) | `prototype1/messages/child-plan/<parent>.json` | `ChildPlan` | Parent-published child plan message. |
| Channel messages | `FileTransport::append` (`channel.rs:757-790`) | `parent-to-child.jsonl`, `child-to-parent.jsonl` | Channel envelope with `ToParent`/`ToChild` body | Durable parent-child IPC. |
| Node projection | `write_node_projection` calls in C1/C2/C3 and runner | `prototype1/nodes/<node>/node.json` | `Prototype1NodeRecord` projection | Current child node status/projection. |
| Runner request | `write_runner_request_at` from C1 | `prototype1/nodes/<node>/runner-request.json` | `Prototype1RunnerRequest` | Child executable request contract. |
| Invocation | `write_invocation` (`invocation.rs:525-538`) | `prototype1/nodes/<node>/invocations/<runtime>.json` | `ExecutableInvocation` | Token/argv/authority for child or successor process. |
| Runner result | `record_attempt_runner_result` (`prototype1_process.rs:2961-2974`) | `prototype1/nodes/<node>/results/<runtime>.json`, plus node runner result path | `Prototype1RunnerResult` | Child terminal result projection. |
| Child binary | C2 build (`c2.rs:481-545`) | `prototype1/nodes/<node>/bin/ploke-eval` | built `ploke-eval` binary | Per-child executable after applying candidate branch. |
| Child streams | C3 spawn (`c3.rs:147-168`) | child stdout/stderr files | spawned child process stdout/stderr | Process observability. |
| Broad headless-TUI diagnostics | `write_broad_headless_tui_diagnostics` (`cli_facing.rs:2042-2053`) | `submitted_result_path.with_extension("headless-tui")` | `tui_adapter::evidence::Summary` from `HeadlessRun` | Compact diagnostic summary for broad-harness parent patch generation. |
| Turn-live replay bundle | `write_broad_headless_tui_turn_live_bundle` (`cli_facing.rs:2055-2088`) | `submitted_result_path.with_extension("turn-live")/{agent-turn-trace.json,agent-turn-summary.json,llm-full-responses.jsonl}` | `AgentTurnArtifactRecord` plus `RawFullResponseRecord`s from `HeadlessRun` | Durable replay/debug evidence for re-running a historical provider prefix through current TUI tools. |
| Git worktree target writes | backend realize (`backend.rs:1997-2063`) | child worktree files | resolved treatment content | Materialized child candidate. |
| Git commits | backend `persist_files` (`backend.rs:1812-1901`) | git commits on artifact/child/parent branches | changed workspace files | Durable artifact branch state. |
| Branch evaluation report | `compare_observed_child_treatment` (`cli_facing.rs:4754-4782`) | branch evaluation JSON | `Prototype1BranchEvaluationReport` | Parent comparison of treatment vs baseline. |
| Branch registry | `record_parent_comparison` (`branch_registry.rs:72-91`, `branch_registry.rs:122-130`) | branch registry JSONL | parent comparison record | Append-only branch evaluation history. |
| Batch eval summary | `run_batch` (`runner.rs:3743-3904`) | `batch-run-summary.json` | `BatchRunSummary` | Batch-level eval attempt summary. |
| MSB submission aggregate | `run_batch` (`runner.rs:3743-3780`) | `multi-swe-bench-submission.jsonl` | per-instance submissions | Aggregated benchmark submission. |
| Protocol artifact | `write_protocol_artifact` (`protocol_artifacts.rs:292-352`) | protocol artifact JSON | typed input/output/artifact body | Durable protocol evidence with model/provider provenance. |

## 7. Persisted data types and semantics

| Persisted type | Originating type / producer | Persisted by | Semantic meaning and use |
|---|---|---|---|
| `CampaignManifest` | setup campaign builder | `prepare_prototype1_loop_campaign` | Durable campaign definition: dataset, model/route/provider, eval/protocol policies, roots. Used by `resolve_campaign_config`. |
| `ResolvedCampaignConfig` | `resolve_campaign_config` from manifest | not usually persisted directly | Runtime materialization of campaign config. Drives eval/protocol/closure execution. |
| `Prototype1RunProfile` | operator TOML profile | `admit_run_profile` | Durable operator-approved Prototype 1 behavior for one campaign. Used by `Prototype1StateRunShape::resolve`, search policy, protocol policy, execution timeout/stop, control caps. |
| `RunProfileCommitment` | admitted profile text + source | `write_commitment` | Integrity/provenance guard for campaign-local profile. Used by admitted-profile load to detect drift. |
| `ParentIdentity` | root setup or selected successor node | `write_parent_identity` | Checkout-carried identity and authority for current parent. Used to prevent ambiguous continuation. |
| `ClosureState` | closure recompute over registry/run/protocol artifacts | `recompute_closure_state` | Snapshot of campaign registry/eval/protocol completeness. Used for baseline and treatment evidence. |
| `ChildPlan` | parent planning / candidate generation | `write_child_plan_file` | Parent-published plan containing one or more child files/nodes/requests. Used to materialize C1 children. |
| `Prototype1NodeRecord` projection | node state transitions | `write_node_projection` | Current persisted projection of child/parent node metadata and status. Used for observability, runner request/result, selection material. |
| `Prototype1RunnerRequest` | C1 materialized child request | `write_runner_request_at` | Durable child executable contract: workspace, binary, target, branch request, stop policy. Loaded through invocation by child runner. |
| `ExecutableInvocation` / `InvocationAuthority` | C3 child spawn or successor handoff | `write_invocation` | Durable authority token plus argv/runtime/channel paths. Separates child invocations from successor parent invocations. |
| Channel envelope with `ToParent` / `ToChild` | parent/child channel send calls | `FileTransport::append` | Durable JSONL IPC between parent and child/successor. Used for ready/evaluating/result/successor status. |
| `JournalEntry` | parent/C1-C4/successor/resource events | `PrototypeJournal::append` | Append-only execution/recovery trace. Primary observability spine for transitions. |
| `Prototype1RunnerResult` | child runner success/failure builder | `record_attempt_runner_result` | Child terminal result projection. Success may be accompanied by treatment evidence in terminal channel result. |
| `Prototype1TreatmentEvidence` | child treatment evaluation | `build_prototype1_treatment_evidence`, then terminal channel/result path | Evidence that treatment campaign completed and can be compared to baseline. |
| `Prototype1BranchEvaluationReport` | parent comparison of treatment vs baseline | `compare_observed_child_treatment` | Branch score/comparison artifact used for selection and baseline promotion in later generations. |
| `BatchRunSummary` | eval runner batch execution | `run_batch` | Summary of batch eval attempts, selected model/provider, successes/failures, per-instance artifact paths. |
| `RunArtifactPaths` / `AgentRunArtifactPaths` | eval runner single-run execution | runner persistence | Paths to run manifest, logs, repo/indexing/snapshot status, run record, response trace, submissions, patch projections. |
| `RunRecord` / agent turn records | live headless TUI/eval runner events | runner record emission | Compressed durable run trace with LLM requests/responses, tool calls, artifacts, metrics. Used by protocol closure and replay. |
| `AgentTurnTraceRecord` / `AgentTurnSummaryRecord` | `HeadlessRun::agent_turn_artifact_record` from broad headless-TUI events | `write_broad_headless_tui_turn_live_bundle` | Passive trace/summary wrappers around `AgentTurnArtifactRecord`; used by replay inspection and turn-live probes. |
| `AgentTurnArtifactRecord` | `HeadlessRun` events plus prompt/model/request id | wrapped into trace/summary records | Semantic turn evidence: original prompt, selected model label, user/assistant ids, observed tool/turn events, and patch outcome projection. |
| `RawFullResponseRecord` | response tap installed by `run_headless_with_model_capture_responses` | `llm-full-responses.jsonl` in the `.turn-live/` bundle | Provider-response tape keyed by assistant message id and response index; replay installs it as recorded provider output. |
| `ReplayBranchTape` | live-step replay captures new `RawFullResponseRecord`s | `ploke-eval run replay turn-live --branch-out` | Optional operator continuation tape for the next `--branch-in`; not History or campaign authority. |
| `StoredProtocolArtifact` | protocol procedure run input/output/artifact | `write_protocol_artifact` | Protocol evidence envelope: schema, procedure, subject, run id, model/provider, input/output/artifact. |
| `StoredProtocolArtifactFile` | filesystem-loaded protocol artifact | `list_protocol_artifacts` / `load_protocol_artifact` | Adds path to stored artifact for summaries, aggregate loading, and compatibility checks. |
| Protocol aggregate types | protocol artifact list/load | protocol aggregate module | Derived coverage state over segmentation/call-review/segment-review artifacts. Used by protocol closure to know missing work. |

## 8. Live API calls and model routing

### 8.1 Eval/chat model routing

The model path for benchmark/eval runs is:

1. Campaign config supplies `model_id`, `provider_slug`, and `route_source`.
2. `advance_eval_closure` passes `Some(config.model_id.clone())` and parsed provider into `execute_batch_eval_for_manifest` (`crates/ploke-eval/src/cli.rs:6937-6945`).
3. `execute_batch_eval_for_manifest` constructs `RunMsbAgentBatchRequest` (`cli.rs:1719-1737`).
4. `RunMsbAgentBatchRequest::run` calls `run_batch` (`runner.rs:3699-3711`).
5. `run_batch` parses requested model id, resolves model, loads provider preference, resolves route, and records selected provider (`runner.rs:3730-3741`).
6. For each instance, `run_batch` runs `RunMsbAgentSingleRequest::run()` sequentially (`runner.rs:3753-3768`).
7. `configure_headless_benchmark_chat` sets benchmark chat policy and model runtime (`runner.rs:126-130`).
8. `configure_eval_model_runtime` sets `RuntimeConfig.active_model`, `RuntimeConfig.active_router`, and model-provider selection (`runner.rs:116-124`).

`resolve_route_for_model` is also a live network boundary for OpenRouter route/provider metadata: direct Google models return a direct route, while OpenRouter models call `OpenRouter::fetch_model_endpoints` through a `reqwest::Client` (`runner.rs:2501-2536`).

### 8.2 Protocol JSON model routing

The protocol model path is:

1. Campaign/protocol policy enters `advance_protocol_closure`.
2. `advance_protocol_closure` passes `config.model_id`, `config.route_source`, `config.provider_slug`, `policy.max_concurrency`, `policy.tool_review_parallelism`, `policy.max_tokens`, and `policy.reasoning` to `execute_protocol_run_tasks` (`cli.rs:7011-7021`).
3. `execute_protocol_run_tasks` fans out per record/run with `JoinSet` and gates review calls with a semaphore (`cli.rs:1745-1846`).
4. `execute_protocol_run_task` performs segmentation, call reviews, and segment reviews (`cli.rs:7249-7357`).
5. `protocol_llm_config` resolves model id and route/provider into `JsonLlmConfig` (`cli.rs:7741-7761`).
6. `effective_protocol_reasoning` disables auto reasoning for direct Google routes (`cli.rs:7764-7773`).
7. `JsonAdjudicator::new(client.clone(), config.clone())` is used for protocol procedures:
   - intent segmentation (`cli.rs:7639-7641`);
   - tool-call review (`cli.rs:7845-7848`);
   - tool-call segment review (`cli.rs:7879-7884`).

Protocol retry behavior is local to each request: malformed JSON parse errors are retried up to `PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS` (`cli.rs:7836-7865`, `cli.rs:7867-7901`).

### 8.3 Broad-harness parent-patcher headless TUI model routing

Broad harness candidate generation is also a live model surface. It is separate from eval/protocol campaign routing: broad parent patch generation uses `BroadTuiAttemptOptions` and the parent-patcher model selection path (`cli_facing.rs:646-715`).

Model path:

1. `BroadTuiAttemptOptions::from_cli` parses explicit broad-TUI `model_id`/provider, or falls back to `load_parent_patcher_model_selection` (`cli_facing.rs:665-694`).
2. `load_parent_patcher_model_selection` reads the parent-patcher selection, falling back to active model selection only if the parent-patcher selection is missing (`cli.rs:2960-2972`).
3. `headless_model_selection` chooses direct Google vs OpenRouter and rejects incompatible direct-Google provider pins (`cli.rs:2917-2945`).
4. `run_broad_headless_tui_attempt_with_options` passes `options.model().cloned()` into `tui_adapter::run_headless_with_model_capture_responses` and records `options.model_label()` into the turn-live bundle, falling back to `unknown-headless-model` only if no model selection is available (`cli_facing.rs:1589-1600`).
5. `start_attempt_runtime` applies the model by setting `RuntimeConfig.active_model`, `RuntimeConfig.active_router`, and the model provider selection before submitting the prompt (`tui_adapter.rs:306-346`).

The live provider call itself occurs inside the normal `ploke-tui` chat/session path after the prompt is submitted. The turn-live bundle captures provider responses through the response tap and writes them to `llm-full-responses.jsonl` for replay (`tui_adapter.rs:152-173`, `tui_adapter.rs:869-889`, `cli_facing.rs:2077-2083`).

### 8.4 Turn-live replay model routing

`ploke-eval run replay turn-live` has a related but distinct model path. The recorded prefix is not a live API call; it is served from `llm-full-responses.jsonl`. A live call happens only if `--tail live` or `--tail live-step` reaches beyond the installed prefix.

Replay model path:

1. `ReplayTurnLiveCommand::run` sets `model = None` for `--tail stop` when no model/provider override is supplied (`cli.rs:4796-4802`).
2. Otherwise `resolve_replay_probe_model_selection` parses `--model-id` and optional provider, rejects provider-without-model, or loads the parent-patcher model selection (`cli.rs:4896-4921`).
3. `ProbeRequest` passes that optional model into `tui_adapter::run_headless_with_model` (`replay/probe.rs:446-453`).
4. `start_attempt_runtime` applies the runtime model config in the same place as production broad generation (`tui_adapter.rs:306-346`).

Replay branch tapes written by `--branch-out` persist captured live responses as `RawFullResponseRecord`s, not new model config authority (`replay/probe.rs:290-360`, `replay/probe.rs:461-473`).

### 8.5 Where model provenance is persisted

Protocol artifacts store optional `model_id` and `provider_slug` in the artifact envelope (`protocol_artifacts.rs:21-35`, `protocol_artifacts.rs:292-352`). Eval batch summaries store selected model/provider (`runner.rs:3882-3904`). Run intent/registration stores model/provider into run registry intent (`runner.rs:158-214`). Broad turn-live artifacts persist only the selected model label in `AgentTurnArtifactRecord.selected_model`; provider/route provenance for that surface remains in runtime config/selection, not in the agent-turn record family (`agent_turn.rs:37-53`, `cli_facing.rs:1589-1600`).

## 9. Existing parallelism and missed/weak parallelism

### 9.1 Existing parallelism

- Child fanout: `run_child_fanout` executes planned children through blocking tasks (`cli_facing.rs:5123-5245`).
- Adaptive child fanout: `run_adaptive_child_fanout` runs batches and can stop early when selection accepts a successor (`cli_facing.rs:5248-5314`).
- Protocol run fanout: `execute_protocol_run_tasks` uses `JoinSet` over protocol run tasks and `Semaphore` for review permits (`cli.rs:1745-1846`).
- Tool-call reviews: `review_calls` spawns one task per call subject and uses semaphore permits (`cli.rs:7775-7807`, `cli.rs:7809-7834`).

### 9.2 Sequential sections that could be parallelized or batched

| Sequential section | Current behavior | Parallelism/batching opportunity | Notes |
|---|---|---|---|
| Eval batch loop | `advance_eval_closure` loops `for plan in &plans` and awaits each batch (`cli.rs:6904-6951`). | Bounded parallel batch execution if registry/storage conflicts are resolved. | Need protect shared batch roots, run registry, provider quota, and logs. |
| Instances inside one eval batch | `run_batch` loops `for task_id in &prepared.instances` and awaits each single run (`runner.rs:3753-3866`). | Bounded per-instance eval fanout. | High value, but must isolate repo/cache/indexing outputs and provider concurrency. |
| Adaptive child fanout | Runs batches and reselects after each batch (`cli_facing.rs:5248-5314`). | If early-stop is disabled or not needed, use larger non-adaptive fanout. | Current sequentiality is partly semantic: early selection can save expensive child runs. |
| Segment reviews | `execute_protocol_run_task` loops missing segment indices sequentially (`cli.rs:7326-7342`). | Use same bounded `JoinSet`/semaphore pattern as `review_calls`. | Independent segment review requests are natural parallel candidates. |
| Protocol artifact listing/loading | `list_protocol_artifacts` scans/read-loads artifacts one by one (`protocol_artifacts.rs:355-380`, `protocol_artifacts.rs:508-520`). | Bounded parallel read/decode with deterministic sort after join. | Useful once protocol artifacts become numerous. |
| Protocol registration sync | `write_protocol_artifact` calls `sync_protocol_registration_status` after every artifact write (`protocol_artifacts.rs:345-352`). | Batch/defer sync once after multi-artifact protocol phases. | Must preserve crash/recovery semantics. |
| Branch comparison writes | Parent compares each successful child as it observes it (`cli_facing.rs:4754-4808`, `cli_facing.rs:5042-5076`). | Batch comparisons after all children if selection does not need incremental results. | Adaptive selection may prefer incremental evidence. |
| Turn-live replay probes | One probe serially consumes provider tape/live responses, executes tool calls, applies/refreshes workspace state, and stops at a boundary (`replay/probe.rs:368-495`). | Run independent cursors/branches in parallel only with isolated workspaces/processes. | Recorded response tape/taps and workspace proposal/index state are process/workspace mutable; shared-workspace fanout would race. |
| Run record summary transforms | Runner builds summaries in loops (`runner.rs:3868-3904` and nearby setup summary code). | Low-risk CPU parallelism for large batches, but likely not the first bottleneck. | Provider/API time likely dominates. |

## 10. Practical stabilization notes

- Treat `parent_identity.json` as authority, not just metadata. Runtime continuation should be rooted in checkout-carried identity or successor invocation.
- Treat the transition journal as the recovery/audit spine. A missing or incomplete journal entry often means a transition was not durably recorded even if projections exist.
- Distinguish projection files from authority files. `node.json`, runner results, and child result files are reconstruction/projection surfaces; the terminal channel result is where successful treatment evidence crosses from child to parent.
- Baseline and treatment evidence both depend on closure completeness. If a branch comparison looks wrong, inspect closure state, run records, and protocol artifacts before blaming selection.
- Model routing has two separate live surfaces: eval/headless TUI chat and protocol JSON adjudication. Campaign/profile changes should be traced through both.
- Broad parent patch generation has its own parent-patcher model route. Turn-live artifacts persist the selected model label and full-response tape, but not a full provider/route authority record.
- Turn-live live observer emissions are stderr progress signals; the `.turn-live/` files are the durable replay evidence. Do not conflate the two when debugging missing observability.
- Parallelism changes should be introduced with explicit resource caps from the admitted profile or campaign policy; do not silently widen fanout beyond admitted limits.

## 11. Quick path index

Entrypoint and config:

- `crates/ploke-eval/src/main.rs:5-20`
- `crates/ploke-eval/src/cli.rs:539-568`
- `crates/ploke-eval/src/cli.rs:609-661`
- `crates/ploke-eval/src/cli.rs:1445-1457`
- `crates/ploke-eval/src/campaign.rs:27-53`
- `crates/ploke-eval/src/campaign.rs:123-138`
- `crates/ploke-eval/src/campaign.rs:478-572`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs:37-59`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs:837-864`
- `crates/ploke-eval/src/cli/prototype1_state/profile.rs:868-903`

Parent runtime:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:946-999`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6477-7043`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5123-5314`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5327-5400`

Child state machine:

- `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c2.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
- `crates/ploke-eval/src/cli/prototype1_state/c4.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs:2961-3333`

Persistence and transport:

- `crates/ploke-eval/src/cli/prototype1_state/identity.rs:222-239`
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:672-713`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs:245-253`
- `crates/ploke-eval/src/cli/prototype1_state/channel.rs:757-790`
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs:490-538`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1812-1901`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1997-2063`
- `crates/ploke-eval/src/protocol_artifacts.rs:292-352`

Turn-live replay and broad headless TUI:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:646-715`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1532-1620`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2055-2088`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:152-173`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:470-889`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1716-1806`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:2031-2354`
- `crates/ploke-records/src/agent_turn.rs:1-65`
- `crates/ploke-records/src/llm_response.rs:1-53`
- `crates/ploke-tree/src/playback/turn.rs:1-259`
- `crates/ploke-eval/src/replay/turn.rs:1-300`
- `crates/ploke-eval/src/replay/probe.rs:1-620`

Live model/API boundaries:

- `crates/ploke-eval/src/runner.rs:116-130`
- `crates/ploke-eval/src/runner.rs:2501-2536`
- `crates/ploke-eval/src/runner.rs:3699-3911`
- `crates/ploke-eval/src/cli.rs:6875-7051`
- `crates/ploke-eval/src/cli.rs:7249-7357`
- `crates/ploke-eval/src/cli.rs:7741-7901`
