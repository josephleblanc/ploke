# Setup Continued Past Commit Request

## Trigger

The user said, "Ok. I did ask you to commit first. but fine, whatever, just
setup the worktree again. I deleted it, and I committed all local changes so
we're clean."

## User-visible failure

The agent treated "also commit" as secondary context while continuing the
Prototype 1 setup flow. The setup completed, but the user had asked for the
commit boundary first.

## Touched code surface

- Prototype 1 setup workflow and parent worktree creation.
- Local operator profile and ignored eval-home size log.
- No tracked repo source file was intentionally changed before setup.

## What the agent did

The agent created the run profile, ran setup, built the new worktree runtime,
and reported that ignored/local-only surfaces were not committed. It did not
stop when the user asked to commit, check the staged/tracked delta, and either
commit the scoped changes or explain that only ignored/local setup surfaces
existed.

## Skipped docs / skills / instructions

- The Git Safety rule that ignored files are local-only unless explicitly
  requested was followed, but the user-facing sequencing request was not.
- The Prototype 1 setup workflow was followed, but it was allowed to outrun a
  direct commit request.

## Why this was risky

Prototype 1 setup creates branches, campaign artifacts, and parent identity
commits. Continuing past a requested commit boundary makes it harder for the
user to tell which changes are durable repo commits, which are local operator
state, and which setup artifacts belong to a now-deleted worktree.

## Prevention rule

If the user asks to commit during an active setup or implementation flow, stop
before the next mutating operation. Run `git status --short`, identify tracked
versus ignored/local-only changes, commit the exact tracked scope when present,
and only then resume setup. If there is nothing committable without force-adding
ignored files, say that explicitly before proceeding.

## Memory hypothesis

Prior setup memory emphasized getting to a verified parent worktree and avoiding
force-adding ignored files. It did not encode the stronger sequencing rule:
commit requests are a barrier before further setup mutations.
