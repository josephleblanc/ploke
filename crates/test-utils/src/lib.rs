use ploke_common::fixtures_dir;
use std::path::Path;
use syn_parser::parser::visitor::analyze_code;

pub fn parse_fixture(
    fixture_name: &str,
) -> Result<syn_parser::parser::graph::CodeGraph, syn::Error> {
    let path = fixtures_dir().join(fixture_name);
    analyze_code(&path)
}
