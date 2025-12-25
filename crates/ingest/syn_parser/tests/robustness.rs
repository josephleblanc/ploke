use std::path::PathBuf;
use syn_parser::{discovery::run_discovery_phase, parser::analyze_files_parallel};

#[test]
fn test_parser_robustness_against_malformed_input() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    // Go up to workspace root from crates/ingest/syn_parser
    let workspace_root = manifest_dir
        .parent().unwrap() // ingest
        .parent().unwrap() // crates
        .parent().unwrap(); // root

    let fixture_path = workspace_root.join("tests/fixture_malformed_crates/fixture_errors");
    
    assert!(fixture_path.exists(), "Fixture path not found: {:?}", fixture_path);

    // 1. Run Discovery
    let discovery = run_discovery_phase(&fixture_path, &[fixture_path.clone()])
        .expect("Discovery failed even though it should handle malformed files (it only looks for files)");

    // 2. Run Parallel Analysis
    // This is where we expect a potential panic or errors
    let results = analyze_files_parallel(&discovery, 1);

    // 3. Analyze Results
    // We expect some errors because the file is malformed.
    // But we MUST NOT panic.
    
    assert!(!results.is_empty(), "Expected results from analysis");
    
    let errors: Vec<_> = results.iter().filter(|r| r.is_err()).collect();
    let oks: Vec<_> = results.iter().filter(|r| r.is_ok()).collect();

    println!("Successes: {}", oks.len());
    println!("Errors: {}", errors.len());

    for err in errors {
        println!("Got expected error: {:?}", err);
    }
    
    // We verify that we actually found the file and attempted to parse it.
    // Since there is 1 file in src/lib.rs, we expect 1 result.
    assert_eq!(results.len(), 1, "Expected 1 file to be processed");
    
    // We expect it to be an error because the file contains deliberate syntax errors
    assert!(results[0].is_err(), "Expected the malformed file to return an error, but it parsed successfully?");
}
