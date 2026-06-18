# 2026-06-18 Walk CLI Help Audit

Scope: `ploke-eval loop walk` operator/debugger command surface.

## Audit outcome

The `walk` surface already had the core commands needed for both live typestate stepping and read-only historical inspection, but help coverage was uneven:

- top-level help described live server stepping but did not foreground the historical replay workflow;
- `replay`/`back`/`forward` defaulted to a noisy 12-entry recent window;
- `forward --help` had only minimal argument descriptions;
- common safety distinctions were spread across prior operator knowledge rather than visible in CLI help.

## Implemented help coverage

Top-level `ploke-eval loop walk --help` now points to common workflows:

- set context with `walk use`;
- start live walk with `walk start`;
- inspect durable progress with `walk summary -v`;
- enter read-only history with `walk replay --index 0`;
- move the replay cursor with `walk forward --steps N --tail M`;
- live-step with `walk step --until r6`.

It also includes safety notes:

- `replay`/`back`/`forward` are read-only historical cursor commands;
- `step` drives live typestate edges;
- long live edges require `--watch`;
- successor handoff checkout mutation requires `--allow git-changes`;
- `branch-live` writes only explicit provenance and requires `--allow provenance-record`.

Subcommand help was enriched for:

- `walk use`
- `walk show`
- `walk replay`
- `walk back`
- `walk forward`
- `walk branch-live`
- `walk step`
- `walk start`

## Replay recent-window change

Default recent-entry window for historical replay commands is now 3 entries:

```bash
ploke-eval loop walk replay
ploke-eval loop walk forward
ploke-eval loop walk back
```

Operators can expand with:

```bash
ploke-eval loop walk replay --tail 20
ploke-eval loop walk forward --steps 10 --tail 20
```

Rendered replay output now includes an expansion hint when the window is truncated.

## Remaining potential follow-ups

- Structured JSON replay entries instead of a human `message` wrapper.
- Cursor-centered replay windows when `--index` is used.
- Separate `walk inspect node` / `walk inspect selection` surfaces over typed evidence.
- A per-operator persisted replay cursor registry, distinct from the durable transition journal.
