use ploke_core::rag_types::CallTargetKind as RagCallTargetKind;
use ploke_db::CallRelationKind;
use ploke_test_utils::CallSiteSelector;

use super::*;

pub(super) fn rag_site_kind(site: CallSiteSelector) -> CallSiteKind {
    match site {
        CallSiteSelector::Path { .. } => CallSiteKind::Path,
        CallSiteSelector::Dynamic { .. } => CallSiteKind::Dynamic,
    }
}

pub(super) fn rag_callee(site: CallSiteSelector) -> CallCalleeInfo {
    match site {
        CallSiteSelector::Path { segments, .. } => CallCalleeInfo::Path {
            path: segments
                .iter()
                .map(|segment| (*segment).to_string())
                .collect(),
        },
        CallSiteSelector::Dynamic { .. } => CallCalleeInfo::Dynamic,
    }
}

pub(super) fn rag_relation_kind(relation: CallRelationKind) -> RagCallTargetKind {
    match relation {
        CallRelationKind::Function => RagCallTargetKind::Function,
        CallRelationKind::DynamicFunction => RagCallTargetKind::DynamicFunction,
        CallRelationKind::Closure => RagCallTargetKind::Closure,
        CallRelationKind::LocalFunction => RagCallTargetKind::LocalFunction,
        CallRelationKind::DynamicClosure => RagCallTargetKind::DynamicClosure,
        CallRelationKind::Method => RagCallTargetKind::Method,
        CallRelationKind::AssociatedFunction => RagCallTargetKind::AssociatedFunction,
        CallRelationKind::TupleStructConstructor => RagCallTargetKind::TupleStructConstructor,
        CallRelationKind::EnumVariantConstructor => RagCallTargetKind::EnumVariantConstructor,
    }
}

pub(super) fn rag_status_kind(status: DbCallStatusKind) -> CallStatusKind {
    match status {
        DbCallStatusKind::Resolved => CallStatusKind::Resolved,
        DbCallStatusKind::Unresolved => CallStatusKind::Unresolved,
        DbCallStatusKind::Ambiguous => CallStatusKind::Ambiguous,
        DbCallStatusKind::External => CallStatusKind::External,
        DbCallStatusKind::Unsupported => CallStatusKind::Unsupported,
    }
}

pub(super) fn targetless_proof_state(
    status: DbCallStatusKind,
    site: CallSiteSelector,
) -> (&'static str, &'static str) {
    match (status, site) {
        (DbCallStatusKind::Unresolved, CallSiteSelector::Path { .. }) => {
            ("unresolved", "type_resolution_missing")
        }
        (DbCallStatusKind::Unresolved, CallSiteSelector::Dynamic { .. }) => {
            ("unresolved", "dynamic_dispatch_unbounded")
        }
        (DbCallStatusKind::Unsupported, CallSiteSelector::Dynamic { .. }) => {
            ("blocked", "dynamic_dispatch_unbounded")
        }
        (DbCallStatusKind::Unsupported, _) => ("blocked", "type_resolution_missing"),
        (DbCallStatusKind::External, _) => ("blocked", "external_dependency_summary_missing"),
        (DbCallStatusKind::Ambiguous, _) => ("ambiguous", "type_resolution_missing"),
        (DbCallStatusKind::Resolved, _) => unreachable!("resolved rows are not targetless"),
    }
}
