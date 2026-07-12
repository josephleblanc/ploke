use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use uuid::Uuid;

use crate::{
    Database, DbError,
    database::{to_string, to_string_list, to_uuid},
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
    proof_graph::{ProofGraphContextRow, ProofGraphStore, ProofInvariantStatus},
};

use super::super::{
    CallBuildDomain, CallContextRow, CallEffectGuardReport, CallEffectPolicyViolation,
    CallGuardReport, CallImpactReport, CallNodeInfo, CallPath, CallPathEdge, CallPathOptions,
    CallProofInvariantFinding, CallReachEffect, CallReachReport, CallRelationKind, CallSiteBucket,
    CallSiteKind, CallSiteRow, CallStatusKind, CallTestEntrypoint, CrateBoundaryEdge,
    ExternalSummaryNeed, ModuleBoundaryEdge, ModuleBoundaryPolicyRule,
    ModuleBoundaryPolicyViolation,
};
use super::metadata::{call_node_info_rank, call_node_infos, decode_call_node_info};

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
        let caller_ids = callers
            .iter()
            .map(|caller| caller.id)
            .collect::<BTreeSet<_>>();
        let test_nodes = test_node_ids(self, &caller_ids)?;
        let (mut test_callers, mut non_test_callers) = (Vec::new(), Vec::new());
        for caller in &callers {
            if test_nodes.contains(&caller.id) {
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
        let source_cfgs = source_cfgs_for_summary(self, &paths, &[direct_call_sites.as_slice()])?;

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
            source_crates: sources.crates,
            source_cfgs,
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
        let unresolved_frontier_calls = frontier_calls
            .iter()
            .filter(|row| row.status.status == CallStatusKind::Unresolved)
            .cloned()
            .collect();
        let ambiguous_frontier_calls = frontier_calls
            .iter()
            .filter(|row| row.status.status == CallStatusKind::Ambiguous)
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
        let source_cfgs = source_cfgs_for_summary(
            self,
            &paths,
            &[direct_call_sites.as_slice(), frontier_calls.as_slice()],
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
            unresolved_frontier_calls,
            ambiguous_frontier_calls,
            source_files: sources.files,
            source_crates: sources.crates,
            source_cfgs,
            source_modules: sources.modules,
        })
    }

    /// Classifies source-to-target paths by whether they pass through `guard_id`.
    ///
    /// This is a resolved-path-only policy helper. It does not infer missing
    /// guards from targetless frontier rows, and it treats no resolved path as
    /// not proven guarded.
    pub fn call_guard_report_between(
        &self,
        source_id: Uuid,
        target_id: Uuid,
        guard_id: Uuid,
        options: CallPathOptions,
    ) -> Result<CallGuardReport, DbError> {
        let source = self.call_node_info(source_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for guard source {source_id}"
            ))
        })?;
        let target = self.call_node_info(target_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for guard target {target_id}"
            ))
        })?;
        let guard = self.call_node_info(guard_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for guard node {guard_id}"
            ))
        })?;
        let paths = self.call_paths_between(source_id, target_id, options)?;
        let violations = paths
            .iter()
            .filter(|path| !path_has_guard(path, guard_id, target_id))
            .cloned()
            .collect::<Vec<_>>();
        let guarded = !paths.is_empty() && violations.is_empty();

        Ok(CallGuardReport {
            source,
            target,
            guard,
            guarded,
            paths,
            violations,
        })
    }

    /// Lists proof effect annotations attached to callsites reachable from `owner_id`.
    ///
    /// Resolved path edges are included by call-site id. Targetless frontier
    /// rows on the owner and all resolved path participants are also included
    /// so source/sink queries can report external, unsupported, unresolved, or
    /// ambiguous effect boundaries without promoting them into traversal edges.
    pub fn call_effects_reachable_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallReachEffect>, DbError> {
        let paths = self.call_paths_from_owner(owner_id, options)?;
        let context_by_site = reachable_callsite_context_rows(self, owner_id, &paths)?;
        let blockers_by_site = proof_blockers_by_call_site(self, context_by_site.keys())?;
        let proof_rows = ProofGraphStore::proof_graphrag_context(self, "")?;
        let mut paths_by_owner = BTreeMap::<Uuid, Vec<CallPath>>::new();
        for path in &paths {
            paths_by_owner
                .entry(path.end_id)
                .or_default()
                .push(path.clone());
        }

        let mut effects = Vec::new();
        for proof in proof_rows
            .iter()
            .filter(|proof| proof.kind == "effect_seed")
        {
            let Some(call_site_id) = proof.call_site_id.as_deref() else {
                continue;
            };
            let Ok(site_id) = Uuid::parse_str(call_site_id) else {
                continue;
            };
            let Some(call_site) = context_by_site.get(&site_id) else {
                continue;
            };

            let effect_seed_id = proof.effect_seed_id.clone().ok_or_else(|| {
                DbError::Cozo(format!(
                    "stored effect_seed proof row {} is missing effect_seed_id",
                    proof.fact_id
                ))
            })?;
            let effect_class = proof.effect_class.clone().ok_or_else(|| {
                DbError::Cozo(format!(
                    "stored effect_seed proof row {effect_seed_id} is missing effect_class"
                ))
            })?;
            let mut blocker_reasons = blockers_by_site
                .get(call_site_id)
                .cloned()
                .unwrap_or_default();
            blocker_reasons.sort();
            blocker_reasons.dedup();

            effects.push(CallReachEffect {
                effect_seed_id,
                effect_class,
                confidence: proof.confidence.clone(),
                blocker_if_unresolved: proof.blocker_if_unresolved,
                paths_to_owner: paths_by_owner
                    .get(&call_site.site.owner_id)
                    .cloned()
                    .unwrap_or_default(),
                call_site: call_site.clone(),
                blocker_reasons,
            });
        }
        effects.extend(summary_effects_for_reachable_sites(
            &proof_rows,
            &context_by_site,
            &blockers_by_site,
            &paths_by_owner,
        )?);

        effects.sort_by_key(|effect| {
            (
                effect.call_site.site.owner_id.as_u128(),
                effect.call_site.site.span,
                effect.effect_class.clone(),
                effect.effect_seed_id.clone(),
            )
        });
        Ok(effects)
    }

    /// Lists reachable effect annotations outside the supplied allowlist.
    ///
    /// This is a derived policy helper over `effect_seed` proof facts. It does
    /// not add proof facts, infer effect classes, or promote targetless
    /// frontier rows into traversal edges.
    pub fn call_effect_policy_violations_for_owner<S: AsRef<str>>(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
        allowed_effects: &[S],
    ) -> Result<Vec<CallEffectPolicyViolation>, DbError> {
        let allowed = allowed_effects
            .iter()
            .map(|effect| effect.as_ref().to_string())
            .collect::<BTreeSet<_>>();
        let allowed_effects = allowed.iter().cloned().collect::<Vec<_>>();

        let mut violations = self
            .call_effects_reachable_from_owner(owner_id, options)?
            .into_iter()
            .filter(|effect| !allowed.contains(&effect.effect_class))
            .map(|effect| CallEffectPolicyViolation {
                allowed_effects: allowed_effects.clone(),
                effect,
            })
            .collect::<Vec<_>>();

        violations.sort_by_key(|violation| {
            (
                violation.effect.call_site.site.owner_id.as_u128(),
                violation.effect.call_site.site.span,
                violation.effect.effect_class.clone(),
                violation.effect.effect_seed_id.clone(),
            )
        });
        Ok(violations)
    }

    /// Classifies reachable proof effect annotations by a required guard node.
    ///
    /// This composes resolved paths to the owner that contains the effect
    /// callsite with existing `effect_seed` proof facts. It never treats the
    /// targetless effect callsite itself as a local traversal edge.
    pub fn call_effect_guard_report_for_owner(
        &self,
        owner_id: Uuid,
        guard_id: Uuid,
        effect_class: &str,
        options: CallPathOptions,
    ) -> Result<CallEffectGuardReport, DbError> {
        if effect_class.is_empty() {
            return Err(DbError::QueryConstruction(
                "effect guard report requires non-empty effect_class".to_string(),
            ));
        }

        let owner = self.call_node_info(owner_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for effect guard owner {owner_id}"
            ))
        })?;
        let guard = self.call_node_info(guard_id)?.ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for effect guard node {guard_id}"
            ))
        })?;
        let effects = self
            .call_effects_reachable_from_owner(owner_id, options)?
            .into_iter()
            .filter(|effect| effect.effect_class == effect_class)
            .collect::<Vec<_>>();
        let violations = effects
            .iter()
            .filter(|effect| !effect_is_guarded(effect, owner_id, guard_id))
            .cloned()
            .collect::<Vec<_>>();
        let guarded = !effects.is_empty() && violations.is_empty();

        Ok(CallEffectGuardReport {
            owner,
            guard,
            effect_class: effect_class.to_string(),
            guarded,
            effects,
            violations,
        })
    }

    /// Lists reachable effect annotations outside the owner's admitted stored policy.
    ///
    /// The policy is read from an admitted `effect_policy` proof fact keyed by
    /// the owner definition id. Missing or ambiguous admitted policies fail
    /// loudly instead of defaulting to an empty or permissive allowlist.
    pub fn call_effect_policy_violations_for_stored_owner_policy(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallEffectPolicyViolation>, DbError> {
        let owner = owner_id.to_string();
        let allowed_effects = self
            .admitted_effect_policy_allowed_effects_for_definition(&owner)?
            .ok_or_else(|| {
                DbError::Cozo(format!(
                    "no admitted effect_policy proof row for owner definition {owner}"
                ))
            })?;

        self.call_effect_policy_violations_for_owner(owner_id, options, &allowed_effects)
    }

    /// Lists proof invariant findings attached to callsites reachable from `owner_id`.
    ///
    /// This is an owner-scoped projection of the proof invariant layer. It
    /// does not create call edges for targetless frontier rows; it only returns
    /// findings whose `call_site_id` is already present in the same reachable
    /// callsite context used by the effect and external-summary helpers.
    pub fn call_proof_invariant_findings_for_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CallProofInvariantFinding>, DbError> {
        let paths = self.call_paths_from_owner(owner_id, options)?;
        let context_by_site = reachable_callsite_context_rows(self, owner_id, &paths)?;
        let mut rows = Vec::new();

        for finding in self.proof_invariant_findings()? {
            let Some(call_site_id) = finding.call_site_id.as_deref() else {
                continue;
            };
            let Ok(site_id) = Uuid::parse_str(call_site_id) else {
                continue;
            };
            let Some(call_site) = context_by_site.get(&site_id).cloned() else {
                continue;
            };

            rows.push(CallProofInvariantFinding {
                invariant: finding.invariant,
                status: proof_invariant_status_label(finding.status).to_string(),
                reason: finding.reason,
                call_site_id: finding.call_site_id,
                call_site: Some(call_site),
            });
        }

        rows.sort_by(|left, right| {
            (
                left.invariant.as_str(),
                left.status.as_str(),
                left.call_site_id.as_deref().unwrap_or_default(),
                left.reason.as_str(),
            )
                .cmp(&(
                    right.invariant.as_str(),
                    right.status.as_str(),
                    right.call_site_id.as_deref().unwrap_or_default(),
                    right.reason.as_str(),
                ))
        });
        Ok(rows)
    }

    /// Lists active external-summary blockers attached to callsites reachable from `owner_id`.
    ///
    /// This is a proof-authoring helper: it reports targetless frontier sites
    /// whose proof graph still has an `external_dependency_summary_missing`
    /// blocker. Admitted linked summaries discharge that blocker through the
    /// proof invariant layer, so discharged sites drop out without gaining a
    /// local traversal edge.
    pub fn external_summary_needs_for_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<ExternalSummaryNeed>, DbError> {
        let paths = self.call_paths_from_owner(owner_id, options)?;
        let context_by_site = reachable_callsite_context_rows(self, owner_id, &paths)?;
        let blockers_by_site = proof_blockers_by_call_site(self, context_by_site.keys())?;
        let mut paths_by_owner = BTreeMap::<Uuid, Vec<CallPath>>::new();
        for path in &paths {
            paths_by_owner
                .entry(path.end_id)
                .or_default()
                .push(path.clone());
        }

        let mut needs = Vec::new();
        for (site, reasons) in blockers_by_site {
            if !reasons
                .iter()
                .any(|reason| reason == "external_dependency_summary_missing")
            {
                continue;
            }
            let Ok(site_id) = Uuid::parse_str(&site) else {
                continue;
            };
            let Some(call_site) = context_by_site.get(&site_id) else {
                continue;
            };
            let blocker_reasons = reasons.into_iter().collect::<BTreeSet<_>>();
            needs.push(ExternalSummaryNeed {
                paths_to_owner: paths_by_owner
                    .get(&call_site.site.owner_id)
                    .cloned()
                    .unwrap_or_default(),
                call_site: call_site.clone(),
                blocker_reasons: blocker_reasons.into_iter().collect(),
            });
        }

        needs.sort_by_key(|need| {
            (
                need.call_site.site.owner_id.as_u128(),
                need.call_site.site.span,
                need.call_site.site.id.as_u128(),
            )
        });
        Ok(needs)
    }

    /// Lists build/test domain proof metadata linked to a call-graph node.
    ///
    /// This is a proof-context summary for build/deployment questions. It
    /// reads existing proof facts only; it does not infer build targets from
    /// source paths or promote generated/test reachability into source call
    /// edges.
    pub fn call_build_domains_for_node(
        &self,
        node_id: Uuid,
    ) -> Result<Vec<CallBuildDomain>, DbError> {
        let rows = self.proof_build_domain_rows_for_definition(&node_id.to_string())?;
        let mut domains = BTreeMap::<String, CallBuildDomain>::new();

        for row in rows.into_iter().filter(|row| row.kind == "build_domain") {
            let Some(build_domain_id) = row.build_domain_id else {
                continue;
            };
            let entry = domains
                .entry(build_domain_id.clone())
                .or_insert_with(|| CallBuildDomain {
                    build_domain_id,
                    target_kind: row.target_kind.clone(),
                    target_name: row.target_name.clone(),
                    target_root: row.target_root.clone(),
                    profile: row.profile.clone(),
                    rustc_version: row.rustc_version.clone(),
                    proof_policy_version: row.proof_policy_version.clone(),
                    active_cfg_hash: row.active_cfg_hash.clone(),
                    evidence_use: row.evidence_use.clone(),
                    blocker_reasons: Vec::new(),
                });
            if let Some(reason) = row.blocker_reason {
                entry.blocker_reasons.push(reason);
            }
        }

        let mut values = domains.into_values().collect::<Vec<_>>();
        for domain in &mut values {
            domain.blocker_reasons.sort();
            domain.blocker_reasons.dedup();
        }
        values.sort_by(|left, right| left.build_domain_id.cmp(&right.build_domain_id));
        Ok(values)
    }

    /// Lists generated/test entrypoint proof metadata linked to a call-graph node.
    ///
    /// This summarizes explicit proof rows only. It keeps generated test
    /// harness coverage separate from source call traversal, so private
    /// uncalled queries can report proof coverage without inventing call edges.
    /// When the same definition has a single admitted `effect_policy` row, the
    /// policy's allowed effects are included as execution-policy metadata.
    pub fn call_test_entrypoints_for_node(
        &self,
        node_id: Uuid,
    ) -> Result<Vec<CallTestEntrypoint>, DbError> {
        let definition_id = node_id.to_string();
        let rows = self.proof_entrypoint_summary_rows_for_definition(&definition_id)?;
        if !rows.iter().any(|row| row.kind == "entrypoint_summary") {
            return Ok(Vec::new());
        }

        let allowed_effects = self
            .admitted_effect_policy_allowed_effects_for_definition(&definition_id)?
            .unwrap_or_default();
        let mut entrypoints = BTreeMap::<String, CallTestEntrypoint>::new();

        for row in rows
            .into_iter()
            .filter(|row| row.kind == "entrypoint_summary")
        {
            let entrypoint_summary_id = row.fact_id.clone();
            let entry = entrypoints
                .entry(entrypoint_summary_id.clone())
                .or_insert_with(|| CallTestEntrypoint {
                    entrypoint_summary_id,
                    build_domain_id: row.build_domain_id.clone(),
                    definition_id: row.definition_id.clone(),
                    target_kind: row.target_kind.clone(),
                    target_name: row.target_name.clone(),
                    target_root: row.target_root.clone(),
                    summary_class: row.summary_class.clone(),
                    artifact_hash: row.artifact_hash.clone(),
                    summary_version: row.summary_version.clone(),
                    review_method: row.review_method.clone(),
                    scope_of_validity: row.scope_of_validity.clone(),
                    required_containment: row.required_containment.clone(),
                    invalidation_conditions: row.invalidation_conditions.clone(),
                    status: row.status.clone(),
                    evidence_use: row.evidence_use.clone(),
                    allowed_effects: allowed_effects.clone(),
                    blocker_reasons: Vec::new(),
                });
            if let Some(reason) = row.blocker_reason {
                entry.blocker_reasons.push(reason);
            }
        }

        let mut values = entrypoints.into_values().collect::<Vec<_>>();
        for entrypoint in &mut values {
            entrypoint.blocker_reasons.sort();
            entrypoint.blocker_reasons.dedup();
        }
        values.sort_by(|left, right| left.entrypoint_summary_id.cmp(&right.entrypoint_summary_id));
        Ok(values)
    }

    /// Lists private executable call-graph nodes with no direct resolved incoming call edge.
    ///
    /// This is a conservative source-call graph query for dead-code triage. It
    /// does not model generated entrypoints, dynamic dispatch, callback
    /// value-flow, or external callers; those remain separate proof/frontier
    /// questions instead of being inferred here.
    pub fn private_uncalled_nodes(&self) -> Result<Vec<CallNodeInfo>, DbError> {
        let script = format!(
            r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

private_node[id, kind, name, vis_kind, is_unsafe, is_async, module_path, file_path] :=
  *function{{ id, name, vis_kind, is_unsafe, is_async @ 'NOW' }},
  vis_kind != "public",
  kind = "Function",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

private_node[id, kind, name, vis_kind, is_unsafe, is_async, module_path, file_path] :=
  *macro{{ id, name, vis_kind @ 'NOW' }},
  vis_kind != "public",
  kind = "Macro",
  is_unsafe = false,
  is_async = false,
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

private_node[id, kind, name, vis_kind, is_unsafe, is_async, module_path, file_path] :=
  *method{{ id, owner_id: method_owner_id, name, vis_kind, is_unsafe, is_async @ 'NOW' }},
  vis_kind != "public",
  kind = "Method",
  ancestor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, file_path @ 'NOW' }}

incoming[id] := *call_relation {{ target_id: id @ 'NOW' }}

?[id, kind, name, vis_kind, is_unsafe, is_async, module_path, file_path] :=
  private_node[id, kind, name, vis_kind, is_unsafe, is_async, module_path, file_path],
  not incoming[id]

:sort kind, file_path, module_path, name, id
"#
        );
        let rows = self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;

        let mut nodes_by_id = BTreeMap::<Uuid, CallNodeInfo>::new();
        for row in &rows.rows {
            let info = decode_call_node_info(row)?;
            let entry = nodes_by_id.entry(info.id).or_insert_with(|| info.clone());
            if call_node_info_rank(&info) < call_node_info_rank(entry) {
                *entry = info;
            }
        }
        let mut nodes = nodes_by_id.into_values().collect::<Vec<_>>();
        nodes.sort_by_key(|node| {
            (
                node.kind,
                node.file_path.clone(),
                node.module_path.clone(),
                node.name.clone(),
                node.id.as_u128(),
            )
        });
        Ok(nodes)
    }

    /// Lists resolved call path edges from `owner_id` that cross module boundaries.
    ///
    /// This is a bounded architecture-review helper. It uses the same
    /// resolved-only path traversal as [`Self::call_paths_from_owner`] and
    /// does not promote targetless frontier rows into edges.
    pub fn module_boundary_edges_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<ModuleBoundaryEdge>, DbError> {
        let mut out = BTreeMap::new();
        let mut node_cache = BTreeMap::new();
        let mut site_cache = BTreeMap::new();
        let mut context_cache = BTreeMap::new();
        for path in self.call_paths_from_owner(owner_id, options)? {
            for edge in path.edges {
                let caller =
                    cached_node_info(self, &mut node_cache, edge.caller_id, "boundary caller")?;
                let callee =
                    cached_node_info(self, &mut node_cache, edge.callee_id, "boundary callee")?;
                if caller.module_path == callee.module_path {
                    continue;
                }
                let site = cached_call_site(self, &mut site_cache, &mut context_cache, &edge)?;
                out.entry((edge.caller_id, edge.callee_id, edge.call_site_id))
                    .or_insert_with(|| ModuleBoundaryEdge {
                        edge,
                        caller,
                        callee,
                        site,
                    });
            }
        }

        let mut edges = out.into_values().collect::<Vec<_>>();
        edges.sort_by_key(|edge| {
            (
                edge.caller.module_path.clone(),
                edge.callee.module_path.clone(),
                edge.caller.name.clone(),
                edge.callee.name.clone(),
                edge.edge.call_site_id.as_u128(),
            )
        });
        Ok(edges)
    }

    /// Lists resolved call path edges from `owner_id` that cross crate boundaries.
    ///
    /// This is a bounded architecture/build helper. It uses the same
    /// resolved-only path traversal as [`Self::call_paths_from_owner`] and
    /// does not promote targetless dependency frontiers into edges.
    pub fn crate_boundary_edges_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
    ) -> Result<Vec<CrateBoundaryEdge>, DbError> {
        let paths = self.call_paths_from_owner(owner_id, options)?;
        let mut ids = BTreeSet::new();
        for path in &paths {
            for edge in &path.edges {
                ids.insert(edge.caller_id);
                ids.insert(edge.callee_id);
            }
        }
        let crates = source_crates_by_node(self, &ids)?;

        let mut out = BTreeMap::new();
        let mut node_cache = BTreeMap::new();
        let mut site_cache = BTreeMap::new();
        let mut context_cache = BTreeMap::new();
        for path in paths {
            for edge in path.edges {
                let caller_crate = crates.get(&edge.caller_id).ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing source crate metadata for crate-boundary caller {}",
                        edge.caller_id
                    ))
                })?;
                let callee_crate = crates.get(&edge.callee_id).ok_or_else(|| {
                    DbError::Cozo(format!(
                        "missing source crate metadata for crate-boundary callee {}",
                        edge.callee_id
                    ))
                })?;
                if caller_crate == callee_crate {
                    continue;
                }

                let key = (edge.caller_id, edge.callee_id, edge.call_site_id);
                if out.contains_key(&key) {
                    continue;
                }
                let caller =
                    cached_node_info(self, &mut node_cache, edge.caller_id, "crate caller")?;
                let callee =
                    cached_node_info(self, &mut node_cache, edge.callee_id, "crate callee")?;
                let site = cached_call_site(self, &mut site_cache, &mut context_cache, &edge)?;
                out.insert(
                    key,
                    CrateBoundaryEdge {
                        edge,
                        caller,
                        caller_crate: caller_crate.clone(),
                        callee,
                        callee_crate: callee_crate.clone(),
                        site,
                    },
                );
            }
        }

        let mut edges = out.into_values().collect::<Vec<_>>();
        edges.sort_by(|left, right| {
            (
                left.caller_crate.as_str(),
                left.callee_crate.as_str(),
                left.caller.module_path.as_slice(),
                left.callee.module_path.as_slice(),
                left.caller.name.as_str(),
                left.callee.name.as_str(),
                left.edge.call_site_id.as_u128(),
            )
                .cmp(&(
                    right.caller_crate.as_str(),
                    right.callee_crate.as_str(),
                    right.caller.module_path.as_slice(),
                    right.callee.module_path.as_slice(),
                    right.caller.name.as_str(),
                    right.callee.name.as_str(),
                    right.edge.call_site_id.as_u128(),
                ))
        });
        Ok(edges)
    }

    /// Lists resolved module-boundary edges that match forbidden architecture rules.
    ///
    /// Rules use module-path prefixes over the same resolved-only boundary
    /// edges returned by [`Self::module_boundary_edges_from_owner`]. This
    /// helper does not infer intended layers from source paths and does not
    /// promote targetless frontier rows into architecture-policy evidence.
    pub fn module_boundary_policy_violations_from_owner(
        &self,
        owner_id: Uuid,
        options: CallPathOptions,
        rules: &[ModuleBoundaryPolicyRule],
    ) -> Result<Vec<ModuleBoundaryPolicyViolation>, DbError> {
        validate_module_boundary_policy_rules(rules)?;

        let mut violations = Vec::new();
        for edge in self.module_boundary_edges_from_owner(owner_id, options)? {
            for rule in rules {
                if module_path_has_prefix(&edge.caller.module_path, &rule.caller_module_prefix)
                    && module_path_has_prefix(&edge.callee.module_path, &rule.callee_module_prefix)
                {
                    violations.push(ModuleBoundaryPolicyViolation {
                        rule_id: rule.rule_id.clone(),
                        edge: edge.clone(),
                    });
                }
            }
        }

        violations.sort_by(|left, right| {
            (
                left.rule_id.as_str(),
                left.edge.caller.module_path.as_slice(),
                left.edge.callee.module_path.as_slice(),
                left.edge.edge.call_site_id.as_u128(),
            )
                .cmp(&(
                    right.rule_id.as_str(),
                    right.edge.caller.module_path.as_slice(),
                    right.edge.callee.module_path.as_slice(),
                    right.edge.edge.call_site_id.as_u128(),
                ))
        });
        Ok(violations)
    }
}

