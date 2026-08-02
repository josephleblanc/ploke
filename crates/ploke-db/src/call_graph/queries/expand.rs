use std::collections::{BTreeMap, btree_map::Entry};

use uuid::Uuid;

use crate::{Database, DbError};

use super::super::{
    CallContextCandidate, CallContextOptions, CallContextRelation, CallContextSeed,
    CallPathOptions, CallStatusKind,
};

impl Database {
    /// Expands an owner or target seed through bounded resolved call paths.
    ///
    /// This is the multi-hop counterpart to [`Self::expand_call_context`].
    /// It returns the same candidate shape so RAG/tool callers can ask
    /// call-chain questions without manually post-processing [`Self::call_paths_from_owner`]
    /// or [`Self::call_paths_to_target`]. Targetless rows remain visible through
    /// direct call-context APIs but do not become path candidates.
    pub fn expand_call_path_context(
        &self,
        seed: CallContextSeed,
        options: CallPathOptions,
    ) -> Result<Vec<CallContextCandidate>, DbError> {
        if options.max_depth == 0 || options.max_paths == 0 {
            return Ok(Vec::new());
        }

        let mut candidates = BTreeMap::new();
        match seed {
            CallContextSeed::Owner(owner_id) => {
                for path in self.call_paths_from_owner(owner_id, options)? {
                    let Some(edge) = path.edges.last() else {
                        continue;
                    };
                    insert_call_context_candidate(
                        &mut candidates,
                        CallContextCandidate {
                            node_id: path.end_id,
                            relation: CallContextRelation::OutgoingTarget,
                            call_site_id: edge.call_site_id,
                            target_id: path.end_id,
                            distance: path.depth,
                        },
                    );
                }
            }
            CallContextSeed::Target(target_id) => {
                for path in self.call_paths_to_target(target_id, options)? {
                    let Some(edge) = path.edges.first() else {
                        continue;
                    };
                    insert_call_context_candidate(
                        &mut candidates,
                        CallContextCandidate {
                            node_id: path.start_id,
                            relation: CallContextRelation::IncomingCaller,
                            call_site_id: edge.call_site_id,
                            target_id,
                            distance: path.depth,
                        },
                    );
                }
            }
        }

        let mut candidates = candidates.into_values().collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            (
                candidate.distance,
                candidate.relation,
                candidate.node_id.as_u128(),
                candidate.call_site_id.as_u128(),
            )
        });
        candidates.truncate(options.max_paths);
        Ok(candidates)
    }

    /// Expands an owner or target seed into graphRAG call-context candidates.
    ///
    /// This is the application-facing layer over the lower-level call-site and
    /// target-centered helpers. It only promotes resolved call edges as context
    /// candidates; unsupported, external, unresolved, or ambiguous rows remain
    /// visible through [`Self::call_context_for_owner`] but do not become
    /// traversal targets.
    pub fn expand_call_context(
        &self,
        seed: CallContextSeed,
        options: CallContextOptions,
    ) -> Result<Vec<CallContextCandidate>, DbError> {
        if options.max_candidates == 0 {
            return Ok(Vec::new());
        }

        let mut candidates = BTreeMap::new();

        match seed {
            CallContextSeed::Owner(owner_id) if options.include_outgoing_targets => {
                for row in self.call_context_for_owner(owner_id)? {
                    if row.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    for target in row.targets {
                        insert_call_context_candidate(
                            &mut candidates,
                            CallContextCandidate {
                                node_id: target.target_id,
                                relation: CallContextRelation::OutgoingTarget,
                                call_site_id: row.site.id,
                                target_id: target.target_id,
                                distance: 1,
                            },
                        );
                    }
                }
            }
            CallContextSeed::Target(target_id) if options.include_incoming_callers => {
                for caller in self.callers_for_target(target_id)? {
                    if caller.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    insert_call_context_candidate(
                        &mut candidates,
                        CallContextCandidate {
                            node_id: caller.site.owner_id,
                            relation: CallContextRelation::IncomingCaller,
                            call_site_id: caller.site.id,
                            target_id: caller.target.target_id,
                            distance: 1,
                        },
                    );
                }
            }
            CallContextSeed::Owner(_) | CallContextSeed::Target(_) => {}
        }

        let mut candidates = candidates.into_values().collect::<Vec<_>>();
        candidates.sort_by_key(|candidate| {
            (
                candidate.distance,
                candidate.relation,
                candidate.node_id.as_u128(),
                candidate.call_site_id.as_u128(),
            )
        });
        candidates.truncate(options.max_candidates);
        Ok(candidates)
    }
}

fn insert_call_context_candidate(
    candidates: &mut BTreeMap<(Uuid, CallContextRelation), CallContextCandidate>,
    candidate: CallContextCandidate,
) {
    match candidates.entry((candidate.node_id, candidate.relation)) {
        Entry::Vacant(entry) => {
            entry.insert(candidate);
        }
        Entry::Occupied(mut entry) => {
            if call_context_candidate_rank(&candidate) < call_context_candidate_rank(entry.get()) {
                entry.insert(candidate);
            }
        }
    }
}

fn call_context_candidate_rank(candidate: &CallContextCandidate) -> (u32, u128, u128) {
    (
        candidate.distance,
        candidate.call_site_id.as_u128(),
        candidate.target_id.as_u128(),
    )
}
