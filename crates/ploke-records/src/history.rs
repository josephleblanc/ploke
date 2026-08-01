//! Passive History record mirrors.
//!
//! `Block<S>` and `Entry<S>` typestate remain in `ploke-eval`. The records here
//! mirror persisted sealed/admitted shapes with public fields for projection and
//! UI access. They do not prove hashes, authorize successor startup, or advance
//! a lineage head.

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::ids::{
    ArtifactId, BlockHash, BlockId, EntryId, HistoryHash, HistoryStateRoot, LineageId, RecordedAt,
};

mod payload;

pub use payload::*;

/// Actor identity as stored in History records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum ActorRefRecord {
    Runtime(crate::ids::RuntimeId),
    Human(String),
    Process(String),
    External(String),
    Unknown { reason: String },
}

macro_rules! value_ref {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub struct $name {
            pub value: String,
        }
    };
}

value_ref! {
    /// Subject of one History entry.
    SubjectRefRecord
}

value_ref! {
    /// Procedure, transition, or policy identity.
    ProcedureRefRecord
}

value_ref! {
    /// Content-addressed or stable evidence reference.
    EvidenceRefRecord
}

/// Stable sha256 identity for one History artifact-boundary reference.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactRefIdRecord(pub String);

mod artifact_ref {
    use super::{ArtifactId, ArtifactRefIdRecord};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(super) enum Repr {
        Tagged(Tagged),
        Legacy(Legacy),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "kind")]
    pub(super) enum Tagged {
        Artifact {
            id: ArtifactRefIdRecord,
            artifact_id: ArtifactId,
        },
        Branch {
            id: ArtifactRefIdRecord,
            branch_id: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(super) struct Legacy {
        pub(super) value: String,
    }

    impl<'de> Deserialize<'de> for Repr {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(Deserialize)]
            #[serde(untagged)]
            enum Untagged {
                Tagged(Tagged),
                Legacy(Legacy),
            }

            match Untagged::deserialize(deserializer)? {
                Untagged::Tagged(tagged) => Ok(Self::Tagged(tagged)),
                Untagged::Legacy(legacy) => Ok(Self::Legacy(legacy)),
            }
        }
    }
}

/// Recoverable artifact identity used by History boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "artifact_ref::Tagged", try_from = "artifact_ref::Repr")]
pub enum ArtifactRefRecord {
    Artifact {
        id: ArtifactRefIdRecord,
        artifact_id: ArtifactId,
    },
    Branch {
        id: ArtifactRefIdRecord,
        branch_id: String,
    },
}

impl ArtifactRefRecord {
    pub fn from_artifact_id(artifact_id: ArtifactId) -> Self {
        Self::Artifact {
            id: artifact_ref_id("artifact", artifact_id.as_str()),
            artifact_id,
        }
    }

    pub fn from_branch_id(branch_id: impl Into<String>) -> Self {
        let branch_id = branch_id.into();
        Self::Branch {
            id: artifact_ref_id("branch", branch_id.as_str()),
            branch_id,
        }
    }

    pub fn id(&self) -> &ArtifactRefIdRecord {
        match self {
            Self::Artifact { id, .. } | Self::Branch { id, .. } => id,
        }
    }

    pub fn artifact_id(&self) -> Option<&ArtifactId> {
        match self {
            Self::Artifact { artifact_id, .. } => Some(artifact_id),
            Self::Branch { .. } => None,
        }
    }

    pub fn branch_id(&self) -> Option<&str> {
        match self {
            Self::Artifact { .. } => None,
            Self::Branch { branch_id, .. } => Some(branch_id.as_str()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Artifact { artifact_id, .. } => artifact_id.as_str(),
            Self::Branch { branch_id, .. } => branch_id.as_str(),
        }
    }

    pub fn graph_entity_key(&self) -> &str {
        match self {
            Self::Artifact { artifact_id, .. } => artifact_id
                .as_str()
                .strip_prefix("artifact:")
                .unwrap_or(artifact_id.as_str()),
            Self::Branch { id, .. } => id.0.as_str(),
        }
    }
}

impl From<ArtifactRefRecord> for artifact_ref::Tagged {
    fn from(value: ArtifactRefRecord) -> Self {
        match value {
            ArtifactRefRecord::Artifact { id, artifact_id } => Self::Artifact { id, artifact_id },
            ArtifactRefRecord::Branch { id, branch_id } => Self::Branch { id, branch_id },
        }
    }
}

impl TryFrom<artifact_ref::Repr> for ArtifactRefRecord {
    type Error = String;

