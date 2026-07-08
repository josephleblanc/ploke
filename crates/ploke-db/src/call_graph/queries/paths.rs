use std::collections::{BTreeMap, BTreeSet};

use cozo::ScriptMutability;
use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallPath, CallPathEdge, CallPathOptions,
    decode::{decode_site, decode_target},
    families::valid_call_target_rules,
};

impl Database {
    /// Returns bounded resolved call paths that start at an owner node.
    ///
    /// Only resolved local call edges are traversed. Targetless rows remain
    /// visible through direct call-context APIs but do not become path edges.
    pub fn call_paths_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallPath>, DbError> {
        if options.max_depth == 0 || options.max_paths == 0 {
            return Ok(Vec::new());
        }

        let mut paths = Vec::new();
        let mut states = vec![(owner_id, Vec::new(), BTreeSet::from([owner_id]))];

        while !states.is_empty() {
            let owners = states
                .iter()
                .filter(|(_, prefix, _)| (prefix.len() as u32) < options.max_depth)
                .map(|(current, _, _)| *current)
                .collect::<BTreeSet<_>>();
            let edges_by_owner = self.resolved_outgoing_call_edges_for_owners(&owners)?;
            let mut next_states = Vec::new();

            for (current, prefix, seen) in states {
                if prefix.len() as u32 >= options.max_depth {
                    continue;
                }

                for edge in edges_by_owner.get(&current).into_iter().flatten() {
                    let mut edges = prefix.clone();
                    edges.push(*edge);
                    paths.push(CallPath {
                        start_id: owner_id,
                        end_id: edge.callee_id,
                        depth: edges.len() as u32,
                        edges: edges.clone(),
                    });
                    if paths.len() >= options.max_paths {
                        sort_call_paths(&mut paths);
                        return Ok(paths);
                    }

                    if seen.contains(&edge.callee_id) {
                        continue;
                    }

                    if edges.len() as u32 >= options.max_depth {
                        continue;
                    }

                    let mut next_seen = seen.clone();
                    next_seen.insert(edge.callee_id);
                    next_states.push((edge.callee_id, edges, next_seen));
                }
            }
            states = next_states;
        }

        sort_call_paths(&mut paths);
        Ok(paths)
    }

