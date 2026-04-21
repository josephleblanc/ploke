use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

use crate::spec::EvalBudget;

/// Stable role classification for one concrete run attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegisteredRunRole {
    Control,
    Treatment,
}

/// Canonical directory roots used to store registration and run artifacts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStorageRoots {
    /// Root directory that contains `runs/<run_id>.json` registrations.
    pub registries_dir: PathBuf,
    /// Root directory that contains per-run artifact directories.
    pub runs_dir: PathBuf,
}

impl RunStorageRoots {
    /// Create storage roots from explicit directories.
    pub fn new(registries_dir: impl Into<PathBuf>, runs_dir: impl Into<PathBuf>) -> Self {
        Self {
            registries_dir: registries_dir.into(),
            runs_dir: runs_dir.into(),
        }
    }

    /// Canonical registration path for a run id.
    pub fn registry_path(&self, run_id: &str) -> PathBuf {
        self.registries_dir
            .join("runs")
            .join(format!("{run_id}.json"))
    }

    /// Canonical run-artifact directory for a run id.
    pub fn run_root(&self, run_id: &str) -> PathBuf {
        self.runs_dir.join(run_id)
    }

    /// Canonical `record.json.gz` path for a run id.
    pub fn record_path(&self, run_id: &str) -> PathBuf {
        self.run_root(run_id).join("record.json.gz")
    }

    /// Canonical final DB attachment path for a run id.
    pub fn final_db_path(&self, run_id: &str) -> PathBuf {
        self.run_root(run_id).join("state-final.db")
    }
}

/// Caller intent for a single eval run before configuration is frozen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunIntent {
    /// Benchmark or task identifier for the run.
    pub task_id: String,
    /// Repository root to evaluate.
    pub repo_root: PathBuf,
    /// Canonical storage roots for registration and run artifacts.
    pub storage_roots: RunStorageRoots,
    /// Optional base revision to check out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    /// Execution budget for the run.
    pub budget: EvalBudget,
    /// Optional model identifier requested for the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Optional provider slug requested for the run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    /// Optional campaign that owns this run attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    /// Optional batch that owns this run attempt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    /// Stable run arm identifier for this attempt.
    pub run_arm_id: String,
    /// Stable role for this attempt.
    pub run_role: RegisteredRunRole,
}

/// Immutable execution configuration captured from a run intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrozenRunSpec {
    /// Benchmark or task identifier for the run.
    pub task_id: String,
    /// Repository root to evaluate.
    pub repo_root: PathBuf,
    /// Canonical storage roots for registration and run artifacts.
    pub storage_roots: RunStorageRoots,
    /// Concrete base revision to use.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_sha: Option<String>,
    /// Concrete execution budget.
    pub budget: EvalBudget,
    /// Concrete model identifier, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    /// Concrete provider slug, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    /// Concrete campaign context, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign_id: Option<String>,
    /// Concrete batch context, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
    /// Stable run arm identifier for this attempt.
    pub run_arm_id: String,
    /// Stable role for this attempt.
    pub run_role: RegisteredRunRole,
}

impl RunIntent {
    /// Freeze the caller intent into an immutable executable specification.
    pub fn freeze(&self) -> FrozenRunSpec {
        FrozenRunSpec {
            task_id: self.task_id.clone(),
            repo_root: self.repo_root.clone(),
            storage_roots: self.storage_roots.clone(),
            base_sha: self.base_sha.clone(),
            budget: self.budget.clone(),
            model_id: self.model_id.clone(),
            provider_slug: self.provider_slug.clone(),
            campaign_id: self.campaign_id.clone(),
            batch_id: self.batch_id.clone(),
            run_arm_id: self.run_arm_id.clone(),
            run_role: self.run_role,
        }
    }
}

impl FrozenRunSpec {
    /// Return a stable fingerprint for the frozen specification.
    pub fn fingerprint(&self) -> Result<String, serde_json::Error> {
        let bytes = serde_json::to_vec(self)?;
        Ok(hex_sha256(&bytes))
    }
}

fn hex_sha256(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut out, "{:02x}", byte).expect("hex encoding");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_intent() -> RunIntent {
        RunIntent {
            task_id: "BurntSushi__ripgrep-2209".to_string(),
            repo_root: PathBuf::from("/tmp/repo"),
            storage_roots: RunStorageRoots::new("/tmp/registries", "/tmp/runs"),
            base_sha: Some("deadbeef".to_string()),
            budget: EvalBudget {
                max_turns: 8,
                max_tool_calls: 20,
                wall_clock_secs: 600,
            },
            model_id: Some("anthropic/claude-sonnet-4".to_string()),
            provider_slug: Some("openrouter".to_string()),
            campaign_id: Some("baseline-smoke".to_string()),
            batch_id: Some("ripgrep-2209".to_string()),
            run_arm_id: "structured-current-policy".to_string(),
            run_role: RegisteredRunRole::Treatment,
        }
    }

    #[test]
    fn freeze_copies_execution_config_once() {
        let intent = sample_intent();
        let frozen = intent.freeze();

        let mut changed = intent.clone();
        changed.storage_roots = RunStorageRoots::new("/tmp/other-registries", "/tmp/other-runs");
        changed.base_sha = Some("cafebabe".to_string());
        changed.budget.max_turns = 99;

        assert_eq!(frozen.task_id, "BurntSushi__ripgrep-2209");
        assert_eq!(frozen.storage_roots.runs_dir, PathBuf::from("/tmp/runs"));
        assert_eq!(frozen.base_sha.as_deref(), Some("deadbeef"));
        assert_eq!(frozen.budget.max_turns, 8);
        assert_eq!(
            frozen.model_id.as_deref(),
            Some("anthropic/claude-sonnet-4")
        );
        assert_eq!(frozen.provider_slug.as_deref(), Some("openrouter"));
        assert_eq!(frozen.campaign_id.as_deref(), Some("baseline-smoke"));
        assert_eq!(frozen.batch_id.as_deref(), Some("ripgrep-2209"));
        assert_eq!(frozen.run_arm_id, "structured-current-policy");
        assert_eq!(frozen.run_role, RegisteredRunRole::Treatment);
        assert_eq!(frozen.storage_roots, intent.storage_roots);
    }
}
