use ploke_db::{Database, DbError};
use uuid::Uuid;

use super::{SiteSeed, insert_call_site, insert_edge, insert_owner_source, insert_status};

#[derive(Clone, Copy)]
pub(in crate::unit) struct TargetlessStatusSeed<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) module: Uuid,
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) file: Option<&'a str>,
    pub(in crate::unit) kind: &'a str,
    pub(in crate::unit) span: (i64, i64),
    pub(in crate::unit) path: Option<&'a [&'a str]>,
    pub(in crate::unit) args: Option<i64>,
    pub(in crate::unit) generics: Option<i64>,
    pub(in crate::unit) status: &'a str,
}

impl<'a> TargetlessStatusSeed<'a> {
    pub(in crate::unit) fn external(
        owner: Uuid,
        module: Uuid,
        site: Uuid,
        file: Option<&'a str>,
        path: &'a [&'a str],
        args: i64,
    ) -> Self {
        Self {
            owner,
            module,
            site,
            file,
            kind: "Path",
            span: (30, 40),
            path: Some(path),
            args: Some(args),
            generics: Some(0),
            status: "External",
        }
    }

    pub(in crate::unit) fn dynamic(
        owner: Uuid,
        module: Uuid,
        site: Uuid,
        file: Option<&'a str>,
    ) -> Self {
        Self {
            owner,
            module,
            site,
            file,
            kind: "Dynamic",
            span: (50, 60),
            path: None,
            args: Some(0),
            generics: None,
            status: "Unsupported",
        }
    }
}

pub(in crate::unit) fn insert_targetless_status(
    db: &Database,
    seed: TargetlessStatusSeed<'_>,
) -> Result<(), DbError> {
    if let Some(file) = seed.file {
        insert_owner_source(db, seed.owner, seed.module, file)?;
    }
    insert_call_site(
        db,
        SiteSeed {
            id: seed.site,
            owner: seed.owner,
            kind: seed.kind,
            span: seed.span,
            path: seed.path.map(|path| path.to_vec()),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: seed.args,
            generic_arg_count: seed.generics,
        },
    )?;
    insert_edge(db, seed.owner, seed.site, seed.kind)?;
    insert_status(db, seed.site, seed.kind, seed.status, None)?;
    Ok(())
}
