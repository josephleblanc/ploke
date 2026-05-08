# Edit Surface Backend Bridge Review

Date: 2026-05-07

Task title: Review backend bridge slice for bounded edit-surface candidate generation

Task description: Review the current uncommitted backend bridge changes for
single-file bounded edit-surface candidate validation, span folding, rejection
coverage, backend-owned after-hash validation, surface commitment for
`ploke-tui` tools, conversion into current child-candidate carriers, and
commit readiness while the live TUI proposal producer is still absent.

Related planning files:

- `AGENTS.md`
- `docs/active/agents/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/active/agents/2026-05-07-edit-surface-phase3-integration-slice.md`
- `docs/active/agents/2026-05-07-edit-surface-phase2-authority-review.md`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`
- `crates/ploke-eval/src/intervention/mod.rs`

## Findings

### Blocker: raw relpaths can escape the declared edit surface

`GitWorktreeBackend::validate_edit_surface_candidate` validates containment
with `is_allowed_edit_surface_path(proposal.surface, &target_relpath)` before
reading `repo_root.join(&target_relpath)`. For `PlokeTuiTools`,
`is_allowed_edit_surface_path` accepts any path whose raw components start with
`crates/ploke-tui/src/tools`.

That is not a containment proof. A proposal relpath like:

```text
crates/ploke-tui/src/tools/../../../ploke-eval/src/lib.rs
```

has the allowed prefix lexically but resolves outside the `ploke-tui` tool
surface. The backend would then read and hash that resolved file, build the
base/derived text-file artifact ids from the attacker-supplied relpath string,
construct `surface::Artifact` entries keyed by the same relpath, and pass the
TUI apply validation against those internally consistent but out-of-surface
keys.

Relevant code:

- `backend.rs:814-830` deduplicates raw proposal relpaths and runs the surface
  check.
- `backend.rs:832-839` reads `repo_root.join(&target_relpath)` after that raw
  check.
- `backend.rs:1723-1729` performs the raw `starts_with` surface check.
- `backend.rs:857-863` derives artifact refs from the unchecked relpath and
  folded text content.

This should be fixed before commit by rejecting absolute paths, `ParentDir`
components, Windows prefixes, and other non-normal relpaths before any
containment or filesystem read. A normalized/canonical containment check can be
added too, but canonicalization alone is awkward for replacement targets that
may not exist in future paths. The durable rule should be: proposal relpaths are
repository-relative normal paths, and only then may they be matched to the
surface allowlist.

Existing tests cover a plainly out-of-surface path, but not a prefixed path
with `..` components.

### Medium: backend-owned after validation is single-file text-owned, not whole-Artifact materialization

The previous phase blocker said the live path must materialize the proposed
edit into a derived Artifact checkout, compute the derived Artifact identity
from that checkout, compute touched file hashes from the derived checkout
contents, construct the post-apply `surface::Artifact` from backend-owned
evidence, and pass it into `tui::Apply::validate`.

This slice does something narrower and mostly correct for a single-file bridge:

- reads the current source file from the backend checkout;
- validates every touch's expected base hash against that read content;
- folds the replacements itself;
- computes proposed content hash, text-file base/derived artifact ids, and
  patch id from backend-owned strings;
- constructs `after_artifact` from the folded content hash;
- compares any reported executor after hash against that backend-owned
  `after_artifact` through `tui::Apply::validate`.

Relevant code:

- `backend.rs:832-860` reads source content, validates stale base hashes, folds
  replacements, and derives ids.
- `backend.rs:978-989` treats `reported_after_file_hash` as executor report
  evidence while constructing the after Artifact from the backend-computed
  folded content hash.
- `backend.rs:990-1003` requires `tui::Apply::validate(&after_artifact)` before
  extracting `ArtifactDelta`.

This is acceptable as a bounded single-file text bridge. It should not be
described as whole-checkout or History-ready Artifact materialization. The
current text-file id helpers explicitly say they are fallback text-surface
identities, not durable whole-worktree Artifact ids.

### Medium: multi-touch span folding is structurally sound but missing edge tests

The span folding path sorts touches by `(start, end)`, rejects invalid byte
ranges and non-char-boundary spans, rejects overlaps, and applies replacements
from the end of the source string toward the beginning. That preserves original
source offsets across multiple non-overlapping edits.

Relevant code:

- `backend.rs:841-852` sorts touches and checks stale base hashes.
- `backend.rs:1736-1769` validates byte ranges, char boundaries, and overlap.
- `backend.rs:1772-1777` folds replacements in reverse order.

The current tests cover one accepted touch and overlapping spans. They do not
cover multiple non-overlapping same-file touches, adjacency (`end == next_start`),
unsorted input, or non-ASCII char-boundary rejection. Those are not blockers for
the slice, but the bridge is new enough that these tests should be added before
calling the folding behavior complete.

### Medium: conversion to existing child carriers does not persist edit-surface evidence

`child_files_from_checked_edit` correctly refuses surface mismatch and target
mismatch, then projects a checked single-file edit into the current
`ResolvedTreatmentBranch` / `ChildFiles` shape. It fills the node's
`operation_target`, `base_artifact_id`, `patch_id`, and `derived_artifact_id`,
and it records proposed content/hash on the branch.

Relevant code:

- `cli_facing.rs:476-539` performs the conversion.
- `cli_facing.rs:483-493` rejects surface and target mismatch.
- `cli_facing.rs:496-529` fills node and branch artifact/patch fields.

This is a compatibility projection, not durable edit-surface evidence. The
resulting `ChildFiles` still does not carry `surface::Check`, `ArtifactDelta`,
grant/bounds digests, proposal run evidence, or touched spans as first-class
History records. That is acceptable only because the live `tui-edit-surface`
path remains fail-closed. Before live admission, History needs an explicit
record for the checked surface transition rather than relying on these legacy
branch fields.

### Non-Issue: live missing TUI producer is handled safely

The CLI path still fails before publishing a `ChildPlan` for
`candidate-generator=tui-edit-surface`, now with the more precise reason that no
TUI proposal producer is wired yet. It does not fall back to legacy generation
and does not fake checked candidates.

Relevant code:

- `cli_facing.rs:442-447` defines `MissingTuiProposalProducer`.
- `cli_facing.rs:669-680` returns that error for the live TUI edit-surface
  selection path.
- `cli_facing.rs:8330-8342` tests the fail-closed error.

Once the raw-relpath containment issue is fixed, the absence of a live TUI
producer should not block a narrow backend-bridge commit. It is correctly
represented as an unavailable producer, not as successful candidate generation.

### Non-Issue: surface commitment now represents the `ploke-tui` tool surface

The surface commitment now includes tool description artifacts, the documented
`ploke-tui` RAG tool files, and tracked files under
`crates/ploke-tui/src/tools`. That makes mutation of
`crates/ploke-tui/src/tools/code_edit.rs` visible in the mutable surface hash.

Relevant code:

- `backend.rs:1700-1720` builds `mutated_surface_paths`.
- `backend.rs:2305-2324` tests that a `ploke-tui` tool mutation changes the
  surface commitment.

This is the right direction for the requested `ploke-tui-tools` surface. It is
still pathspec/list based, not a typed `SurfaceGrant` persistence model, but it
does not regress the current commitment surface.

### Non-Issue: `Apply::delta` accessor preserves the sealed transition shape

`tui::Apply` was already patched after the Phase 2 review into an opaque struct
with private state. The new `delta()` accessor only returns a delta for the
private `Applied` state and returns `None` for `Reported` or `Rejected`.

Relevant code:

- `edit_surface/tui.rs:755-772` exposes state predicates and `delta()`.
- `backend.rs:990-1003` only extracts a delta after `from_results(...).validate(...)`.

This does not reopen the old `Reported` construction hole.

## Naming And Structure

The new bridge names are mostly local and concrete: `EditProposal`,
`ProposedTouch`, and `CheckedSurfaceEdit` are not Prototype 1 role/state names,
and they sit inside the backend boundary rather than pretending to be protocol
states. `CheckedSurfaceEdit` is a reasonable compatibility carrier for the
single-file bridge.

The structural risk is not identifier length; it is boundary placement. The
bridge currently builds a mock graph, TUI projection, surface grant, staged
proposal, apply result, and legacy child projection in one backend method. That
is acceptable as a narrow bridge slice, but it is a future blob risk. If this
expands beyond single-file candidate validation, split it around the real
domain steps:

- normalize and admit proposal relpaths;
- resolve touches into a bounded surface check;
- realize backend-owned after evidence;
- project checked deltas into legacy child files.

## Test Evidence

Commands run:

```text
cargo test -p ploke-eval edit_surface_bridge --lib 2>&1 | tail -n 30
cargo test -p ploke-eval checked_edit_surface_candidate_converts_to_single_file_child_files --lib 2>&1 | tail -n 30
cargo check -p ploke-eval 2>&1 | tail -n 80
```

Results:

- `edit_surface_bridge`: passed, 7 tests.
- `checked_edit_surface_candidate_converts_to_single_file_child_files`: passed,
  1 test.
- `cargo check -p ploke-eval`: passed with existing warnings.

## Commit Recommendation

Do not commit this slice as-is. The live missing TUI proposal producer is not
the problem; the current path correctly fails closed there. The blocker is the
raw-relpath containment hole in the backend validator.

After adding normal repository-relative path validation and a regression test
for a prefixed `..` path that escapes `crates/ploke-tui/src/tools`, this should
be safe to commit as a narrow backend bridge with an explicit limitation:
single-file text-surface validation/conversion exists, but live TUI candidate
production and durable History admission of checked edit-surface evidence are
still pending.

## Follow-Up Patch

After this review, the raw-relpath containment blocker was patched:

- edit proposal paths must now be normal repository-relative paths;
- absolute paths, root/prefix components, `.`, and `..` are rejected before
  surface allowlist checks or filesystem reads;
- a regression test covers a path that lexically starts with
  `crates/ploke-tui/src/tools` and then escapes with `..`.

Validation after the patch:

```text
cargo fmt --all
cargo test -p ploke-eval edit_surface_bridge --lib 2>&1 | tail -n 30
cargo test -p ploke-eval prototype1_state --lib 2>&1 | tail -n 20
cargo check -p ploke-eval 2>&1 | tail -n 80
```

Results:

- `edit_surface_bridge`: passed, 8 tests.
- `prototype1_state`: passed, 187 tests.
- `cargo check -p ploke-eval`: passed with existing warnings.
