use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Serialize;

use crate::cli::{
    InspectOutputFormat, RegistryCommand, RegistryRecomputeCommand, RegistryShowCommand,
    RegistryStatusCommand, RegistrySubcommand,
};
use crate::spec::PrepareError;
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RegistryDatasetView<'a> {
    pub(crate) dataset: &'a str,
    pub(crate) state_path: PathBuf,
    pub(crate) source_paths: Vec<PathBuf>,
    pub(crate) entries: Vec<&'a RegistryEntry>,
}
impl RegistryCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            RegistrySubcommand::Recompute(cmd) => cmd.run().await,
            RegistrySubcommand::Status(cmd) => cmd.run().await,
            RegistrySubcommand::Show(cmd) => cmd.run().await,
        }
    }
}
impl RegistryRecomputeCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let (path, registry) = recompute_target_registry(RegistryRecomputeRequest {
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_keys: self.dataset_key,
            dataset_files: self.dataset,
        })?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_target_registry_status(&registry));
                println!("state: {}", path.display());
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&registry).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl RegistryStatusCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let registry = load_target_registry(BenchmarkFamily::MultiSweBenchRust)?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_target_registry_status(&registry));
                println!(
                    "state: {}",
                    target_registry_path(BenchmarkFamily::MultiSweBenchRust)?.display()
                );
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&registry).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl RegistryShowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let registry = load_target_registry(BenchmarkFamily::MultiSweBenchRust)?;
        let view = registry_dataset_view(&registry, &self.dataset)?;

        match self.format {
            InspectOutputFormat::Table => print_registry_dataset_view(&view),
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&view).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}
pub(crate) fn registry_dataset_view<'a>(
    registry: &'a TargetRegistry,
    dataset: &'a str,
) -> Result<RegistryDatasetView<'a>, PrepareError> {
    let entries: Vec<_> = registry
        .entries
        .iter()
        .filter(|entry| entry.dataset_label == dataset)
        .collect();
    if entries.is_empty() {
        let available = registry
            .entries
            .iter()
            .map(|entry| entry.dataset_label.as_str())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "registry dataset '{}' not found; available datasets: {}",
                dataset, available
            ),
        });
    }

    let source_paths = registry
        .dataset_sources
        .iter()
        .filter(|source| source.label == dataset || source.key.as_deref() == Some(dataset))
        .map(|source| source.path.clone())
        .collect();

    Ok(RegistryDatasetView {
        dataset,
        state_path: target_registry_path(registry.benchmark_family)?,
        source_paths,
        entries,
    })
}

fn print_registry_dataset_view(view: &RegistryDatasetView<'_>) {
    println!("dataset: {}", view.dataset);
    println!("instances: {}", view.entries.len());
    println!("registry: {}", view.state_path.display());
    if !view.source_paths.is_empty() {
        println!("sources:");
        for path in &view.source_paths {
            println!("  {}", path.display());
        }
    }
    println!();
    println!("instance ids:");
    for entry in &view.entries {
        println!("  {}", entry.instance_id);
    }
}
