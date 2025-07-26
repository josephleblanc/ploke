
use std::collections::HashSet;
use syn_parser::cfg::cfg_enabled;

#[test]
fn test_new_cfg() {
    let mut active = HashSet::new();
    active.insert("feature = \"fixture\"");
    active.insert("target_os = \"linux\"");

    assert!(cfg_enabled("feature = \"fixture\"", &active));
    assert!(cfg_enabled("any(target_os = \"linux\", target_os = \"macos\")", &active));
    assert!(!cfg_enabled("all(feature = \"fixture\", not(target_os = \"linux\"))", &active));
}
