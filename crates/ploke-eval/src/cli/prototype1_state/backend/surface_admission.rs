//! Edit-surface admission: proposal validation and checked edit carriers.

use std::fs;
use std::path::Path;

use crate::intervention::{text_file_artifact_id, text_replacement_patch_id};
use crate::loop_graph::{ArtifactId, PatchId};

use super::git_worktree::GitWorktreeBackend;
use super::{
    BackendError, CheckedSurfaceEdit, EditProposal, EditSurfaceAdmission, ProposedTouch,
    content_hash, edit_surface_contains_path, fold_touches, path_matches_surface_policy,
    validate_normal_repo_relpath, validate_touch_spans,
};
use crate::cli::prototype1_state::edit_surface::{self, graph, request_policy, surface, tui};
use crate::cli::prototype1_state::event::ContentHash;
use crate::cli::prototype1_state::history::{
    CheckedSurface, CheckedSurfaceTransition, HistoryError, ProcedureRef, SurfaceArtifactRef,
    SurfaceEvidence, SurfaceTouch, SurfaceWritable, grant,
};

impl GitWorktreeBackend {
    pub(crate) fn validate_edit_surface_candidate(
        &self,
        repo_root: &Path,
        admission: EditSurfaceAdmission,
        proposal: EditProposal,
    ) -> Result<CheckedSurfaceEdit, BackendError> {
        use crate::cli::prototype1_state::edit_surface::graph::View as _;

        let EditProposal {
            surface: proposal_surface,
            proposal_id,
            run_id,
            proposal_producer,
            generator_surface,
            touches: proposal_touches,
            reported_after_file_hash,
        } = proposal;

        if proposal_touches.is_empty() {
            return Err(BackendError::EmptyEditTouches {
                surface: proposal_surface,
            });
        }

        let mut paths = proposal_touches
            .iter()
            .map(|touch| touch.relpath.clone())
            .collect::<Vec<_>>();
        paths.sort();
        paths.dedup();
        if paths.len() != 1 {
            return Err(BackendError::MultiFileEdit { paths });
        }
        let target_relpath = paths.remove(0);
        validate_normal_repo_relpath(&target_relpath)?;
        if !path_matches_surface_policy(proposal_surface, &target_relpath)
            || !edit_surface_contains_path(repo_root, proposal_surface, &target_relpath)?
        {
            return Err(BackendError::OutOfEditSurface {
                surface: proposal_surface,
                path: target_relpath,
            });
        }

        let absolute_target = repo_root.join(&target_relpath);
        let source_content =
            fs::read_to_string(&absolute_target).map_err(|source| BackendError::ReadTarget {
                path: absolute_target,
                source,
            })?;
        let source_hash = content_hash(&source_content);
        let source_surface_hash = surface::Hash::new(source_hash.clone());

        let mut touches = proposal_touches;
        touches.sort_by_key(|touch| (touch.start, touch.end));
        validate_touch_spans(&target_relpath, &source_content, &touches)?;
        for touch in &touches {
            if touch.expected_file_hash != source_hash {
                return Err(BackendError::StaleEditBaseHash {
                    path: target_relpath.clone(),
                    expected: touch.expected_file_hash.clone(),
                    actual: source_hash.clone(),
                });
            }
        }

        let proposed_content = fold_touches(&source_content, &touches);
        let proposed_hash = content_hash(&proposed_content);
        let proposed_surface_hash = surface::Hash::new(proposed_hash.clone());
        let base_artifact_id = admission.base_artifact_id()?.clone();
        let derived_artifact_id = text_file_artifact_id(&target_relpath, &proposed_content);
        let patch_id =
            text_replacement_patch_id(&target_relpath, &source_content, &proposed_content);
        proposal_producer
            .verify_complete(&base_artifact_id, &proposal_id, &run_id)
            .map_err(|detail| BackendError::EditSurfaceCheck { detail })?;
        let expected_generator_surface = self.generator_surface_for_proposed_touches(
            &target_relpath,
            &source_content,
            &touches,
        )?;
        if generator_surface != expected_generator_surface {
            return Err(BackendError::EditSurfaceCheck {
                detail: format!(
                    "proposal generator surface mismatch for '{}': expected {:?}, got {:?}",
                    target_relpath.display(),
                    expected_generator_surface,
                    generator_surface
                ),
            });
        }
        let base_ref = surface::Ref::new(base_artifact_id.clone(), source_surface_hash.clone());
        let after_ref =
            surface::Ref::new(derived_artifact_id.clone(), proposed_surface_hash.clone());

        let artifact = surface::Artifact::new(
            base_ref.clone(),
            [(target_relpath.clone(), source_surface_hash.clone())],
        );
        let targets = touches
            .iter()
            .enumerate()
            .map(|(index, touch)| {
                graph::Target::new(
                    target_relpath.clone(),
                    format!("{}:{}", touch.target, index),
                )
            })
            .collect::<Vec<_>>();
        let graph_nodes = touches
            .iter()
            .zip(targets.iter())
            .map(|(touch, target)| {
                graph::Node::new(
                    target.clone(),
                    target_relpath.clone(),
                    touch.start,
                    touch.end,
                )
            })
            .collect::<Vec<_>>();
        let graph = graph::Mock::new(graph_nodes.clone(), []);
        let projection =
            graph
                .project(&artifact)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let rules = targets
            .iter()
            .cloned()
            .map(graph::Rule::Include)
            .collect::<Vec<_>>();
        let graph_bounds =
            graph
                .bounds(&projection, &rules)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let tui_projection = tui::generator_projection(&projection);
        let tui_bounds =
            tui::generator_bounds(&projection, graph_bounds.clone()).map_err(|err| {
                BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                }
            })?;

