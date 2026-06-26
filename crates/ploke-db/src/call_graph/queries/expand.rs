use crate::{Database, DbError};

use super::super::{
    CallContextCandidate, CallContextOptions, CallContextRelation, CallContextSeed, CallStatusKind,
};

impl Database {
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

        let mut candidates = Vec::new();

        match seed {
            CallContextSeed::Owner(owner_id) if options.include_outgoing_targets => {
                for row in self.call_context_for_owner(owner_id)? {
                    if row.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.extend(row.targets.into_iter().map(|target| CallContextCandidate {
                        node_id: target.target_id,
                        relation: CallContextRelation::OutgoingTarget,
                        call_site_id: row.site.id,
                        target_id: target.target_id,
                        distance: 1,
                    }));
                }
            }
            CallContextSeed::Target(target_id) if options.include_incoming_callers => {
                for caller in self.callers_for_target(target_id)? {
                    if caller.status.status != CallStatusKind::Resolved {
                        continue;
                    }
                    candidates.push(CallContextCandidate {
                        node_id: caller.site.owner_id,
                        relation: CallContextRelation::IncomingCaller,
                        call_site_id: caller.site.id,
                        target_id: caller.target.target_id,
                        distance: 1,
                    });
                }
            }
            CallContextSeed::Owner(_) | CallContextSeed::Target(_) => {}
        }

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
