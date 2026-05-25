# Bug: syn 2.x fails to parse Rust 2015 bare trait objects

**Date Discovered:** 2026-04-10  
**Date Fixed:** 2026-04-11 (implementation), 2026-04-12 (`P2B` sentinel re-entry validated)  
**Crate Affected:** `syn_parser`  
**Severity:** High - Blocks evaluation on common Rust 2015 crates  
**Status:** Fixed and revalidated on ripgrep sentinel

## Summary

`syn` version 2.x was designed for post-Rust-2018 edition semantics and rejects bare trait objects (without `dyn`) that are valid in Rust 2015 code. This causes `syn_parser` to fail on crates like `ignore` (part of ripgrep) which use patterns like `Arc<Fn(...)> + Send + Sync>`.

## Error Details

**Error Type:** `SynParserError::PartialParsing`  
**Error Message:** `expected \`,\``  
**Location:** `crates/ingest/syn_parser/src/parser/visitor/mod.rs:1101`

### Failing Code Pattern

```rust
// From ignore/src/walk.rs:484 (Rust 2015 edition)
enum Sorter {
    ByName(Arc<Fn(&OsStr, &OsStr) -> cmp::Ordering + Send + Sync + 'static>),
    //       ^^ syn 2.x fails here - expects `,` or `dyn`
}
```

### Why This Fails

| Component | Behavior |
|-----------|----------|
| `rustc --edition 2015` | Accepts bare trait objects (deprecated warning) |
| `syn 2.x` | Rejects bare trait objects - hard error |
| `syn 1.x` | Accepts bare trait objects |

Syn 2.x aligns with Rust 2021 edition where bare trait objects are a hard error. The parser has no edition-awareness.

## Reproduction Steps

1. Attempt to parse ripgrep's `ignore` crate:
   ```bash
   cargo run -p ploke-eval -- run-msb-agent-single --instance BurntSushi__ripgrep-1294
   ```

2. Observe parsing failure:
   ```
   Parse failed for crate: globset
   Partial parsing success: 6 succeeded, 1 failed
   Syn parsing error: expected `,` (file: ignore/src/walk.rs, line: 484, col: 18)
   ```

## Impact

- **Immediate:** Cannot evaluate on ripgrep dataset (major SWE-bench subset)
- **Broader:** Any Rust 2015 crate with bare trait objects will fail
- **A2 Hypothesis:** Blocks "data fidelity for accurate code lookup" validation
- **Future:** Dependencies will include many Rust 2015 crates

## Alternatives Considered

### Option 1: Source Rewrite (Pattern Matching) - REJECTED

Insert `dyn ` before trait object patterns during preprocessing.

**Why rejected:**
- Cannot reliably distinguish `Arc<Fn()>` (trait object) from `Vec<MyFn>` (type parameter)
- False positives: `struct Send; let x: Box<Send>;` → `Box<dyn Send>` (wrong!)
- Brittle - each new Rust 2015 pattern requires another heuristic

### Option 2: rustfmt Preprocessor - REJECTED

Run `rustfmt --edition 2018` before parsing to add `dyn` keywords.

**Why rejected:**
- Loses precise span information (rewritten source != original)
- Slower (external process per file)
- Changes formatting, not just `dyn` insertion
- Requires rustfmt as dependency

### Option 3: Tree-sitter Parser - REJECTED

Replace syn with tree-sitter and maintain Rust 2015 grammar.

**Why rejected:**
- Complete architectural change (months of work)
- Different AST shape - would rewrite `syn_parser` entirely
- No syn compatibility for proc-macro ecosystem

### Option 4: rustc Parser Library - REJECTED

Use `rustc-ap-syntax` or `rustc_interface` for parsing.

**Why rejected:**
- Requires nightly Rust compiler
- Unstable API (breaks frequently)
- Heavy dependency
- AST is different from syn (conversion layer needed)

### Option 5: rust-analyzer Parser - CONSIDERED

Vendor rust-analyzer's edition-aware parser.

**Why not selected:**
- 3-6 months effort
- Would replace syn entirely
- Significant refactoring of syn_parser
- Maintenance burden of vendored code

### Option 6: Dual Syn Versions - SELECTED

Use both syn 1.x and syn 2.x:
- syn 2.x for modern Rust (2018+)
- syn 1.x for legacy Rust (2015)

