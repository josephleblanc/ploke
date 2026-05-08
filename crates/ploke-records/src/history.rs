//! Passive History record mirrors.
//!
//! `Block<S>` and `Entry<S>` typestate remain in `ploke-eval`. The records here
//! mirror persisted sealed/admitted shapes with public fields for projection and
//! UI access. They do not prove hashes, authorize successor startup, or advance
//! a lineage head.

use serde::{Deserialize, Serialize};

use crate::ids::{
    BlockHash, BlockId, EntryId, HistoryHash, HistoryStateRoot, LineageId, RecordedAt,
};
use crate::value::JsonRecordValue;

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

value_ref! {
    /// Recoverable artifact identity used by History boundaries.
    ArtifactRefRecord
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
    SelectionDecision(JsonRecordValue),
    IngressImport(JsonRecordValue),
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

/// Regime context committed in block headers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegimeRecord {
    pub step: JsonRecordValue,
    pub phase: JsonRecordValue,
    pub risk: JsonRecordValue,
}

/// Artifact surface commitment carried in block headers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceCommitmentRecord {
    pub immutable: JsonRecordValue,
    pub mutated: JsonRecordValue,
    pub ambient: JsonRecordValue,
}

/// Authority basis recorded for a block opening.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum OpeningAuthorityRecord {
    Genesis(JsonRecordValue),
    Predecessor(JsonRecordValue),
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
    pub claims: JsonRecordValue,
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
    pub private: JsonRecordValue,
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
    use crate::identity::{ParentIdentityRecord, PARENT_IDENTITY_SCHEMA_VERSION};
    use crate::invocation::{InvocationRecord, Role};

    fn value(value: &str) -> JsonRecordValue {
        JsonRecordValue::String(value.to_string())
    }

    #[test]
    fn invocation_record_roundtrips() {
        let record = InvocationRecord {
            schema_version: crate::invocation::INVOCATION_SCHEMA_VERSION.to_string(),
            role: Role::Successor,
            campaign_id: "campaign-1".to_string(),
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
            campaign_id: "campaign-1".to_string(),
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
        let artifact = ArtifactRefRecord {
            value: "artifact:successor".to_string(),
        };
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
                            step: value("0"),
                            phase: value("consolidation"),
                            risk: value("balanced"),
                        },
                        opening_authority: OpeningAuthorityRecord::Genesis(value("bootstrap")),
                        opened_by: actor.clone(),
                        opened_from_artifact: artifact.clone(),
                        ruling_authority: actor.clone(),
                        policy_ref: ProcedureRefRecord {
                            value: "policy:prototype1".to_string(),
                        },
                        surface: SurfaceCommitmentRecord {
                            immutable: value("immutable"),
                            mutated: value("mutated"),
                            ambient: value("ambient"),
                        },
                        opened_at: crate::ids::RecordedAt(1),
                    },
                    crown_lock_transition: evidence.clone(),
                    selected_successor: SuccessorRefRecord {
                        runtime: actor,
                        artifact: artifact.clone(),
                    },
                    active_artifact: artifact,
                    claims: JsonRecordValue::Object(Default::default()),
                    sealed_at: crate::ids::RecordedAt(2),
                    entry_count: 0,
                    entries_root: crate::ids::HistoryHash("1".repeat(64)),
                    block_hash: crate::ids::BlockHash("2".repeat(64)),
                },
                private: JsonRecordValue::Object(Default::default()),
            },
            entries: vec![],
        };

        let json = serde_json::to_string(&block).expect("serialize block");
        let parsed: SealedBlockRecord = serde_json::from_str(&json).expect("parse block");

        assert_eq!(parsed, block);
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
