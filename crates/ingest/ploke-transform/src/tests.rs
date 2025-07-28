#![cfg(test)]

use ploke_test_utils::try_run_phases_and_collect_path;
use ploke_common::workspace_root;

#[test]
fn test_transform_syn() -> Result<(), ploke_error::Error> {

    let project_root = workspace_root(); // Use workspace root for context
    let crate_path = workspace_root().join("crates").join("ingest").join("syn_parser");
    try_run_phases_and_collect_path(&project_root, crate_path)?;
    Ok(())
}
