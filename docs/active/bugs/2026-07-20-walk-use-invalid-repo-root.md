# Walk Use Persists a Non-Existent Repository Root

Status: fixed and live verified.

## Broken Contract

`ploke-eval loop walk use` must reject a root that does not exist or is not a
directory. It must never replace a valid saved context with an unusable path.

## Evidence

The saved context at `/home/brasides/.ploke-eval/walk/context.json` contained:

```json
{
  "schema_version": "walk-context.v1",
  "repo_root": "p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018",
  "socket": "/run/user/1000/ploke-eval/walk/p1walk-36adacc97be848b7.sock"
}
```

That `repo_root` is a campaign identifier, not a path beneath the working
directory. Short commands consequently target a non-existent relative root and
time out while trying to start a server at the saved socket.

## Source Trace

`walk use <campaign-id>`
-> client `use_context`
-> `paths::resolve_use_repo_root`
-> `normalize_path`
-> failed canonicalization returns the original relative path
-> `save_context` persists it.

The shared resolver also serves driver paths and is therefore not the safe
repair boundary for this CLI-only admission error.

## Docs and Operator Expectation

`walk use` is the operator shortcut that makes later commands safe and concise.
Its saved root must be an existing repository checkout. A typo or campaign ID
should fail before any context file is changed.

## Repro Coverage

Added with the repair:

- a command-boundary regression rejecting a missing root;
- a command-boundary regression rejecting a regular file;
- acceptance of an existing directory.

## Implemented Repair

Validate the resolved root in the low-risk `use_context` command boundary before
computing its socket or saving context. Leave the shared
`resolve_use_repo_root` behavior unchanged for driver callers.

## Live Validation

Passing the V29 campaign identifier as a relative root now exits with an error
that the walk context root must be an existing directory. The existing context
file's SHA-256 did not change on rejection.

Passing the absolute V29 parent checkout succeeds and saves:

```text
/home/brasides/.ploke-eval/setup-seeds/p1-v29-strictpatchhandoff-mbe-g35f-direct-3g1x3-p3-obs2400-ptok8k-20260719-001018
```

Subsequent short-form walk commands used its repo-hashed socket and reached the
same protocol-v11 read-only server.
