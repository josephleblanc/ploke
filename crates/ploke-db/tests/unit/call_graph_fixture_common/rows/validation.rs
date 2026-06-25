use super::*;

pub(in crate::unit) fn assert_valid_status_shape(
    site_id: Uuid,
    status: &str,
    resolution: Option<&str>,
) {
    let valid = match status {
        "Resolved" => resolution == Some("LocalExact"),
        "Unresolved" | "Ambiguous" | "External" | "Unsupported" => resolution.is_none(),
        other => panic!("unexpected call status kind {other} for {site_id}"),
    };
    assert!(
        valid,
        "call_resolution_status resolution_kind {resolution:?} is invalid for {status} call site {site_id}"
    );
}
