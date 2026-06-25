use ploke_db::{Database, DbError};
use uuid::Uuid;

use super::{
    SiteSeed, insert_call_site, insert_edge, insert_owner_source, insert_relation, insert_status,
};

const DEFAULT_PATH: &[&str] = &["crate", "helper"];

#[derive(Clone, Copy)]
pub(in crate::unit) struct ResolvedGraphSeed<'a> {
    pub(in crate::unit) owner: Uuid,
    pub(in crate::unit) module: Uuid,
    pub(in crate::unit) site: Uuid,
    pub(in crate::unit) target: Uuid,
    pub(in crate::unit) file: Option<&'a str>,
    pub(in crate::unit) span: (i64, i64),
    pub(in crate::unit) path: &'a [&'a str],
}

impl<'a> ResolvedGraphSeed<'a> {
    pub(in crate::unit) fn path_call(
        owner: Uuid,
        module: Uuid,
        site: Uuid,
        target: Uuid,
        file: Option<&'a str>,
    ) -> Self {
        Self {
            owner,
            module,
            site,
            target,
            file,
            span: (10, 24),
            path: DEFAULT_PATH,
        }
    }
}

pub(in crate::unit) fn insert_resolved_graph(
    db: &Database,
    seed: ResolvedGraphSeed<'_>,
) -> Result<(), DbError> {
    if let Some(file) = seed.file {
        insert_owner_source(db, seed.owner, seed.module, file)?;
    }
    insert_call_site(
        db,
        SiteSeed {
            id: seed.site,
            owner: seed.owner,
            kind: "Path",
            span: seed.span,
            path: Some(seed.path.to_vec()),
            method: None,
            macro_name: None,
            receiver: None,
            arg_count: Some(0),
            generic_arg_count: Some(0),
        },
    )?;
    insert_edge(db, seed.owner, seed.site, "Path")?;
    insert_relation(db, seed.site, seed.target, "Function", "Path", "Function")?;
    insert_status(db, seed.site, "Path", "Resolved", Some("LocalExact"))?;
    Ok(())
}