    fn try_from(value: artifact_ref::Repr) -> Result<Self, Self::Error> {
        match value {
            artifact_ref::Repr::Tagged(tagged) => match tagged {
                artifact_ref::Tagged::Artifact { id, artifact_id } => {
                    let expected = artifact_ref_id("artifact", artifact_id.as_str());
                    if id != expected {
                        return Err(format!(
                            "artifact ref id mismatch: expected {}, got {}",
                            expected.0, id.0
                        ));
                    }
                    Ok(Self::Artifact { id, artifact_id })
                }
                artifact_ref::Tagged::Branch { id, branch_id } => {
                    let expected = artifact_ref_id("branch", branch_id.as_str());
                    if id != expected {
                        return Err(format!(
                            "artifact ref id mismatch: expected {}, got {}",
                            expected.0, id.0
                        ));
                    }
                    Ok(Self::Branch { id, branch_id })
                }
            },
            artifact_ref::Repr::Legacy(artifact_ref::Legacy { value }) => {
                if let Some(artifact_id) = value.strip_prefix("artifact:") {
                    return Ok(Self::from_artifact_id(ArtifactId(artifact_id.to_owned())));
                }
                if let Some(branch_id) = value.strip_prefix("branch:") {
                    return Ok(Self::from_branch_id(branch_id));
                }
                Err(format!("unsupported legacy artifact ref value: {value}"))
            }
        }
    }
}

fn artifact_ref_id(kind: &str, value: &str) -> ArtifactRefIdRecord {
    let mut hasher = Sha256::new();
    hasher.update("prototype1.history.artifact_ref.v1");
    hasher.update([0]);
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    ArtifactRefIdRecord(format!("{:x}", hasher.finalize()))
}

/// Operational environment in which an entry occurred or was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationalEnvironmentRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<crate::ids::RuntimeId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<ArtifactRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binary: Option<EvidenceRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_surface: Option<EvidenceRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub procedure_version: Option<ProcedureRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code_graph: Option<EvidenceRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_task: Option<EvidenceRefRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorder: Option<EvidenceRefRecord>,
}

/// Kind of fact admitted into History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKindRecord {
    Observation,
    ProcedureRun,
    Judgment,
    Decision,
    Transition,
    Projection,
}

/// Entry-local payload committed by the entry hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum EntryPayloadRecord {
    Direct,
    SelectionDecision(SelectionDecisionEntryRecord),
    IngressImport(IngressImportRecord),
}

/// Entry fields common to all stored entry states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntryCoreRecord {
    pub entry_id: EntryId,
    pub entry_kind: EntryKindRecord,
    pub subject: SubjectRefRecord,
    pub executor: ActorRefRecord,
    pub input_refs: Vec<EvidenceRefRecord>,
    pub output_refs: Vec<EvidenceRefRecord>,
    pub occurred_at: RecordedAt,
    pub payload: EntryPayloadRecord,
}

/// Observed state material stored inside admitted entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedEntryRecord {
    pub observer: ActorRefRecord,
    pub recorder: ActorRefRecord,
    pub operational_environment: OperationalEnvironmentRecord,
    pub payload_ref: EvidenceRefRecord,
    pub payload_hash: HistoryHash,
    pub observed_at: RecordedAt,
    pub recorded_at: RecordedAt,
}

/// Admitted state material stored for a History entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmittedEntryStateRecord {
    pub observed: ObservedEntryRecord,
    pub proposer: ActorRefRecord,
    pub procedure_or_policy: ProcedureRefRecord,
    pub admitting_authority: ActorRefRecord,
    pub ruling_authority: ActorRefRecord,
    pub lineage_id: LineageId,
    pub block_id: BlockId,
    pub block_height: u64,
}

/// Stored admitted History entry record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdmittedEntryRecord {
    pub core: EntryCoreRecord,
    pub state: AdmittedEntryStateRecord,
}

