use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{CallImpactReport, CallNodeInfo, CallPath, CallPathOptions};

impl Database {
    /// Summarizes bounded incoming call paths for impact/navigation questions.
    ///
    /// The report is intentionally resolved-path-only: unsupported, external,
    /// unresolved, or ambiguous targetless callsites remain visible through
    /// direct call-context APIs and are not promoted into impact paths.
    pub fn call_impact_for_target(
        &self,
        target_id: Uuid,
        options: CallPathOptions,
    ) -> Result<CallImpactReport, DbError> {
        let target = self.call_node_info(target_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for impact target {target_id}"
            ))
        })?;
        let paths = self.call_paths_to_target(target_id, options)?;
        let callers = caller_info_for_paths(self, &paths, |path| Some(path.start_id))?;
        let direct_callers = caller_info_for_paths(self, &paths, |path| {
            (path.depth == 1).then_some(path.start_id)
        })?;
        let public_callers = callers
            .iter()
            .filter(|caller| caller.is_public)
            .cloned()
            .collect();

        Ok(CallImpactReport {
            target,
            paths,
            callers,
            direct_callers,
            public_callers,
        })
    }
}

fn caller_info_for_paths(
    db: &Database,
    paths: &[CallPath],
    select: impl Fn(&CallPath) -> Option<Uuid>,
) -> Result<Vec<CallNodeInfo>, DbError> {
    let mut callers = BTreeMap::new();
    for path in paths {
        let Some(node_id) = select(path) else {
            continue;
        };
        let info = db.call_node_info(node_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for caller {node_id}"
            ))
        })?;
        callers.entry(node_id).or_insert(info);
    }

    let mut callers = callers.into_values().collect::<Vec<_>>();
    callers.sort_by_key(|caller| (!caller.is_public, caller.name.clone(), caller.id.as_u128()));
    Ok(callers)
}
