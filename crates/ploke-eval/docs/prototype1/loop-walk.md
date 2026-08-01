# Prototype 1 `loop walk` guide

`ploke-eval loop walk` and `ploke-walk-ui` are active sibling clients of one
walk service. The server owns the active controller lease over the durable
session and admits every Prototype 1 typestate transition requested by these two
walk frontends. Neither frontend executes an R-state edge directly. Direct
`prototype1-state`, `prototype1-step`, and `prototype1-continue` commands share
the same durable session boundary but are not walk-service clients. The owner
database is an evolving evidence projection rather than controller authority;
each walk database-evidence query reads one immutable, revision-tagged snapshot
of it.

- **set up and drive the live outer loop** (typed setup preview/admission,
  `walk start`, and `walk step`);
- **review recorded evidence** (`walk summary`, `walk replay`, `walk llm ...`).

Every typestate mutation is bound to a freshly observed server epoch and
durable session version. Start and step return typed jobs and terminal
receipts; status, blockers, and allowed actions come from the same service
contract in both clients.

## Which binary to use

Use the binary that matches the authority you need:

- For local prompt/format/debugger iteration, use the local dev binary from the checkout you are editing, for example:

  ```bash
  ./target/debug/ploke-eval loop walk --help
  ```

- For the authority-bearing walk server and live parent runtime, use the binary
  from the active parent checkout/worktree for that run. Historical
  campaign/worktree paths are inputs; they do not automatically select the
  server binary under test.
- A CLI or UI sibling client may be built from another checkout. A mutation is
  admitted only when the protocol, transition graph, target repository
  coordinates and source status, and embedded build fingerprint match the
  server. Distinct client/server executable paths are not by themselves an
  incompatibility.
- The build fingerprint is a sibling-client contract key over 15 explicit
  `ploke-eval` boundary files: `build.rs`, the public setup and walk clients,
  and selected walk audit/config/endpoint/epoch/IPC/trace/query/protocol
  modules. It intentionally excludes the owning `ploke-records` and
  `ploke-protocol` crates, Cargo metadata, build environment, server/controller
  implementation, `ploke-tui`, and `ploke-llm`. A transitive carrier change
  outside the explicit boundary must advance the walk protocol or transition
  graph version; it is not made compatible by an unchanged fingerprint.

After changing walk protocol structs or output formatting, restart the walk server before relying on new commands. A running server keeps the old binary in memory.

## Starting, attaching, and stopping the walk server

Typical setup:

```bash
P1_PARENT_ROOT="${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"
./target/debug/ploke-eval loop walk start --repo-root "$P1_PARENT_ROOT"
./target/debug/ploke-eval loop walk show --repo-root "$P1_PARENT_ROOT"
./target/debug/ploke-eval loop walk stop --repo-root "$P1_PARENT_ROOT"
```

Most commands accept `--repo-root` to identify the parent checkout and `--socket`/format flags through the shared walk control options.

`walk start` attaches the server to setup-derived Parent(0) authority through
one guarded operation. It does not replace setup, invent missing configuration,
or create a second controller. `walk stop` means **stop an idle server**. It is
not pause, halt, or cancellation: an admitted effectful job must first reach a
durable terminal receipt.

## Driving the live outer loop

`walk step` advances the live Prototype 1 typestate edge. It is not a historical replay command.

A bare Step admits one transition and returns its exact realized edge. A bounded
`--until` target is accepted only when it post-dominates every graph path from
the current phase (for example R1-to-R3, R4a-to-R4c, or R10-to-R12). A
branch-specific bound is rejected before the first edge claim; take one bare
Step, inspect its typed receipt, and then choose the next target. In particular,
R12 has no unconditional bounded target.

Examples:

```bash
./target/debug/ploke-eval loop walk step
./target/debug/ploke-eval loop walk step --watch
./target/debug/ploke-eval loop walk step --until r8 --allow-live-api --watch
```

Outer `walk step --watch` only follows the accepted job to its terminal receipt;
it is optional for long edges and handoff, and never grants provider authority.
The separate nested `walk llm --source live` debugger retains its own mandatory
`--watch` admission rule.

Workspace mutations are intentionally gated. Use mutation flags only when you intend to let the live parent proceed through effectful edges.

The native UI exposes the same guarded start/attach and exact single-transition
operations. Provider-backed edges require an explicit live-API grant, and
R12-to-R13b checkout handoff requires an explicit Git-change grant. The UI
retains the operation id, polls the typed operation status, and displays the
server job and receipt rather than inferring success from console text.

