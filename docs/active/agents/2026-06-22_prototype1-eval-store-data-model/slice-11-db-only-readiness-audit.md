# Slice 11 DB-Only Readiness Audit

Date: 2026-06-23

Status: DB-only runtime enablement is not ready. Slice 11 completes as an
audit gate only.

## Scope

This audit covers the Slice 11 question from
`slice-by-slice-implementation-plan.md`: whether any Prototype 1 surface can
run DB-only after the Slice 1-10 eval-store mirror work.

The answer is no for production runtime behavior. The eval-store rows are now
useful for query/review evidence, but current production consumers still either
read compatibility files directly or depend on authority domains that must not
be replaced by generic eval-store rows.

## Findings

### Legacy scheduler discovery is already removed from active execution

`resolve_prototype1_candidate_node_id` now fails when `--node-id` is absent and
states that active Prototype 1 execution no longer reads `scheduler.json`,
`branches.json`, or `selection.json` to discover runnable nodes. That removes
one legacy DB-only temptation from the live execution path.

Remaining scheduler compatibility still matters:

- `prototype1_scheduler_path`, `prototype1_node_record_path`,
  `prototype1_runner_request_path`, and `prototype1_runner_result_path` still
  define the compatibility file surfaces.
- `save_scheduler_state` still writes `scheduler.json`.
- `save_node_record` still writes `node.json`.
- `resolve_parent_policy_budget` still falls back to
  `load_scheduler_state(...).policy` when no admitted run profile exists.

Verdict: no DB-only claim for scheduler compatibility. Active candidate
discovery no longer uses it, but fallback policy and compatibility projection
paths still read files.

### Node and runner-result rows are passive refs

`node.json` and `runner-result.json` have explicit authority-negative coverage.
`prototype1_storage_authority_negative_node_ref_cannot_replace_file` writes a
matching DB record ref, removes the file authority, and still requires
`load_node_record` to fail with `ReadManifest`. The corresponding
`prototype1_storage_authority_negative_runner_result_ref_cannot_replace_file`
does the same for `load_runner_result`.

Production recovery also depends on the runner result file:

- terminal child status loads the latest runner result before deriving terminal
  channel evidence;
- stored-child outcome recovery permits a missing runner result only while
  deciding that a stored child has no reusable terminal result;
- durable reconstruction fails when a terminal child is missing
  `runner-result.json`.

Verdict: no DB-only claim for `node.json` or latest `runner-result.json`.

### Branch registry remains compatibility file state

`prototype1_branch_registry_path` resolves to `branches.json`, and
`load_or_default_branch_registry` reads that file line-by-line, returning an
empty default only when the file is absent. Malformed content remains a parse
error.

Verdict: no DB-only claim for branch registry state. A DB-backed branch consumer
would need a production read migration plus negative tests proving DB rows do
not silently tolerate malformed or missing filesystem authority where that
authority is still required.

### Invocation JSON remains executable bootstrap authority

Invocation rows are passive evidence. The authority-negative invocation tests
seed matching DB invocation/attempt rows and still require
`load_executable(&invocation_path)` to fail when the executable invocation file
is absent.

Production recovery also scans invocation JSON files to recover terminal channel
evidence. `terminal_channel_runtime` reads the child invocation directory,
loads child invocation authority from each JSON file, validates the runtime, and
then opens the child channel.

Verdict: no DB-only claim for invocation/bootstrap.

### Channel evidence remains channel authority

`channel_terminal` constructs `Channel::for_role(runtime, endpoints,
FileTransport)` and reads child messages with `recv_from_child`. It rejects
missing endpoints, failed channel reads, and multiple terminal `Result`
messages. Recovery and durable reconstruction both require a valid terminal
channel result that agrees with the runner result.

Verdict: no DB-only claim for channels or channel JSONL.

### Child-plan MessageBox remains typed lock/unlock authority

The child-plan path goes through the typed `MessageBox` system. `MessageBox`
defines exactly one lock transition and one unlock transition for a filesystem
box. `resolve_parent_child_plan_authority` either receives an existing child
plan from that authority path or creates one, then validates child membership,
generation, and parent lineage.

Verdict: no DB-only claim for MessageBox files or child-plan authority.

### Transition journal and sealed History remain dedicated authorities

`historical_traversal_guard` loads entries from the transition journal JSONL.
Sealed History still verifies current artifact tree and current surface through
the History/locator/backend boundary rather than trusting invocation or DB
rows.

Verdict: no DB-only claim for transition-journal replay, sealed History, Crown,
or artifact/worktree mutation.

## Exit Decision

No production runtime surface is enabled DB-only in Slice 11.

This is the correct Slice 11 result because every safe DB-only candidate either:

1. still has production file consumers that have not been migrated; or
2. belongs to a protected authority domain where generic eval-store rows must
   remain passive evidence.

The next useful implementation work is consumer migration/checkpoint parity for
selected compatibility surfaces, not a DB-only mode flag. A future DB-backed
surface may be enabled only after its production reads move to typed rows or
are explicitly classified as filesystem authority, with focused
authority-negative tests for that exact surface.