fn validate_module_boundary_policy_rules(
    rules: &[ModuleBoundaryPolicyRule],
) -> Result<(), DbError> {
    for rule in rules {
        if rule.rule_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "module boundary policy rule requires non-empty rule_id".to_string(),
            ));
        }
        if rule.caller_module_prefix.is_empty() {
            return Err(DbError::QueryConstruction(format!(
                "module boundary policy rule {} requires non-empty caller_module_prefix",
                rule.rule_id
            )));
        }
        if rule.callee_module_prefix.is_empty() {
            return Err(DbError::QueryConstruction(format!(
                "module boundary policy rule {} requires non-empty callee_module_prefix",
                rule.rule_id
            )));
        }
    }
    Ok(())
}

fn module_path_has_prefix(path: &[String], prefix: &[String]) -> bool {
    path.len() >= prefix.len()
        && path
            .iter()
            .zip(prefix.iter())
            .all(|(segment, expected)| segment == expected)
}

fn cached_node_info(
    db: &Database,
    cache: &mut BTreeMap<Uuid, CallNodeInfo>,
    node_id: Uuid,
    label: &str,
) -> Result<CallNodeInfo, DbError> {
    if let Some(info) = cache.get(&node_id) {
        return Ok(info.clone());
    }
    let info = db.call_node_info(node_id)?.ok_or_else(|| {
        DbError::Cozo(format!(
            "missing call graph node metadata for {label} {node_id}"
        ))
    })?;
    cache.insert(node_id, info.clone());
    Ok(info)
}

