use super::*;

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetProofSite {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) site: Uuid,
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct ProofSiteCase<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) kind: CallSiteKind,
    pub(in crate::unit) path: &'a [&'a str],
    pub(in crate::unit) relation: CallRelationKind,
    pub(in crate::unit) endpoint: CallTargetKind,
}

impl<'a> ProofSiteCase<'a> {
    pub(in crate::unit) fn path(
        owner: Uuid,
        path: &'a [&'a str],
        relation: CallRelationKind,
        endpoint: CallTargetKind,
    ) -> Self {
        Self {
            owner,
            kind: CallSiteKind::Path,
            path,
            relation,
            endpoint,
        }
    }

    pub(in crate::unit) fn dynamic(
        owner: Uuid,
        path: &'a [&'a str],
        relation: CallRelationKind,
        endpoint: CallTargetKind,
    ) -> Self {
        Self {
            owner,
            kind: CallSiteKind::Dynamic,
            path,
            relation,
            endpoint,
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::unit) struct ProofMethodCase<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) method: &'a str,
    pub(in crate::unit) receiver: &'a CallReceiver,
    pub(in crate::unit) relation: CallRelationKind,
    pub(in crate::unit) endpoint: CallTargetKind,
}

impl<'a> ProofMethodCase<'a> {
    pub(in crate::unit) fn method(
        owner: Uuid,
        method: &'a str,
        receiver: &'a CallReceiver,
    ) -> Self {
        Self {
            owner,
            method,
            receiver,
            relation: CallRelationKind::Method,
            endpoint: CallTargetKind::Method,
        }
    }
}

pub(in crate::unit) fn assert_proof_site_cases(
    db: &Database,
    callers: &[CallCallerRow],
    cases: &[ProofSiteCase<'_>],
) -> Result<Vec<TargetProofSite>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        let row = row_by_kind_path(&context, case.kind, case.path);
        let caller = caller_by_owner_kind_path(callers, case.owner, case.kind, case.path);
        assert_eq!(caller.site.id, row.site.id);
        assert_eq!(caller.target.relation, case.relation);
        assert_eq!(caller.target.source_kind, case.kind);
        assert_eq!(caller.target.target_kind, case.endpoint);
        expected.push(TargetProofSite {
            owner: case.owner,
            site: row.site.id,
        });
    }

    Ok(expected)
}

pub(in crate::unit) fn assert_proof_method_cases(
    db: &Database,
    callers: &[CallCallerRow],
    cases: &[ProofMethodCase<'_>],
) -> Result<Vec<TargetProofSite>, DbError> {
    let mut expected = Vec::new();

    for case in cases {
        let context = db.call_context_for_owner(case.owner)?;
        let row = row_by_method_receiver(&context, case.method, case.receiver);
        let caller =
            caller_by_owner_method_receiver(callers, case.owner, case.method, case.receiver);
        assert_eq!(caller.site.id, row.site.id);
        assert_eq!(caller.target.relation, case.relation);
        assert_eq!(caller.target.source_kind, CallSiteKind::Method);
        assert_eq!(caller.target.target_kind, case.endpoint);
        expected.push(TargetProofSite {
            owner: case.owner,
            site: row.site.id,
        });
    }

    Ok(expected)
}