        let mut checked_touches = Vec::new();
        for (index, touch) in touches.iter().enumerate() {
            let material = tui::MaterialSpan::new(
                targets[index].clone(),
                target_relpath.clone(),
                touch.start,
                touch.end,
                source_surface_hash.clone(),
                tui::MaterialSource::TuiSplice,
            );
            let checked = tui_bounds
                .touch(&artifact, material, touch.replacement.clone())
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
            checked_touches.push(checked);
        }

        let grant = surface::Grant::for_coordinate(
            admission.coordinate.clone(),
            admission.policy.clone(),
            base_ref.clone(),
            graph_bounds,
            surface::Area::new(
                checked_touches
                    .iter()
                    .map(|touch| touch.span().clone())
                    .collect::<Vec<_>>(),
            ),
        )
        .map_err(|err| BackendError::EditSurfaceCheck {
            detail: err.to_string(),
        })?;
        let staged = tui::Proposal::stage(tui::Stage {
            proposal: &proposal_id,
            run: &run_id,
            base: &base_ref,
            after: after_ref.clone(),
            projection: &tui_projection,
            touches: checked_touches.clone(),
            auto_apply: false,
        })
        .map_err(|err| BackendError::EditSurfaceCheck {
            detail: err.to_string(),
        })?;
        let check = grant
            .check(staged.draft())
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?;
        let reported_after_hash = reported_after_file_hash
            .map(surface::Hash::new)
            .unwrap_or_else(|| proposed_surface_hash.clone());
        let writes = checked_touches
            .iter()
            .map(|touch| tui::Write::applied(touch, reported_after_hash.clone()))
            .collect::<Vec<_>>();
        let after_artifact = surface::Artifact::new(
            after_ref,
            [(target_relpath.clone(), proposed_surface_hash.clone())],
        );
        let applied = tui::Apply::from_results(staged, check, writes)
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?
            .validate(&after_artifact)
            .map_err(|err| BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            })?;
        let applied_authority = applied.authority().clone();
        let delta = applied
            .delta()
            .cloned()
            .ok_or_else(|| BackendError::EditSurfaceCheck {
                detail: "checked edit did not reach applied state".to_string(),
            })?;
        let transition = CheckedSurfaceTransition {
            target_relpath: target_relpath.clone(),
            base: SurfaceArtifactRef {
                artifact_id: delta.base().id().clone(),
                hash: delta.base().hash().as_str().to_string(),
            },
            after: SurfaceArtifactRef {
                artifact_id: delta.after().id().clone(),
                hash: delta.after().hash().as_str().to_string(),
            },
            patch_id: patch_id.clone(),
        };
        let grant = grant::Grant::<grant::Checked>::checked(
            applied_authority.coordinate().clone(),
            ProcedureRef::new(applied_authority.policy().as_str()),
            SurfaceWritable {
                target_relpath: target_relpath.clone(),
            },
            &transition,
        )
        .map_err(|err| BackendError::EditSurfaceCheck {
            detail: err.to_string(),
        })?;
        let checked_surface = CheckedSurface { grant, transition };

        Ok(CheckedSurfaceEdit {
            surface: proposal_surface,
            proposal_id,
            run_id,
            proposal_producer,
            generator_surface,
            checked_surface,
            policy: applied_authority.policy().clone(),
            source_content,
            proposed_content,
            source_content_hash: source_hash,
            proposed_content_hash: proposed_hash,
            delta,
        })
    }
}
impl GitWorktreeBackend {
    pub(crate) fn generator_surface_for_proposed_touches(
        &self,
        target_relpath: &Path,
        source_content: &str,
        touches: &[ProposedTouch],
    ) -> Result<tui::GeneratorSurfaceVersion, BackendError> {
        let target_specs = touches
            .iter()
            .enumerate()
            .map(|(index, touch)| {
                (
                    format!("{}:{}", touch.target, index),
                    touch.start,
                    touch.end,
                )
            })
            .collect::<Vec<_>>();
        self.generator_surface_for_named_spans(target_relpath, source_content, &target_specs)
    }

