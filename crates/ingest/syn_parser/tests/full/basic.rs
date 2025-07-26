#[test]
pub fn basic_test() -> Result<(), ploke_error::Error> {
    use crate::common::resolution::try_build_tree_for_tests;
    try_build_tree_for_tests("").expect_err("Should error on invalid input of empty string");
    Ok(())
}

#[test]
fn test_all_fixtures() -> Result<(), ploke_error::Error> {
    use crate::common::resolution::try_build_tree_for_tests;
    let _ = env_logger::builder()
        .is_test(true)
        .format_timestamp(None) // Disable timestamps
        .format_file(true)
        .format_line_number(true)
        .try_init();
    let fixture_dirs = [
        "duplicate_name_fixture_1",
        "duplicate_name_fixture_2",
        "example_crate",
        "file_dir_detection",
        "fixture_attributes",
        "fixture_conflation",
        "fixture_cyclic_types",
        "fixture_edge_cases",
        "fixture_generics",
        "fixture_macros",
        "fixture_nodes",
        "fixture_path_resolution",
        "fixture_spp_edge_cases_no_cfg",
        "fixture_tracking_hash",
        "fixture_types",
        "simple_crate",
        "fixture_spp_edge_cases",
    ];
    for dir in fixture_dirs {
        eprintln!("dir: {dir}");
        try_build_tree_for_tests(dir)?;
    }
    Ok(())
}
