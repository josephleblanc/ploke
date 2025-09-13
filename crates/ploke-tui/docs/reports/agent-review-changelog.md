# Agent Review Change-Log

- 2025-09-13T20:37:28Z — 8836d215
  - Scope: `model_browser.rs` migrated to strict types (`ModelId`, `ModelName`).
  - UI: Title rendering updated to use typed IDs and names; fallback to `id.to_string()` when name empty/absent.
  - Pricing: Leaves numeric `f64` (`input_cost`/`output_cost`) but multiplies by 1e6 for display; recommend adding unit label and considering strong newtypes for currency units.
  - Docs: Found incomplete doc comment for `input_cost`; recommended fix.
  - Formatting: `rustfmt --check` suggests small tidy-ups (imports order, whitespace).
  - Build: `cargo check -p ploke-tui --features "test_harness llm_refactor"` → 27 errors, 10 warnings (unrelated broader refactor).