fn cached_call_site(
    db: &Database,
    cache: &mut BTreeMap<Uuid, CallSiteRow>,
    context_cache: &mut BTreeMap<Uuid, Vec<CallContextRow>>,
    edge: &CallPathEdge,
) -> Result<CallSiteRow, DbError> {
    if let Some(site) = cache.get(&edge.call_site_id) {
        return Ok(site.clone());
    }
    let site = cached_context_site(db, context_cache, edge, "module boundary edge")?;
    cache.insert(edge.call_site_id, site.clone());
    Ok(site)
}

fn cached_context_site(
    db: &Database,
    cache: &mut BTreeMap<Uuid, Vec<CallContextRow>>,
    edge: &CallPathEdge,
    label: &str,
) -> Result<CallSiteRow, DbError> {
    let context = match cache.entry(edge.caller_id) {
        std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(db.call_context_for_owner(edge.caller_id)?)
        }
    };
    context
        .iter()
        .find(|row| row.site.id == edge.call_site_id)
        .map(|row| row.site.clone())
        .ok_or_else(|| {
            DbError::Cozo(format!(
                "missing callsite {} while collecting {label}",
                edge.call_site_id
            ))
        })
}

fn proof_invariant_status_label(status: ProofInvariantStatus) -> &'static str {
    match status {
        ProofInvariantStatus::Pass => "pass",
        ProofInvariantStatus::Fail => "fail",
        ProofInvariantStatus::Blocked => "blocked",
    }
}

