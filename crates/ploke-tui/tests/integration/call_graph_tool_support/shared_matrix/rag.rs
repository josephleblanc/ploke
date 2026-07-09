use ploke_core::rag_types::CallTargetKind as RagCallTargetKind;
use ploke_db::CallRelationKind;
use ploke_test_utils::{CallReceiverSelector, CallSiteSelector};

use super::*;

pub(super) fn rag_site_kind(site: CallSiteSelector) -> CallSiteKind {
    match site {
        CallSiteSelector::Path { .. } => CallSiteKind::Path,
        CallSiteSelector::Dynamic { .. } => CallSiteKind::Dynamic,
        CallSiteSelector::Method { .. } => CallSiteKind::Method,
    }
}

pub(super) fn rag_callee_matches(callee: &CallCalleeInfo, site: CallSiteSelector) -> bool {
    match site {
        CallSiteSelector::Path { segments, .. } => {
            callee
                == &(CallCalleeInfo::Path {
                    path: segments
                        .iter()
                        .map(|segment| (*segment).to_string())
                        .collect(),
                })
        }
        CallSiteSelector::Dynamic { .. } => callee == &CallCalleeInfo::Dynamic,
        CallSiteSelector::Method { name, receiver, .. } => {
            rag_method_matches(callee, name, receiver)
        }
    }
}

fn rag_method_matches(
    callee: &CallCalleeInfo,
    name: &str,
    receiver: Option<CallReceiverSelector>,
) -> bool {
    let CallCalleeInfo::Method {
        name: method,
        receiver: actual,
    } = callee
    else {
        return false;
    };

    method == name && rag_receiver_matches(actual, receiver)
}

fn rag_receiver_matches(
    actual: &Option<CallReceiverInfo>,
    expected: Option<CallReceiverSelector>,
) -> bool {
    match expected {
        None => true,
        Some(CallReceiverSelector::SelfField { path }) => matches!(
            actual,
            Some(CallReceiverInfo::SelfField { path: actual })
                if actual.iter().map(String::as_str).eq(path.iter().copied())
        ),
        Some(CallReceiverSelector::Unsupported) => {
            matches!(actual, Some(CallReceiverInfo::Unsupported))
        }
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
        (DbCallStatusKind::Unresolved, CallSiteSelector::Method { .. }) => {
            ("unresolved", "type_resolution_missing")
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
