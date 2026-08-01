use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

use crate::cli::prototype1_process;
use crate::cli::prototype1_state::cli_facing::{
    BroadHarnessAttemptProjection, BroadTuiAttemptOptions,
    run_broad_harness_attempt_from_request_path, run_broad_harness_attempt_sweep,
};
use crate::cli::{
    InspectOutputFormat, Prototype1HarnessAttemptCommand, Prototype1HarnessCommand,
    Prototype1HarnessSubcommand, Prototype1HarnessSweepCommand, Prototype1RunnerCommand,
};
use crate::spec::PrepareError;

impl Prototype1RunnerCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let Some(invocation) = self.invocation else {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner requires --invocation".to_string(),
            });
        };
        if !self.execute {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner currently requires --execute".to_string(),
            });
        }
        let _ = self.campaign;
        let _ = self.node_id;
        let _ = self.stop_on_error;
        let _ = self.format;
        prototype1_process::execute_prototype1_runner_invocation(&invocation)
            .await
            .map(|_| ())
    }
}

impl Prototype1HarnessCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            Prototype1HarnessSubcommand::Attempt(cmd) => cmd.run().await,
            Prototype1HarnessSubcommand::Sweep(cmd) => cmd.run().await,
        }
    }
}

impl Prototype1HarnessAttemptCommand {
    async fn run(self) -> Result<(), PrepareError> {
        let options = BroadTuiAttemptOptions::from_cli(
            self.model_id,
            self.provider,
            self.max_attempts,
            self.timeout_secs,
        )?;
        let row = run_broad_harness_attempt_from_request_path(self.request, options).await?;
        print_broad_harness_attempt_rows(std::slice::from_ref(&row), self.format)
    }
}

impl Prototype1HarnessSweepCommand {
    async fn run(self) -> Result<(), PrepareError> {
        let requests = collect_broad_harness_request_paths(self.requests, self.requests_dir)?;
        let lanes = broad_harness_sweep_lanes(
            requests,
            self.model_ids,
            self.provider,
            self.max_attempts,
            self.timeout_secs,
        )?;
        let rows = run_broad_harness_attempt_sweep(lanes, self.parallel).await?;
        print_broad_harness_attempt_rows(&rows, self.format)
    }
}

fn collect_broad_harness_request_paths(
    mut requests: Vec<PathBuf>,
    requests_dir: Option<PathBuf>,
) -> Result<Vec<PathBuf>, PrepareError> {
    if let Some(dir) = requests_dir {
        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir).map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read requests directory '{}': {source}",
                dir.display()
            ),
        })? {
            let entry = entry.map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "could not read entry in requests directory '{}': {source}",
                    dir.display()
                ),
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                entries.push(path);
            }
        }
        entries.sort();
        requests.extend(entries);
    }

    if requests.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "prototype1-harness sweep requires --request or --requests-dir".to_string(),
        });
    }

    let mut seen = BTreeSet::new();
    for path in &requests {
        if !seen.insert(path.clone()) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "prototype1-harness sweep received duplicate request path '{}'",
                    path.display()
                ),
            });
        }
    }

    Ok(requests)
}

fn broad_harness_sweep_lanes(
    requests: Vec<PathBuf>,
    model_ids: Vec<String>,
    provider: Option<String>,
    max_attempts: Option<u32>,
    timeout_secs: Option<u64>,
) -> Result<Vec<(PathBuf, BroadTuiAttemptOptions)>, PrepareError> {
    if !model_ids.is_empty() && model_ids.len() != 1 && model_ids.len() != requests.len() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1-harness sweep requires one --model-id for all requests or one per request; got {} model(s) for {} request(s)",
                model_ids.len(),
                requests.len()
            ),
        });
    }

    match model_ids.len() {
        0 => {
            let options =
                BroadTuiAttemptOptions::from_cli(None, provider, max_attempts, timeout_secs)?;
            Ok(requests
                .into_iter()
                .map(|request| (request, options.clone()))
                .collect())
        }
        1 => {
            let options = BroadTuiAttemptOptions::from_cli(
                Some(model_ids[0].clone()),
                provider,
                max_attempts,
                timeout_secs,
            )?;
            Ok(requests
                .into_iter()
                .map(|request| (request, options.clone()))
                .collect())
        }
        _ => requests
            .into_iter()
            .enumerate()
            .map(|(index, request)| {
                BroadTuiAttemptOptions::from_cli(
                    Some(model_ids[index].clone()),
                    provider.clone(),
                    max_attempts,
                    timeout_secs,
                )
                .map(|options| (request, options))
            })
            .collect(),
    }
}

fn print_broad_harness_attempt_rows(
    rows: &[BroadHarnessAttemptProjection],
    format: InspectOutputFormat,
) -> Result<(), PrepareError> {
    match format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(rows).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => {
            println!(
                "{:<8} {:<42} {:<28} {:>8} {:>8} {:>7} {}",
                "status", "request_id", "model", "timeout", "elapsed", "paths", "request"
            );
            for row in rows {
                let model = row.model_id.as_deref().unwrap_or("default");
                let changed = row.changed_paths.len();
                println!(
                    "{:<8} {:<42} {:<28} {:>8} {:>8} {:>7} {}",
                    row.status,
                    row.request_id,
                    model,
                    row.timeout_secs,
                    row.elapsed_ms,
                    changed,
                    row.request_path.display()
                );
                if let Some(error) = row.error.as_deref() {
                    println!("  error: {error}");
                }
            }
        }
    }
    Ok(())
}