Launch the UI from a compatible checkout:

```bash
cargo run -p ploke-walk-ui
```

Use **New Run** to preview and admit setup, **Run Config** to read back the
admitted configuration, **Walk controls** to drive guarded transitions, and
**Database Query** for named immutable evidence views.

## Fresh-run setup and admitted configuration

The UI setup form calls typed facades over the canonical run-profile parser and
the same setup planner/admission path as the CLI. Its required inputs are an
existing checkout, an existing prepared batch (path or id), a new campaign id,
and a selected run profile. Optional model, protocol, embedding, and
primary-instance overrides are canonical setup inputs; search, generation,
selection, execution, storage, and control policy remain in the selected
profile.

For an existing profile, choose **Load selected profile**. The returned exact
path, normalized `RunProfileRecord`, and SHA-256 binding can be used immediately
for setup. For a new profile:

1. choose **Restore canonical defaults** and edit the typed record;
2. choose **Validate / review profile** to run the production parser and
   validator;
3. choose **Save reviewed profile** to create the exact reviewed bytes.

Save is hash-bound, atomic, create-only, and read-back validated; it never
overwrites an existing profile. A reviewed but unsaved draft is not eligible
for setup. Editing a reviewed target or policy invalidates the review and exact
binding. Next choose **Preview setup**, review the returned plan SHA-256, and
choose **Admit reviewed setup**. Any changed setup input invalidates admission.

Profile selector kinds are intentionally distinct. **Name** accepts exactly one
non-`.toml` path component and resolves it in the registered profile directory.
**Path** is always explicit; a relative path is first made absolute from the UI
process working directory. `Name("foo.toml")`, empty or non-UTF-8 paths, and
multi-component names fail closed, while `Path("foo")` can never silently turn
into registered profile name `foo`.

Setup is a two-phase operation:

1. Preview resolves the canonical CLI/schema defaults, selected batch/profile,
   campaign manifest, runtime-validated profile, effective control, Git base,
   and exact content hashes without admission writes.
2. Admit rebuilds the plan and requires the reviewed plan SHA-256. Input drift
   fails before admission. A successful admission is read back through
   `WalkConfigSnapshot`, so the UI displays the exact admitted campaign,
   profile commitment, effective values, and provenance.

The UI does not prepare batches, create a checkout/worktree, run doctor or live
provider preflights, or watch profile/setup files for external changes. Those
remain explicit operator prerequisites or CLI checks.

The UI does not reconstruct setup from local policy defaults or editable raw
JSON. Keep these three layers distinct when reviewing a run:

- CLI/schema defaults used only when the operator omitted a canonical setup
  option;
- cohort and eval budget selected by the prepared batch, and loop policy
  selected by the run profile;
- effective admitted values and derivation provenance returned by
  `WalkConfigSnapshot`.

The `Run Config` surface reads the same snapshot for an existing run. Campaign
`p1-v36-walkop-handoff-g35f-global-3g1x3-20260731-151050` in worktree
`/home/brasides/.ploke-eval/setup-seeds/p1-v36-walkop-handoff-g35f-global-3g1x3-20260731-151050`
live-validated setup/config read-back and a successful Parent(0) R5-to-R6
baseline edge. Its durable summary remains nonterminal at generation 0 with
active node `node-5798cc0855390ba4`, three admitted and observed children, and
journal entry 40 `observe_child` as the latest entry. It has no selected
successor, continuation decision, or handoff. That is bounded negative evidence,
not the requested three-parent packet. A local preview or UI rendering alone is
not live proof.

## UI-local auto-advance

The UI labels its scheduler **UI-local automation · Step-mode server**. It
submits exactly one guarded Step operation, waits for that operation's terminal
receipt, refreshes authoritative status, and only then submits another Step.
This is deliberately not called `Continuous`, `Halt`, or server `Resume`:

- The canonical profile default is `mode = Continuous` with no explicit
  `parallel_cap`. Manual exact Step remains available for that admitted
  preference, but UI-local auto-advance requires an admitted Step-mode profile.
  Restoring defaults therefore does not enable automation.

- **Auto-advance transitions** is enabled only for an admitted Step-mode run,
  an unpinned endpoint-following client, an Active durable session, a complete
  coherent set of advertised Step outcomes, and grants covering every possible
  live-API or Git-changing outcome.
- **Stop after current transition** suppresses the next submission; it does not
  cancel the operation already admitted by the server.
- **Resume auto-advance** restarts only this UI-local scheduler.
- **End local automation** explicitly discards a paused scheduler intent and
  releases the UI run/socket binding for another selection.
