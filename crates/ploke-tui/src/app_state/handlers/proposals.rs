use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::app_state::core::EditProposal;
use crate::AppState;

fn default_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ploke")
        .join("proposals.json")
}

pub async fn save_proposals(state: &Arc<AppState>) {
    let guard = state.proposals.read().await;
    let list: Vec<EditProposal> = guard.values().cloned().collect();
    drop(guard);
    let path = default_path();
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(path, json);
    }
}

pub async fn load_proposals(state: &Arc<AppState>) {
    let path = std::env::var("PLOKE_PROPOSALS_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_path());
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(list) = serde_json::from_str::<Vec<EditProposal>>(&content) {
            let mut map: HashMap<uuid::Uuid, EditProposal> = HashMap::new();
            for p in list.into_iter() { map.insert(p.request_id, p); }
            let mut guard = state.proposals.write().await;
            *guard = map;
        }
    }
}

pub async fn save_proposals_to_path(state: &Arc<AppState>, path: &Path) {
    let guard = state.proposals.read().await;
    let list: Vec<EditProposal> = guard.values().cloned().collect();
    drop(guard);
    if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = std::fs::write(path, json);
    }
}

pub async fn load_proposals_from_path(state: &Arc<AppState>, path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(list) = serde_json::from_str::<Vec<EditProposal>>(&content) {
            let mut map: HashMap<uuid::Uuid, EditProposal> = HashMap::new();
            for p in list.into_iter() { map.insert(p.request_id, p); }
            let mut guard = state.proposals.write().await;
            *guard = map;
        }
    }
}
