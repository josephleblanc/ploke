# Resource Growth

## Domain

Disk use, cargo target growth, temporary child worktrees, stream logs, build artifacts, cleanup, and bounded-resource health.

## Questions To Answer

- During a 5-10 generation run, is the active checkout's Cargo `target/`
  growing monotonically, and by how many bytes per parent turn?
- Does each child evaluation leave behind a temporary node worktree,
  `nodes/<node-id>/target/`, or `nodes/<node-id>/bin/ploke-eval` after its
  result has been recorded?
- Which generation/node owns the largest temporary workspace, node-local Cargo
  target, copied child binary, or stream log?
- Did cleanup run at the intended boundary after compile failure, treatment
  failure, successful child observation, and selected-successor handoff?
- Did cleanup refuse a path because it was outside the node directory, because
  a worktree was unmanaged, or because a worktree branch no longer matched the
  expected child branch?
- Are stdout/stderr stream logs bounded by node/runtime, and are failures
  diagnosable from paths/excerpts without importing full logs as History?
- Is successor build output landing in the stable active checkout `target/`,
  while child builds stay isolated under `nodes/<node-id>/target/`?
- For longer runs, what is the slope of active `target/` growth, aggregate
  node-directory growth, stream-log growth, and orphaned worktree count per
  generation?
- For longer runs, do bounded-resource policies need absolute byte/file-count
  limits, per-generation cleanup deadlines, or a "stop before disk pressure"
  health gate?

## Already Answered By Persisted Data

- The typed parent path already writes `Resource` samples to
  `prototype1/transition-journal.jsonl` for the active checkout Cargo
  `target/` at `ParentStart` and `ParentComplete`, including campaign, parent,
  node, generation, optional runtime id, path, status, bytes, and error.
- The journal can already tell which child/successor transition paths were
  involved: materialize/build/spawn records carry `repo_root`,
  `workspace_root`, `binary_path`, `target_relpath`, and `absolute_path`; spawn
  records may carry stdout/stderr stream paths.
- Scheduler/node records already persist `node_dir`, `workspace_root`,
  `binary_path`, `runner_request_path`, and `runner_result_path`, so the
  expected child worktree, node-local target, binary, and latest result
  locations are discoverable.
- Attempt-scoped invocation/result files and successor ready/completion files
  identify runtime-local sidecars. They answer "where should I inspect this
  runtime's artifacts?" but not "how large were they?".
- The History preview already projects journal `Resource` entries as
  observation evidence with status and bytes. This is useful introspection, not
  Crown or History authority.

## Partially Derivable

- Active Cargo `target/` growth is derivable only for typed parent turns that
  reach the current sampling sites. A failing or aborted turn may miss the
  `ParentComplete` sample.
- Child worktree, node-local `target/`, copied binary, and stream-log residue
  are derivable by combining persisted node paths with current filesystem
  state, but that is a point-in-time inspection, not an append-only record of
  cleanup success.
- Cleanup success is partly inferable from absence of
  `nodes/<node-id>/worktree/`, `nodes/<node-id>/target/`, and
  `nodes/<node-id>/bin/ploke-eval`, plus absence of cleanup error phases. This
  does not distinguish "removed successfully", "never existed", "already
  cleaned by another attempt", and "not reached".
- Successor active-checkout build output is inferable from the successor
  `binary_path` and active root recorded in successor/handoff entries, but the
  build's byte delta is not recorded at that boundary.
- Stream-log byte growth is derivable from stream paths when spawn/handoff
  records exist and files still exist, but no persisted aggregate records log
  stdout/stderr file sizes, digests, truncation status, or cleanup disposition.
- Resource budgets are derivable only outside the current record model by
  running filesystem/git inspections over node directories and worktrees.

## Requires New Logging

- Resource samples for child-owned subjects: child worktree root,
  node-local Cargo `target/`, copied child binary, stream stdout/stderr files,
  attempt result file, invocation file, successor ready/completion files, and
  active checkout Cargo `target/` around successor build.
- Cleanup result events at the boundary that attempts cleanup, with subject,
  path, action, result, error category, and bounded before/after byte and file
  counts when cheap enough to measure.
- A cleanup reachability marker for each terminal child path: compile failed,
  child treatment failed, successful child observed, selected successor
  installed, and successor handoff acknowledged/timed out.
- A bounded-resource health sample per parent turn with aggregate node-dir
  bytes, active target bytes, stream-log bytes, orphaned managed worktree
  count, and any configured budget/threshold status.
- Failure classification for cleanup refusals: outside node dir, unmanaged
  path, branch mismatch, git worktree remove failure, missing path, permission
  or IO failure.
- Optional digests or capped excerpts for stream logs only when needed for a
  diagnostic observation; raw logs should remain referenced evidence.

## Natural Recording Surface

- Use one shared tracing-backed JSONL operational event stream, not additional
  domain-specific files. The existing `JournalEntry::Resource` is the closest
  current surface, but its subject set only covers active Cargo `target/`.
- Natural emit points are transition boundaries that already own the relevant
  resource:
  - parent start/complete: active checkout Cargo `target/` sample;
  - child build before/after: node-local `target/` and copied binary sample;
  - child spawn/observe: stream path sample and terminal stream size sample;
  - child cleanup: worktree, node-local target, and copied binary cleanup
    result;
  - successor prepare/build/handoff: active checkout `target/`, successor
    binary path, stream path, invocation, ready/completion sidecars, and child
    cleanup after selected artifact installation.
