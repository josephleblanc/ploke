# Ploke Book

This directory contains the project mdBook for durable Ploke documentation.
It is intentionally separate from `docs/workflow/evalnomicon`, which remains the evaluation-methodology book.

Build locally with:

```bash
cargo install mdbook --locked
mdbook build docs/book
```

Serve locally with:

```bash
mdbook serve docs/book --open
```

The generated HTML output is written to `docs/book/book/`, which is ignored by git.
