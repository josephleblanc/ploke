use super::*;

pub(in crate::unit) const AMBIGUOUS_DYNAMIC_OWNERS: [&str; 2] = [
    "call_if_ambiguous_function_item",
    "call_match_ambiguous_function_item",
];

pub(in crate::unit) const MIXED_DYNAMIC_OWNERS: [&str; 2] =
    ["call_if_closure_branch", "call_match_closure_arm"];

pub(in crate::unit) const AMBIGUOUS_PATH_OWNER: &str =
    "call_if_ambiguous_initialized_function_item_binding";

pub(in crate::unit) const AMBIGUOUS_PATH_FUNCTION_CANDIDATES: [(&str, &[&str]); 5] = [
    ("call_multi_conflicting_function_pointer_param", &["f"]),
    ("call_forwarded_conflicting_function_pointer_leaf", &["f"]),
    (
        "call_two_hop_forwarded_conflicting_function_pointer_leaf",
        &["f"],
    ),
    (
        "call_multi_conflicting_generic_fn_once_param",
        &["generic_f"],
    ),
    ("call_forwarded_conflicting_boxed_dyn_fn_leaf", &["f"]),
];

pub(in crate::unit) const AMBIGUOUS_DYNAMIC_FUNCTION_CANDIDATES: [(&str, &[&str]); 3] = [
    (
        "call_multi_conflicting_named_field_function_param",
        &["holder", "callback"],
    ),
    (
        "call_forwarded_conflicting_named_field_leaf",
        &["holder", "callback"],
    ),
    (
        "call_two_hop_forwarded_conflicting_named_field_leaf",
        &["holder", "callback"],
    ),
];

pub(in crate::unit) const RETURNED_CONFLICTING_FUNCTION_POINTER_OWNERS: [&str; 2] = [
    "call_returned_conflicting_forwarded_function_pointer_param_with_local_target",
    "call_returned_conflicting_forwarded_function_pointer_param_with_other_target",
];

pub(in crate::unit) const OTHER_TARGET_DIRECT_RESOLVED_OWNERS: [&str; 1] =
    ["direct_self_field_other"];

pub(in crate::unit) const OTHER_TARGET_AMBIGUOUS_CANDIDATE_COUNT: usize = AMBIGUOUS_DYNAMIC_OWNERS
    .len()
    + 1
    + AMBIGUOUS_PATH_FUNCTION_CANDIDATES.len()
    + AMBIGUOUS_DYNAMIC_FUNCTION_CANDIDATES.len()
    + RETURNED_CONFLICTING_FUNCTION_POINTER_OWNERS.len();

pub(in crate::unit) const OTHER_TARGET_CALLER_COUNT: usize =
    OTHER_TARGET_AMBIGUOUS_CANDIDATE_COUNT + OTHER_TARGET_DIRECT_RESOLVED_OWNERS.len();

pub(in crate::unit) fn dynamic_candidates(db: &Database) -> Result<Vec<Uuid>, DbError> {
    let mut expected = vec![
        function_id_by_name(db, "local_target")?,
        function_id_by_name(db, "other_target")?,
    ];
    expected.sort_unstable();
    Ok(expected)
}

pub(in crate::unit) fn candidate_strings(candidates: &[Uuid]) -> Vec<String> {
    let mut expected = candidates
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

pub(in crate::unit) fn assert_path_function_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected_path: &[&str],
    expected: &[Uuid],
    label: &str,
) {
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(expected_path)));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(
        row.targets.len(),
        expected.len(),
        "{label} targets: {row:#?}"
    );
    assert!(row.targets.iter().all(|target| {
        target.relation == CallRelationKind::Function
            && target.source_kind == CallSiteKind::Path
            && target.target_kind == CallTargetKind::Function
    }));

    let mut actual = row
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "{label} should expose proven ambiguous path candidates"
    );
}

pub(in crate::unit) fn assert_dynamic_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected: &[Uuid],
    label: &str,
) {
    assert_dynamic_function_candidates(row, owner, None, 0, expected, label);
}

pub(in crate::unit) fn assert_dynamic_path_function_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected_path: &[&str],
    expected: &[Uuid],
    label: &str,
) {
    assert_dynamic_function_candidates(row, owner, Some(expected_path), 0, expected, label);
}

pub(in crate::unit) fn assert_dynamic_path_function_candidates_with_args(
    row: &CallContextRow,
    owner: Uuid,
    expected_path: &[&str],
    expected_arg_count: u32,
    expected: &[Uuid],
    label: &str,
) {
    assert_dynamic_function_candidates(
        row,
        owner,
        Some(expected_path),
        expected_arg_count,
        expected,
        label,
    );
}

fn assert_dynamic_function_candidates(
    row: &CallContextRow,
    owner: Uuid,
    expected_path: Option<&[&str]>,
    expected_arg_count: u32,
    expected: &[Uuid],
    label: &str,
) {
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path, expected_path.map(path));
    assert_eq!(row.site.arg_count, Some(expected_arg_count));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(
        row.targets.len(),
        expected.len(),
        "{label} targets: {row:#?}"
    );
    assert!(row.targets.iter().all(|target| {
        target.relation == CallRelationKind::DynamicFunction
            && target.source_kind == CallSiteKind::Dynamic
            && target.target_kind == CallTargetKind::Function
    }));

    let mut actual = row
        .targets
        .iter()
        .map(|target| target.target_id)
        .collect::<Vec<_>>();
    actual.sort_unstable();
    assert_eq!(
        actual, expected,
        "{label} should expose proven ambiguous dynamic candidates"
    );
}

pub(in crate::unit) fn assert_mixed_dynamic_candidates(
    row: &CallContextRow,
    owner: Uuid,
    function: Uuid,
    closure: Uuid,
    label: &str,
) {
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path, None);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert_eq!(row.targets.len(), 2, "{label} targets: {row:#?}");
    assert!(
        row.targets.iter().any(|target| {
            target.target_id == function
                && target.relation == CallRelationKind::DynamicFunction
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Function
        }),
        "{label} should expose local function candidate {function}: {row:#?}"
    );
    assert!(
        row.targets.iter().any(|target| {
            target.target_id == closure
                && target.relation == CallRelationKind::DynamicClosure
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Closure
        }),
        "{label} should expose local closure candidate {closure}: {row:#?}"
    );
}
