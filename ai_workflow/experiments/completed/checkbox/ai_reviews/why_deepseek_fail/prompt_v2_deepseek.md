Here's a revised prompt specifically tailored for Deepseek-R1's capabilities:

```markdown
# Template Generation Prompt (Deepseek-R1 Optimized)

**Objective:** Create templated task lists in STRICT MARKDOWN FORMAT that help track code changes through incremental, testable steps.

## Format Requirements 
Your response MUST use:
```markdown
## [TASK NAME]
**Feature Flag:** `feature_[meaningful_short_name]` (ONLY if needed)
- [ ] **Analysis:** Code locations needing changes
- [ ] **Impl:** 
  - [ ] Core code change
  - [ ] Add `#[cfg(feature = "...")]` guards
- [ ] **Tests:** 
  - [ ] New test module
  - [ ] Both flag-enabled and default test cases
- [ ] **Docs:** `///[Required]` comments added
```

## Input Specifications
1. **Atomic Steps:** Each task must be <10 lines changed
2. **Safety First:** Use feature flags when:
   - Changing public API signatures
   - Modifying core data structures
   - Refactoring cross-crate dependencies
3. **Code Examples:** Include 1-line Rust snippets ONLY when demonstrating cfg usage:
```rust
// GOOD
#[cfg(feature = "bytes_type")]
pub type ContentHash = bytes::Bytes;

// BAD (no multi-line examples)
```

## Example Template
```markdown
## Align CozoDB Types
**Feature Flag:** `feature_cozo_types_v1`
- [ ] **Analysis:** Find all type usages in syn_parser/src/parser/
- [ ] **Impl:** 
  - [ ] Change `String` to `Bytes` in visitor.rs:L23
  - [ ] Add `cfg(feature = "cozo_types_v1")` guards
- [ ] **Tests:** 
  - [ ] Create test_type_migration module
  - [ ] Test both feature-enabled and default builds
- [ ] **Docs:** 
  - [ ] Add "Uses Bytes from bytes crate when enabled"
```

## Your Task
Generate 3 templates for:
1. [Insert User Request Here]
```

**Key Adaptations for Deepseek-R1:**
1. **Concrete Format Spec** - Explicit markdown structure reduces ambiguity
2. **Code Injection Slots** - Allows code snippets in controlled contexts
3. **Decision Triggers** - Clear criteria for feature flag use
4. **Anti-Example** - Shows what multi-line code to avoid
5. **Structured Validation** - Mandatory analysis/test/doc steps enforce completeness

Would you like me to generate a version with your specific task filled in?
