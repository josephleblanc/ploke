use std::collections::HashMap;
use std::fs;

use ploke_test_utils::workspace_root;
use regex::Regex;

#[test]
fn estimated_tokens_are_paired_with_actual_usage() {
    let root = workspace_root();
    let fixture = root.join("tests/fixture_chat/tokens_sample.log");
    let contents =
        fs::read_to_string(&fixture).expect("token fixture log should be present for tests");

    let est_re = Regex::new(
        r#"parent_id=([a-f0-9-]+).*?kind="estimate_input".*?estimated_tokens=(\d+)"#,
    )
    .unwrap();
    let actual_re = Regex::new(
        r#"parent_id=([a-f0-9-]+).*?kind="actual_usage".*?prompt_tokens=(\d+)\s+completion_tokens=(\d+)\s+total_tokens=(\d+)"#,
    )
    .unwrap();

    let mut estimates: HashMap<String, u64> = HashMap::new();
    for cap in est_re.captures_iter(&contents) {
        let parent = cap[1].to_string();
        let tokens: u64 = cap[2].parse().unwrap();
        estimates.insert(parent, tokens);
    }

    let mut actuals: HashMap<String, (u64, u64, u64)> = HashMap::new();
    for cap in actual_re.captures_iter(&contents) {
        let parent = cap[1].to_string();
        let prompt: u64 = cap[2].parse().unwrap();
        let completion: u64 = cap[3].parse().unwrap();
        let total: u64 = cap[4].parse().unwrap();
        actuals.insert(parent, (prompt, completion, total));
    }

    assert!(
        !estimates.is_empty(),
        "expected at least one estimate in the fixture"
    );
    assert_eq!(
        estimates.len(),
        actuals.len(),
        "estimates and actuals should have matching pairs"
    );

    for (parent, estimated) in &estimates {
        let (prompt, completion, total) = actuals
            .get(parent)
            .unwrap_or_else(|| panic!("missing actual usage for parent_id={parent}"));

        assert_eq!(
            prompt + completion,
            *total,
            "prompt+completion should equal total for parent_id={parent}"
        );
        assert!(
            *total >= *estimated,
            "expected actual total >= estimated for parent_id={parent} (est={estimated}, total={total})"
        );

        // Ballpark sanity: actual should not exceed 5x the estimate for this fixture.
        let ratio = *total as f64 / *estimated as f64;
        assert!(
            ratio <= 5.0,
            "actual/estimated ratio too high ({ratio:.2}) for parent_id={parent}"
        );
    }
}
