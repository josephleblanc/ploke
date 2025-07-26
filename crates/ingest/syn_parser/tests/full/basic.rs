use syn_parser::error::SynParserError;

use crate::common::build_tree_for_tests;


#[test]
pub fn basic_test() -> Result<(), SynParserError> {
    build_tree_for_tests("");
    Ok(())
}
