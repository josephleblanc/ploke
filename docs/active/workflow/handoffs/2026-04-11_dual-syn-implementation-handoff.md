# Handoff: Dual Syn Version Implementation

**Date:** 2026-04-11 (Updated 2026-04-12)  
**Workstream:** A2 Data Fidelity / Parser  
**Gate:** H0 Interpretation (enables A2 validation)  
**Status:** READY FOR RIPGREP TESTING - Conversion layer complete, DRY refactoring done  
**Branch:** Main (commits: `83b97568`, `f5ee41ed`)  

---

## What Was Done

### Problem
`syn` 2.x hard-rejects Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`), blocking evaluation on ripgrep and other Rust 2015 crates.

### Solution Implemented
Dual syn version support with unified processing via conversion layer:
- **syn 1.x** for Rust 2015 edition crates (accepts bare trait objects)
- **syn 2.x** for Rust 2018+ edition crates (default behavior)
- **Conversion layer** syn1→syn2 enables code reuse

### Files Created

| File | Purpose |
|------|---------|
| `code_visitor_syn1.rs` | Syn1 visitor (copied from code_visitor.rs, adapted) |
| `attribute_processing_syn1.rs` | **NEW:** Thin adapter - converts and delegates to attribute_processing.rs |
| `type_processing_syn1.rs` | **NEW:** Thin adapter - converts and delegates to type_processing.rs |

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added `syn1` dependency |
| `mod.rs` | Edition-based dispatch logic, `analyze_file_phase2_syn1()` |
| `utils.rs` | **COMPLETE:** syn1→syn2 conversion functions for types, attributes |
| `error.rs` | Added `Syn1ToSyn2AttributeConversion` error variant |
| `type_processing_syn1.rs` | **REFACTORED:** 173 lines → 21 lines (convert + delegate) |
| `attribute_processing_syn1.rs` | **REFACTORED:** 279 lines → 75 lines (convert + delegate) |

### Architecture

```
Rust 2015: syn1::Type → convert_type_syn1_to_syn2() → syn::Type → type_processing::get_or_create_type()
Rust 2018+: syn::Type → type_processing::get_or_create_type()

Rust 2015: syn1::Attribute → convert_attribute_syn1_to_syn2() → syn::Attribute → attribute_processing::*
Rust 2018+: syn::Attribute → attribute_processing::*
```

### DRY Refactoring Results

| Component | Before | After | Saved |
|-----------|--------|-------|-------|
| `type_processing_syn1.rs` | 173 lines | 21 lines | ~150 lines |
| `attribute_processing_syn1.rs` | 279 lines | 75 lines | ~200 lines |
| **Total** | **452 lines** | **96 lines** | **~350 lines** |

### Completed Conversions (utils.rs)

- ✅ `Type` variants (Path, Reference, Slice, Array, etc.)
- ✅ `Path`, `PathSegment`, `PathArguments`
- ✅ `GenericArgument` (including `Const(Expr)` via token roundtrip)
- ✅ `TypeParamBound`, `TraitBound`
- ✅ `ReturnType`, `BoundLifetimes`
- ✅ `Abi`, `Macro`, `MacroDelimiter`
- ✅ `AttrStyle`, `Attribute`

### Expr Handling

Both `Type::Array` and `GenericArgument::Const` use token stream roundtrip:
- syn1 → TokenStream (preserves text) → parse as syn2
- Handles array lengths `[T; N]` and const generics `<T, N>`

---

## Test Results

- **All 378 tests pass** ✅
- **Edition 2015 bare trait objects parse successfully** ✅ (unit tests)
- **Edition 2015 async identifiers parse successfully** ✅ (unit tests)

**Commits:**
- `83b97568` - wip: dual syn version support
- `f5ee41ed` - wip: refactor attribute_processing_syn1 to convert and delegate

### Ripgrep Dataset Test - INCONCLUSIVE

Ran `BurntSushi__ripgrep-1294` with fresh indexing:
- Indexing status: "completed" (per `indexing-status.json`)
- `globset` code appears in RAG context (log shows snippets from `globset/src/lib.rs`)
- **BUT**: Cannot verify all 9 crates indexed without introspection capability
- **BLOCKED**: `turn.db_state().lookup()` / `replay_query()` not implemented (Phase 1 gap)
- **BLOCKED**: No method to query `*crate_context` from RunRecord or validate parse coverage

**Parse failure artifact exists** at `~/.ploke-eval/runs/BurntSushi__ripgrep-1294/parse-failure.json` but may be from cached/previous run.

---

## Next Step: Validation Capability

**Problem:** Cannot validate dual-syn parsing on ripgrep because introspection API is incomplete.

**Missing:** `turn.db_state().lookup()` and `replay_query()` were Phase 1 deliverables but not implemented.

**What we need:**
1. Method to query indexed crates from a run (via RunRecord or direct DB query)
2. Way to verify `globset` (Rust 2015) parsed successfully vs failed silently
3. Parse error diagnostics that are capture-time fresh, not cached

**Options:**
- A: Add `indexed_crates()` method to RunRecord (captures at indexing time)
- B: Implement `replay_query(turn, query)` with DB path (queries at introspection time)
- C: Quick CLI tool to query `*crate_context` from Cozo DB directly

**Current state:** Implementation done, validation blocked on introspection gap.

---

## Recovery Info

**Commits to revert if issues:**
```bash
git revert f5ee41ed  # attribute_processing refactor
git revert 83b97568  # dual syn support
```

**Key files:**
- `utils.rs` - Type/attribute conversion functions
- `type_processing_syn1.rs` - Thin adapter (21 lines)
- `attribute_processing_syn1.rs` - Thin adapter (75 lines)

---

## Related Documents

- [Bug Report: syn 2.x fails on Rust 2015 bare trait objects](../../bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
- [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md)
