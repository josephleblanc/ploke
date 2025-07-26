
use std::collections::HashSet;

/// Robust `#[cfg(...)]` evaluator with system information support.
///
/// Accepts the raw token string that lives between the parentheses
/// (e.g. `feature = "alpha"`, `all(feature = "a", target_os = "linux")`)
/// and a set of currently active names.
///
/// Returns true if the predicate evaluates to true under the supplied
/// active set and system configuration.
pub fn cfg_enabled(expr: &str, active: &HashSet<&str>) -> bool {
    let expr = expr.trim();
    if expr.is_empty() {
        return true;
    }

    let system = get_system_cfg();
    parse_and_evaluate(expr, active, &system)
}

/// System configuration detection
fn get_system_cfg() -> HashSet<String> {
    let mut system = HashSet::new();
    
    // Add target OS
    if cfg!(target_os = "linux") {
        system.insert("target_os = \"linux\"".to_string());
    } else if cfg!(target_os = "windows") {
        system.insert("target_os = \"windows\"".to_string());
    } else if cfg!(target_os = "macos") {
        system.insert("target_os = \"macos\"".to_string());
    }
    
    // Add target architecture
    if cfg!(target_arch = "x86_64") {
        system.insert("target_arch = \"x86_64\"".to_string());
    } else if cfg!(target_arch = "aarch64") {
        system.insert("target_arch = \"aarch64\"".to_string());
    }
    
    // Add debug/release configuration
    if cfg!(debug_assertions) {
        system.insert("debug_assertions".to_string());
    }
    
    // Add test configuration
    if cfg!(test) {
        system.insert("test".to_string());
    }
    
    system
}

/// Parse and evaluate a cfg expression with proper handling of nested parentheses
fn parse_and_evaluate(expr: &str, active: &HashSet<&str>, system: &HashSet<String>) -> bool {
    let expr = expr.trim();
    
    // Handle combinators
    if let Some(rest) = expr.strip_prefix("all(") {
        if let Some((items, _)) = split_balanced(rest) {
            return items.iter().all(|item| parse_and_evaluate(item.trim(), active, system));
        }
    }
    
    if let Some(rest) = expr.strip_prefix("any(") {
        if let Some((items, _)) = split_balanced(rest) {
            return items.iter().any(|item| parse_and_evaluate(item.trim(), active, system));
        }
    }
    
    if let Some(rest) = expr.strip_prefix("not(") {
        if let Some((items, _)) = split_balanced(rest) {
            if let Some(item) = items.first() {
                return !parse_and_evaluate(item.trim(), active, system);
            }
        }
    }
    
    // Base predicate
    let full_active: HashSet<&str> = active
        .iter()
        .copied()
        .chain(system.iter().map(|s| s.as_str()))
        .collect();
    
    full_active.contains(expr)
}

/// Split a string by commas while respecting balanced parentheses
fn split_balanced(s: &str) -> Option<(Vec<String>, usize)> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth = 0;
    
    for (i, ch) in s.chars().enumerate() {
        match ch {
            '(' => depth += 1,
            ')' => {
                if depth == 0 {
                    // Found the closing paren for the outer expression
                    items.push(current.trim().to_string());
                    return Some((items, i + 1));
                }
                depth -= 1;
            }
            ',' if depth == 0 => {
                items.push(current.trim().to_string());
                current.clear();
                continue;
            }
            _ => {}
        }
        current.push(ch);
    }
    
    None // Unbalanced parentheses
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