- Closing the UI loses the auto-advance intent.
- There is no exclusive continuous lease between Step operations; another
  client may act between them, subject to the same epoch/version guards.
- While an operation is unresolved or auto-advance is active or paused, the UI
  keeps the selected run and socket binding fixed so operation polling cannot
  move to a different controller.
- After R12-to-R13b, the client waits for and follows the successor endpoint,
  then reloads and validates its `WalkConfigSnapshot` before considering
  another Step.
- An R12-to-R13c receipt halts automation for operator reconciliation; it is not
  retried or treated as a resumable handoff.
- The scheduler does not drive the predecessor-only R13b-to-R14b retirement
  edge.

An explicitly pinned client rejects any advertised Step that can realize
R12-to-R13b before creating an operation id; a stop-only R12 Step remains
available. Manual and automatic handoff must use the unpinned endpoint-following
client so the same operator surface can discover the successor.

This mode does not claim durable `RunMode::Continuous`. Use the job/receipt
display and `walk status --with-version` to determine what the server actually
accepted.

## Operator state and database evidence

Use typed walk commands for ordinary status inspection:

```bash
./target/debug/ploke-eval loop walk status --with-version
./target/debug/ploke-eval loop walk config --with-version
./target/debug/ploke-eval loop walk session-history --with-version
./target/debug/ploke-eval loop walk summary --verbose
```

`status` is the live source for controller authority, session version, current
position, active/last job, blocker, and allowed actions. `config` reports the
admitted configuration and provenance. `session-history` reports ordered
attempt/receipt evidence, and `summary --verbose` reports durable campaign
progress.

Use the closed database views when you need revision-tagged owner-DB evidence:

```bash
./target/debug/ploke-eval loop walk db_query --view relations
./target/debug/ploke-eval loop walk db_query --view counts
./target/debug/ploke-eval loop walk db_query --view config-evidence
./target/debug/ploke-eval loop walk db_query --view lineage
./target/debug/ploke-eval loop walk db_query --view progress
./target/debug/ploke-eval loop walk db_query --view handoff-evidence
```

These views are immutable projections over one exact owner-DB snapshot. They do
not report server liveness, controller ownership, or permission to mutate.
The client sends a typed view identity and the server selects the canonical
script. Expert `--script` queries remain explicitly unnamed even when their
text happens to equal a canonical view script.
`relations` is the complete installed-relation inventory. `counts` is a
curated core operator projection that includes critical typed trace relations;
it is not a count of every installed relation. `progress` keeps generation
roles separate: `actor_generation` identifies the parent turn,
`subject_generation` identifies the referenced subject, and
`successor_generation` identifies child work in plan/scheduler rows and the
next generation only when continuation selects a successor branch. A stop
continuation therefore reports no successor generation. Evaluation rows resolve
the evaluated branch to its scheduler node for `subject_generation` and retain
that node's parent as `actor_generation`; an evaluation that cannot be joined
remains visible with null identity/generation fields rather than borrowing the
parent as its subject.

The database revision and the server observation are separate evidence planes.
The service queries the owner-DB snapshot first, then observes server phase and
session revision. Those values are intentionally non-atomic; use `status` and
`session-history` rather than treating a query response as a
transition-consistent controller snapshot.

Raw `--script` remains an expert escape hatch. Do not use `jq` over campaign
artifacts to decide whether a loop is active; reserve `jq` for bounded
JSON/log/artifact forensics after the typed status/evidence surfaces identify a
specific anomaly.

## Read-only run review

These commands do not call providers, execute tools, mutate the checkout, or advance typestate:

```bash
./target/debug/ploke-eval loop walk summary
./target/debug/ploke-eval loop walk replay
./target/debug/ploke-eval loop walk back
./target/debug/ploke-eval loop walk forward
```

Use `summary` for the current campaign overview and `replay/back/forward` as a cursor over the durable transition journal.

## LLM/tool-loop lane review

Tool-loop checkpoints are stored under:

```text
<campaign>/prototype1/debug/tool-loop/<session-id>/
```

Start with lanes and timeline:

```bash
P1_LANE_ID="${P1_LANE_ID:?set P1_LANE_ID from the lanes output}"
./target/debug/ploke-eval loop walk llm lanes
./target/debug/ploke-eval loop walk llm focus "$P1_LANE_ID"
./target/debug/ploke-eval loop walk llm timeline
```

Then drill down:

