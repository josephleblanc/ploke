# Handoff: Dual Syn Version Implementation

**Date:** 2026-04-11 (Updated 2026-04-12)  
**Workstream:** A2 Data Fidelity / Parser  
**Gate:** H0 Interpretation (enables A2 validation)  
**Status:** IN PROGRESS - syn1→syn2 type conversion ongoing  
**Branch:** Main (ready for commit)  

---

## What Was Done

### Problem
`syn` 2.x hard-rejects Rust 2015 bare trait objects (e.g., `Arc<Fn(...)>`), blocking evaluation on ripgrep and other Rust 2015 crates.

### Solution Implemented
Dual syn version support:
- **syn 1.x** for Rust 2015 edition crates (accepts bare trait objects)
- **syn 2.x** for Rust 2018+ edition crates (default behavior)

### Files Created

| File | Purpose |
|------|---------|
| `crates/ingest/syn_parser/src/parser/visitor/code_visitor_syn1.rs` | Syn1 visitor (copied from code_visitor.rs, adapted) |
| `crates/ingest/syn_parser/src/parser/visitor/attribute_processing_syn1.rs` | Syn1 attribute helpers |
| `crates/ingest/syn_parser/src/parser/visitor/type_processing_syn1.rs` | Syn1 type processing |

### Files Modified

| File | Changes |
|------|---------|
| `Cargo.toml` | Added `syn1 = { package = "syn", version = "1.0", features = ["full", "visit"] }` |
| `mod.rs` | Added module declarations, edition check, dispatch logic, `analyze_file_phase2_syn1()` function |
| `tests/repro/fail/edition_2015_bare_trait_object.rs` | Converted from fail test to success test |
| `tests/repro/fail/edition_2015_async_identifier.rs` | Converted from fail test to success test |
| `src/parser/utils.rs` | **IN PROGRESS:** Adding syn1→syn2 type conversion functions |
| `src/error.rs` | Added `Syn1ToSyn2AttributeConversion` error variant |

### Key Implementation Details

**Dispatch logic** in `analyze_file_phase2()`:
```rust
let edition = crate_effective_edition_inner(crate_context);
if edition == Some(cargo_toml::Edition::E2015) {
    return analyze_file_phase2_syn1(...);
}
// Continue with syn2 path
```

**Syn1 adaptations made:**
- `attr.path()` → `attr.path` (field not method)
- `attr.meta` → `attr.parse_meta()` (method call)
- `MetaList.tokens` → `MetaList.nested` (pre-parsed in syn1)
- `MetaNameValue.value` → `MetaNameValue.lit` (literal only in syn1)
- `StaticMutability::Mut(_)` → direct `Option<Mut>` check
- `ImplItem::Fn` → `ImplItem::Method`
- `TraitItem::Fn` → `TraitItem::Method`

---

## What's In Progress: Syn1→Syn2 Type Conversion

**Goal:** Create conversion functions in `parser/utils.rs` to convert syn1 types to syn2 types, enabling code reuse between syn1 and syn2 visitors.

**Approach:**
- Implement `convert_type_syn1_to_syn2()` and related conversion functions
- Skip types containing `Expr` (too complex to convert)
- Add proper error handling with `CodeVisitorError::Syn1ToSyn2AttributeConversion`

**Completed conversions:**
- ✅ `Type` variants (Path, Reference, Slice, etc.)
- ✅ `Path`, `PathSegment`, `PathArguments`
- ✅ `GenericArgument` (partial - `Expr` skipped)
- ✅ `TypeParamBound`, `TraitBound`
- ✅ `ReturnType`
- ✅ `BoundLifetimes`
- ✅ `Abi`
- ✅ `Macro`, `MacroDelimiter`
- ✅ `AttrStyle`
- ✅ `Attribute` (with Result-returning conversion)

**Remaining issues:**
- 🔄 `Expr` in `GenericArgument::Const` - skipped
- 🔄 `Pat` in `BareVariadic` - using placeholder
- 🔄 `Attribute` clones in various places - need conversion or skipping
- 🔄 `AssocType` and `Constraint` struct field mismatches

**Known compilation errors:**
```
error[E0308]: GenericArgument::Const(expr) - expected syn::Expr, found syn1::Expr
error[E0560]: AssocType has no field named `gen_args` (syn2 uses `generics`)
error[E0063]: missing field `generics` in Constraint
error[E0308]: expected syn::Attribute, found syn1::Attribute
```

---

## Test Results

- **All 378 original tests pass** (before conversion work)
- **Edition 2015 bare trait objects now parse successfully**
- **Edition 2015 async identifiers now parse successfully**

**New test coverage:**
- `edition_2015_bare_trait_object` - validates parsing of `Arc<Fn(...)>``
- `edition_2015_async_identifier` - validates parsing of `fn async(&self)`

---

## Next Steps

1. **Complete syn1→syn2 conversion** - Fix remaining compilation errors in `parser/utils.rs`
   - Handle `AssocType` vs `Binding` field differences
   - Handle `Constraint` generics field
   - Skip or convert remaining Attribute references
2. **Integrate conversion** - Use `convert_type_syn1_to_syn2()` in `process_fn_arg_syn1`
3. **Verify on ripgrep dataset** - Test the full ripgrep dataset to ensure no regressions

---

## Recovery Info

**To continue work:**
```bash
# Check current compilation errors
cargo test -p syn_parser 2>&1 | grep "^error"

# View conversion code
cat crates/ingest/syn_parser/src/parser/utils.rs | head -300
```

**Key files:**
- `crates/ingest/syn_parser/src/parser/utils.rs` - Type conversion functions
- `crates/ingest/syn_parser/src/error.rs` - Error types

**Key constraint from AGENTS.md:**
> Do not relax internal correctness, consistency, validation, schema, or import semantics without explicit user approval first.

---

## Related Documents

- [Bug Report: syn 2.x fails on Rust 2015 bare trait objects](../../bugs/2026-04-10-syn-2-fails-on-rust-2015-bare-trait-objects.md)
- [CURRENT_FOCUS.md](../../CURRENT_FOCUS.md) - Will need update after compaction
- [Phase 1 RunRecord Tracking](../../plans/evals/phase-1-runrecord-tracking.md)