/// Absolute cycle coordinate for strategy scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepRecord(pub u64);

/// Coarse phase of a periodic strategy cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhaseRecord {
    Expansion,
    Evaluation,
    Consolidation,
    Hardening,
}

/// Coarse ordinal level for one risk axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelRecord {
    Low,
    Medium,
    High,
}

/// Coarse risk profile for the current strategy phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskRecord {
    pub exploration: LevelRecord,
    pub mutation: LevelRecord,
    pub finality: LevelRecord,
}

/// Regime context committed in block headers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeRecord {
    pub step: StepRecord,
    pub phase: PhaseRecord,
    pub risk: RiskRecord,
}

/// Artifact surface commitment carried in block headers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceCommitmentRecord {
    pub immutable: SurfaceRecord,
    pub mutated: SurfaceDeltaRecord,
    pub ambient: SurfaceDeltaRecord,
}

/// Digest commitment to one partition of an Artifact surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRecord {
    pub root: SurfaceRootRecord,
}

/// Root digest for a declared Artifact surface partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceRootRecord {
    pub hash: HistoryHash,
}

/// Before/after commitment for a surface partition that policy compares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceDeltaRecord {
    pub before: SurfaceRecord,
    pub after: SurfaceRecord,
}

/// Artifact-relative path used by block claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactPathRecord {
    pub value: String,
}

/// Stable commitment to a backend-owned clean tree key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeKeyHashRecord {
    pub hash: HistoryHash,
}

/// Typed content digest for a recoverable History object.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DigestRecord {
    pub hash: HistoryHash,
}

/// Witness for a block claim produced under the current ruling authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RulerWitnessRecord {
    pub ruler: ActorRefRecord,
    pub environment: OperationalEnvironmentRecord,
    pub witnessed_at: RecordedAt,
}

/// Admission decision for a witnessed block claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionRecord {
    pub admitting_authority: ActorRefRecord,
    pub policy: ProcedureRefRecord,
    pub admitted_at: RecordedAt,
}

/// Flat stored form for one admitted, witnessed, verifiable claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FlatClaimRecord<K, D> {
    pub key: K,
    pub digest: D,
    pub witness: RulerWitnessRecord,
    pub admission: AdmissionRecord,
}

/// Flattened block claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClaimsRecord {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<FlatClaimRecord<ArtifactPathRecord, DigestRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub surface: Option<FlatClaimRecord<ArtifactPathRecord, DigestRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest: Option<FlatClaimRecord<ArtifactPathRecord, DigestRecord>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact: Option<FlatClaimRecord<TreeKeyHashRecord, DigestRecord>>,
}

/// Parent identity evidence committed into a parent-capable Artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentIdentityRefRecord {
    pub evidence: EvidenceRefRecord,
}

/// Authority that opens the first block for a lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenesisAuthorityRecord {
    pub bootstrap_policy: ProcedureRefRecord,
    pub tree_key: TreeKeyHashRecord,
    pub parent_identity: ParentIdentityRefRecord,
}

/// Authority that opens a non-genesis block from a sealed predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PredecessorAuthorityRecord {
    pub predecessor_block_hash: BlockHash,
}

/// Authority basis recorded for a block opening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum OpeningAuthorityRecord {
    Genesis(GenesisAuthorityRecord),
    Predecessor(PredecessorAuthorityRecord),
}

/// Selected successor named in a sealed block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuccessorRefRecord {
    pub runtime: ActorRefRecord,
    pub artifact: ArtifactRefRecord,
}

/// Header fields common to stored block states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockCommonRecord {
    pub schema_version: u32,
    pub block_id: BlockId,
    pub lineage_id: LineageId,
    pub block_height: u64,
    pub parent_block_hashes: Vec<BlockHash>,
    pub opened_from_state: HistoryStateRoot,
    pub regime: RegimeRecord,
    pub opening_authority: OpeningAuthorityRecord,
    pub opened_by: ActorRefRecord,
    pub opened_from_artifact: ArtifactRefRecord,
    pub ruling_authority: ActorRefRecord,
    pub policy_ref: ProcedureRefRecord,
    pub surface: SurfaceCommitmentRecord,
    pub opened_at: RecordedAt,
}