    /// Returns bounded resolved call paths that end at a target node.
    ///
    /// Path edges are ordered in normal call direction: caller to callee.
    pub fn call_paths_to_target(
        &self,
        target_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallPath>, DbError> {
        if options.max_depth == 0 || options.max_paths == 0 {
            return Ok(Vec::new());
        }

        let mut paths = Vec::new();
        let mut states = vec![(target_id, Vec::new(), BTreeSet::from([target_id]))];

        while !states.is_empty() {
            let targets = states
                .iter()
                .filter(|(_, suffix, _)| (suffix.len() as u32) < options.max_depth)
                .map(|(current, _, _)| *current)
                .collect::<BTreeSet<_>>();
            let edges_by_target = self.resolved_incoming_call_edges_for_targets(&targets)?;
            let mut next_states = Vec::new();

            for (current, suffix, seen) in states {
                if suffix.len() as u32 >= options.max_depth {
                    continue;
                }

                for edge in edges_by_target.get(&current).into_iter().flatten() {
                    let mut edges = Vec::with_capacity(suffix.len() + 1);
                    edges.push(*edge);
                    edges.extend(suffix.iter().copied());
                    paths.push(CallPath {
                        start_id: edge.caller_id,
                        end_id: target_id,
                        depth: edges.len() as u32,
                        edges: edges.clone(),
                    });
                    if paths.len() >= options.max_paths {
                        sort_call_paths(&mut paths);
                        return Ok(paths);
                    }

                    if seen.contains(&edge.caller_id) {
                        continue;
                    }

                    if edges.len() as u32 >= options.max_depth {
                        continue;
                    }

                    let mut next_seen = seen.clone();
                    next_seen.insert(edge.caller_id);
                    next_states.push((edge.caller_id, edges, next_seen));
                }
            }
            states = next_states;
        }

        sort_call_paths(&mut paths);
        Ok(paths)
    }

    /// Returns bounded resolved call paths from `owner_id` to `target_id`.
    ///
    /// This is the direct reachability helper over [`Self::call_paths_from_owner`].
    /// It preserves the same resolved-only traversal semantics and path ordering.
    pub fn call_paths_between(
        &self,
        owner_id: Uuid,
        target_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallPath>, DbError> {
        let mut paths = self.call_paths_from_owner(owner_id, options)?;
        paths.retain(|path| path.end_id == target_id);
        Ok(paths)
    }

    /// Returns bounded resolved call paths that start at `owner_id` and return
    /// to that same owner.
    ///
    /// This is a recursion/cycle helper over [`Self::call_paths_from_owner`].
    /// It keeps the same resolved-only traversal semantics and does not promote
    /// targetless frontier rows into cycle evidence.
    pub fn call_cycles_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallPath>, DbError> {
        let mut paths = self.call_paths_from_owner(owner_id, options)?;
        paths.retain(|path| path.end_id == owner_id && !path.edges.is_empty());
        Ok(paths)
    }

    fn resolved_outgoing_call_edges_for_owners(
        &self,
        owner_ids: &BTreeSet<Uuid>,
    ) -> Result<BTreeMap<Uuid, Vec<CallPathEdge>>, DbError> {
        if owner_ids.is_empty() {
            return Ok(BTreeMap::new());
        }

        let input_rows = owner_ids
            .iter()
            .map(|id| format!("[to_uuid(\"{id}\")]"))
            .collect::<Vec<_>>()
            .join(",\n");

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            input_owner[owner_id] <- [
"#,
        );
        script.push_str(&input_rows);
        script.push_str(
            r#"
            ]

            ?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                unsafe_block,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count,
                relation_site_id,
                relation_target_id,
                relation_kind,
                source_kind,
                target_kind
            ] :=
                input_owner[owner_id],
                *call_site_edge {
                    source_id: owner_id,
                    target_id: id,
                    relation_kind: "BodyContainsCall",
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    unsafe_block,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                },
                *call_resolution_status {
                    source_id: id,
                    source_kind: call_kind,
                    status_kind: "Resolved" @ 'NOW'
                },
                relation_site_id = id,
                source_kind = call_kind,
                *call_relation {
                    source_id: relation_site_id,
                    target_id: relation_target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[relation_target_id, relation_kind, source_kind, target_kind]
            :sort span, id, relation_target_id"#,
        );

        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
        let mut edges = rows
            .rows
            .iter()
            .map(|row| {
                let site = decode_site(&row[..13])?;
                let target = decode_target(&row[13..])?;
                Ok(CallPathEdge {
                    caller_id: site.owner_id,
                    callee_id: target.target_id,
                    call_site_id: site.id,
                    span: site.span,
                    relation: target.relation,
                    source_kind: target.source_kind,
                    target_kind: target.target_kind,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        sort_call_edges(&mut edges);
        edges.dedup();
        let mut by_owner = BTreeMap::<Uuid, Vec<CallPathEdge>>::new();
        for edge in edges {
            by_owner.entry(edge.caller_id).or_default().push(edge);
        }
        Ok(by_owner)
    }

    fn resolved_incoming_call_edges_for_targets(
        &self,
        target_ids: &BTreeSet<Uuid>,
    ) -> Result<BTreeMap<Uuid, Vec<CallPathEdge>>, DbError> {
        if target_ids.is_empty() {
            return Ok(BTreeMap::new());
        }

        let input_rows = target_ids
            .iter()
            .map(|id| format!("[to_uuid(\"{id}\")]"))
            .collect::<Vec<_>>()
            .join(",\n");

        let mut script = valid_call_target_rules();
        script.push_str(
            r#"
            input_target[target_id] <- [
"#,
        );
        script.push_str(&input_rows);
        script.push_str(
            r#"
            ]

            ?[
                id,
                owner_id,
                call_kind,
                span,
                cfgs,
                unsafe_block,
                path,
                method_name,
                macro_name,
                receiver_kind,
                receiver_path,
                arg_count,
                generic_arg_count,
                relation_site_id,
                relation_target_id,
                relation_kind,
                source_kind,
                target_kind
            ] :=
                input_target[relation_target_id],
                *call_relation {
                    source_id: relation_site_id,
                    target_id: relation_target_id,
                    relation_kind,
                    source_kind,
                    target_kind @ 'NOW'
                },
                valid_target[relation_target_id, relation_kind, source_kind, target_kind],
                id = relation_site_id,
                call_kind = source_kind,
                *call_resolution_status {
                    source_id: id,
                    source_kind: call_kind,
                    status_kind: "Resolved" @ 'NOW'
                },
                *call_site_edge {
                    source_id: owner_id,
                    target_id: id,
                    relation_kind: "BodyContainsCall",
                    target_kind: call_kind @ 'NOW'
                },
                *call_site {
                    id,
                    owner_id,
                    call_kind,
                    span,
                    cfgs,
                    unsafe_block,
                    path,
                    method_name,
                    macro_name,
                    receiver_kind,
                    receiver_path,
                    arg_count,
                    generic_arg_count @ 'NOW'
                }
            :sort owner_id, span, relation_kind"#,
        );

        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
        let mut edges = rows
            .rows
            .iter()
            .map(|row| {
                let site = decode_site(&row[..13])?;
                let target = decode_target(&row[13..])?;
                Ok(CallPathEdge {
                    caller_id: site.owner_id,
                    callee_id: target.target_id,
                    call_site_id: site.id,
                    span: site.span,
                    relation: target.relation,
                    source_kind: target.source_kind,
                    target_kind: target.target_kind,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        sort_call_edges(&mut edges);
        edges.dedup();
        let mut by_target = BTreeMap::<Uuid, Vec<CallPathEdge>>::new();
        for edge in edges {
            by_target.entry(edge.callee_id).or_default().push(edge);
        }
        Ok(by_target)
    }
}

fn sort_call_paths(paths: &mut [CallPath]) {
    paths.sort_by_key(|path| {
        (
            path.depth,
            path.start_id.as_u128(),
            path.end_id.as_u128(),
            path.edges
                .first()
                .map(|edge| edge.call_site_id.as_u128())
                .unwrap_or_default(),
        )
    });
}

fn sort_call_edges(edges: &mut [CallPathEdge]) {
    edges.sort_by_key(|edge| {
        (
            edge.caller_id.as_u128(),
            edge.callee_id.as_u128(),
            edge.call_site_id.as_u128(),
        )
    });
}
