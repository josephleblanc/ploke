# 2026-05-17: Headless Harness Probe Used Primary Gitdir

## Trigger

After a live-ish `prototype1-harness attempt` probe failed on the primary
checkout's shared git worktree state, the user asked why the test was run from
the primary directory at all and pointed out that it should have been set up in
another directory.

## User-visible failure

The agent treated the hidden broad harness replay command as a bounded probe but
ran it against a published request whose candidate workspace and git worktree
metadata were tied back to `/home/brasides/code/ploke/.git`. The probe first
encountered an existing dirty prepared workspace, then required escalation to
create an index lock under the primary checkout's shared `.git/worktrees`
directory.

## Touched Code Surface

- `.codex/skills/prototype1-loop-runtime/SKILL.md`
- `~/.ploke-eval/campaigns/.../prototype1/messages/edit-harness-request/*.json`
- `~/.ploke-eval/campaigns/.../prototype1/workspaces/edit-harness/*`
- `/home/brasides/code/ploke/.git/worktrees/*` through the published request's
  git worktree metadata

No source file edit was needed to trigger the risk. The authority mistake was
in choosing the execution surface.

## What The Agent Did

The agent used the primary checkout's `./target/debug/ploke-eval` and reused an
existing published broad harness request. When the first request slot was dirty,
the agent selected a newer slot. When sandboxing blocked the git index lock, the
agent escalated the same probe instead of stopping to ask whether the request
should be republished or copied into an isolated checkout.

## Skipped Docs / Skills / Instructions

- The Prototype 1 runtime skill did not yet state that broad harness published
  requests are location-bound and may carry gitdir authority back to the primary
  checkout.
- The agent did not apply the repository-level git safety instinct to hidden
  harness replay: a `git reset --hard` inside the request workspace is still a
  sensitive operation when the workspace's gitdir points into the primary repo.
- The agent over-focused on bounded command output and provider reachability,
  while under-checking the workspace and gitdir authority boundary.

## Why This Was Risky

A published harness request is not just an input file. It carries source
repository, candidate workspace, prompt, result, and diagnostics paths. Changing
the shell `cwd` does not isolate those paths. If the candidate is a git worktree
linked to the primary repo, backend preparation can reset or validate the
candidate while writing locks under the primary `.git` directory.

That makes an apparently bounded live probe capable of depending on or
mutating primary repository state. It also weakens the verification story: a
failure may be caused by stale campaign workspace state instead of the
`non_semantic_patch` recovery path being tested.

## Concrete Prevention Rule

For `prototype1-harness attempt` or `sweep`, do not reuse a published request
bound to the primary checkout unless the user explicitly asks to probe that
exact campaign surface.

The default live-ish probe must use an isolated checkout and an isolated
published request whose source repository, candidate workspace, prompt,
submitted result, diagnostics, and gitdir all live under the probe root.
Merely running from a different directory is insufficient.

## Memory Hypothesis

Prior memory correctly emphasized that live tool tests should verify event
payloads instead of model wording, but it did not encode the stronger workspace
authority rule: the request carrier itself determines which repository state is
touched.
