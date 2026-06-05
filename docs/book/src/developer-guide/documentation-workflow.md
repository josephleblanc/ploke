# Documentation Workflow

This mdBook is for durable project documentation.

## Keep separate

- Project mdBook: `docs/book`
- Eval methodology mdBook: `docs/workflow/evalnomicon`
- Active agent handoffs and short-lived working notes: `docs/active/agents`
- Historical agent notes: `docs/archive/agents`

## Build the book

Install mdBook if needed:

```bash
cargo install mdbook --locked
```

Then build:

```bash
mdbook build docs/book
```

## Serve the book locally

```bash
mdbook serve docs/book --open
```

## Update rules

- Prefer stable system descriptions over day-by-day status.
- Link to source paths and durable docs, not transient logs.
- Avoid copying long code blocks that will drift.
- If a design invariant is important for safe edits, document it here and keep implementation-specific handoffs elsewhere.
- When a page becomes stale, fix it or mark the stale boundary explicitly.

## Current scaffold status

This book was reset as a fresh scaffold. The first pass prioritizes navigation, architecture shape, and contributor workflow. Detailed crate API pages and diagrams should be added after source spot-checks.
