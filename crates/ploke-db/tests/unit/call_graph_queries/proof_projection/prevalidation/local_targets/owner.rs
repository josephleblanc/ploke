use super::helpers::*;
use super::*;

#[test]
fn proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    assert_owner_projection_rejects(path_row(
        0x51,
        0x54,
        "src/lib.rs",
        (70, 90),
        UNKNOWN_PATH,
        "Unresolved",
        None,
    ))
}

#[test]
fn proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    assert_owner_projection_rejects(method_row(0x151, 0x154, "src/lib.rs", (100, 130)))
}
