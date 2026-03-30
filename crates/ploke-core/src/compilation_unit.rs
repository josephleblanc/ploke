use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Namespace UUID used for deterministic compilation-unit IDs (UUID v5).
pub const COMPILATION_UNIT_ID_NAMESPACE: Uuid = uuid::uuid!("a1b2c3d4-e5f6-5a7b-8c9d-0e1f2a3b4c5d");

/// Cargo target kind used in compilation-unit identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CompilationUnitTargetKind {
    Lib,
    Bin,
    Test,
    Example,
    Bench,
}

impl CompilationUnitTargetKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lib => "lib",
            Self::Bin => "bin",
            Self::Test => "test",
            Self::Example => "example",
            Self::Bench => "bench",
        }
    }
}

/// Stable identity for a compilation unit: crate namespace, Cargo target, triple, profile, features.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CompilationUnitKey {
    pub namespace: Uuid,
    pub target_kind: CompilationUnitTargetKind,
    pub target_name: String,
    pub target_root: PathBuf,
    pub target_triple: String,
    pub profile: String,
    pub features: Vec<String>,
}

/// Sort, dedupe, and normalize a feature set for stable keys and hashing.
pub fn normalize_features(features: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut v: Vec<String> = features.into_iter().collect();
    v.sort();
    v.dedup();
    v
}

/// Deterministic UUID derived from the normalized feature list (UUID v5).
pub fn features_hash_uuid(features: &[String]) -> Uuid {
    let joined = features.join("\0");
    Uuid::new_v5(&COMPILATION_UNIT_ID_NAMESPACE, joined.as_bytes())
}

impl CompilationUnitKey {
    /// Constructs a key and normalizes `features`.
    pub fn new(
        namespace: Uuid,
        target_kind: CompilationUnitTargetKind,
        target_name: String,
        target_root: PathBuf,
        target_triple: String,
        profile: String,
        features: Vec<String>,
    ) -> Self {
        Self {
            namespace,
            target_kind,
            target_name,
            target_root,
            target_triple,
            profile,
            features: normalize_features(features),
        }
    }

    /// Returns normalized features (sorted, deduped).
    pub fn normalized_features(&self) -> Vec<String> {
        normalize_features(self.features.iter().cloned())
    }

    /// Hash of the feature list only (for storage and debugging).
    pub fn features_hash(&self) -> Uuid {
        features_hash_uuid(&self.normalized_features())
    }

    /// Deterministic compilation-unit identifier (UUID v5 over the full canonical key).
    pub fn compilation_unit_id(&self) -> Uuid {
        Uuid::new_v5(&COMPILATION_UNIT_ID_NAMESPACE, &self.canonical_bytes())
    }

    fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(self.namespace.as_bytes());
        out.extend_from_slice(b"\0");
        out.extend_from_slice(self.target_kind.as_str().as_bytes());
        out.extend_from_slice(b"\0");
        out.extend_from_slice(self.target_name.as_bytes());
        out.extend_from_slice(b"\0");
        out.extend_from_slice(self.target_root.to_string_lossy().as_bytes());
        out.extend_from_slice(b"\0");
        out.extend_from_slice(self.target_triple.as_bytes());
        out.extend_from_slice(b"\0");
        out.extend_from_slice(self.profile.as_bytes());
        out.extend_from_slice(b"\0");
        for feature in self.normalized_features() {
            out.extend_from_slice(feature.as_bytes());
            out.extend_from_slice(b"\0");
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_features_sorts_and_dedupes() {
        let v = normalize_features(["b".into(), "a".into(), "a".into()]);
        assert_eq!(v, vec!["a", "b"]);
    }

    #[test]
    fn compilation_unit_id_is_deterministic_for_feature_order() {
        let k1 = CompilationUnitKey::new(
            Uuid::nil(),
            CompilationUnitTargetKind::Lib,
            "foo".into(),
            PathBuf::from("/tmp/lib.rs"),
            "x86_64-unknown-linux-gnu".into(),
            "dev".into(),
            vec!["b".into(), "a".into()],
        );
        let k2 = CompilationUnitKey::new(
            Uuid::nil(),
            CompilationUnitTargetKind::Lib,
            "foo".into(),
            PathBuf::from("/tmp/lib.rs"),
            "x86_64-unknown-linux-gnu".into(),
            "dev".into(),
            vec!["a".into(), "b".into()],
        );
        assert_eq!(k1.compilation_unit_id(), k2.compilation_unit_id());
    }
}
