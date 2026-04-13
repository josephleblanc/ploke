# date: 2026-03-30
# task title: syn_parser repro RCA summary
# task description: summarize the root causes and follow-up recommendations for the `repro::fail` expected-failure cases
# related planning files: /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/2026-03-30_syn_parser_repro_rca-plan.md, /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_manifest_errors_report.md, /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_proc_macro_parsing_report.md, /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_partial_parsing_report.md, /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_cfg_gates_report.md, /home/brasides/code/ploke/docs/active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_file_links_report.md

## Overall Finding

- `cargo test -p syn_parser --test mod repro::fail` completed successfully.
- The `repro::fail::*` suite is a set of expected-failure assertions, so the interesting work was the root-cause analysis behind each scenario, not the test harness status itself.
- The 11 expected-failure repros collapse into 5 root-cause clusters.
- `repro::fail::cfg_gates::fixture_duplicate_cfg_test_mods_is_valid_rust` is a positive-control test and was not part of the failure analysis.

## Root Causes and Follow-Ups

### 1. Manifest discovery is too strict about Cargo-defaulted fields

- Cases:
  - `repro_workspace_package_missing_version_manifest_parse_error`
  - `repro_bin_target_missing_path_manifest_parse_error`
- Report: [manifest_errors_report.md](2026-03-30_manifest_errors_report.md)
- Summary:
  - The manifest schema requires fields that Cargo normally defaults or inherits.
  - Discovery fails early with `toml::from_str` instead of applying Cargo-compatible defaults.
- Follow-up:
  - Make the affected fields optional in the schema.
  - Apply inheritance/defaulting during discovery resolution instead of at deserialize time.

### 2. Raw `syn::parse_file` cannot accept placeholder-style pre-expansion syntax

- Case:
  - `repro_duplicate_item_placeholder_trait_signatures`
- Report: [proc_macro_parsing_report.md](2026-03-30_proc_macro_parsing_report.md)
- Summary:
  - The parser is operating on raw source before expansion.
  - Placeholder-oriented syntax is not valid `syn::File` input, so parse failure is expected.
- Follow-up:
  - Prefer macro expansion integration before parse.
  - If that is not feasible, add explicit preprocessing or an opt-in recovery path for this syntax class.

### 3. Discovery is over-inclusive for partial-parsing scenarios

- Case:
  - `repro_partial_parsing_with_template_placeholders`
- Report: [partial_parsing_report.md](2026-03-30_partial_parsing_report.md)
- Summary:
  - Every `.rs` file under `src/` is parsed, including template-like files that are not compilation-relevant.
  - One invalid discovered file turns the run into `SynParserError::PartialParsing`.
- Follow-up:
  - Move to module-reachability-based discovery from real targets.
  - If partial parsing is intentionally allowed, make that an explicit policy mode rather than the default behavior.

### 4. cfg-gated duplicates are detected before the tree can be pruned or cfg-resolved

- Cases:
  - `repro_duplicate_quantized_metal_mod_merge_error`
  - `repro_duplicate_cfg_gated_module_merge_error`
- Report: [cfg_gates_report.md](2026-03-30_cfg_gates_report.md)
- Summary:
  - File-based modules are indexed even when unreachable because inline modules exist.
  - Duplicate detection runs before pruning.
  - cfg alternatives are not modeled robustly enough to prevent false collisions.
- Follow-up:
  - Defer duplicate detection until after unlinked/pruned modules are removed.
  - Extend cfg evaluation enough to cover the target atoms used by these cases, or treat cfg alternatives as mutually exclusive during merge.

### 5. File-link normalization is too broad and collides with inline module definitions

- Cases:
  - `repro_duplicate_inline_protos_module_merge_error`
  - `repro_duplicate_cli_binary_module_merge_error`
  - `repro_duplicate_scheduler_queue_mod_merge_error`
  - `repro_duplicate_logging_inline_file_mod_merge_error`
  - `repro_duplicate_image_inline_file_mod_merge_error`
- Report: [file_links_report.md](2026-03-30_file_links_report.md)
- Summary:
  - `logical_module_path_for_file` is normalizing `main.rs` too broadly.
  - File-based modules are inserted before pruning, so they collide with inline definitions that should shadow them.
- Follow-up:
  - Restrict `main.rs` special-casing to actual crate roots.
  - Reorder the pipeline so shadowed file-based modules are removed before duplicate-path checks.

## Conclusion

- The repro suite points to three classes of parser/schema issues and two module-tree/discovery ordering issues.
- The highest-value next changes are:
  - schema defaulting for manifests,
  - reachability-based discovery,
  - and a merge pipeline that respects cfg/shadowing before duplicate enforcement.
