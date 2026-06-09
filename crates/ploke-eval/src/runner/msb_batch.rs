use std::fs;
use std::path::PathBuf;

use ploke_llm::ProviderKey;

use crate::model_registry::resolve_model_for_run;
use crate::spec::PrepareError;

use super::artifacts::*;
use super::*;

impl RunMsbBatchRequest {
    pub async fn run(self) -> Result<BatchRunArtifactPaths, PrepareError> {
        run_batch(
            self.batch_manifest,
            self.index_debug_snapshots,
            self.use_default_model,
            self.model_id,
            self.provider,
            None,
            None,
            self.stop_on_error,
            false,
        )
        .await
    }
}

impl RunMsbAgentBatchRequest {
    pub async fn run(self) -> Result<BatchRunArtifactPaths, PrepareError> {
        run_batch(
            self.batch_manifest,
            self.index_debug_snapshots,
            self.use_default_model,
            self.model_id,
            self.provider,
            self.embedding_model_id,
            self.embedding_provider,
            self.stop_on_error,
            true,
        )
        .await
    }
}

pub(crate) async fn run_batch(
    batch_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    model_id: Option<String>,
    provider: Option<ProviderKey>,
    embedding_model_id: Option<String>,
    embedding_provider: Option<ProviderKey>,
    stop_on_error: bool,
    agent_mode: bool,
) -> Result<BatchRunArtifactPaths, PrepareError> {
    let run_arm = RunArm::for_agent_mode(agent_mode);
    let (manifest_path, prepared) = load_prepared_batch(batch_manifest)?;
    fs::create_dir_all(&prepared.output_dir).map_err(|source| PrepareError::CreateOutputDir {
        path: prepared.output_dir.clone(),
        source,
    })?;

    let requested_model = parse_requested_model_id(model_id.as_deref())?;
    let selected_model = resolve_model_for_run(requested_model.as_ref(), use_default_model)?;
    let selected_model_id = selected_model.id.clone();
    let preferred_provider =
        load_provider_preference_for_selected_model(&selected_model, provider.as_ref())?;
    let requested_provider = provider_request_for_selected_model(
        &selected_model,
        provider.as_ref(),
        preferred_provider.as_ref(),
    );
    let route = resolve_route_for_model(&selected_model, requested_provider).await?;
    let selected_provider = route.selected_provider_slug();

    let summary_path = prepared.output_dir.join("batch-run-summary.json");
    let submission_path = prepared.output_dir.join("multi-swe-bench-submission.jsonl");
    fs::write(&submission_path, "").map_err(|source| PrepareError::WriteManifest {
        path: submission_path.clone(),
        source,
    })?;

    let mut stopped_early = false;
    let mut instance_results = Vec::with_capacity(prepared.instances.len());

    for task_id in &prepared.instances {
        let run_manifest = prepared.instances_root.join(task_id).join("run.json");
        if agent_mode {
            match agent_single_request_for_batch(
                run_manifest.clone(),
                index_debug_snapshots,
                use_default_model,
                model_id.clone(),
                provider.clone(),
                embedding_model_id.clone(),
                embedding_provider.clone(),
                &prepared.batch_id,
            )
            .run()
            .await
            {
                Ok(artifacts) => {
                    if let Some(path) = artifacts.base.msb_submission.as_ref() {
                        let blob = fs::read_to_string(path).map_err(|source| {
                            PrepareError::ReadManifest {
                                path: path.clone(),
                                source,
                            }
                        })?;
                        if !blob.trim().is_empty() {
                            append_jsonl_blob(&submission_path, &blob)?;
                        }
                    }
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: Some(artifacts.base.execution_log),
                        record_path: artifacts.base.record_path,
                        turn_summary: Some(artifacts.turn_summary),
                        msb_submission: artifacts.base.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: None,
                        record_path: None,
                        turn_summary: None,
                        msb_submission: None,
                        status: "failed".to_string(),
                        error: Some(err.to_string()),
                    });
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                }
            }
        } else {
            match (RunMsbSingleRequest {
                run_manifest: run_manifest.clone(),
                batch_id: Some(prepared.batch_id.clone()),
                index_debug_snapshots,
                use_default_model,
                model_id: model_id.clone(),
                provider: provider.clone(),
                embedding_model_id: embedding_model_id.clone(),
                embedding_provider: embedding_provider.clone(),
            })
            .run()
            .await
            {
                Ok(artifacts) => {
                    if let Some(path) = artifacts.msb_submission.as_ref() {
                        let blob = fs::read_to_string(path).map_err(|source| {
                            PrepareError::ReadManifest {
                                path: path.clone(),
                                source,
                            }
                        })?;
                        if !blob.trim().is_empty() {
                            append_jsonl_blob(&submission_path, &blob)?;
                        }
                    }
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: Some(artifacts.execution_log),
                        record_path: artifacts.record_path,
                        turn_summary: None,
                        msb_submission: artifacts.msb_submission,
                        status: "completed".to_string(),
                        error: None,
                    });
                }
                Err(err) => {
                    instance_results.push(BatchInstanceResult {
                        task_id: task_id.clone(),
                        run_arm: run_arm.clone(),
                        run_manifest,
                        execution_log: None,
                        record_path: None,
                        turn_summary: None,
                        msb_submission: None,
                        status: "failed".to_string(),
                        error: Some(err.to_string()),
                    });
                    if stop_on_error {
                        stopped_early = true;
                        break;
                    }
                }
            }
        }
    }

    let instances_succeeded = instance_results
        .iter()
        .filter(|result| result.status == "completed")
        .count();
    let instances_failed = instance_results
        .iter()
        .filter(|result| result.status == "failed")
        .count();
    let msb_submission = match fs::metadata(&submission_path) {
        Ok(metadata) if metadata.len() > 0 => Some(submission_path.clone()),
        Ok(_) => None,
        Err(_) => None,
    };

    let summary = BatchRunSummary {
        batch_id: prepared.batch_id,
        mode: if agent_mode {
            "msb_agent_batch".to_string()
        } else {
            "msb_batch".to_string()
        },
        run_arm: run_arm.clone(),
        batch_manifest: manifest_path.clone(),
        output_dir: prepared.output_dir,
        dataset_file: prepared.dataset_file,
        repo_cache: prepared.repo_cache,
        instances_root: prepared.instances_root,
        selected_model: Some(selected_model_id),
        selected_provider: Some(selected_provider),
        instances_total: prepared.instances.len(),
        instances_attempted: instance_results.len(),
        instances_succeeded,
        instances_failed,
        stopped_early,
        instance_results,
    };
    write_json(&summary_path, &summary)?;

    Ok(BatchRunArtifactPaths {
        batch_manifest: manifest_path,
        summary: summary_path,
        msb_submission,
    })
}

