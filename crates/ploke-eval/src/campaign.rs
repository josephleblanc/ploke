use std::fs;
use std::path::PathBuf;

use ploke_llm::{ModelId, ProviderKey};
use serde::{Deserialize, Serialize};

use crate::layout::{batches_dir, campaigns_dir, runs_dir};
use crate::model_registry::{load_active_model, load_model_registry, registry_has_model};
use crate::provider_prefs::load_provider_for_model;
use crate::runner::resolve_provider_for_model;
use crate::spec::{EvalBudget, FrameworkConfig, PrepareError};
use crate::target_registry::{
    BenchmarkFamily, RegistryDatasetSource, RegistryRecomputeRequest, TargetRegistry,
    load_target_registry, recompute_target_registry, resolve_registry_dataset_sources,
};

pub const CAMPAIGN_MANIFEST_SCHEMA_VERSION: &str = "campaign-manifest.v1";

const DEFAULT_REQUIRED_PROCEDURES: [&str; 3] = [
    "tool-call-intent-segments",
    "tool-call-review",
    "tool-call-segment-review",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CampaignManifest {
    pub schema_version: String,
    pub campaign_id: String,
    #[serde(default = "default_benchmark_family")]
    pub benchmark_family: BenchmarkFamily,
    #[serde(default)]
    pub dataset_sources: Vec<RegistryDatasetSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_procedures: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runs_root: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batches_root: Option<PathBuf>,
    #[serde(default)]
    pub eval: EvalCampaignPolicy,
    #[serde(default)]
    pub protocol: ProtocolCampaignPolicy,
    #[serde(default, skip_serializing_if = "FrameworkConfig::is_default")]
    pub framework: FrameworkConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EvalCampaignPolicy {
    #[serde(default)]
    pub include_partial: bool,
    #[serde(default)]
    pub stop_on_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub include_dataset_labels: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude_dataset_labels: Vec<String>,
    #[serde(default)]
    pub budget: EvalBudget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolCampaignPolicy {
    #[serde(default = "default_true")]
    pub include_partial: bool,
    #[serde(default)]
    pub include_incompatible: bool,
    #[serde(default)]
    pub include_failed: bool,
    #[serde(default)]
    pub stop_on_error: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit_runs: Option<usize>,
    #[serde(default = "default_protocol_max_concurrency")]
    pub max_concurrency: usize,
}

impl Default for ProtocolCampaignPolicy {
    fn default() -> Self {
        Self {
            include_partial: true,
            include_incompatible: false,
            include_failed: false,
            stop_on_error: false,
            limit_runs: None,
            max_concurrency: default_protocol_max_concurrency(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct CampaignOverrides {
    pub dataset_keys: Vec<String>,
    pub dataset_files: Vec<PathBuf>,
    pub model_id: Option<String>,
    pub provider_slug: Option<String>,
    pub required_procedures: Vec<String>,
    pub runs_root: Option<PathBuf>,
    pub batches_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedCampaignConfig {
    pub campaign_id: String,
    pub benchmark_family: BenchmarkFamily,
    pub dataset_sources: Vec<RegistryDatasetSource>,
    pub model_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub required_procedures: Vec<String>,
    pub runs_root: PathBuf,
    pub batches_root: PathBuf,
    pub eval: EvalCampaignPolicy,
    pub protocol: ProtocolCampaignPolicy,
    pub framework: FrameworkConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignValidationCheck {
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CampaignListEntry {
    pub campaign_id: String,
    pub has_manifest: bool,
    pub has_closure_state: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct StoredClosureStateEnvelope {
    #[serde(default)]
    config: StoredClosureConfig,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredClosureConfig {
    #[serde(default = "default_benchmark_family")]
    benchmark_family: BenchmarkFamily,
    #[serde(default)]
    dataset_sources: Vec<RegistryDatasetSource>,
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    provider_slug: Option<String>,
    #[serde(default)]
    required_procedures: Vec<String>,
    #[serde(default)]
    runs_root: Option<PathBuf>,
    #[serde(default)]
    batches_root: Option<PathBuf>,
    #[serde(default)]
    framework: FrameworkConfig,
}

impl Default for StoredClosureConfig {
    fn default() -> Self {
        Self {
            benchmark_family: default_benchmark_family(),
            dataset_sources: Vec::new(),
            model_id: None,
            provider_slug: None,
            required_procedures: Vec::new(),
            runs_root: None,
            batches_root: None,
            framework: FrameworkConfig::default(),
        }
    }
}

fn default_benchmark_family() -> BenchmarkFamily {
    BenchmarkFamily::MultiSweBenchRust
}

fn default_true() -> bool {
    true
}

fn default_protocol_max_concurrency() -> usize {
    100
}

impl CampaignManifest {
    pub fn new(campaign_id: String) -> Self {
        Self {
            schema_version: CAMPAIGN_MANIFEST_SCHEMA_VERSION.to_string(),
            campaign_id,
            benchmark_family: default_benchmark_family(),
            dataset_sources: Vec::new(),
            model_id: None,
            provider_slug: None,
            required_procedures: default_required_procedures(),
            runs_root: None,
            batches_root: None,
            eval: EvalCampaignPolicy::default(),
            protocol: ProtocolCampaignPolicy::default(),
            framework: FrameworkConfig::default(),
        }
    }
}

impl CampaignOverrides {
    pub fn is_empty(&self) -> bool {
        self.dataset_keys.is_empty()
            && self.dataset_files.is_empty()
            && self.model_id.is_none()
            && self.provider_slug.is_none()
            && self.required_procedures.is_empty()
            && self.runs_root.is_none()
            && self.batches_root.is_none()
    }
}

impl ResolvedCampaignConfig {
    pub fn parsed_model_id(&self) -> Result<ModelId, PrepareError> {
        self.model_id
            .parse()
            .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                phase: "campaign_model_id",
                detail: err.to_string(),
            })
    }
}

pub fn campaign_manifest_path(campaign_id: &str) -> Result<PathBuf, PrepareError> {
    Ok(campaigns_dir()?.join(campaign_id).join("campaign.json"))
}

pub fn campaign_closure_state_path(campaign_id: &str) -> Result<PathBuf, PrepareError> {
    Ok(campaigns_dir()?
        .join(campaign_id)
        .join("closure-state.json"))
}

pub fn load_campaign_manifest(campaign_id: &str) -> Result<CampaignManifest, PrepareError> {
    let path = campaign_manifest_path(campaign_id)?;
    let text = fs::read_to_string(&path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            PrepareError::MissingCampaignManifest(path.clone())
        } else {
            PrepareError::ReadCampaignManifest {
                path: path.clone(),
                source,
            }
        }
    })?;
    let manifest: CampaignManifest =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseCampaignManifest {
            path: path.clone(),
            source,
        })?;
    if manifest.campaign_id != campaign_id {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_manifest",
            detail: format!(
                "campaign manifest id '{}' does not match requested campaign '{}'",
                manifest.campaign_id, campaign_id
            ),
        });
    }
    Ok(manifest)
}

pub fn save_campaign_manifest(manifest: &CampaignManifest) -> Result<PathBuf, PrepareError> {
    let path = campaign_manifest_path(&manifest.campaign_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteCampaignManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json =
        serde_json::to_string_pretty(manifest).map_err(PrepareError::SerializeCampaignManifest)?;
    fs::write(&path, json).map_err(|source| PrepareError::WriteCampaignManifest {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn adopt_campaign_manifest_from_closure_state(
    campaign_id: &str,
) -> Result<CampaignManifest, PrepareError> {
    let path = campaign_closure_state_path(campaign_id)?;
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    let stored: StoredClosureStateEnvelope =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: path.clone(),
            source,
        })?;

    if stored.config.dataset_sources.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_init_from_closure_state",
            detail: format!(
                "closure state '{}' does not declare any dataset sources",
                path.display()
            ),
        });
    }

    Ok(CampaignManifest {
        schema_version: CAMPAIGN_MANIFEST_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        benchmark_family: stored.config.benchmark_family,
        dataset_sources: stored.config.dataset_sources,
        model_id: stored.config.model_id,
        provider_slug: stored.config.provider_slug,
        required_procedures: normalize_required_procedures(&stored.config.required_procedures)?,
        runs_root: stored.config.runs_root,
        batches_root: stored.config.batches_root,
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: stored.config.framework,
    })
}

pub fn adopt_campaign_manifest_from_registry(
    campaign_id: &str,
) -> Result<CampaignManifest, PrepareError> {
    let benchmark_family = default_benchmark_family();
    let registry = load_target_registry(benchmark_family)?;
    if registry.dataset_sources.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_init_from_registry",
            detail: "target registry does not declare any dataset sources".to_string(),
        });
    }

    let active_model = load_active_model()?;
    let provider_slug = load_provider_for_model(&active_model.model_id)?
        .map(|provider| provider.slug.as_str().to_string());

    Ok(CampaignManifest {
        schema_version: CAMPAIGN_MANIFEST_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        benchmark_family,
        dataset_sources: registry.dataset_sources,
        model_id: Some(active_model.model_id.to_string()),
        provider_slug,
        required_procedures: default_required_procedures(),
        runs_root: Some(runs_dir()?),
        batches_root: Some(batches_dir()?),
        eval: EvalCampaignPolicy::default(),
        protocol: ProtocolCampaignPolicy::default(),
        framework: FrameworkConfig::default(),
    })
}

pub fn apply_campaign_overrides(
    manifest: &mut CampaignManifest,
    overrides: &CampaignOverrides,
) -> Result<(), PrepareError> {
    if !overrides.dataset_keys.is_empty() || !overrides.dataset_files.is_empty() {
        manifest.dataset_sources =
            resolve_registry_dataset_sources(&overrides.dataset_keys, &overrides.dataset_files)?;
    }
    if let Some(model_id) = overrides.model_id.clone() {
        manifest.model_id = Some(model_id);
    }
    if let Some(provider_slug) = overrides.provider_slug.clone() {
        manifest.provider_slug = Some(provider_slug);
    }
    if !overrides.required_procedures.is_empty() {
        manifest.required_procedures =
            normalize_required_procedures(&overrides.required_procedures)?;
    }
    if let Some(runs_root) = overrides.runs_root.clone() {
        manifest.runs_root = Some(runs_root);
    }
    if let Some(batches_root) = overrides.batches_root.clone() {
        manifest.batches_root = Some(batches_root);
    }
    Ok(())
}

pub fn list_campaigns() -> Result<Vec<CampaignListEntry>, PrepareError> {
    let root = campaigns_dir()?;
    let mut campaigns = Vec::new();
    if !root.exists() {
        return Ok(campaigns);
    }

    for entry in fs::read_dir(&root).map_err(|source| PrepareError::ReadCampaignManifest {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadCampaignManifest {
            path: root.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let campaign_id = entry.file_name().to_string_lossy().to_string();
        let has_manifest = path.join("campaign.json").exists();
        let has_closure_state = path.join("closure-state.json").exists();
        if !has_manifest && !has_closure_state {
            continue;
        }
        campaigns.push(CampaignListEntry {
            campaign_id,
            has_manifest,
            has_closure_state,
        });
    }

    campaigns.sort_by(|left, right| left.campaign_id.cmp(&right.campaign_id));
    Ok(campaigns)
}

pub fn resolve_campaign_config(
    campaign_id: &str,
    overrides: &CampaignOverrides,
) -> Result<ResolvedCampaignConfig, PrepareError> {
    let manifest = load_campaign_manifest(campaign_id)?;

    let dataset_sources = if overrides.dataset_keys.is_empty() && overrides.dataset_files.is_empty()
    {
        manifest.dataset_sources.clone()
    } else {
        resolve_registry_dataset_sources(&overrides.dataset_keys, &overrides.dataset_files)?
    };
    if dataset_sources.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_dataset_sources",
            detail: "campaign must declare at least one dataset source".to_string(),
        });
    }

    let model_id = match overrides
        .model_id
        .clone()
        .or_else(|| manifest.model_id.clone())
    {
        Some(model_id) => model_id,
        None => load_active_model()?.model_id.to_string(),
    };
    let parsed_model_id: ModelId =
        model_id
            .parse()
            .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                phase: "campaign_model_id",
                detail: err.to_string(),
            })?;

    let provider_slug = overrides
        .provider_slug
        .clone()
        .or_else(|| manifest.provider_slug.clone())
        .or_else(|| {
            load_provider_for_model(&parsed_model_id)
                .ok()
                .flatten()
                .map(|value| value.slug.as_str().to_string())
        });

    let required_procedures = if overrides.required_procedures.is_empty() {
        normalize_required_procedures(&manifest.required_procedures)?
    } else {
        normalize_required_procedures(&overrides.required_procedures)?
    };

    let runs_root = overrides
        .runs_root
        .clone()
        .or_else(|| manifest.runs_root.clone())
        .unwrap_or(runs_dir()?);
    let batches_root = overrides
        .batches_root
        .clone()
        .or_else(|| manifest.batches_root.clone())
        .unwrap_or(batches_dir()?);

    Ok(ResolvedCampaignConfig {
        campaign_id: campaign_id.to_string(),
        benchmark_family: manifest.benchmark_family,
        dataset_sources,
        model_id,
        provider_slug,
        required_procedures,
        runs_root,
        batches_root,
        eval: manifest.eval,
        protocol: manifest.protocol,
        framework: manifest.framework,
    })
}

