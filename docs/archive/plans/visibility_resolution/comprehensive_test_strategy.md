# Visibility Resolution Testing Strategy

## TL;DR
- **Goal**: Ensure correct visibility resolution across module hierarchies
- **Approach**: Layered testing (unit → integration → system) with property-based validation
- **Key Principles**:
  - Follow Rust's visibility RFCs (136, 142)
  - Adhere to IDIOMATIC_RUST and CONVENTIONS
  - Test both positive and negative cases
  - Document expected behavior in fixtures
- **Organization**: Feature-flagged tests in clear hierarchy (unit/integration/system)

## 1. Testing Philosophy

### 1.1 Core Principles
- **Correctness**: Verify all visibility modifiers work as specified in Rust RFCs
- **Composability**: Ensure visibility rules compose correctly in nested scenarios
- **Performance**: Maintain O(1) visibility checks for common cases
- **Diagnostics**: Provide clear error messages for visibility violations

### 1.2 Rust Compliance
Tests must verify:
- RFC 136 (pub/private basics)
- RFC 142 (pub restricted)
- Edition-specific behaviors (2015 vs 2018+)

## 2. Test Architecture

### 2.1 Test Pyramid
```text
          System Tests (10%)
            /       \
Integration (20%)  Property (10%)
            \       /
          Unit Tests (60%)
```

### 2.2 Test Categories

#### Unit Tests (per-feature)
- Location: `tests/visibility/unit/`
- Scope:
  - Individual visibility modifiers
  - Basic module hierarchies
  - Edge cases per modifier
- Examples:
  - `pub` in root module
  - `pub(crate)` in nested module
  - `pub(in path)` restrictions

#### Integration Tests (cross-feature)
- Location: `tests/visibility/integration/`  
- Scope:
  - Cross-module visibility
  - Trait impl visibility
  - Macro hygiene interactions
- Examples:
  - `pub use` re-exports
  - Visibility through `mod` boundaries
  - `#[macro_export]` visibility

#### System Tests (end-to-end)
- Location: `tests/visibility/system/`
- Scope:
  - Full crate resolution
  - Performance benchmarks
  - Complex real-world scenarios
- Examples:
  - Deeply nested module trees
  - Mixed visibility hierarchies
  - Large-scale visibility checks

#### Property Tests
- Location: `tests/visibility/property/`
- Scope:
  - Generative test cases
  - Fuzz testing
  - Invariant verification
- Examples:
  - Visibility preservation through transformations
  - Compositional correctness
  - Idempotency checks

## 3. Implementation Guidelines

### 3.1 Code Organization
```bash
tests/
  visibility/
    unit/               # 60% coverage
      basic/            # Fundamental visibility
      modules/          # Module interactions  
      traits/           # Trait visibility
      macros/           # Macro visibility
      edge_cases/       # Special scenarios
    
    integration/        # 20% coverage
      resolution/       # Cross-module
      shadowing/        # Name resolution
      inheritance/      # Visibility rules
    
    system/            # 10% coverage
      benchmarks/       # Performance
      real_world/       # Complex cases
    
    property/          # 10% coverage
      generators/       # Test case generation
      invariants/       # Rule verification
    
    common/            # Shared utilities
      mod.rs           # Test helpers
```

### 3.2 Test Authoring

#### Per IDIOMATIC_RUST:
- Use `?` not `unwrap()` (C-QUESTION-MARK)
- Document examples (C-EXAMPLE)
- Include failure cases (C-FAILURE)
- Follow naming conventions (C-CASE)

#### Per CONVENTIONS:
- Derive common traits for test types
- Use feature flags for test categories
- Maintain zero-copy where possible

### 3.3 Example Test Structure
```rust
//! Tests for `pub(in path)` restricted visibility  
//!
//! Verifies RFC 142 compliance for path-restricted pub

#[test]
fn pub_in_path_visible_in_specified_module() -> TestResult {
    let graph = parse_fixture("visibility/restricted.rs")?;
    let item = find_item(&graph, "PATH_VISIBLE_ITEM")?;
    
    // Should be visible in specified path
    assert_visible_in(&graph, item.id(), &["crate", "allowed"])?;
    
    // Should NOT be visible elsewhere
    assert_not_visible_in(&graph, item.id(), &["crate", "other"])?;
    Ok(())
}
```

## 4. Feature Flags

```toml
[features]
test-visibility = [
    "visibility-unit",
    "visibility-integration",
    "visibility-system"
]

visibility-unit = []       # Basic visibility cases
visibility-integration = [] # Cross-feature cases  
visibility-system = []     # End-to-end scenarios
```

## 5. Validation Matrix

| Test Type       | Coverage Target | Execution Time | Fixture Complexity |
|-----------------|-----------------|----------------|--------------------|
| Unit            | 60%             | Fast (<100ms)  | Simple             |
| Integration     | 20%             | Medium (<1s)   | Moderate           |  
| System          | 10%             | Slow (>1s)     | Complex            |
| Property        | 10%             | Variable       | Generated          |

## 6. Maintenance Plan

1. **Pre-commit**:
   - Run unit tests
   - Verify documentation
2. **Nightly**:
   - Run integration tests
   - Check benchmarks
3. **Weekly**:
   - Run system tests
   - Update generative tests