fn path_has_guard(path: &CallPath, guard_id: Uuid, target_id: Uuid) -> bool {
    let mut nodes = Vec::with_capacity(path.edges.len() + 1);
    nodes.push(path.start_id);
    nodes.extend(path.edges.iter().map(|edge| edge.callee_id));

    let Some(target_pos) = nodes.iter().position(|id| *id == target_id) else {
        return false;
    };
    nodes[..target_pos].iter().any(|id| *id == guard_id)
}

fn effect_is_guarded(effect: &CallReachEffect, owner_id: Uuid, guard_id: Uuid) -> bool {
    let effect_owner = effect.call_site.site.owner_id;
    if owner_id == guard_id || effect_owner == guard_id {
        return true;
    }

    !effect.paths_to_owner.is_empty()
        && effect
            .paths_to_owner
            .iter()
            .all(|path| path_has_guard(path, guard_id, effect_owner))
}

fn summary_effects_for_reachable_sites(
    rows: &[ProofGraphContextRow],
    context: &BTreeMap<Uuid, CallContextRow>,
    blockers: &BTreeMap<String, Vec<String>>,
    paths: &BTreeMap<Uuid, Vec<CallPath>>,
) -> Result<Vec<CallReachEffect>, DbError> {
    let mut summaries = BTreeMap::<String, &ProofGraphContextRow>::new();
    for row in rows.iter().filter(|row| {
        row.kind == "external_summary"
            && row.status.as_deref() == Some("admitted")
            && row.blocker_reason.is_none()
            && !row.allowed_effects.is_empty()
    }) {
        if let Some(id) = row.external_summary_id.as_ref() {
            summaries.entry(id.clone()).or_insert(row);
        }
    }

    let mut effects = BTreeMap::<(Uuid, String, String), CallReachEffect>::new();
    for link in rows.iter().filter(|row| {
        row.kind == "call_resolution"
            && row.resolution_state.as_deref() == Some("externally_summarized")
            && row.blocker_reason.is_none()
    }) {
        let Some(summary_id) = link.external_summary_id.as_ref() else {
            continue;
        };
        let Some(summary) = summaries.get(summary_id) else {
            continue;
        };
        let Some(raw_site) = link.call_site_id.as_deref() else {
            continue;
        };
        let Ok(site_id) = Uuid::parse_str(raw_site) else {
            continue;
        };
        let Some(call_site) = context.get(&site_id) else {
            continue;
        };

        let mut reasons = blockers.get(raw_site).cloned().unwrap_or_default();
        reasons.retain(|reason| reason != "external_dependency_summary_missing");
        reasons.sort();
        reasons.dedup();

        for effect_class in &summary.allowed_effects {
            let effect_id = format!("summary-effect:{summary_id}:{effect_class}");
            effects
                .entry((site_id, summary_id.clone(), effect_class.clone()))
                .or_insert_with(|| CallReachEffect {
                    effect_seed_id: effect_id,
                    effect_class: effect_class.clone(),
                    confidence: summary.review_method.clone(),
                    blocker_if_unresolved: Some(false),
                    paths_to_owner: paths
                        .get(&call_site.site.owner_id)
                        .cloned()
                        .unwrap_or_default(),
                    call_site: call_site.clone(),
                    blocker_reasons: reasons.clone(),
                });
        }
    }

    Ok(effects.into_values().collect())
}

