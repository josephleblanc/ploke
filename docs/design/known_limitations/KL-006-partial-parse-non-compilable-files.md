# KL-006 Partial parse failure when a crate mixes valid modules with non-compilable sources

## Description

`syn_parser` parses each source file with `syn::parse_file` before visitor logic
runs. If **any** file in a crate fails to parse, the pipeline can surface
`SynParserError::PartialParsing` (or related aggregates) listing successes and
per-file `Syn` errors.

Crates may **ship** template files, examples, or scratch modules that are
**intentionally not valid Rust** (e.g. placeholder `...` / `..` in signatures).
Those files are not buildable by `rustc`, but they sit beside ordinary modules.
Parsing such a crate therefore hits a **hard failure** on the bad file while
others succeed—unlike proc-macro cases ([KL-002](KL-002-proc-macro-pre-expansion-syntax.md)),
where the toolchain may accept raw source that `syn` still rejects.

## Why skipping “bad” files is not automatic

Treating unparsed files as empty would weaken graph completeness and can hide
missing items from downstream analysis. Current policy is **fail closed** on
partial success with an explicit error (see `SynParserError::PartialParsing`
semantics in code).

## Crate-level summary

See [syn_parser known limitations — L4](../syn_parser_known_limitations.md).

## Repro tests (`syn_parser`)

- [`repro_partial_parsing_with_template_placeholders`](../../../crates/ingest/syn_parser/tests/repro/fail/partial_parsing.rs) —
  minimal temp crate: `template.rs` uses invalid placeholder syntax; `ok_one.rs` parses.

## Possible future resolution paths

1. **Explicit allowlist / exclude globs** in parse config for paths known to be
   non-Rust templates.
2. **Per-file recovery** only when explicitly opted in, with markers in the graph
   that the file was skipped.
3. Leave as-is: invalid Rust remains unsupported; callers fix or exclude sources.

## Current policy

- Surface partial parse outcomes explicitly; do not silently omit failed files
  from the model without an explicit policy.
