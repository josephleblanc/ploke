use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::PROJECT_NAMESPACE_UUID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CrateId(Uuid);

impl CrateId {
    pub fn new(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn uuid(self) -> Uuid {
        self.0
    }

    pub fn from_root_path(path: &Path) -> Self {
        let canon = canonicalize_best_effort(path);
        let id_bytes = canon.to_string_lossy();
        let uuid = Uuid::new_v5(&PROJECT_NAMESPACE_UUID, id_bytes.as_bytes());
        Self(uuid)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrateInfo {
    pub id: CrateId,
    pub namespace: Uuid,
    pub root_path: PathBuf,
    pub name: String,
}

impl CrateInfo {
    pub fn from_root_path(root_path: PathBuf) -> Self {
        let canon = canonicalize_best_effort(&root_path);
        let name = canon
            .file_name()
            .map(|os| os.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        let id = CrateId::from_root_path(&canon);
        Self {
            id,
            namespace: id.uuid(),
            root_path: canon,
            name,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct WorkspaceRoots {
    pub crates: Vec<CrateInfo>,
}

impl WorkspaceRoots {
    pub fn new(crates: Vec<CrateInfo>) -> Self {
        Self { crates }
    }

    pub fn find_by_id(&self, id: CrateId) -> Option<&CrateInfo> {
        self.crates.iter().find(|info| info.id == id)
    }

    pub fn find_by_root_path(&self, path: &Path) -> Option<&CrateInfo> {
        let canon = canonicalize_best_effort(path);
        self.crates.iter().find(|info| info.root_path == canon)
    }

    pub fn upsert(&mut self, info: CrateInfo) {
        if let Some(existing) = self
            .crates
            .iter_mut()
            .find(|crate_info| crate_info.id == info.id)
        {
            *existing = info;
        } else {
            self.crates.push(info);
        }
    }
}

fn canonicalize_best_effort(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}