fn reachable_callsite_context_rows(
    db: &Database,
    owner_id: Uuid,
    paths: &[CallPath],
) -> Result<BTreeMap<Uuid, CallContextRow>, DbError> {
    let mut owners = BTreeSet::from([owner_id]);
    let mut resolved_sites = BTreeSet::new();
    for path in paths {
        for edge in &path.edges {
            owners.insert(edge.caller_id);
            owners.insert(edge.callee_id);
            resolved_sites.insert(edge.call_site_id);
        }
    }

    let mut rows = BTreeMap::new();
    for owner in owners {
        for row in db.call_context_for_owner(owner)? {
            if resolved_sites.contains(&row.site.id)
                || row.status.status != CallStatusKind::Resolved
            {
                rows.entry(row.site.id).or_insert(row);
            }
        }
    }
    Ok(rows)
}

fn proof_blockers_by_call_site<'a>(
    db: &Database,
    site_ids: impl IntoIterator<Item = &'a Uuid>,
) -> Result<BTreeMap<String, Vec<String>>, DbError> {
    let site_ids = site_ids
        .into_iter()
        .map(Uuid::to_string)
        .collect::<BTreeSet<_>>();
    let mut out = BTreeMap::<String, Vec<String>>::new();
    for blocker in ProofGraphStore::proof_blockers_for_call_sites(db, &site_ids)? {
        let Some(call_site_id) = blocker.call_site_id else {
            continue;
        };
        out.entry(call_site_id).or_default().push(blocker.reason);
    }
    Ok(out)
}

