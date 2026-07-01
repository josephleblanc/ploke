use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallContextRow, CallImpactReport, CallNodeInfo, CallPath, CallPathOptions, CallReachReport,
    CallStatusKind,
};

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
        let direct_call_sites = self.call_context_for_target(target_id)?;
        let public_callers: Vec<CallNodeInfo> = callers
            .iter()
            .filter(|caller| caller.is_public)
            .cloned()
            .collect();
        let source_files = source_files_for_summary(
            self,
            &paths,
            std::iter::once(&target)
                .chain(callers.iter())
                .chain(direct_callers.iter())
                .chain(public_callers.iter()),
        )?;

        Ok(CallImpactReport {
            target,
            paths,
            callers,
            direct_callers,
            direct_call_sites,
            public_callers,
            source_files,
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
        let direct_call_sites = resolved_direct_call_sites_for_owner(self, owner_id)?;
        let public_callees: Vec<CallNodeInfo> = callees
            .iter()
            .filter(|callee| callee.is_public)
            .cloned()
            .collect();
        let frontier_calls = frontier_calls_for_paths(self, owner_id, &paths)?;
        let external_frontier_calls = frontier_calls
            .iter()
            .filter(|row| row.status.status == CallStatusKind::External)
            .cloned()
            .collect();
        let unsupported_frontier_calls = frontier_calls
            .iter()
            .filter(|row| row.status.status == CallStatusKind::Unsupported)
            .cloned()
            .collect();
        let source_files = source_files_for_summary(
            self,
            &paths,
            std::iter::once(&owner)
                .chain(callees.iter())
                .chain(direct_callees.iter())
                .chain(public_callees.iter()),
        )?;

        Ok(CallReachReport {
            owner,
            paths,
            callees,
            direct_callees,
            direct_call_sites,
            public_callees,
            frontier_calls,
            external_frontier_calls,
            unsupported_frontier_calls,
            source_files,
        })
    }
}

fn frontier_calls_for_paths(
    db: &Database,
    owner_id: Uuid,
    paths: &[CallPath],
) -> Result<Vec<CallContextRow>, DbError> {
    let mut owners = BTreeSet::from([owner_id]);
    for path in paths {
        for edge in &path.edges {
            owners.insert(edge.caller_id);
            owners.insert(edge.callee_id);
        }
    }

    let mut rows = BTreeMap::new();
    for owner in owners {
        for row in db.call_context_for_owner(owner)? {
            if row.status.status == CallStatusKind::Resolved {
                continue;
            }
            rows.entry(row.site.id).or_insert(row);
        }
    }

    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_by_key(|row| {
        (
            row.site.owner_id.as_u128(),
            row.site.span,
            row.site.id.as_u128(),
        )
    });
    Ok(rows)
}

fn resolved_direct_call_sites_for_owner(
    db: &Database,
    owner_id: Uuid,
) -> Result<Vec<CallContextRow>, DbError> {
    db.call_context_for_owner(owner_id).map(|rows| {
        rows.into_iter()
            .filter(|row| row.status.status == CallStatusKind::Resolved)
            .collect()
    })
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

fn source_files_for_summary<'a>(
    db: &Database,
    paths: &[CallPath],
    nodes: impl Iterator<Item = &'a CallNodeInfo>,
) -> Result<Vec<String>, DbError> {
    let mut files = BTreeSet::new();
    for node in nodes {
        files.insert(node.file_path.clone());
    }

    let mut path_nodes = BTreeSet::new();
    for path in paths {
        path_nodes.insert(path.start_id);
        path_nodes.insert(path.end_id);
        for edge in &path.edges {
            path_nodes.insert(edge.caller_id);
            path_nodes.insert(edge.callee_id);
        }
    }

    for node_id in path_nodes {
        let info = db.call_node_info(node_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for summary source file {node_id}"
            ))
        })?;
        files.insert(info.file_path);
    }

    Ok(files.into_iter().collect())
}