pub async fn validate_campaign_config(
    config: &ResolvedCampaignConfig,
) -> Result<Vec<CampaignValidationCheck>, PrepareError> {
    let mut checks = Vec::new();

    checks.push(CampaignValidationCheck {
        label: "campaign".to_string(),
        detail: config.campaign_id.clone(),
    });

    for source in &config.dataset_sources {
        if !source.path.exists() {
            return Err(PrepareError::MissingDatasetFile(source.path.clone()));
        }
    }
    checks.push(CampaignValidationCheck {
        label: "dataset_sources".to_string(),
        detail: format!("{} source(s)", config.dataset_sources.len()),
    });

    let registry = recompute_target_registry(RegistryRecomputeRequest {
        benchmark_family: config.benchmark_family,
        dataset_keys: dataset_keys_from_sources(&config.dataset_sources),
        dataset_files: dataset_files_from_sources(&config.dataset_sources),
    })?
    .1;
    checks.push(CampaignValidationCheck {
        label: "registry".to_string(),
        detail: format!("{} active entries", active_registry_count(&registry)),
    });

    let model_registry = load_model_registry()?;
    let model_id = config.parsed_model_id()?;
    if !registry_has_model(&model_registry, &model_id) {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_validate_model",
            detail: format!(
                "model '{}' was not found in the cached model registry",
                config.model_id
            ),
        });
    }
    let selected_model = model_registry
        .data
        .into_iter()
        .find(|item| item.id == model_id)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "campaign_validate_model",
            detail: format!(
                "model '{}' was not found in the cached model registry",
                config.model_id
            ),
        })?;
    checks.push(CampaignValidationCheck {
        label: "model".to_string(),
        detail: config.model_id.clone(),
    });

    let requested_provider = match config.provider_slug.as_deref() {
        Some(provider) => {
            Some(
                ProviderKey::new(provider).map_err(|err| PrepareError::DatabaseSetup {
                    phase: "campaign_validate_provider",
                    detail: err.to_string(),
                })?,
            )
        }
        None => None,
    };
    let resolved_provider =
        resolve_provider_for_model(&selected_model, requested_provider.as_ref()).await?;
    checks.push(CampaignValidationCheck {
        label: "provider".to_string(),
        detail: if config.provider_slug.is_some() {
            resolved_provider.provider.slug.as_str().to_string()
        } else {
            format!("auto -> {}", resolved_provider.provider.slug.as_str())
        },
    });

    fs::create_dir_all(&config.runs_root).map_err(|source| PrepareError::CreateOutputDir {
        path: config.runs_root.clone(),
        source,
    })?;
    checks.push(CampaignValidationCheck {
        label: "runs_root".to_string(),
        detail: config.runs_root.display().to_string(),
    });

    fs::create_dir_all(&config.batches_root).map_err(|source| PrepareError::CreateOutputDir {
        path: config.batches_root.clone(),
        source,
    })?;
    checks.push(CampaignValidationCheck {
        label: "batches_root".to_string(),
        detail: config.batches_root.display().to_string(),
    });

    if config.required_procedures.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "campaign_required_procedures",
            detail: "campaign must declare at least one required protocol procedure".to_string(),
        });
    }
    checks.push(CampaignValidationCheck {
        label: "required_procedures".to_string(),
        detail: config.required_procedures.join(", "),
    });

    Ok(checks)
}

