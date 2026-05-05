# Prototype 1 Fanout Selection Review: Runtime Correctness

## Findings

### High: concurrent child tasks race on `scheduler.json`, node mirrors, and runner requests

The new live path runs child transitions concurrently through `run_child_fanout` by spawning a blocking task per child in the current wave (`cli_facing.rs:5411-5458`). Each task calls `run_planned_child`, which performs materialize, build, spawn, and observe transitions (`cli_facing.rs:5235-5408`).

Those transitions write scheduler-owned state through `update_node_status` and `update_node_workspace_root`:

- materialize updates node status and workspace root (`c1.rs:578-600`)
- build updates node status on failure and success (`c2.rs:435-446`, `c2.rs:503-514`, `c2.rs:562-573`)
- spawn updates node status on ready or failed handshake (`c3.rs:631-642`, `c3.rs:697-708`)
- the scheduler helpers load the whole scheduler, mutate one node, then rewrite `scheduler.json` with `fs::write` (`scheduler.rs:556-600`, `scheduler.rs:603-638`, `scheduler.rs:413-425`)

There is no file lock, compare-and-swap, append-only scheduler mutation log, or single parent-side writer. Two child tasks can load the same scheduler snapshot, update different nodes, and then overwrite each other's changes. This can lose node status, workspace root, frontier/completed/failed membership, and `updated_at` changes. It can also leave the per-node `node.json` mirror and scheduler aggregate disagreeing.

This conflicts with the module invariant that scheduler files are mutable projections, not authority, but still must not mislead the controller (`mod.rs:409-411`, `mod.rs:647-669`). It also directly affects runtime behavior because later selection and monitoring read these mutable files.

Suggested fix: keep child transition execution concurrent only for process/build work, but serialize scheduler mutation through a parent-owned coordinator, or add a campaign-local scheduler lock around every load/mutate/write operation. Longer term, make scheduler updates append-only transition records and rebuild the projection.

### High: shared transition journal reads can fail while another task or child process is appending

Every child task receives the same transition journal path (`cli_facing.rs:5444-5454`). The journal appender opens the JSONL file with append mode and writes one serialized line followed by `sync_data` (`journal.rs:655-687`). Spawn readiness and result observation read the whole journal repeatedly:

- `wait_for_ready` calls `Handoff::find_ready`, which loads and parses all journal entries (`c3.rs:747-764`, `c3.rs:194-218`)
- `ObserveChild` calls `child_result_path`, which loads and parses all journal entries (`c4.rs:227-240`, `c4.rs:304-307`)
- `PrototypeJournal::load_entries` treats any parse failure on a non-empty line as fatal (`journal.rs:370-401`)

With fanout, there are multiple parent task writers, multiple child process writers, and multiple readers on the same JSONL file. A reader can observe a partially written line and fail the child transition with `ReadJournal`. The current append implementation also does not provide an explicit record boundary or advisory lock, so concurrent appends rely on filesystem behavior rather than the Prototype 1 message/History model.

This matters because child-ready and child-result records are live protocol observations, not just debug logs. A transient parse of an in-progress append can become a child failure or parent failure.

Suggested fix: use a journal access layer with a shared lock for append and load, or make `load_entries` tolerate a final incomplete line while keeping parse failures for completed lines fatal. Prefer one parent-owned journal reader/projection loop over each child task repeatedly reparsing the shared file.

### High: ready timeout can leave a child runtime unmanaged

`SpawnChild::transition` spawns the child process, waits up to `READY_TIMEOUT`, and on timeout returns `Outcome::Rejected` (`c3.rs:578-626`, `c3.rs:778-784`, `c3.rs:718-736`). The timeout path records a rejected spawn observation, but it does not kill, wait for, detach with a durable lease, or otherwise manage the spawned process.

Under fanout, a timed-out child can continue running while the parent proceeds to other children or returns an error. That violates the intended runtime protocol that cleanup and explicit handoff are semantic protocol steps, not optional polish (`mod.rs:180-187`, `mod.rs:510-513`). It also makes the review question about "errors handled without leaving unmanaged children" fail for at least the ready-timeout case.

Suggested fix: introduce an explicit timeout disposition for spawned child processes: terminate-and-record, observe-to-terminal, or durable-detach-with-lease. For the current prototype, terminate-and-record is the safest default.

### Medium: one child task error aborts the whole wave even if sibling evidence is usable

