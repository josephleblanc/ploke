## Overall assessment

Implementing `cfg_attr` evaluation in `syn_parser` is the right next step, but it has wider blast radius than just attribute parsing. In this codebase, `cfg` filtering, raw cfg-string capture, module `#[path]` resolution, and workspace discovery are tightly coupled. If `cfg_attr` is added only in one place (for example, only for `#[path]` extraction), behavior will stay inconsistent and likely produce graph-level correctness bugs.

Safety/risk: **medium-high** as currently scoped.  
Main reason: the parser currently makes inclusion decisions and node metadata from *different attribute views* (raw `cfg`, raw `cfg_attr`, and extracted non-cfg attrs). Adding `cfg_attr` support requires introducing one canonical “effective attributes” pass and reusing it across all item visitors, plus tests for `mod` path behavior and `cfg(test)` interactions.

## Inline-style comments on impacted hunks

- **ID C1**
  - **Location:** `crates/ingest/syn_parser/src/parser/visitor/attribute_processing.rs` (`should_include_item`, `extract_cfg_strings`, `extract_attributes`)
  - **Severity:** high
  - **Comment:** `should_include_item` evaluates only explicit `#[cfg(...)]`, while `extract_attributes` excludes only `cfg` but keeps `cfg_attr` opaque. This creates split semantics: gating decisions ignore `cfg_attr`, and metadata captures unresolved `cfg_attr` text. Required change: compute effective attrs first (expand active `cfg_attr` into synthetic attrs, drop inactive), then run both inclusion and extraction from that same result.

- **ID C2**
  - **Location:** `attribute_processing.rs` (`should_include_item` special-case for `cfg(test)`)
  - **Severity:** high
  - **Comment:** special-casing `cfg(test)` to force include works only for direct `#[cfg(test)]`. Equivalent forms like `#[cfg_attr(test, cfg(test))]` will currently bypass this behavior. If this asymmetry remains, test code presence will depend on syntax shape, not semantics.

- **ID C3**
  - **Location:** `crates/ingest/syn_parser/src/parser/nodes/mod.rs` (`extract_path_attr_from_node`) + `crates/ingest/syn_parser/src/resolve/module_tree.rs` (`resolve_pending_path_attrs`)
  - **Severity:** critical
  - **Comment:** module-tree path resolution only looks for concrete `path` attributes already stored on the `ModuleNode`. A `cfg_attr(..., path = "...")` never materializes as `path`, so declarations won’t link to file definitions. This directly affects module graph correctness and import resolution downstream.

- **ID C4**
  - **Location:** `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs` (`visit_item_mod` and all `visit_item_*` cfg checks)
  - **Severity:** medium
  - **Comment:** each visitor calls `should_include_item` independently and then separately extracts cfg strings/attrs. Introducing `cfg_attr` ad hoc in only one visitor (common pitfall: modules) will cause inconsistent behavior across node types. Required: central helper returning `{included, effective_cfgs, effective_attrs}` reused uniformly.

- **ID C5**
  - **Location:** `crates/ingest/syn_parser/src/discovery/mod.rs` (`run_discovery_phase`)
  - **Severity:** high (scope/design)
  - **Comment:** test-only Cargo targets (packages without `src/`) fail discovery with `SrcNotFound` when no files were collected. `cfg_attr` work won’t fix this category. If the goal includes “test target behavior parity,” discovery must also enumerate test target roots (`[[test]]` and `tests/*.rs`) and include them in crate files.

- **ID C6**
  - **Location:** `crates/ingest/syn_parser/src/parser/visitor/cfg_evaluator.rs`
  - **Severity:** medium
  - **Comment:** evaluator supports limited atoms; unsupported atoms evaluate false. Once `cfg_attr` is evaluated, more attrs may become conditionally active/inactive under unsupported atoms, increasing false-negative pruning. At minimum, this should be explicitly tested and surfaced in diagnostics to avoid silent graph shrinkage.

- **ID C7**
  - **Location:** `analyze_file_phase2` in `crates/ingest/syn_parser/src/parser/visitor/mod.rs`
  - **Severity:** medium
  - **Comment:** file-level cfg handling (`#![cfg(...)]`) is separate from item attribute handling. If file-level `cfg_attr` is in scope (e.g., crate-level attrs), you’ll need clear policy: either support it in phase-2 effective CFG tracking or explicitly defer with tests guarding current behavior.

## Tests & verification to add/update

- `cfg_attr_enables_cfg_gate_on_item`
  - Scenario: `#[cfg_attr(feature = "x", cfg(feature = "y"))] fn ...`
  - Verify inclusion changes with active features and matches direct `#[cfg(...)]` semantics.

- `cfg_attr_test_equivalent_to_cfg_test`
  - Scenario: compare `#[cfg(test)]` vs `#[cfg_attr(test, cfg(test))]`
  - Verify both paths produce same include behavior under current `cfg(test)` policy.

- `cfg_attr_path_on_mod_declaration_resolves_definition`
  - Scenario: `#[cfg_attr(..., path="...")] mod m;`
  - Verify `CustomPath` relation is created when condition is true, absent when false.

- `cfg_attr_multiple_attrs_materialize_in_order`
  - Scenario: one `cfg_attr` expands to multiple attrs (`allow`, `path`, etc.)
  - Verify extraction + resolution sees all effective attrs deterministically.

- `discovery_includes_test_only_package_targets` (if target-level parity is desired)
  - Scenario: crate with only `tests/*.rs` and no `src/`
  - Verify workspace parse doesn’t hard-fail and target files are discoverable.

- Regression updates around existing path/cfg fixtures:
  - `tests/uuid_phase3_resolution/path_attribute.rs`
  - `tests/full/mock_serde_parse.rs`
  - `tests/uuid_phase1_discovery/discovery_tests.rs`

## Summary of must-fix vs nice-to-have

**Must-fix before merge:**
- **C1** Single canonical effective-attribute evaluation used by include + extraction.
- **C2** Preserve `cfg(test)` semantics for syntactically equivalent `cfg_attr` forms.
- **C3** Make `cfg_attr(..., path=...)` visible to module path resolution.
- **C4** Apply uniformly across all visited item kinds, not only modules/functions.
- **C5** (if in scope of this effort) handle test-only Cargo targets in discovery; otherwise explicitly document as out-of-scope in PR.

**Nice-to-have:**
- **C6** Better diagnostics for unsupported cfg atoms during evaluation.
- **C7** Clarify and test file-level `cfg_attr` policy (supported vs intentionally deferred).

If useful, I can also draft a concrete “minimal safe implementation order” (3-4 commits) that reduces risk while keeping behavior stable.