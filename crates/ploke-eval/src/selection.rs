use std::fs;
use std::path::Path;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::campaign::load_campaign_manifest;
use crate::layout::{active_selection_file, batches_dir};
use crate::spec::{PrepareError, PreparedMsbBatch};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSelection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub batch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ActiveSelectionSlot {
    Campaign,
    Batch,
    Instance,
    Attempt,
}

impl ActiveSelection {
    pub fn is_empty(&self) -> bool {
        self.campaign.is_none()
            && self.batch.is_none()
            && self.instance.is_none()
            && self.attempt.is_none()
    }
}

pub fn load_active_selection() -> Result<ActiveSelection, PrepareError> {
    let path = active_selection_file()?;
    load_active_selection_from_path(&path)
}

pub(crate) fn load_active_selection_at(eval_home: &Path) -> Result<ActiveSelection, PrepareError> {
    let path = eval_home.join("selection.json");
    load_active_selection_from_path(&path)
}

fn load_active_selection_from_path(path: &Path) -> Result<ActiveSelection, PrepareError> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: path.to_path_buf(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(ActiveSelection::default())
        }
        Err(source) => Err(PrepareError::ReadManifest {
            path: path.to_path_buf(),
            source,
        }),
    }
}

pub fn save_active_selection(selection: &ActiveSelection) -> Result<(), PrepareError> {
    let path = active_selection_file()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let json = serde_json::to_string_pretty(selection).map_err(PrepareError::Serialize)?;
    fs::write(&path, json).map_err(|source| PrepareError::WriteManifest { path, source })
}

pub fn clear_active_selection() -> Result<(), PrepareError> {
    save_active_selection(&ActiveSelection::default())
}

pub fn unset_active_selection_slot(slot: ActiveSelectionSlot) -> Result<(), PrepareError> {
    let mut selection = load_active_selection()?;
    match slot {
        ActiveSelectionSlot::Campaign => selection.campaign = None,
        ActiveSelectionSlot::Batch => selection.batch = None,
        ActiveSelectionSlot::Instance => {
            selection.instance = None;
            selection.attempt = None;
        }
        ActiveSelectionSlot::Attempt => selection.attempt = None,
    }
    save_active_selection(&selection)
}

pub fn render_selection_warnings(selection: &ActiveSelection) -> Vec<String> {
    let mut warnings = Vec::new();

    if let (Some(campaign_id), Some(instance_id)) =
        (selection.campaign.as_deref(), selection.instance.as_deref())
    {
        match campaign_includes_instance(campaign_id, instance_id) {
            Ok(true) => {}
            Ok(false) => warnings.push(format!(
                "selected campaign {campaign_id} does not include selected instance {instance_id}"
            )),
            Err(err) => warnings.push(format!(
                "selected campaign {campaign_id} could not be checked against selected instance {instance_id}: {err}"
            )),
        }
    }

    if let (Some(batch_id), Some(instance_id)) =
        (selection.batch.as_deref(), selection.instance.as_deref())
    {
        match batch_includes_instance(batch_id, instance_id) {
            Ok(true) => {}
            Ok(false) => warnings.push(format!(
                "selected batch {batch_id} does not include selected instance {instance_id}"
            )),
            Err(err) => warnings.push(format!(
                "selected batch {batch_id} could not be checked against selected instance {instance_id}: {err}"
            )),
        }
    }

    warnings
}

fn campaign_includes_instance(campaign_id: &str, instance_id: &str) -> Result<bool, PrepareError> {
    let manifest = load_campaign_manifest(campaign_id)?;
    let Some(instance_family) = instance_family(instance_id) else {
        return Ok(false);
    };
    Ok(manifest
        .dataset_sources
        .iter()
        .any(|source| source.label == instance_family))
}

fn batch_includes_instance(batch_id: &str, instance_id: &str) -> Result<bool, PrepareError> {
    let path = batches_dir()?.join(batch_id).join("batch.json");
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadBatchManifest {
        path: path.clone(),
        source,
    })?;
    let batch: PreparedMsbBatch =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseBatchManifest {
            path: path.clone(),
            source,
        })?;
    Ok(batch
        .instances
        .iter()
        .any(|candidate| candidate == instance_id))
}

fn instance_family(instance_id: &str) -> Option<&str> {
    instance_id.rsplit_once('-').map(|(family, _)| family)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_slot_unset_also_clears_attempt() {
        let mut selection = ActiveSelection {
            campaign: Some("camp".to_string()),
            batch: Some("batch".to_string()),
            instance: Some("org__repo-1".to_string()),
            attempt: Some(2),
        };
        match ActiveSelectionSlot::Instance {
            ActiveSelectionSlot::Campaign => selection.campaign = None,
            ActiveSelectionSlot::Batch => selection.batch = None,
            ActiveSelectionSlot::Instance => {
                selection.instance = None;
                selection.attempt = None;
            }
            ActiveSelectionSlot::Attempt => selection.attempt = None,
        }
        assert!(selection.instance.is_none());
        assert!(selection.attempt.is_none());
    }

    #[test]
    fn instance_family_uses_prefix_before_final_dash() {
        assert_eq!(
            instance_family("BurntSushi__ripgrep-2209"),
            Some("BurntSushi__ripgrep")
        );
        assert_eq!(
            instance_family("not-a-normal-instance"),
            Some("not-a-normal")
        );
    }
}
