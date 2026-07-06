use std::collections::{BTreeSet, VecDeque};

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
        let mut queue = VecDeque::new();
        queue.push_back((owner_id, Vec::new(), BTreeSet::from([owner_id])));

        while let Some((current, prefix, seen)) = queue.pop_front() {
            if prefix.len() as u32 >= options.max_depth {
                continue;
            }

            for edge in self.resolved_outgoing_call_edges(current)? {
                let mut edges = prefix.clone();
                edges.push(edge);
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
                queue.push_back((edge.callee_id, edges, next_seen));
            }
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
        let mut queue = VecDeque::new();
        queue.push_back((target_id, Vec::new(), BTreeSet::from([target_id])));

        while let Some((current, suffix, seen)) = queue.pop_front() {
            if suffix.len() as u32 >= options.max_depth {
                continue;
            }

            for edge in self.resolved_incoming_call_edges(current)? {
                let mut edges = Vec::with_capacity(suffix.len() + 1);
                edges.push(edge);
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
                queue.push_back((edge.caller_id, edges, next_seen));
            }
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

    fn resolved_outgoing_call_edges(&self, owner_id: Uuid) -> Result<Vec<CallPathEdge>, DbError> {
        let mut edges = Vec::new();
        for row in self.call_context_for_owner(owner_id)? {
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
        Ok(edges)
    }

    fn resolved_incoming_call_edges(&self, target_id: Uuid) -> Result<Vec<CallPathEdge>, DbError> {
        let mut edges = Vec::new();
        for caller in self.callers_for_target(target_id)? {
            if caller.status.status != CallStatusKind::Resolved {
                continue;
            }

            edges.push(CallPathEdge {
                caller_id: caller.site.owner_id,
                callee_id: caller.target.target_id,
                call_site_id: caller.site.id,
                span: caller.site.span,
                relation: caller.target.relation,
                source_kind: caller.target.source_kind,
                target_kind: caller.target.target_kind,
            });
        }
        sort_call_edges(&mut edges);
        edges.dedup();
        Ok(edges)
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