**Why selected:**
- Correct handling of all Rust 2015 syntax
- Maintains span accuracy
- No false positives
- Controlled complexity (contained to visitor layer)
- Acceptable effort (3-4 weeks)
- Can share IR types between both paths

## Solution Design

### Architecture

```
syn_parser/src/parser/visitor/
├── mod.rs              (dispatch logic)
├── code_visitor_syn1.rs (syn 1.x visitor - NEW)
├── code_visitor_syn2.rs (syn 2.x visitor - current, renamed)
└── shared_helpers.rs   (extracted common logic)
```

### Dependency Changes

```toml
[dependencies]
syn = { version = "2.0", features = ["full", "visit"] }
syn1 = { package = "syn", version = "1.0", features = ["full", "visit"] }
```

### Dispatch Logic

```rust
// In visitor/mod.rs
fn try_parse_file(file_content: &str, edition: Edition) -> Result<ParsedCodeGraph, Error> {
    match edition {
        Edition::E2015 => {
            let file = syn1::parse_file(file_content)?;
            let mut visitor = CodeVisitorSyn1::new(&mut state);
            visitor.visit_file(&file);
        }
        _ => {
            let file = syn::parse_file(file_content)?;
            let mut visitor = CodeVisitorSyn2::new(&mut state);
            visitor.visit_file(&file);
        }
    }
}
```

### Code Sharing Strategy

Both visitors implement the same IR construction:

```rust
// Shared helper - same for both versions
fn create_function_node(
    &mut self,
    name: String,
    visibility: VisibilityKind,
    attrs: Vec<Attribute>,
    // ... other fields
) -> FunctionNode {
    // Same implementation
}

// Thin wrappers - syn 2 version
fn visit_item_fn(&mut self, func: &'ast syn::ItemFn) {
    let node = self.create_function_node(
        func.sig.ident.to_string(),
        self.convert_visibility(&func.vis),
        // ... extract fields
    );
}

// Thin wrappers - syn 1 version  
fn visit_item_fn(&mut self, func: &'ast syn1::ItemFn) {
    let node = self.create_function_node(
        func.sig.ident.to_string(),
        self.convert_visibility(&func.vis),
        // ... extract fields
    );
}
```

### Key Differences Between Syn Versions

| Aspect | syn 1.x | syn 2.x |
|--------|---------|---------|
| Attribute path | `attr.path.is_ident(...)` | `attr.path().is_ident(...)` |
| Visit trait | `Visit<'ast>` | `Visit<'_>` |
| Generic args | Simpler | More complex |

Most code (~80%) is identical - only type paths and attribute access differ.

## Implementation Plan

### Phase 1: Refactor for Sharing (Week 1)
- Extract shared helpers from `code_visitor.rs`
- Create `shared_helpers.rs` with IR construction logic
- Ensure `VisitorState` works with both versions

### Phase 2: Syn 1.x Visitor (Week 1-2)
- Create `code_visitor_syn1.rs`
- Copy `code_visitor.rs` as starting point
- Adapt for syn 1.x types (attribute access, generics)
- Test on Rust 2015 fixtures

### Phase 3: Dispatch Logic (Week 2)
- Add edition detection in `visitor/mod.rs`
- Route to correct visitor based on crate edition
- Add syn 1.x dependency to Cargo.toml

### Phase 4: Testing (Week 3-4)
- Test on ripgrep dataset
- Verify span accuracy
- Ensure no regressions for syn 2.x path

## Test Coverage

Already implemented:
- `repro_edition_2015_bare_trait_object.rs` (fail test without fix)
- `repro_edition_2015_bare_trait_object_fallback.rs` (success test with fix)

Will pass once dual visitor is implemented.

## Related Code

- `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs` - Current syn 2.x visitor
- `crates/ingest/syn_parser/src/parser/visitor/mod.rs` - Dispatch and rewrite logic
- `crates/ingest/syn_parser/tests/repro/fail/edition_2015_bare_trait_object.rs` - Repro test
- `crates/ingest/syn_parser/tests/repro/success/edition_2015_bare_trait_object_fallback.rs` - Success test

## Future Considerations

- If a 3rd edition-related issue arises, consider vendoring rust-analyzer parser
- Monitor syn for edition-awareness features (unlikely - design decision)
- Document which crates require syn 1.x path for maintenance

---

**Tags:** `syn`, `rust-2015`, `edition`, `trait-objects`, `parsing`, `ripgrep`  
**Related Issues:** Blocks A2 hypothesis validation  
**Implementation Ticket:** (To be created when work begins)
