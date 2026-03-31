# date: 2026-03-30
# task title: syn_parser RCA for proc_macro_parsing repro
# task description: root-cause analysis and fix suggestions for the `repro_duplicate_item_placeholder_trait_signatures` expected-failure repro (no code edits)
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md, /home/brasides/code/ploke/docs/design/known_limitations/KL-002-proc-macro-pre-expansion-syntax.md

## Failure: `repro_duplicate_item_placeholder_trait_signatures`

**Root cause**

`syn_parser` requires each source file to parse successfully via `syn::parse_file` before any traversal/skip policy can apply. The repro’s source is “compile-valid” only once the `duplicate` proc-macro attribute (`#[duplicate_item(...)]`) expands it, but the *raw* pre-expansion token payload includes placeholder-oriented constructs that are not valid Rust syntax for `syn`’s file parser.

Concretely, the fixture contains placeholder “call-like” syntax in positions that are not legal pre-expansion Rust (for example, `self: __ref_type([Self])` without a `!` macro invocation), so `syn::parse_file` fails early and the visitor never runs.

**Evidence**

- The repro itself documents this as a gap between toolchain acceptance (with proc-macro expansion) and `syn::parse_file` strictness, and asserts that the error is a `SynParserError::Syn` wrapped into `SynParserError::MultipleErrors`.  
  See [`proc_macro_parsing.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/proc_macro_parsing.rs).
- The parsing pipeline calls `syn::parse_file(&file_content)?;` as the first structured step for each file.  
  See [`visitor/mod.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs).
- This exact case is already documented as a known limitation: `KL-002 Proc-macro pre-expansion syntax`, with `#[duplicate_item(...)]` called out and example failures like `expected ','`.  
  See [`KL-002-proc-macro-pre-expansion-syntax.md`](/home/brasides/code/ploke/docs/design/known_limitations/KL-002-proc-macro-pre-expansion-syntax.md).

Notes:
- `KL-002` currently links to an older repro path (`crates/ingest/syn_parser/tests/repro/duplicate_item.rs`), but the live repro appears to be [`proc_macro_parsing.rs`](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/proc_macro_parsing.rs).

**Suggested fix / mitigation (no implementation here)**

This is an architectural limitation; there is no “small syn tweak” that makes invalid pre-expansion syntax parseable as a `syn::File`.

1. Preferred (architecturally strong): integrate a real macro-expansion pipeline and feed *expanded* Rust into `syn_parser`.
   - Examples: `rustc -Zunpretty=expanded`-style extraction, `cargo expand`-like integration, or a rust-analyzer based expansion path.
   - Tradeoffs: heavy, toolchain-coupled, more moving parts; but preserves correctness and can fully support proc-macro-heavy projects.

2. Targeted workaround (brittle): token-level preprocessing for known proc macros (like `duplicate_item`) to remove or stub the annotated items before calling `syn::parse_file`.
   - This can salvage *other* items in a file that contains one unparseable macro region.
   - Tradeoff: this weakens completeness (drops code) and must be explicit/opt-in to avoid silently creating graph holes.

3. Partial within-file recovery: replace the single `syn::parse_file` step with an “item-by-item” tolerant parser that can skip over a bad item and still visit subsequent items, while surfacing the error.
   - This does not solve cases where the entire file is effectively one bad item (like this minimal repro), but improves real-world usability when only a portion of a file is affected.
   - Tradeoff: nontrivial parsing work; still cannot parse fundamentally invalid constructs into typed `syn` nodes.

Given current correctness guardrails (fail closed, no silent dropping), (1) is the long-term fix; (2) and (3) are feasible mitigations if explicitly accepted as incomplete/opt-in behaviors.

**Confidence**

High. The failure happens before any syn_parser visitor logic due to the unconditional `syn::parse_file` requirement and the presence of proc-macro placeholder syntax that is not valid pre-expansion Rust.
