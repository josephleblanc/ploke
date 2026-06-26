use super::*;

pub(super) fn assert_resolved_target(
    call: &CallContextInfo,
    target: Uuid,
    relation: CallTargetKind,
) {
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, relation);
}

pub(super) fn assert_incoming_expansion(
    part: &ConciseContext,
    call: &CallContextInfo,
    target: Uuid,
) {
    assert_expansion(
        part,
        target,
        target,
        call.site_id,
        CallExpansionKind::IncomingCaller,
    );
}

pub(super) fn assert_expansion(
    part: &ConciseContext,
    seed: Uuid,
    target: Uuid,
    site: Uuid,
    relation: CallExpansionKind,
) {
    let expansion = part
        .call_expansion
        .expect("expanded part should carry call-expansion provenance");
    assert_eq!(expansion.seed_id, seed);
    assert_eq!(expansion.relation, relation);
    assert_eq!(expansion.call_site_id, site);
    assert_eq!(expansion.target_id, target);
    assert_eq!(expansion.distance, 1);
}

pub(super) fn path(segments: &[&str]) -> Vec<String> {
    segments
        .iter()
        .map(|segment| (*segment).to_string())
        .collect()
}
