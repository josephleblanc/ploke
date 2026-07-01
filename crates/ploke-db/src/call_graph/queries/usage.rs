use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{CallImpactReport, CallNodeInfo, CallPath, CallPathOptions, CallReachReport};

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
        let callers = node_info_for_paths(self, &paths, "caller", |path| Some(path.start_id))?;
        let direct_callers = node_info_for_paths(self, &paths, "caller", |path| {
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

    /// Summarizes bounded outgoing call paths for navigation/reachability questions.
    ///
    /// Like [`Self::call_impact_for_target`], this report is resolved-path-only.
    /// Unsupported, external, unresolved, or ambiguous targetless callsites stay
    /// visible through direct call-context APIs and are not promoted into reach
    /// paths.
    pub fn call_reach_for_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<CallReachReport, DbError> {
        let owner = self.call_node_info(owner_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for reach owner {owner_id}"
            ))
        })?;
        let paths = self.call_paths_from_owner(owner_id, options)?;
        let callees = node_info_for_paths(self, &paths, "callee", |path| Some(path.end_id))?;
        let direct_callees = node_info_for_paths(self, &paths, "callee", |path| {
            (path.depth == 1).then_some(path.end_id)
        })?;
        let public_callees = callees
            .iter()
            .filter(|callee| callee.is_public)
            .cloned()
            .collect();

        Ok(CallReachReport {
            owner,
            paths,
            callees,
            direct_callees,
            public_callees,
        })
    }
}

fn node_info_for_paths(
    db: &Database,
    paths: &[CallPath],
    label: &str,
    select: impl Fn(&CallPath) -> Option<Uuid>,
) -> Result<Vec<CallNodeInfo>, DbError> {
    let mut nodes = BTreeMap::new();
    for path in paths {
        let Some(node_id) = select(path) else {
            continue;
        };
        let info = db.call_node_info(node_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for {label} {node_id}"
            ))
        })?;
        nodes.entry(node_id).or_insert(info);
    }

    let mut nodes = nodes.into_values().collect::<Vec<_>>();
    nodes.sort_by_key(|node| (!node.is_public, node.name.clone(), node.id.as_u128()));
    Ok(nodes)
}
