use super::helpers::*;
use super::*;

#[test]
fn target_centered_proof_projection_rejects_non_resolved_local_targets() -> Result<(), DbError> {
    let target = 0x65;
    assert_target_projection_rejects(
        Uuid::from_u128(target),
        &[
            path_row(
                0x61,
                target,
                "src/resolved.rs",
                (10, 30),
                CRATE_TARGET_PATH,
                "Resolved",
                Some("LocalExact"),
            ),
            path_row(
                0x71,
                target,
                "src/unresolved.rs",
                (40, 60),
                CRATE_TARGET_PATH,
                "Unresolved",
                None,
            ),
        ],
    )
}

#[test]
fn target_centered_proof_projection_rejects_ambiguous_local_targets() -> Result<(), DbError> {
    let target = 0x163;
    assert_target_projection_rejects(
        Uuid::from_u128(target),
        &[method_row(0x161, target, "src/ambiguous.rs", (140, 170))],
    )
}
