//! Exploration of `cfg` attribute processing
//!
//! Testing Strategy:
//! - Use direct string parsing (no fixtures) for rapid iteration
//! - Validate both parsing AND semantic understanding of conditions
//! - Test edge cases in cfg predicate syntax
//!
//! Crates being explored:
//! - `cfg-expr`: Main parser for cfg conditions, provides:
//!   - Structured AST for cfg predicates
//!   - Logical operator support (any/all/not)
//!   - Feature flag and target configuration parsing
//! - `target-lexicon`: Provides current target information for evaluation
//!   - Can check if conditions match the current build target
//!   - Useful for testing conditional compilation scenarios

use cfg_expr::{Expression, Predicate, TargetPredicate}; // Added TargetPredicate

#[test]
#[ignore = "Future tests for cfg feature tracking"]
fn test_basic_cfg_parsing() -> Result<(), Box<dyn std::error::Error>> {
    // Test 1: Simple feature flag
    let simple_feature = Expression::parse(r#"feature = "test""#)?;
    // predicates() returns an iterator. For simple expressions, get the first/only item.
    if let Some(Predicate::Feature(f)) = simple_feature.predicates().next() {
        assert_eq!(f, "test");
    } else {
        panic!("Expected a single Feature predicate");
    }

    // Test 2: Target configuration
    let target_cfg = Expression::parse(r#"target_os = "linux""#)?;
    // Match Target variant, then the inner TargetPredicate::Os
    if let Some(Predicate::Target(TargetPredicate::Os(os))) = target_cfg.predicates().next() {
        assert_eq!(os.to_string(), "linux");
    } else {
        panic!("Expected a single Target predicate with os");
    }

    // Test 3: Existence check (no value) - This parses as TargetPredicate::HasAtomicCas
    // Let's adjust the test slightly to use a more direct target property like 'family'
    // or stick to the original 'unix' which cfg-expr interprets as TargetPredicate::Family("unix")
    let exists_check = Expression::parse("unix")?;
    // Match Target variant, then the inner TargetPredicate::Family
    if let Some(Predicate::Target(TargetPredicate::Family(family))) =
        exists_check.predicates().next()
    {
        assert_eq!(family.to_string(), "unix");
    } else {
        panic!("Expected a single Target predicate with family 'unix'");
    }

    // Test 4: Verify expression display (using debug format)
    // Note: `all(...)` expressions involve multiple predicates, handled differently.
    // Let's keep this test focused on simple predicates for now.
    // We'll test complex expressions in `test_cfg_logical_operators`.
    // Re-test roundtrip with a simple predicate.
    let roundtrip_expr = r#"target_arch = "wasm32""#;
    let parsed = Expression::parse(roundtrip_expr)?;
    // Use Debug format for comparison as Expression doesn't implement Display
    assert_eq!(format!("{:?}", parsed), roundtrip_expr);

    Ok(())
}

// --- Existing tests below ---

#[test]
#[ignore = "Future tests for cfg feature tracking"]
fn test_cfg_logical_operators() {
    // Test combinations like any/all/not
    todo!("Explore complex logical combinations");
}

#[test]
#[ignore = "Future tests for cfg feature tracking"]
fn test_target_specific_conditions() {
    // Test target_os, target_arch etc.
    todo!("Explore target-specific conditions");
}

#[test]
#[ignore = "Future tests for cfg feature tracking"]
fn test_cfg_attribute_roundtrip() {
    // Test parsing -> processing -> reconstruction
    todo!("Verify we can reconstruct equivalent cfg attributes");
}

// Helper module for test utilities
mod test_utils {
    // We'll add helpers here as we develop them
}