    pub(crate) fn generator_surface_for_surface_touches(
        &self,
        target_relpath: &Path,
        source_content: &str,
        touches: &[SurfaceTouch],
    ) -> Result<tui::GeneratorSurfaceVersion, BackendError> {
        let target_specs = touches
            .iter()
            .map(|touch| (touch.target_name.clone(), touch.start, touch.end))
            .collect::<Vec<_>>();
        self.generator_surface_for_named_spans(target_relpath, source_content, &target_specs)
    }

    fn generator_surface_for_named_spans(
        &self,
        target_relpath: &Path,
        source_content: &str,
        targets: &[(String, usize, usize)],
    ) -> Result<tui::GeneratorSurfaceVersion, BackendError> {
        use crate::cli::prototype1_state::edit_surface::graph::View as _;

        let source_hash = content_hash(source_content);
        let source_surface_hash = surface::Hash::new(source_hash);
        let artifact = surface::Artifact::new(
            surface::Ref::new(
                text_file_artifact_id(target_relpath, source_content),
                source_surface_hash.clone(),
            ),
            [(target_relpath.to_path_buf(), source_surface_hash.clone())],
        );
        let graph_targets = targets
            .iter()
            .map(|(name, _, _)| graph::Target::new(target_relpath.to_path_buf(), name.clone()))
            .collect::<Vec<_>>();
        let graph_nodes = targets
            .iter()
            .zip(graph_targets.iter())
            .map(|((_, start, end), target)| {
                graph::Node::new(target.clone(), target_relpath.to_path_buf(), *start, *end)
            })
            .collect::<Vec<_>>();
        let graph = graph::Mock::new(graph_nodes, []);
        let projection =
            graph
                .project(&artifact)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let rules = graph_targets
            .into_iter()
            .map(graph::Rule::Include)
            .collect::<Vec<_>>();
        let graph_bounds =
            graph
                .bounds(&projection, &rules)
                .map_err(|err| BackendError::EditSurfaceCheck {
                    detail: err.to_string(),
                })?;
        let tui_bounds = tui::generator_bounds(&projection, graph_bounds).map_err(|err| {
            BackendError::EditSurfaceCheck {
                detail: err.to_string(),
            }
        })?;
        Ok(tui::GeneratorSurfaceVersion::capture(&tui_bounds))
    }
}
