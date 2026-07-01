use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct ConstructorCase {
    pub(in crate::unit) label: &'static str,
    pub(in crate::unit) fixture: &'static str,
    pub(in crate::unit) domain: &'static str,
    pub(in crate::unit) owner: ConstructorOwner,
    pub(in crate::unit) path: &'static [&'static str],
    pub(in crate::unit) target: ConstructorTarget,
    pub(in crate::unit) relation: CallRelationKind,
    pub(in crate::unit) endpoint: CallTargetKind,
    pub(in crate::unit) source_suffix: &'static str,
}

#[derive(Clone, Copy)]
pub(in crate::unit) enum ConstructorOwner {
    FunctionInModule {
        module: &'static [&'static str],
        name: &'static str,
    },
    InherentMethod {
        self_type: &'static str,
        name: &'static str,
    },
}

#[derive(Clone, Copy)]
pub(in crate::unit) enum ConstructorTarget {
    Struct {
        name: &'static str,
    },
    Variant {
        enum_name: &'static str,
        variant_name: &'static str,
    },
}

pub(in crate::unit) struct ResolvedConstructor {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) target: Uuid,
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) span: (u32, u32),
    pub(in crate::unit) proof_count: usize,
    pub(in crate::unit) owner_str: String,
    pub(in crate::unit) target_str: String,
    pub(in crate::unit) site_str: String,
}

const CONSTRUCTOR_CASES: &[ConstructorCase] = &[
    ConstructorCase {
        label: "tuple-struct constructor",
        fixture: "fixture_call_graph",
        domain: "bd:fixture-call-graph",
        owner: ConstructorOwner::FunctionInModule {
            module: &["crate"],
            name: "call_new_type_constructor",
        },
        path: &["NewType"],
        target: ConstructorTarget::Struct { name: "NewType" },
        relation: CallRelationKind::TupleStructConstructor,
        endpoint: CallTargetKind::Struct,
        source_suffix: "fixture_call_graph/src/lib.rs",
    },
    ConstructorCase {
        label: "Self tuple-struct constructor",
        fixture: "fixture_call_graph",
        domain: "bd:fixture-call-graph",
        owner: ConstructorOwner::InherentMethod {
            self_type: "SelfTupleConstructor",
            name: "make",
        },
        path: &["Self"],
        target: ConstructorTarget::Struct {
            name: "SelfTupleConstructor",
        },
        relation: CallRelationKind::TupleStructConstructor,
        endpoint: CallTargetKind::Struct,
        source_suffix: "fixture_call_graph/src/lib.rs",
    },
    ConstructorCase {
        label: "enum-variant constructor",
        fixture: "fixture_nodes",
        domain: "bd:fixture-nodes",
        owner: ConstructorOwner::FunctionInModule {
            module: &["crate", "imports"],
            name: "use_imported_items",
        },
        path: &["EnumWithData", "Variant1"],
        target: ConstructorTarget::Variant {
            enum_name: "EnumWithData",
            variant_name: "Variant1",
        },
        relation: CallRelationKind::EnumVariantConstructor,
        endpoint: CallTargetKind::Variant,
        source_suffix: "fixture_nodes/src/imports.rs",
    },
];

pub(in crate::unit) fn constructor_cases() -> &'static [ConstructorCase] {
    CONSTRUCTOR_CASES
}

impl ConstructorOwner {
    fn id(self, db: &Database) -> Result<Uuid, DbError> {
        match self {
            Self::FunctionInModule { module, name } => {
                function_id_by_name_in_module(db, module, name)
            }
            Self::InherentMethod { self_type, name } => {
                method_id_by_impl_self_type_name(db, self_type, name)
            }
        }
    }
}

impl ConstructorTarget {
    fn id(self, db: &Database) -> Result<Uuid, DbError> {
        match self {
            Self::Struct { name } => struct_id_by_name(db, name),
            Self::Variant {
                enum_name,
                variant_name,
            } => variant_id_by_enum_and_variant_names(db, enum_name, variant_name),
        }
    }
}

pub(in crate::unit) fn assert_constructor_context(
    db: &Database,
    case: &ConstructorCase,
) -> Result<ResolvedConstructor, DbError> {
    let owner = case.owner.id(db)?;
    let target = case.target.id(db)?;
    let context = db.call_context_for_owner(owner)?;
    let proof_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let row = row_by_path(&context, case.path);
    assert_resolved_target(
        row,
        target,
        case.relation,
        CallSiteKind::Path,
        case.endpoint,
    );

    Ok(ResolvedConstructor {
        owner,
        target,
        site: row.site.id,
        span: row.site.span,
        proof_count,
        owner_str: owner.to_string(),
        target_str: target.to_string(),
        site_str: row.site.id.to_string(),
    })
}

pub(in crate::unit) fn assert_constructor_callers(
    db: &Database,
    case: &ConstructorCase,
    resolved: &ResolvedConstructor,
) -> Result<Vec<CallCallerRow>, DbError> {
    let callers = db.callers_for_target(resolved.target)?;
    let caller = caller_by_owner_kind_path(&callers, resolved.owner, CallSiteKind::Path, case.path);
    assert_eq!(caller.site.id, resolved.site);
    assert_eq!(
        caller.target.relation, case.relation,
        "{} caller relation",
        case.label
    );
    assert_eq!(caller.target.source_kind, CallSiteKind::Path);
    assert_eq!(
        caller.target.target_kind, case.endpoint,
        "{} caller endpoint kind",
        case.label
    );
    Ok(callers)
}