fn agent_single_request_for_batch(
    run_manifest: PathBuf,
    index_debug_snapshots: bool,
    use_default_model: bool,
    model_id: Option<String>,
    provider: Option<ProviderKey>,
    embedding_model_id: Option<String>,
    embedding_provider: Option<ProviderKey>,
    batch_id: &str,
) -> RunMsbAgentSingleRequest {
    RunMsbAgentSingleRequest {
        run_manifest,
        batch_id: Some(batch_id.to_string()),
        index_debug_snapshots,
        use_default_model,
        model_id,
        provider,
        embedding_model_id,
        embedding_provider,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_batch_request_preserves_embedding_overrides() {
        let provider = ProviderKey::new("google").expect("provider key");
        let embedding_provider = ProviderKey::new("perplexity").expect("embedding provider key");

        let request = agent_single_request_for_batch(
            PathBuf::from("run.json"),
            true,
            false,
            Some("google/gemini-3.5-flash".to_string()),
            Some(provider.clone()),
            Some("perplexity/pplx-embed-v1-4b".to_string()),
            Some(embedding_provider.clone()),
            "batch-1",
        );

        assert_eq!(request.batch_id.as_deref(), Some("batch-1"));
        assert_eq!(
            request.embedding_model_id.as_deref(),
            Some("perplexity/pplx-embed-v1-4b")
        );
        assert_eq!(request.embedding_provider, Some(embedding_provider));
        assert_eq!(request.provider, Some(provider));
    }
}
