
use std::collections::HashSet;

/// Very small hand-rolled `#[cfg(...)]` evaluator.
///
/// Accepts the raw token string that lives between the parentheses
/// (e.g. `feature = "alpha"`, `all(feature = "a", target_os = "linux")`)
/// and a set of currently active names.
///
/// Returns true if the predicate evaluates to true under the supplied
/// active set.
pub fn cfg_enabled(expr: &str, active: &HashSet<&str>) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true;
    }

    // Simple recursive descent for the three combinators we need:
    //   all(...), any(...), not(...)
    if let Some(inner) = expr.strip_prefix("all(").and_then(|s| s.strip_suffix(')')) {
        eprintln!("all inner tokens: {:?}", inner.split(',').collect::<Vec<_>>());
        return inner
            .split(',')
            .map(|s| cfg_enabled(s.trim(), active))
            .all(|b| b);
    }
    if let Some(inner) = expr.strip_prefix("any(").and_then(|s| s.strip_suffix(')')) {
        eprintln!("any inner tokens: {:?}", inner.split(',').collect::<Vec<_>>());
        return inner
            .split(',')
            .map(|s| cfg_enabled(s.trim(), active))
            .any(|b| b);
    }
    if let Some(inner) = expr.strip_prefix("not(").and_then(|s| s.strip_suffix(')')) {
        return !cfg_enabled(inner.trim(), active);
    }

    // Base predicates: `feature = "value"`, `test`, `target_os = "linux"` etc.
    active.contains(expr)
}

#[test]
fn test_new_cfg() {
    let mut active = HashSet::new();
    active.insert("feature = \"fixture\"");
    active.insert("target_os = \"linux\"");

    assert!(cfg_enabled("feature = \"fixture\"", &active));
    assert!(cfg_enabled("any(target_os = \"linux\", target_os = \"macos\")", &active));
    assert!(!cfg_enabled("all(feature = \"fixture\", not(target_os = \"linux\"))", &active));
}

#[test]
fn test_cfg_feature_on_off() {
    let mut active = HashSet::new();
    active.insert("feature = \"alpha\"");
    assert!(cfg_enabled("feature = \"alpha\"", &active));
    assert!(!cfg_enabled("feature = \"beta\"", &active));
}

#[test]
fn test_cfg_target_os() {
    let mut active = HashSet::new();
    active.insert("target_os = \"linux\"");
    assert!(cfg_enabled("target_os = \"linux\"", &active));
    assert!(!cfg_enabled("target_os = \"windows\"", &active));
}

#[test]
fn test_cfg_not() {
    let mut active = HashSet::new();
    active.insert("test");
    assert!(!cfg_enabled("not(test)", &active));
    assert!(cfg_enabled("not(debug_assertions)", &active));
}

#[test]
fn test_cfg_all_any() {
    let mut active = HashSet::new();
    active.insert("feature = \"a\"");
    active.insert("target_os = \"linux\"");
    let expr = "all(feature = \"a\", any(target_os = \"linux\", not(windows)))";
    eprintln!("evaluating: {}", expr);
    assert!(cfg_enabled(expr, &active));
}

#[test]
fn test_cfg_on_mod() {
    // Ensure the evaluator can decide to keep / discard an entire module.
    let mut active = HashSet::new();
    active.insert("feature = \"special\"");
    assert!(cfg_enabled("feature = \"special\"", &active));
    assert!(!cfg_enabled("not(feature = \"special\")", &active));
}

#[test]
fn test_cfg_with_path_attr() {
    // Same as above – the predicate is still the cfg expression.
    let mut active = HashSet::new();
    active.insert("feature = \"special\"");
    assert!(cfg_enabled("feature = \"special\"", &active));
}

#[test]
fn test_empty_cfg() {
    assert!(cfg_enabled("", &HashSet::new()));
}
