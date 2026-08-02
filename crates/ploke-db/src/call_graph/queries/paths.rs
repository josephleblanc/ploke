use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{CallPath, CallPathEdge, CallPathOptions, CallStatusKind};

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
        let mut edges = Vec::new();
        for row in self.call_context_for_owners(owner_ids)? {
            if row.status.status != CallStatusKind::Resolved {
                continue;
            }
            for target in row.targets {
                edges.push(CallPathEdge {
                    caller_id: row.site.owner_id,
                    callee_id: target.target_id,
                    call_site_id: row.site.id,
                    span: row.site.span,
                    relation: target.relation,
                    source_kind: target.source_kind,
                    target_kind: target.target_kind,
                });
            }
        }
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
        let mut edges = Vec::new();
        for target_id in target_ids {
            for row in self.call_context_for_target(*target_id)? {
                if row.status.status != CallStatusKind::Resolved {
                    continue;
                }
                for target in row.targets {
                    if target.target_id != *target_id {
                        continue;
                    }
                    edges.push(CallPathEdge {
                        caller_id: row.site.owner_id,
                        callee_id: target.target_id,
                        call_site_id: row.site.id,
                        span: row.site.span,
                        relation: target.relation,
                        source_kind: target.source_kind,
                        target_kind: target.target_kind,
                    });
                }
            }
        }
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