fn test_node_ids(db: &Database, node_ids: &BTreeSet<Uuid>) -> Result<BTreeSet<Uuid>, DbError> {
    if node_ids.is_empty() {
        return Ok(BTreeSet::new());
    }

    let input_rows = node_ids
        .iter()
        .map(|id| format!("[to_uuid(\"{id}\")]"))
        .collect::<Vec<_>>()
        .join(",\n");

    let script = format!(
        r#"
input[id] <- [
{input_rows}
]

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

owner_anchor[id, mod_id] := *function{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, mod_id] := *macro{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, mod_id] := *method{{ id, owner_id: method_owner_id @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, mod_id] := *const{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, mod_id] := *static{{ id @ 'NOW' }}, ancestor[id, mod_id]
owner_anchor[id, mod_id] := *call_body_owner{{ id, parent_id @ 'NOW' }}, owner_anchor[parent_id, mod_id]

?[id, module_path, file_path] :=
  input[id],
  owner_anchor[id, mod_id],
  *module{{ id: mod_id, path: module_path @ 'NOW' }},
  file_owner_for_module[mod_id, file_id],
  *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
    );

    let rows = db.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
    let mut tests = BTreeSet::new();
    for row in &rows.rows {
        let module_path = to_string_list(&row[1])?;
        let file_path = to_string(&row[2])?;
        if module_path.iter().any(|segment| segment == "tests")
            || file_path.contains("/tests/")
            || file_path.contains("\\tests\\")
        {
            tests.insert(to_uuid(&row[0])?);
        }
    }
    Ok(tests)
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
        CallRelationKind::Closure => 2,
        CallRelationKind::LocalFunction => 3,
        CallRelationKind::DynamicClosure => 4,
        CallRelationKind::MethodCallbackFunction => 5,
        CallRelationKind::MethodCallbackClosure => 6,
        CallRelationKind::Method => 7,
        CallRelationKind::AssociatedFunction => 8,
        CallRelationKind::TupleStructConstructor => 9,
        CallRelationKind::EnumVariantConstructor => 10,
    }
}

