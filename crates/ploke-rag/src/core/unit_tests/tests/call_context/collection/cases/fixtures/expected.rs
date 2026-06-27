use super::super::super::super::super::*;

pub(super) struct ExpectedCall {
    pub(super) kind: CallSiteKind,
    pub(super) callee: CallCalleeInfo,
    pub(super) target: Uuid,
    pub(super) relation: CallTargetKind,
}

pub(super) struct CallCase {
    pub(super) label: &'static str,
    pub(super) owner: Uuid,
    pub(super) calls: Vec<ExpectedCall>,
}

pub(super) fn assert_expected_call(
    context: &[CallContextInfo],
    expected: &ExpectedCall,
    label: &str,
) {
    let call = context
        .iter()
        .find(|call| {
            call.kind == expected.kind
                && call.callee == expected.callee
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == expected.target)
        })
        .unwrap_or_else(|| panic!("{label} should retain expected call context: {context:#?}"));
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, expected.target);
    assert_eq!(call.targets[0].relation, expected.relation);
}

pub(super) fn path_call(segments: &[&str]) -> CallCalleeInfo {
    CallCalleeInfo::Path {
        path: path(segments),
    }
}

pub(super) fn method_call(name: &str, receiver: CallReceiverInfo) -> CallCalleeInfo {
    CallCalleeInfo::Method {
        name: name.to_string(),
        receiver: Some(receiver),
    }
}

pub(super) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