```bash
P1_LLM_STEP="${P1_LLM_STEP:-0}"
./target/debug/ploke-eval loop walk llm show --step "$P1_LLM_STEP"
./target/debug/ploke-eval loop walk llm prompt --step "$P1_LLM_STEP"
./target/debug/ploke-eval loop walk llm tool --step "$P1_LLM_STEP" --call 1
./target/debug/ploke-eval loop walk llm protocol
```

Read-only lane commands include:

- `lanes` / `focus` — choose a default lane/session for subsequent commands;
- `timeline` — compact scan of recorded responses and tool batches;
- `show` — checkpoint transcript, tool results, and protocol hints when present;
- `prompt` — persisted request messages sent for a step;
- `tool` — selected persisted tool arguments plus current-renderer tool schema;
- `protocol` — persisted protocol artifact summary and per-call review feedback when present;
- `back` / `forward` / `head` — move the read-only lane cursor.

Important provenance distinction: historical request messages and tool arguments come from persisted checkpoints. Current tool definitions in `walk llm tool` come from the current renderer checkout and are labeled as such.

## Protocol review output

`walk llm protocol` is a read-only review command:

```bash
./target/debug/ploke-eval loop walk llm protocol
./target/debug/ploke-eval loop walk llm protocol --json
```

If no artifacts are available for the selected session, it says:

```text
protocol: not present for this tool-loop session
```

When artifacts exist, it summarizes artifact counts, call-review coverage, segment-review counts, missing reviews, and per-call feedback. Protocol review output is debugging evidence only; it is not child-plan admission authority and does not override protected-write enforcement.

## Effectful nested LLM debugger

The nested debugger can branch from a checkpoint and execute more tool-loop work. These commands are effectful. The `--source live` and `finish` examples are live provider/model API runs:

```bash
P1_LLM_STEP="${P1_LLM_STEP:-0}"
./target/debug/ploke-eval loop walk llm step \
  --step "$P1_LLM_STEP" \
  --source historical \
  --allow workspace-mutation
./target/debug/ploke-eval loop walk llm step \
  --step "$P1_LLM_STEP" \
  --source live \
  --watch \
  --allow workspace-mutation
./target/debug/ploke-eval loop walk llm finish \
  --source live \
  --watch \
  --allow workspace-mutation
```

Rules of thumb:

- `--allow workspace-mutation` is required because tools may change the candidate workspace.
- Live mode requires `--watch` because it may call the provider and execute tool batches.
- `walk llm step`/`finish` create or focus a new debug session; they do not rewrite the historical session.
- Protected-write denials remain authoritative. Do not bypass them.

## Lightweight prompt-lab commands

For prompt iteration outside the full parent loop, prefer the hidden harness helpers. The `prototype1-harness attempt` command is a live provider/model API probe:

```bash
P1_PARENT_ROOT="${P1_PARENT_ROOT:?set P1_PARENT_ROOT to the active parent checkout}"
P1_HARNESS_REQUEST="${P1_HARNESS_REQUEST:?set P1_HARNESS_REQUEST to an edit-harness request JSON}"
./target/debug/ploke-eval loop prototype1-prompt --repo-root "$P1_PARENT_ROOT"
./target/debug/ploke-eval loop prototype1-harness attempt \
  --request "$P1_HARNESS_REQUEST" \
  --model-id google/gemini-2.5-pro \
  --provider google \
  --max-attempts 1 \
  --timeout-secs 300 \
  --format json
./target/debug/ploke-eval loop prototype1-harness sweep --help
```

Use these when you want to rerun a published broad-harness request without advancing the outer typestate walk.

## Pre-live-run review checklist

1. Restart the walk server after local protocol/output changes.
2. `walk summary` to confirm campaign phase and admission counts.
3. `walk llm lanes` and `walk llm timeline` to locate the session under review.
4. `walk llm show --step "$P1_LLM_STEP"` for the transcript and tool outcomes.
5. `walk llm prompt --step "$P1_LLM_STEP"` to inspect persisted request messages.
6. `walk llm tool --step "${P1_LLM_STEP:-0}" --call "${P1_TOOL_CALL:?set P1_TOOL_CALL from the tool-call list}"` for selected tool arguments/schema provenance.
7. `walk llm protocol` for available protocol review feedback.
8. Only then use effectful `walk step` or `walk llm step/finish` with explicit mutation gates.

For a live operator session, begin with `walk status --with-version` and
`walk config --with-version` before this artifact-review checklist. Do not
substitute a DB row, raw JSON file, or `jq` result for the server's typed
authority snapshot.