fn boundary_edges_for_paths(
    db: &Database,
    paths: &[CallPath],
) -> Result<Vec<CallPathEdge>, DbError> {
    let mut out = BTreeMap::new();
    let mut node_cache = BTreeMap::new();
    for path in paths {
        for edge in &path.edges {
            let caller = cached_node_info(db, &mut node_cache, edge.caller_id, "boundary caller")?;
            let callee = cached_node_info(db, &mut node_cache, edge.callee_id, "boundary callee")?;
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
    let node_ids = paths.iter().filter_map(select).collect::<BTreeSet<_>>();
    let infos = call_node_infos(db, &node_ids)?;
    let mut nodes = BTreeMap::new();
    for node_id in node_ids {
        let info = infos.get(&node_id).cloned().ok_or_else(|| {
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
    crates: Vec<String>,
    modules: Vec<Vec<String>>,
}

fn sources_for_summary<'a>(
    db: &Database,
    paths: &[CallPath],
    nodes: impl Iterator<Item = &'a CallNodeInfo>,
) -> Result<SummarySources, DbError> {
    let mut files = BTreeSet::new();
    let mut node_ids = BTreeSet::new();
    let mut modules = BTreeSet::new();
    for node in nodes {
        node_ids.insert(node.id);
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
    node_ids.extend(path_nodes);

    let infos = call_node_infos(db, &node_ids)?;
    for node_id in &node_ids {
        let info = infos.get(node_id).ok_or_else(|| {
            DbError::Cozo(format!(
                "missing call graph node metadata for summary source file {node_id}"
            ))
        })?;
        files.insert(info.file_path.clone());
        modules.insert(info.module_path.clone());
    }
    let crates = source_crates_for_nodes(db, &node_ids)?;

    Ok(SummarySources {
        files: files.into_iter().collect(),
        crates,
        modules: modules.into_iter().collect(),
    })
}

fn source_crates_for_nodes(
    db: &Database,
    node_ids: &BTreeSet<Uuid>,
) -> Result<Vec<String>, DbError> {
    let crates = source_crates_by_node(db, node_ids)?
        .into_values()
        .collect::<BTreeSet<_>>();
    Ok(crates.into_iter().collect())
}

fn source_crates_by_node(
    db: &Database,
    node_ids: &BTreeSet<Uuid>,
) -> Result<BTreeMap<Uuid, String>, DbError> {
    if node_ids.is_empty() {
        return Ok(BTreeMap::new());
    }

    let input_rows = node_ids
        .iter()
        .map(|id| format!("[to_uuid(\"{id}\")]"))
        .collect::<Vec<_>>()
        .join(",\n");
    let script = format!(
        r#"
input[id] <- [
{input_rows}
]

ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file_mod[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_owner_id] := module_has_file_mod[mod_id], file_owner_id = mod_id
file_owner_for_module[mod_id, file_owner_id] := ancestor[mod_id, parent], module_has_file_mod[parent], file_owner_id = parent

node_anchor[id, mod_id] := *function{{ id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *macro{{ id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *method{{ id, owner_id: method_owner_id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *const{{ id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *static{{ id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *call_body_owner{{ id, parent_id @ 'NOW' }}, node_anchor[parent_id, mod_id]
node_anchor[id, mod_id] := *struct{{ id @ 'NOW' }}, ancestor[id, mod_id]
node_anchor[id, mod_id] := *variant{{ id, owner_id: enum_id @ 'NOW' }}, ancestor[enum_id, mod_id]

?[id, name] :=
  input[id],
  node_anchor[id, mod_id],
  file_owner_for_module[mod_id, file_owner_id],
  *file_mod{{ owner_id: file_owner_id, namespace @ 'NOW' }},
  *crate_context{{ name, namespace @ 'NOW' }}

:sort id, name
"#
    );

    let rows = db.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)?;
    let mut crates = BTreeMap::new();
    for row in &rows.rows {
        let id = to_uuid(&row[0])?;
        let name = to_string(&row[1])?;
        if let Some(existing) = crates.insert(id, name.clone())
            && existing != name
        {
            return Err(DbError::Cozo(format!(
                "node {id} maps to multiple source crates: {existing}, {name}"
            )));
        }
    }
    Ok(crates)
}

fn source_cfgs_for_summary(
    db: &Database,
    paths: &[CallPath],
    row_groups: &[&[CallContextRow]],
) -> Result<Vec<String>, DbError> {
    let mut cfgs = BTreeSet::new();
    let mut site_ids = BTreeSet::new();
    let mut context_cache = BTreeMap::new();
    for rows in row_groups {
        for row in *rows {
            if site_ids.insert(row.site.id) {
                cfgs.extend(row.site.cfgs.iter().cloned());
            }
        }
    }

    for path in paths {
        for edge in &path.edges {
            if !site_ids.insert(edge.call_site_id) {
                continue;
            }
            let site = cached_context_site(db, &mut context_cache, edge, "summary cfgs")?;
            cfgs.extend(site.cfgs.iter().cloned());
        }
    }

    Ok(cfgs.into_iter().collect())
}