`run_child_fanout` records the first task error while joining a wave and returns it after the wave is joined (`cli_facing.rs:5461-5486`). That means one transition error aborts generation selection entirely, even if a sibling in the same wave completed and produced an accepted selection input.

Some transition errors should remain parent-fatal, especially authority, manifest, and typed-plan violations. But many runtime failures are child-local: build invocation failure, stream file failure for one child, spawn failure for one binary, or evaluation artifact load failure for one node. The current code does not distinguish those classes.

Suggested fix: make `PlannedChildOutcome` carry `Ok`, `Rejected`, and `Failed` evidence. Only abort the parent for errors that invalidate the parent turn or shared authority surface. Child-local failures should be recorded and included in generation-level selection/reporting.

### Medium: generation selection is based only on successful runner evaluations

`run_planned_child` sets `selection_input` only when `ObservedChild::Succeeded` is present (`cli_facing.rs:5379-5386`). Failed runner results, build failures, spawn failures, and timeout failures produce outcome strings but no selection evidence. `generation_selection` therefore only sees completed successful evaluation reports (`cli_facing.rs:5512-5520`).

That is acceptable for selecting an accepted successor, but it is incomplete for "best rejected child" exploration. A child rejected by the runner or build system may still be important evidence, and a generation with all children failing before evaluation will produce `selection=none` rather than a policy-readable generation failure.

Suggested fix: either define that best-rejected fallback only applies to children with successful evaluation reports, or add a child failure evidence variant that `decide_generation` can rank or explicitly exclude with a recorded reason.

### Medium: the one-row state report hides fanout outcomes

After fanout, `Prototype1StateReport` is populated from `outcome_for_report`, which returns the selected child outcome or the last completed outcome (`cli_facing.rs:5523-5530`, `cli_facing.rs:5942-6064`). The `outcome` string includes `children_ran` and `children_planned`, but `node_id`, `node_status`, `workspace_root`, `binary_path`, and `child_runtime` refer to only one child.

For a fanout run this can be misleading: a report may display the selected child while hiding failed siblings, or display the last child when no selection exists. This is a projection problem rather than an authority violation, but it can confuse operator decisions and monitor output.

Suggested fix: add a fanout summary section or structured `children: Vec<PlannedChildOutcomeReport>` to the report. Keep the selected child as a separate field.

### Low: `min_children` is fanout width, but not a minimum completed-child requirement

`run_child_fanout` sets `fanout_width = min(child_budget.min, nodes.len()).max(1)` and only warns if fewer planned nodes exist than the configured minimum (`cli_facing.rs:5426-5435`). That matches the current "min as fanout width" interpretation, but the CLI/policy name can still be read as "at least this many children must be evaluated."

Suggested fix: document this explicitly in the CLI help and scheduler policy docs, or rename the field later when the policy surface is cleaned up.

## Open Questions / Assumptions

- I assume `run_child_fanout` is the only new live typed parent fanout path under review.
- I assume local filesystem append behavior is not being treated as a semantic record-boundary guarantee. If that guarantee is intended, it should be written down and tested under concurrent appends/readers.
- I assume child-local failures should not necessarily abort a generation if sibling children have already produced usable evidence.
- I assume `--stop-after materialize|build|spawn` intentionally remains single-child. The current code enforces that by replacing the configured child budget with `min=1,max=1` for non-`Complete` stops (`cli_facing.rs:5917-5922`), so those semantics are preserved.
- I did not find a direct successor-History authority break in the selection-to-handoff segment. `spawn_and_handoff_prototype1_successor` still consumes `Parent<Selectable>`, prepares the selected Artifact in the active parent root, seals/appends the History block, and only then spawns the successor (`prototype1_process.rs:1122-1227`, `prototype1_process.rs:1331-1411`).

## Summary

The new path has the right high-level control shape: resolve a typed child plan, run a bounded fanout, join child evidence, select accepted first, then fall back through generation selection (`cli_facing.rs:5162-5233`, `cli_facing.rs:5411-5520`, `cli_facing.rs:5937-6005`). `--stop-after` is preserved as single-child for partial debug stops.

The implementation is not yet runtime-safe for real concurrent fanout. The main blockers are shared mutable scheduler rewrites, shared JSONL journal read/write races, unmanaged spawned children after ready timeout, and fail-fast error handling that treats child-local failures as parent-fatal. Fix those before relying on multi-child fanout for longer runs.
