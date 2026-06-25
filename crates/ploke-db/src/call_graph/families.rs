use std::fmt::Write as _;

use super::{CallRelationKind, CallSiteKind, CallTargetKind, CallTargetRow};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CallTargetFamily {
    relation: CallRelationKind,
    source: CallSiteKind,
    target: CallTargetKind,
    target_relation: &'static str,
}

const VALID_CALL_TARGET_FAMILIES: [CallTargetFamily; 6] = [
    CallTargetFamily {
        relation: CallRelationKind::Function,
        source: CallSiteKind::Path,
        target: CallTargetKind::Function,
        target_relation: "function",
    },
    CallTargetFamily {
        relation: CallRelationKind::DynamicFunction,
        source: CallSiteKind::Dynamic,
        target: CallTargetKind::Function,
        target_relation: "function",
    },
    CallTargetFamily {
        relation: CallRelationKind::Method,
        source: CallSiteKind::Method,
        target: CallTargetKind::Method,
        target_relation: "method",
    },
    CallTargetFamily {
        relation: CallRelationKind::AssociatedFunction,
        source: CallSiteKind::Path,
        target: CallTargetKind::Method,
        target_relation: "method",
    },
    CallTargetFamily {
        relation: CallRelationKind::TupleStructConstructor,
        source: CallSiteKind::Path,
        target: CallTargetKind::Struct,
        target_relation: "struct",
    },
    CallTargetFamily {
        relation: CallRelationKind::EnumVariantConstructor,
        source: CallSiteKind::Path,
        target: CallTargetKind::Variant,
        target_relation: "variant",
    },
];

impl CallTargetFamily {
    fn matches(self, target: &CallTargetRow) -> bool {
        self.relation == target.relation
            && self.source == target.source_kind
            && self.target == target.target_kind
    }
}

pub fn valid_call_target_family(relation_kind: &str, source_kind: &str, target_kind: &str) -> bool {
    let (Ok(relation), Ok(source), Ok(target)) = (
        CallRelationKind::from_str(relation_kind),
        CallSiteKind::from_str(source_kind),
        CallTargetKind::from_str(target_kind),
    ) else {
        return false;
    };

    VALID_CALL_TARGET_FAMILIES.iter().any(|family| {
        family.relation == relation && family.source == source && family.target == target
    })
}

pub fn call_target_endpoint_relation(target_kind: &str) -> Option<&'static str> {
    let Ok(target) = CallTargetKind::from_str(target_kind) else {
        return None;
    };

    VALID_CALL_TARGET_FAMILIES
        .iter()
        .find(|family| family.target == target)
        .map(|family| family.target_relation)
}

pub(super) fn valid_call_target_rules() -> String {
    let mut rules = String::new();
    for family in VALID_CALL_TARGET_FAMILIES {
        let relation = family.relation.as_str();
        let source = family.source.as_str();
        let target = family.target.as_str();
        let target_relation = family.target_relation;
        writeln!(
            &mut rules,
            r#"
            valid_target[target_id, relation_kind, source_kind, target_kind] :=
                relation_kind = "{relation}",
                source_kind = "{source}",
                target_kind = "{target}",
                *{target_relation} {{ id: target_id @ 'NOW' }}
"#
        )
        .expect("writing call target rule to String should not fail");
    }
    rules
}

pub(super) fn valid_call_target(target: &CallTargetRow) -> bool {
    VALID_CALL_TARGET_FAMILIES
        .iter()
        .any(|family| family.matches(target))
}
