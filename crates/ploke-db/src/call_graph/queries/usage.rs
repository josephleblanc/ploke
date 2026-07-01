use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{
    Database, DbError,
    database::{to_string, to_string_list},
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
};

use super::super::{
    CallContextRow, CallImpactReport, CallNodeInfo, CallPath, CallPathEdge, CallPathOptions,
    CallReachReport, CallRelationKind, CallSiteBucket, CallSiteKind, CallStatusKind,
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
        let callsite_buckets = callsite_buckets(target_id, &direct_call_sites)?;
        let public_callers: Vec<CallNodeInfo> = callers
            .iter()
            .filter(|caller| caller.is_public)
            .cloned()
            .collect();
        let (mut test_callers, mut non_test_callers) = (Vec::new(), Vec::new());
        for caller in &callers {
            if is_test_node(self, caller.id)? {
                test_callers.push(caller.clone());
            } else {
                non_test_callers.push(caller.clone());
            }
        }
        let sources = sources_for_summary(
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
            callsite_buckets,
            public_callers,
            test_callers,
            non_test_callers,
            source_files: sources.files,
            source_modules: sources.modules,
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
        let boundary_call_sites = boundary_call_sites(self, &owner, &direct_call_sites)?;
        let boundary_edges = boundary_edges_for_paths(self, &paths)?;
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
        let sources = sources_for_summary(
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
            boundary_call_sites,
            boundary_edges,
            public_callees,
            frontier_calls,
            external_frontier_calls,
            unsupported_frontier_calls,
            source_files: sources.files,
            source_modules: sources.modules,
        })
    }
}

fn is_test_node(db: &Database, node_id: Uuid) -> Result<bool, DbError> {
    let mut params = BTreeMap::new();
    params.insert("node_id".to_string(), DataValue::Uuid(UuidWrapper(node_id)));

    let script = format!(
        r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

owner[id] := id = $node_id, *function{{ id @ 'NOW' }}
owner[id] := id = $node_id, *method{{ id @ 'NOW' }}
owner[id] := id = $node_id, *const{{ id @ 'NOW' }}
owner[id] := id = $node_id, *static{{ id @ 'NOW' }}

?[module_path, file_path] :=
  owner[id],
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_id],
  *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );

    let rows = db.run_script(&script, params, ScriptMutability::Immutable)?;
    for row in rows.rows {
        let module_path = to_string_list(&row[0])?;
        let file_path = to_string(&row[1])?;
        if module_path.iter().any(|segment| segment == "tests")
            || file_path.contains("/tests/")
            || file_path.contains("\\tests\\")
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn boundary_call_sites(
    db: &Database,
    owner: &CallNodeInfo,
    rows: &[CallContextRow],
) -> Result<Vec<CallContextRow>, DbError> {
    let mut out = Vec::new();
    for row in rows {
        let mut crosses = false;
        for target in &row.targets {
            let Some(callee) = db.call_node_info(target.target_id)? else {
                return Err(DbError::Cozo(format!(
                    "missing call graph node metadata for boundary target {}",
                    target.target_id
                )));
            };
            if callee.module_path != owner.module_path {
                crosses = true;
                break;
            }
        }
        if crosses {
            out.push(row.clone());
        }
    }
    Ok(out)
}

fn callsite_buckets(
    target_id: Uuid,
    rows: &[CallContextRow],
) -> Result<Vec<CallSiteBucket>, DbError> {
    let mut buckets = Vec::<CallSiteBucket>::new();
    for row in rows {
        let mut matched = false;
        for target in row
            .targets
            .iter()
            .filter(|target| target.target_id == target_id)
        {
            matched = true;
            if let Some(bucket) = buckets
                .iter_mut()
                .find(|bucket| bucket.kind == row.site.kind && bucket.relation == target.relation)
            {
                bucket.count += 1;
            } else {
                buckets.push(CallSiteBucket {
                    kind: row.site.kind,
                    relation: target.relation,
                    count: 1,
                });
            }
        }
        if !matched {
            return Err(DbError::Cozo(format!(
                "target-centered callsite {} missing requested target {target_id}",
                row.site.id
            )));
        }
    }
    buckets.sort_by_key(|bucket| {
        (
            site_kind_rank(bucket.kind),
            relation_kind_rank(bucket.relation),
        )
    });
    Ok(buckets)
}

fn site_kind_rank(kind: CallSiteKind) -> u8 {
    match kind {
        CallSiteKind::Path => 0,
        CallSiteKind::Method => 1,
        CallSiteKind::Dynamic => 2,
        CallSiteKind::Macro => 3,
    }
}

fn relation_kind_rank(kind: CallRelationKind) -> u8 {
    match kind {
        CallRelationKind::Function => 0,
        CallRelationKind::DynamicFunction => 1,
        CallRelationKind::Method => 2,
        CallRelationKind::AssociatedFunction => 3,
        CallRelationKind::TupleStructConstructor => 4,
        CallRelationKind::EnumVariantConstructor => 5,
    }
}

fn boundary_edges_for_paths(
    db: &Database,
    paths: &[CallPath],
) -> Result<Vec<CallPathEdge>, DbError> {
    let mut out = BTreeMap::new();
    for path in paths {
        for edge in &path.edges {
            let caller = db.call_node_info(edge.caller_id)?.ok_or_else(|| {
                DbError::Cozo(format!(
                    "missing call graph node metadata for boundary caller {}",
                    edge.caller_id
                ))
            })?;
            let callee = db.call_node_info(edge.callee_id)?.ok_or_else(|| {
                DbError::Cozo(format!(
                    "missing call graph node metadata for boundary callee {}",
                    edge.callee_id
                ))
            })?;
            if caller.module_path != callee.module_path {
                out.entry(edge.call_site_id).or_insert(*edge);
            }
        }
    }
    Ok(out.into_values().collect())
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

struct SummarySources {
    files: Vec<String>,
    modules: Vec<Vec<String>>,
}

fn sources_for_summary<'a>(
    db: &Database,
    paths: &[CallPath],
    nodes: impl Iterator<Item = &'a CallNodeInfo>,
) -> Result<SummarySources, DbError> {
    let mut files = BTreeSet::new();
    let mut modules = BTreeSet::new();
    for node in nodes {
        files.insert(node.file_path.clone());
        modules.insert(node.module_path.clone());
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
        modules.insert(info.module_path);
    }

    Ok(SummarySources {
        files: files.into_iter().collect(),
        modules: modules.into_iter().collect(),
    })
}