- Minimal uniform fields: `recorded_at`, `event_kind`,
  `resource_subject`, `phase`, `campaign_id`, `parent_id`, `node_id`,
  `generation`, `runtime_id`, `path`, `status`, `bytes`, `file_count`,
  `action`, `error_kind`, `error`, and `authority_status =
  "telemetry_not_history"`.
- Keep paths as evidence refs and aggregate values as operational facts. Do not
  copy directory listings, full stream logs, or Cargo output into the event.
- If this remains in the transition journal, normalize the event before History
  import. If it moves behind tracing, preserve the same JSONL shape so
  operators can query one stream across domains.

## Essential

- Active checkout Cargo `target/` bytes at parent start and parent complete.
- Child worktree cleanup result for each terminal child evaluation path.
- Node-local Cargo `target/` cleanup result and copied child binary cleanup
  result.
- Successor active-checkout build result with active `target/` byte delta or
  before/after samples.
- Stream stdout/stderr paths plus terminal byte sizes for child/successor
  runtimes.
- Aggregate bounded-resource health per generation: total node-dir bytes,
  total active-target bytes, stream-log bytes, managed child worktree count,
  and threshold status.
- Cleanup refusal/error category with path and owning node/runtime.

## Nice To Have

- File counts alongside byte counts for worktrees, targets, and stream dirs.
- Largest-resource top N per generation for node directories, streams, and
  node-local targets.
- Cargo incremental/deps split under `target/` when diagnosing runaway build
  growth.
- Stream log digest and capped first/last excerpts for failed attempts.
- Cleanup duration and git worktree remove duration.
- Budget policy identity and configured thresholds attached to each health
  sample.

## Too Granular Or Noisy

- Full recursive file listings for `target/`, worktrees, or stream dirs.
- Per-file byte events for Cargo build artifacts, incremental cache files, or
  dependency fingerprints.
- Full stdout/stderr contents for successful runs.
- Per-poll successor-ready checks or repeated "path still exists" events during
  waits.
- Duplicate resource samples at every helper call when no transition boundary
  changed ownership, status, or size materially.
- Treating CLI report output, current filesystem absence, branch names, worktree
  paths, or process ids as authority. They are diagnostics or evidence refs.

## Source Notes

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:160` describes the stable
  active parent root and temporary child worktrees; `:166`-`:177` names cleanup
  of temporary child checkout/build products as part of the intended loop.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:431`-`:434` states that
  temporary child worktrees are cleanup targets and should not become the next
  Parent's long-lived home.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:647`-`:684` lists the
  persisted Prototype 1 files, including transition journal, node records,
  invocation/result sidecars, worktree root, `bin/`, and `target/`.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:731`-`:748` calls out the
  missing first-class attempt record and says cleanup should use an append-only
  observation stream rather than another parallel status document.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:50`-`:62` defines
  History as sealed authority and projections/evidence as non-authoritative;
  resource events must stay on the telemetry side of that boundary.
- `docs/reports/prototype1-record-audit/history-admission-map.md:31` treats
  `transition-journal.jsonl` as append-only transition evidence; `:47` and
  `:75` treat process streams as evidence refs/excerpts, not raw authority.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:214`-`:270` defines
  the current `resource::Sample`: telemetry in the shared transition journal,
  with subject `CargoTarget`, phases `ParentStart`/`ParentComplete`, status,
  bytes, and error.
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs:93`-`:145` shows
  build/spawn records carrying paths and optional stream refs; `:147`-`:152`
  defines stdout/stderr stream paths.
- `crates/ploke-eval/src/cli/prototype1_state/event.rs:110`-`:118` defines the
  path context available on transition records: repo root, workspace root,
  binary path, target relpath, and absolute target path.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3173`-`:3241`
  measures active checkout `target/` bytes and appends the resource sample;
  `:3335`-`:3358` emits parent-start sampling; `:3637`-`:3647` emits
  parent-complete sampling.
- `crates/ploke-eval/src/cli/prototype1_process.rs:439`-`:493` builds the
  successor binary in the active checkout and returns `target/debug/ploke-eval`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:653`-`:714` validates child
  cleanup paths, removes copied child binary and `nodes/<node-id>/target/`, and
  removes the managed child workspace.
- `crates/ploke-eval/src/cli/prototype1_process.rs:856`-`:888` creates
  runtime stream paths and opens stdout/stderr log files.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930`-`:1075` performs
  successor handoff, records binary/invocation/ready/stream paths, and appends
  successor handoff evidence.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1352`-`:1419` isolates child
  Cargo scratch under `node/target/` and copies the runnable child binary to
  `node/bin/ploke-eval`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1728`-`:1745` documents the
  legacy child evaluation shape, including cleanup after result recording;
  `:1808`-`:1810`, `:1880`-`:1882`, and `:1899` show cleanup calls on compile
  failure, treatment failure, and successful evaluation.
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs:836`-`:843` makes
  worktree creation non-destructive and cleanup explicit; `:930`-`:967`
  removes only a managed child worktree that still matches the expected branch.
- `crates/ploke-eval/src/cli/prototype1_state/workspace.rs:24`-`:28` says
  shared records must stay outside git-tracked workspaces; `:50`-`:54` says
  child worktrees are cache-like; `:120`-`:126` models cleanup as pruning one
  child worktree when policy allows.
- `crates/ploke-eval/src/intervention/scheduler.rs:97`-`:101` persists
  node-owned `node_dir`, `workspace_root`, `binary_path`, and runner paths;
  `:242`-`:245` shows node-local `bin/ploke-eval` and result paths.
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:741`-`:773`
  projects resource samples as observation entries with status and bytes.