/// Header material committed by a stored sealed block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SealedBlockHeaderRecord {
    pub common: BlockCommonRecord,
    pub crown_lock_transition: EvidenceRefRecord,
    pub selected_successor: SuccessorRefRecord,
    pub active_artifact: ArtifactRefRecord,
    pub claims: ClaimsRecord,
    pub sealed_at: RecordedAt,
    pub entry_count: usize,
    pub entries_root: HistoryHash,
    pub block_hash: BlockHash,
}

/// Stored sealed block state wrapper.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SealedBlockStateRecord {
    pub header: SealedBlockHeaderRecord,
    #[serde(rename = "_private")]
    pub private: (),
}

/// Stored sealed History block record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SealedBlockRecord {
    pub state: SealedBlockStateRecord,
    pub entries: Vec<AdmittedEntryRecord>,
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;
    use crate::identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentityRecord};
    use crate::ids::CampaignId;
    use crate::invocation::{InvocationRecord, Role};

    fn hash(value: char) -> crate::ids::HistoryHash {
        crate::ids::HistoryHash(value.to_string().repeat(64))
    }

    fn surface(value: char) -> SurfaceRecord {
        SurfaceRecord {
            root: SurfaceRootRecord { hash: hash(value) },
        }
    }

    fn surface_delta(before: char, after: char) -> SurfaceDeltaRecord {
        SurfaceDeltaRecord {
            before: surface(before),
            after: surface(after),
        }
    }

    #[test]
    fn invocation_record_roundtrips() {
        let record = InvocationRecord {
            schema_version: crate::invocation::INVOCATION_SCHEMA_VERSION.to_string(),
            role: Role::Successor,
            campaign_id: CampaignId::from("campaign-1"),
            node_id: "node-2".to_string(),
            runtime_id: crate::ids::RuntimeId("runtime-1".to_string()),
            journal_path: "/tmp/prototype1/journal.jsonl".into(),
            channel_root: Some("/tmp/prototype1/channels/runtime-1".into()),
            node: None,
            request: None,
            resolved: None,
            active_parent_root: Some("/repo/parent".into()),
            created_at: "2026-05-08T00:00:00Z".to_string(),
        };

        let json = serde_json::to_string(&record).expect("serialize invocation");
        let parsed: InvocationRecord = serde_json::from_str(&json).expect("parse invocation");

        assert_eq!(parsed, record);
    }

    #[test]
    fn parent_identity_record_roundtrips() {
        let record = ParentIdentityRecord {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: CampaignId::from("campaign-1"),
            parent_id: "node-1".to_string(),
            node_id: "node-1".to_string(),
            generation: 0,
            instance_id: Some("instance-1".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-1".to_string(),
            artifact_branch: Some("prototype1-parent-0".to_string()),
            created_at: "2026-05-08T00:00:00Z".to_string(),
        };

        let json = serde_json::to_value(&record).expect("serialize identity");
        let parsed: ParentIdentityRecord = serde_json::from_value(json).expect("parse identity");

        assert_eq!(parsed, record);
    }

    #[test]
    fn sealed_block_record_roundtrips_as_passive_data() {
        let actor = ActorRefRecord::Process("prototype1".to_string());
        let evidence = EvidenceRefRecord {
            value: "evidence:crown-lock".to_string(),
        };
        let artifact = ArtifactRefRecord::from_artifact_id(crate::ids::ArtifactId(
            "artifact:successor".to_string(),
        ));
        let block = SealedBlockRecord {
            state: SealedBlockStateRecord {
                header: SealedBlockHeaderRecord {
                    common: BlockCommonRecord {
                        schema_version: 1,
                        block_id: crate::ids::BlockId(
                            "00000000-0000-0000-0000-000000000001".to_string(),
                        ),
                        lineage_id: crate::ids::LineageId("lineage:a".to_string()),
                        block_height: 0,
                        parent_block_hashes: vec![],
                        opened_from_state: crate::ids::HistoryStateRoot("0".repeat(64)),
                        regime: RegimeRecord {
                            step: StepRecord(0),
                            phase: PhaseRecord::Consolidation,
                            risk: RiskRecord {
                                exploration: LevelRecord::Medium,
                                mutation: LevelRecord::Medium,
                                finality: LevelRecord::Medium,
                            },
                        },
                        opening_authority: OpeningAuthorityRecord::Genesis(
                            GenesisAuthorityRecord {
                                bootstrap_policy: ProcedureRefRecord {
                                    value: "policy:bootstrap".to_string(),
                                },
                                tree_key: TreeKeyHashRecord { hash: hash('a') },
                                parent_identity: ParentIdentityRefRecord {
                                    evidence: EvidenceRefRecord {
                                        value: "parent-identity".to_string(),
                                    },
                                },
                            },
                        ),
                        opened_by: actor.clone(),
                        opened_from_artifact: artifact.clone(),
                        ruling_authority: actor.clone(),
                        policy_ref: ProcedureRefRecord {
                            value: "policy:prototype1".to_string(),
                        },
                        surface: SurfaceCommitmentRecord {
                            immutable: surface('b'),
                            mutated: surface_delta('c', 'd'),
                            ambient: surface_delta('e', 'f'),
                        },
                        opened_at: crate::ids::RecordedAt(1),
                    },
                    crown_lock_transition: evidence.clone(),
                    selected_successor: SuccessorRefRecord {
                        runtime: actor,
                        artifact: artifact.clone(),
                    },
                    active_artifact: artifact,
                    claims: ClaimsRecord {
                        policy: None,
                        surface: None,
                        manifest: None,
                        artifact: None,
                    },
                    sealed_at: crate::ids::RecordedAt(2),
                    entry_count: 0,
                    entries_root: crate::ids::HistoryHash("1".repeat(64)),
                    block_hash: crate::ids::BlockHash("2".repeat(64)),
                },
                private: (),
            },
            entries: vec![],
        };

        let json = serde_json::to_string(&block).expect("serialize block");
        let parsed: SealedBlockRecord = serde_json::from_str(&json).expect("parse block");

        assert_eq!(parsed, block);
    }

    #[test]
    fn artifact_ref_record_deserializes_legacy_string_shape() {
        let parsed: ArtifactRefRecord =
            serde_json::from_str(r#"{"value":"artifact:artifact:git-commit:95e4ee12"}"#)
                .expect("deserialize legacy artifact ref");

        assert_eq!(
            parsed,
            ArtifactRefRecord::from_artifact_id(crate::ids::ArtifactId(
                "artifact:git-commit:95e4ee12".to_string(),
            ))
        );
    }

    #[test]
    fn public_api_has_no_authority_surface() {
        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut files = Vec::new();
        collect_rs_files(&src, &mut files);

        let forbidden = [
            concat!("pub enum ", "Block<"),
            concat!("pub type ", "Block"),
            concat!("pub struct ", "Block<"),
            concat!("pub struct ", "Entry<"),
            concat!("pub struct ", "Parent<"),
            concat!("pub struct ", "Crown<"),
            concat!("pub fn ", "new"),
            concat!("pub fn ", "verify"),
            concat!("pub fn ", "seal"),
            concat!("pub fn ", "admit"),
            concat!("pub fn ", "append"),
            concat!("pub fn ", "lock"),
            concat!("pub fn ", "open"),
            concat!("pub fn ", "verify_from_record"),
        ];

        for file in files {
            // playback.rs is passive vocabulary; RunPlayback::new is a data
            // constructor, not an authority-bearing operation.
            if file.file_name().is_some_and(|n| n == "playback.rs") {
                continue;
            }
            let text = fs::read_to_string(&file).expect("read source");
            for pattern in forbidden {
                for line in text.lines() {
                    let trimmed = line.trim_start();
                    if trimmed.starts_with("//") {
                        continue;
                    }
                    assert!(
                        !line.contains(pattern),
                        "{} contains forbidden public authority API pattern {pattern:?}",
                        file.display()
                    );
                }
            }
        }
    }

    fn collect_rs_files(dir: &Path, files: &mut Vec<std::path::PathBuf>) {
        for entry in fs::read_dir(dir).expect("read src dir") {
            let entry = entry.expect("read entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rs_files(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                files.push(path);
            }
        }
    }
}