pub fn render_resolved_campaign_config(config: &ResolvedCampaignConfig) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "campaign {} | benchmark {:?}\n",
        config.campaign_id, config.benchmark_family
    ));
    out.push_str(&format!("model: {}\n", config.model_id));
    out.push_str(&format!(
        "provider: {}\n",
        config.provider_slug.as_deref().unwrap_or("auto/openrouter")
    ));
    out.push_str(&format!("runs_root: {}\n", config.runs_root.display()));
    out.push_str(&format!(
        "batches_root: {}\n",
        config.batches_root.display()
    ));
    out.push_str(&format!(
        "required_procedures: {}\n",
        config.required_procedures.join(", ")
    ));
    out.push_str("\ndataset_sources\n");
    for source in &config.dataset_sources {
        let key = source.key.as_deref().unwrap_or("-");
        out.push_str(&format!(
            "  - {} | key {} | {}\n",
            source.label,
            key,
            source.path.display()
        ));
    }
    out.push_str("\neval\n");
    out.push_str(&format!(
        "  budget: turns {} | tool_calls {} | wall_clock_secs {}\n",
        config.eval.budget.max_turns,
        config.eval.budget.max_tool_calls,
        config.eval.budget.wall_clock_secs
    ));
    out.push_str(&format!(
        "  include_partial: {} | stop_on_error: {} | limit: {}\n",
        config.eval.include_partial,
        config.eval.stop_on_error,
        config
            .eval
            .limit
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
    out.push_str(&format!(
        "  include_dataset_labels: {}\n",
        if config.eval.include_dataset_labels.is_empty() {
            "(all)".to_string()
        } else {
            config.eval.include_dataset_labels.join(", ")
        }
    ));
    out.push_str(&format!(
        "  exclude_dataset_labels: {}\n",
        if config.eval.exclude_dataset_labels.is_empty() {
            "(none)".to_string()
        } else {
            config.eval.exclude_dataset_labels.join(", ")
        }
    ));
    out.push_str(&format!(
        "  batch_prefix: {}\n",
        config
            .eval
            .batch_prefix
            .as_deref()
            .unwrap_or(&config.campaign_id)
    ));
    out.push_str("\nprotocol\n");
    out.push_str(&format!(
        "  include_partial: {} | include_incompatible: {} | include_failed: {} | stop_on_error: {} | limit_runs: {} | max_concurrency: {}\n",
        config.protocol.include_partial,
        config.protocol.include_incompatible,
        config.protocol.include_failed,
        config.protocol.stop_on_error,
        config
            .protocol
            .limit_runs
            .map(|value| value.to_string())
            .unwrap_or_else(|| "none".to_string()),
        config.protocol.max_concurrency
    ));
    out.push_str("\nframework tools\n");
    if config.framework.tools.is_empty() {
        out.push_str("  - (none declared)\n");
    } else {
        for (tool, cfg) in &config.framework.tools {
            out.push_str(&format!(
                "  - {} | version {}\n",
                tool,
                cfg.version.as_deref().unwrap_or("none")
            ));
        }
    }
    out
}

pub fn dataset_keys_from_sources(sources: &[RegistryDatasetSource]) -> Vec<String> {
    sources
        .iter()
        .filter_map(|source| source.key.clone())
        .collect()
}

pub fn dataset_files_from_sources(sources: &[RegistryDatasetSource]) -> Vec<PathBuf> {
    sources
        .iter()
        .filter(|source| source.key.is_none())
        .map(|source| source.path.clone())
        .collect()
}

fn default_required_procedures() -> Vec<String> {
    DEFAULT_REQUIRED_PROCEDURES
        .iter()
        .map(|value| value.to_string())
        .collect()
}

fn normalize_required_procedures(values: &[String]) -> Result<Vec<String>, PrepareError> {
    let values = if values.is_empty() {
        default_required_procedures()
    } else {
        values.to_vec()
    };

    let mut normalized = Vec::new();
    for value in values {
        let normalized_value = match value.as_str() {
            "tool-call-intent-segments"
            | "tool_call_intent_segmentation"
            | "tool-call-intent-segmentation" => "tool-call-intent-segments",
            "tool-call-review" | "tool_call_review" => "tool-call-review",
            "tool-call-segment-review" | "tool_call_segment_review" => "tool-call-segment-review",
            other => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "campaign_required_procedures",
                    detail: format!("unknown protocol procedure '{other}'"),
                });
            }
        };
        if !normalized
            .iter()
            .any(|existing| existing == normalized_value)
        {
            normalized.push(normalized_value.to_string());
        }
    }
    Ok(normalized)
}

fn active_registry_count(registry: &TargetRegistry) -> usize {
    registry
        .entries
        .iter()
        .filter(|entry| {
            matches!(
                entry.state,
                crate::target_registry::RegistryEntryState::Active
            )
        })
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_handles_empty_framework() {
        let cfg = ResolvedCampaignConfig {
            campaign_id: "demo".to_string(),
            benchmark_family: BenchmarkFamily::MultiSweBenchRust,
            dataset_sources: vec![RegistryDatasetSource {
                key: Some("ripgrep".to_string()),
                path: PathBuf::from("/tmp/ripgrep.jsonl"),
                label: "ripgrep".to_string(),
                url: None,
            }],
            model_id: "x-ai/grok-4-fast".to_string(),
            provider_slug: None,
            required_procedures: default_required_procedures(),
            runs_root: PathBuf::from("/tmp/runs"),
            batches_root: PathBuf::from("/tmp/batches"),
            eval: EvalCampaignPolicy::default(),
            protocol: ProtocolCampaignPolicy::default(),
            framework: FrameworkConfig::default(),
        };

        let rendered = render_resolved_campaign_config(&cfg);
        assert!(rendered.contains("framework tools"));
        assert!(rendered.contains("(none declared)"));
    }
}
