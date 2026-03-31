# date: 2026-03-30
# task title: syn_parser RCA - partial parsing with template placeholders
# task description: root-cause analysis for `repro_partial_parsing_with_template_placeholders` (expected failure repro) and fix suggestions (no code edits)
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-29_corpus-triage/2026-03-30_corpus-triage-run-1774867607815.md

## Failure: `repro_partial_parsing_with_template_placeholders`

### Root Cause

`syn_parser` currently attempts to parse *every* discovered `.rs` file under `src/` (and other conventional target dirs), and it treats any per-file `syn` parse failure as an error that bubbles up as `SynParserError::PartialParsing` when there are also successes.

In this repro, `src/template.rs` is intentionally invalid Rust (it contains placeholder tokens `...` and `..` in a function signature), so `syn` fails to parse that file. The other files parse successfully (`src/lib.rs`, `src/ok_one.rs`), so `try_run_phases_and_resolve` returns `SynParserError::PartialParsing { successes, errors }` instead of a fully successful parse.

This matches the corpus triage case it was modeled on (`linera-io/linera-protocol::plugins`), where a non-compilable template-like `.rs` file lives alongside otherwise-parseable modules and triggers a resolve-stage failure.

### Evidence

- Repro injects intentionally invalid syntax (`...` / `..`) into a `.rs` file and expects `SynParserError::PartialParsing`.
  - [/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/partial_parsing.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/tests/repro/fail/partial_parsing.rs)
- Discovery collects all `.rs` files under `src/` (no module-reachability filtering).
  - `collect_rs_files_under` inserts any file with extension `.rs` found by `WalkDir`.
  - [/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/discovery/mod.rs#L485)
- Parse phase maps any `syn` parse error to `SynParserError::Syn` for that `source_path`.
  - `analyze_files_parallel` calls `analyze_file_phase2(..)` and maps error via `SynParserError::syn_parse_in_file`.
  - [/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/parser/visitor/mod.rs#L511)
- Aggregation turns “some ok + some err” into `SynParserError::PartialParsing`.
  - [`try_run_phases_and_resolve_with_target`] returns `PartialParsing` when `successes` is non-empty and `error_list` is non-empty.
  - [/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/lib.rs#L213)
- The error variant is explicitly modeled as an error outcome (not a warning).
  - [/home/brasides/code/ploke/crates/ingest/syn_parser/src/error.rs](/home/brasides/code/ploke/crates/ingest/syn_parser/src/error.rs#L65)

### Suggested Fix / Mitigation (No Edits Made)

1. **More-correct fix (recommended direction): discover and parse only compilation-relevant modules.**
   - Replace (or augment) `collect_rs_files_under`-style “parse everything under `src/`” with a module-reachability walk from root targets (`lib.rs`, `main.rs`, explicit `[[bin]]`, etc.), honoring `mod`, `#[path = \"...\"]`, and directory modules.
   - This avoids treating non-compilable template `.rs` files (that are not part of the compiled module tree) as hard failures, and reduces parse load.
   - Tradeoff: significantly more complex module resolution logic; must be consistent with Rust’s module system to avoid missing real modules.

2. **Policy/config fix: add an explicit “allow partial parsing” mode for ingestion/corpus workflows.**
   - Keep strict default behavior, but allow callers to opt into:
     - continuing with `successes` even if some files fail, while preserving the error list as warnings/diagnostics; or
     - a threshold-based policy (e.g., “fail only if root target file fails” or “fail if >N files fail”).
   - This would require careful UX and invariants: the produced graph would be incomplete by construction.

3. **Heuristic mitigation (least preferred): skip known-template files on parse failure.**
   - For example, when `syn` fails and the file contains obvious template markers (`...`, `{{`, etc.), down-rank it to a warning and continue.
   - Tradeoff: can silently drop genuinely important modules if heuristics misfire; likely violates strictness expectations unless gated behind a mode.

### Confidence

High. The repro is directly exercising the documented `PartialParsing` aggregation behavior, and the discovery phase’s unconditional `.rs` enumeration plus strict per-file parsing makes this outcome deterministic whenever any discovered file is not valid Rust syntax.
