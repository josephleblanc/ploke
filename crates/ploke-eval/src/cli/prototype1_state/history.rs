//! Prototype 1 History invariant framework.
//!
//! Status recorded 2026-04-27 14:32 PDT. Implementation started from this
//! specification on 2026-04-27.
//!
//! This module defines the local invariant core for Prototype 1 History. It is
//! intentionally not wired into the live successor handoff yet. Current live
//! handoff still uses typed transition scaffolding, a transition journal,
//! invocation files, successor-ready files, and mutable scheduler/branch
//! projections. Those records can be imported as evidence later; they are not
//! themselves sealed History blocks.
//!
//! The weekly review policy in `AGENTS.md` applies here: reviewers must compare
//! these claims against the actual code at least once per week while this
//! architecture is active, and must either narrow the claims or fix the
//! implementation when the code does not enforce them.
//!
//! ## Formal vocabulary
//!
//! The notation follows the style of
//! `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`, using
//! ASCII symbols in Rust comments.
//!
//! ```text
//! A    = set of actors
//! R    = set of runtime identities
//! L    = set of lineage identities
//! E    = set of evidence artifacts
//! P    = set of named procedures or policies
//! O    = set of operational environments
//! T    = set of typed transition states
//! H    = set of History states
//! I    = set of ingress observations
//! Ref  = set of content-addressed references
//! Hash = set of deterministic content hashes
//! ```
//!
//! Actor roles are distinct even when one process occupies more than one role:
//!
//! ```text
//! observer(a)            a observed an event or artifact
//! recorder(a)            a wrote the durable record
//! proposer(a)            a proposed admission into History
//! admitting_authority(a) a accepted an entry under policy
//! ruling_authority(a)    a held lineage authority for the block epoch
//! executor(a)            a executed a procedure or transition
//! ```
//!
//! The implementation below preserves these roles in `Entry<Admitted>` rather
//! than flattening them into a generic writer/status record.
//!
//! ## Implemented state carriers
//!
//! ```text
//! Block<block::Open>
//! Block<block::Sealed>
//! Entry<Draft>
//! Entry<Observed>
//! Entry<Proposed>
//! Entry<Admitted>
//! Ingress<ingress::Open>
//! Ingress<ingress::Imported>
//! ```
//!
//! The state parameter is semantic, not decorative. The fields of stateful
//! carriers are private, and advanced states are reachable through typed
//! transitions. Authoritative carriers serialize, but they intentionally do not
//! derive `Deserialize`; verified loading from disk must become its own
//! transition rather than an implicit constructor.
//!
//! ## Blocked live wiring
//!
//! The next live integration still needs concrete authority carriers outside
//! this module. The intended shape is:
//!
//! ```ignore
//! // Blocked until Prototype 1 has real live authority carriers:
//! //
//! // Parent<Ruling>
//! //   .lock_crown(selected_successor, open_block)
//! //   -> (Crown<Locked>, Block<Sealed>)
//! //
//! // Successor
//! //   .verify(crown_locked, sealed_block, active_artifact)
//! //   -> Successor<Admitted>
//! //
//! // Successor<Admitted>
//! //   .rule()
//! //   -> Parent<Ruling>
//! ```
//!
//! Blocking reasons:
//!
//! - `Parent<Ruling>`, `Crown<Locked>`, and `Successor<Admitted>` are not yet
//!   live handoff gates.
//! - live successor validation still consults mutable scheduler/invocation
//!   state instead of a sealed block hash.
//! - whole-artifact and runtime/build identities still need final canonical
//!   refs in several paths.
//! - existing journals and reports must be imported as pre-History evidence
//!   with explicit degraded provenance rather than treated as sealed History.
//!
//! ## Legacy record normalization
//!
//! Existing Prototype 1 records are admissible evidence sources, not the History
//! ontology. In particular, flattened journal names such as
//! `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, and
//! `SuccessorHandoffEntry` should not become History entry kinds. They should
//! first be normalized into role/state facts such as `Artifact<Committed>`,
//! `Checkout<Advanced>`, `Successor<Ready>`, or `Parent<Ruling>` evidence, then
//! proposed as History entries with explicit observer, recorder, proposer,
//! authority, operational environment, and payload hash.
//!
//! This keeps legacy replay compatibility separate from the typed model we want
//! History to enforce. When the structural import path is tested, the flattened
//! journal records can be marked deprecated at their source while remaining
//! readable as historical evidence.
//!
//! ## Forward-facing ancestry
//!
//! `BlockCommon::parent_block_hashes` is a list, not a singleton. The current
//! single-lineage chain is the one-parent case. Branch merges and consensus
//! extensions should not require rewriting the sealed block shape.

#![allow(dead_code)] // Self-contained invariant core; live wiring is intentionally deferred.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::event::{RecordedAt, RuntimeId};

const SCHEMA_VERSION: u32 = 1;

/// Deterministic content digest used by History entries and blocks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct HistoryHash(String);

impl HistoryHash {
    pub(crate) fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    pub(crate) fn of_domain_json<T: Serialize>(
        domain: &'static str,
        value: &T,
    ) -> Result<Self, HistoryError> {
        let bytes = serde_json::to_vec(&HashPreimage { domain, value })
            .map_err(HistoryError::StableJson)?;
        Ok(Self::of_bytes(&bytes))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize)]
struct HashPreimage<'a, T: Serialize> {
    domain: &'static str,
    value: &'a T,
}

/// Durable block hash. Kept distinct from payload hashes in type signatures.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BlockHash(String);

impl BlockHash {
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<HistoryHash> for BlockHash {
    fn from(value: HistoryHash) -> Self {
        Self(value.0)
    }
}

/// Durable identity for one block before it is sealed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct BlockId(Uuid);

impl BlockId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Durable identity for a History entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct EntryId(Uuid);

impl EntryId {
    pub(crate) fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

/// Durable identity for a lineage.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct LineageId(String);

impl LineageId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Actor identity in a custody role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub(crate) enum ActorRef {
    Runtime(RuntimeId),
    Human(String),
    Process(String),
    External(String),
    Unknown { reason: String },
}

impl ActorRef {
    pub(crate) fn unknown(reason: impl Into<String>) -> Self {
        Self::Unknown {
            reason: reason.into(),
        }
    }
}

/// Subject of one History entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SubjectRef {
    value: String,
}

impl SubjectRef {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Procedure, transition, or policy identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ProcedureRef {
    value: String,
}

impl ProcedureRef {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Content-addressed or stable evidence reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvidenceRef {
    value: String,
}

impl EvidenceRef {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Recoverable artifact identity used by History block boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ArtifactRef {
    value: String,
}

impl ArtifactRef {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Selected successor named in a sealed block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SuccessorRef {
    runtime: ActorRef,
    artifact: ArtifactRef,
}

impl SuccessorRef {
    pub(crate) fn new(runtime: ActorRef, artifact: ArtifactRef) -> Self {
        Self { runtime, artifact }
    }
}

/// Operational environment in which an entry occurred or was observed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct OperationalEnvironment {
    runtime: Option<RuntimeId>,
    artifact: Option<ArtifactRef>,
    binary: Option<EvidenceRef>,
    tool_surface: Option<EvidenceRef>,
    procedure_version: Option<ProcedureRef>,
    model: Option<String>,
    code_graph: Option<EvidenceRef>,
    oracle_task: Option<EvidenceRef>,
    recorder: Option<EvidenceRef>,
}

impl OperationalEnvironment {
    pub(crate) fn new() -> Self {
        Self {
            runtime: None,
            artifact: None,
            binary: None,
            tool_surface: None,
            procedure_version: None,
            model: None,
            code_graph: None,
            oracle_task: None,
            recorder: None,
        }
    }

    pub(crate) fn runtime(mut self, runtime: RuntimeId) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub(crate) fn artifact(mut self, artifact: ArtifactRef) -> Self {
        self.artifact = Some(artifact);
        self
    }

    pub(crate) fn binary(mut self, binary: EvidenceRef) -> Self {
        self.binary = Some(binary);
        self
    }

    pub(crate) fn tool_surface(mut self, tool_surface: EvidenceRef) -> Self {
        self.tool_surface = Some(tool_surface);
        self
    }

    pub(crate) fn procedure_version(mut self, procedure_version: ProcedureRef) -> Self {
        self.procedure_version = Some(procedure_version);
        self
    }

    pub(crate) fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub(crate) fn code_graph(mut self, code_graph: EvidenceRef) -> Self {
        self.code_graph = Some(code_graph);
        self
    }

    pub(crate) fn oracle_task(mut self, oracle_task: EvidenceRef) -> Self {
        self.oracle_task = Some(oracle_task);
        self
    }

    pub(crate) fn recorder(mut self, recorder: EvidenceRef) -> Self {
        self.recorder = Some(recorder);
        self
    }
}

/// Kind of fact admitted into History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EntryKind {
    Observation,
    ProcedureRun,
    Judgment,
    Decision,
    Transition,
    Projection,
}

/// Entry-local payload committed by the entry hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum EntryPayload {
    Direct,
    IngressImport(IngressImportPayload),
}

/// Import disposition committed to the History entry produced by ingress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ImportDisposition {
    AcceptedAsObservation,
    AcceptedAsLateTerminalStatus,
    AcceptedAsDiagnosticOnly,
}

/// Ingress chain-of-custody payload that must be sealed with the entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct IngressImportPayload {
    ingress_id: Uuid,
    prior_block_hash: BlockHash,
    original_payload_ref: EvidenceRef,
    original_payload_hash: HistoryHash,
    observed_by: ActorRef,
    observed_at: RecordedAt,
    recorded_by: ActorRef,
    recorded_at: RecordedAt,
    imported_by: ActorRef,
    import_policy: ProcedureRef,
    imported_at: RecordedAt,
    import_disposition: ImportDisposition,
    imported_into_lineage: LineageId,
    imported_into_block: BlockId,
    imported_into_height: u64,
}

/// Initial fields needed to construct an `Entry<Draft>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DraftEntry {
    pub(crate) entry_kind: EntryKind,
    pub(crate) subject: SubjectRef,
    pub(crate) executor: ActorRef,
    pub(crate) input_refs: Vec<EvidenceRef>,
    pub(crate) output_refs: Vec<EvidenceRef>,
    pub(crate) occurred_at: RecordedAt,
}

/// Observation fields required for `Entry<Draft> -> Entry<Observed>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Observation {
    pub(crate) observer: ActorRef,
    pub(crate) recorder: ActorRef,
    pub(crate) operational_environment: OperationalEnvironment,
    pub(crate) payload_ref: EvidenceRef,
    pub(crate) payload_hash: HistoryHash,
    pub(crate) observed_at: RecordedAt,
    pub(crate) recorded_at: RecordedAt,
}

/// Proposal fields required for `Entry<Observed> -> Entry<Proposed>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Proposal {
    pub(crate) proposer: ActorRef,
    pub(crate) procedure_or_policy: ProcedureRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct EntryCore {
    entry_id: EntryId,
    entry_kind: EntryKind,
    subject: SubjectRef,
    executor: ActorRef,
    input_refs: Vec<EvidenceRef>,
    output_refs: Vec<EvidenceRef>,
    occurred_at: RecordedAt,
    payload: EntryPayload,
}

/// Draft entry state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Draft {
    _private: Private,
}

/// Observed entry state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Observed {
    observer: ActorRef,
    recorder: ActorRef,
    operational_environment: OperationalEnvironment,
    payload_ref: EvidenceRef,
    payload_hash: HistoryHash,
    observed_at: RecordedAt,
    recorded_at: RecordedAt,
}

/// Proposed entry state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Proposed {
    observed: Observed,
    proposer: ActorRef,
    procedure_or_policy: ProcedureRef,
}

/// Admitted entry state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct Admitted {
    observed: Observed,
    proposer: ActorRef,
    procedure_or_policy: ProcedureRef,
    admitting_authority: ActorRef,
    ruling_authority: ActorRef,
    lineage_id: LineageId,
    block_id: BlockId,
    block_height: u64,
}

/// A provenance-bearing fact in one typed History state.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Entry<S> {
    core: EntryCore,
    state: S,
}

impl Entry<Draft> {
    pub(crate) fn draft(fields: DraftEntry) -> Self {
        Self {
            core: EntryCore {
                entry_id: EntryId::new(),
                entry_kind: fields.entry_kind,
                subject: fields.subject,
                executor: fields.executor,
                input_refs: fields.input_refs,
                output_refs: fields.output_refs,
                occurred_at: fields.occurred_at,
                payload: EntryPayload::Direct,
            },
            state: Draft { _private: Private },
        }
    }

    pub(crate) fn observe(self, observation: Observation) -> Entry<Observed> {
        Entry {
            core: self.core,
            state: Observed {
                observer: observation.observer,
                recorder: observation.recorder,
                operational_environment: observation.operational_environment,
                payload_ref: observation.payload_ref,
                payload_hash: observation.payload_hash,
                observed_at: observation.observed_at,
                recorded_at: observation.recorded_at,
            },
        }
    }
}

impl Entry<Observed> {
    pub(crate) fn propose(self, proposal: Proposal) -> Entry<Proposed> {
        Entry {
            core: self.core,
            state: Proposed {
                observed: self.state,
                proposer: proposal.proposer,
                procedure_or_policy: proposal.procedure_or_policy,
            },
        }
    }
}

impl Entry<Admitted> {
    pub(crate) fn entry_id(&self) -> EntryId {
        self.core.entry_id
    }

    pub(crate) fn payload_hash(&self) -> &HistoryHash {
        &self.state.observed.payload_hash
    }

    pub(crate) fn entry_hash(&self) -> Result<HistoryHash, HistoryError> {
        HistoryHash::of_domain_json("prototype1.history.entry.v1", self)
    }

    pub(crate) fn lineage_id(&self) -> &LineageId {
        &self.state.lineage_id
    }

    pub(crate) fn block_height(&self) -> u64 {
        self.state.block_height
    }

    pub(crate) fn block_id(&self) -> BlockId {
        self.state.block_id
    }
}

/// Data required to open a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenBlock {
    pub(crate) lineage_id: LineageId,
    pub(crate) block_height: u64,
    pub(crate) parent_block_hashes: Vec<BlockHash>,
    pub(crate) opened_by: ActorRef,
    pub(crate) opened_from_artifact: ArtifactRef,
    pub(crate) ruling_authority: ActorRef,
    pub(crate) policy_ref: ProcedureRef,
    pub(crate) opened_at: RecordedAt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct BlockCommon {
    schema_version: u32,
    block_id: BlockId,
    lineage_id: LineageId,
    block_height: u64,
    parent_block_hashes: Vec<BlockHash>,
    opened_by: ActorRef,
    opened_from_artifact: ArtifactRef,
    ruling_authority: ActorRef,
    policy_ref: ProcedureRef,
    opened_at: RecordedAt,
}

/// Data required for `Block<block::Open> -> Block<block::Sealed>`.
///
/// Implementation gap recorded 2026-04-28 11:59 PDT: this transition commits
/// a `crown_lock_transition` reference into the sealed header, but it does not
/// yet require the live `Crown<Locked>` authority carrier described in
/// `history-blocks-and-crown-authority.md`. The reference is header material,
/// not an authority token. The live integration should make block sealing a
/// projection of the Crown lock transition rather than a direct public
/// `Block<Open>` operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SealBlock {
    pub(crate) crown_lock_transition: EvidenceRef,
    pub(crate) selected_successor: SuccessorRef,
    pub(crate) active_artifact: ArtifactRef,
    pub(crate) sealed_at: RecordedAt,
}

/// Block typestate payloads.
pub(crate) mod block {
    use serde::Serialize;

    use super::{BlockCommon, Private, SealedBlockHeader};

    /// Open block state.
    #[derive(Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct Open {
        pub(super) common: BlockCommon,
        pub(super) _private: Private,
    }

    /// Sealed block state.
    #[derive(Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct Sealed {
        pub(super) header: SealedBlockHeader,
        pub(super) _private: Private,
    }
}

/// Header material committed by a sealed block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SealedBlockHeader {
    common: BlockCommon,
    crown_lock_transition: EvidenceRef,
    selected_successor: SuccessorRef,
    active_artifact: ArtifactRef,
    sealed_at: RecordedAt,
    entry_count: usize,
    entries_root: HistoryHash,
    block_hash: BlockHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct SealedBlockPreimage {
    common: BlockCommon,
    crown_lock_transition: EvidenceRef,
    selected_successor: SuccessorRef,
    active_artifact: ArtifactRef,
    sealed_at: RecordedAt,
    entry_count: usize,
    entries_root: HistoryHash,
}

/// One authority epoch in the History chain.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Block<S> {
    entries: Vec<Entry<Admitted>>,
    state: S,
}

impl Block<block::Open> {
    pub(crate) fn open(fields: OpenBlock) -> Result<Self, HistoryError> {
        Self::open_with_block_id(BlockId::new(), fields)
    }

    fn open_with_block_id(block_id: BlockId, fields: OpenBlock) -> Result<Self, HistoryError> {
        if fields.block_height == 0 && !fields.parent_block_hashes.is_empty() {
            return Err(HistoryError::GenesisHasParents);
        }
        if fields.block_height > 0 && fields.parent_block_hashes.is_empty() {
            return Err(HistoryError::NonGenesisWithoutParents);
        }

        Ok(Self {
            entries: Vec::new(),
            state: block::Open {
                common: BlockCommon {
                    schema_version: SCHEMA_VERSION,
                    block_id,
                    lineage_id: fields.lineage_id,
                    block_height: fields.block_height,
                    parent_block_hashes: fields.parent_block_hashes,
                    opened_by: fields.opened_by,
                    opened_from_artifact: fields.opened_from_artifact,
                    ruling_authority: fields.ruling_authority,
                    policy_ref: fields.policy_ref,
                    opened_at: fields.opened_at,
                },
                _private: Private,
            },
        })
    }

    pub(crate) fn admit(
        &mut self,
        entry: Entry<Proposed>,
        admitting_authority: ActorRef,
    ) -> Result<EntryId, HistoryError> {
        if let EntryPayload::IngressImport(payload) = &entry.core.payload {
            if payload.imported_into_lineage != self.state.common.lineage_id {
                return Err(HistoryError::WrongLineage);
            }
            if payload.imported_into_block != self.state.common.block_id {
                return Err(HistoryError::WrongBlock);
            }
            if payload.imported_into_height != self.state.common.block_height {
                return Err(HistoryError::WrongBlockHeight);
            }
        }

        if self
            .entries
            .iter()
            .any(|existing| existing.entry_id() == entry.core.entry_id)
        {
            return Err(HistoryError::DuplicateEntry(entry.core.entry_id));
        }

        let entry_id = entry.core.entry_id;
        self.entries.push(Entry {
            core: entry.core,
            state: Admitted {
                observed: entry.state.observed,
                proposer: entry.state.proposer,
                procedure_or_policy: entry.state.procedure_or_policy,
                admitting_authority,
                ruling_authority: self.state.common.ruling_authority.clone(),
                lineage_id: self.state.common.lineage_id.clone(),
                block_id: self.state.common.block_id,
                block_height: self.state.common.block_height,
            },
        });
        Ok(entry_id)
    }

    pub(crate) fn seal(self, fields: SealBlock) -> Result<Block<block::Sealed>, HistoryError> {
        let entry_hashes = self
            .entries
            .iter()
            .map(Entry::entry_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let entries_root =
            HistoryHash::of_domain_json("prototype1.history.entries_root.v1", &entry_hashes)?;
        let preimage = SealedBlockPreimage {
            common: self.state.common.clone(),
            crown_lock_transition: fields.crown_lock_transition,
            selected_successor: fields.selected_successor,
            active_artifact: fields.active_artifact,
            sealed_at: fields.sealed_at,
            entry_count: self.entries.len(),
            entries_root,
        };
        let block_hash = BlockHash::from(HistoryHash::of_domain_json(
            "prototype1.history.block.v1",
            &preimage,
        )?);
        let header = SealedBlockHeader {
            common: preimage.common,
            crown_lock_transition: preimage.crown_lock_transition,
            selected_successor: preimage.selected_successor,
            active_artifact: preimage.active_artifact,
            sealed_at: preimage.sealed_at,
            entry_count: preimage.entry_count,
            entries_root: preimage.entries_root,
            block_hash,
        };

        Ok(Block {
            entries: self.entries,
            state: block::Sealed {
                header,
                _private: Private,
            },
        })
    }

    pub(crate) fn lineage_id(&self) -> &LineageId {
        &self.state.common.lineage_id
    }

    pub(crate) fn block_id(&self) -> BlockId {
        self.state.common.block_id
    }

    pub(crate) fn block_height(&self) -> u64 {
        self.state.common.block_height
    }
}

impl Block<block::Sealed> {
    pub(crate) fn header(&self) -> &SealedBlockHeader {
        &self.state.header
    }

    pub(crate) fn block_hash(&self) -> &BlockHash {
        &self.header().block_hash
    }

    pub(crate) fn entries(&self) -> &[Entry<Admitted>] {
        &self.entries
    }

    pub(crate) fn verify_hash(&self) -> Result<(), HistoryError> {
        let header = self.header();
        if header.entry_count != self.entries.len() {
            return Err(HistoryError::EntryCountMismatch {
                header: header.entry_count,
                actual: self.entries.len(),
            });
        }

        let entry_hashes = self
            .entries
            .iter()
            .map(Entry::entry_hash)
            .collect::<Result<Vec<_>, _>>()?;
        let entries_root =
            HistoryHash::of_domain_json("prototype1.history.entries_root.v1", &entry_hashes)?;
        if entries_root != header.entries_root {
            return Err(HistoryError::EntriesRootMismatch);
        }

        let preimage = SealedBlockPreimage {
            common: header.common.clone(),
            crown_lock_transition: header.crown_lock_transition.clone(),
            selected_successor: header.selected_successor.clone(),
            active_artifact: header.active_artifact.clone(),
            sealed_at: header.sealed_at,
            entry_count: header.entry_count,
            entries_root,
        };
        let block_hash = BlockHash::from(HistoryHash::of_domain_json(
            "prototype1.history.block.v1",
            &preimage,
        )?);
        if &block_hash != self.block_hash() {
            return Err(HistoryError::BlockHashMismatch);
        }

        Ok(())
    }

    pub(crate) fn verify_expected_hash(&self, expected: &BlockHash) -> Result<(), HistoryError> {
        self.verify_hash()?;
        if self.block_hash() != expected {
            return Err(HistoryError::ExpectedBlockHashMismatch);
        }
        Ok(())
    }

    pub(crate) fn open_successor(
        &self,
        fields: OpenSuccessorBlock,
    ) -> Result<Block<block::Open>, HistoryError> {
        let mut parent_block_hashes = vec![self.block_hash().clone()];
        parent_block_hashes.extend(fields.additional_parent_block_hashes);
        Block::<block::Open>::open(OpenBlock {
            lineage_id: self.header().common.lineage_id.clone(),
            block_height: self.header().common.block_height + 1,
            parent_block_hashes,
            opened_by: fields.opened_by,
            opened_from_artifact: fields.opened_from_artifact,
            ruling_authority: fields.ruling_authority,
            policy_ref: fields.policy_ref,
            opened_at: fields.opened_at,
        })
    }
}

/// Data for opening a successor block after verifying a sealed predecessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenSuccessorBlock {
    pub(crate) additional_parent_block_hashes: Vec<BlockHash>,
    pub(crate) opened_by: ActorRef,
    pub(crate) opened_from_artifact: ArtifactRef,
    pub(crate) ruling_authority: ActorRef,
    pub(crate) policy_ref: ProcedureRef,
    pub(crate) opened_at: RecordedAt,
}

/// Ingress typestate payloads.
pub(crate) mod ingress {
    use serde::Serialize;

    use super::{ImportedIngress, Private};

    /// Late observation not yet imported into History.
    #[derive(Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct Open {
        pub(super) _private: Private,
    }

    /// Late observation imported under an explicit policy.
    #[derive(Debug, PartialEq, Eq, Serialize)]
    pub(crate) struct Imported {
        pub(super) record: ImportedIngress,
        pub(super) _private: Private,
    }
}

/// Late observation before or after it is imported into a later block.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Ingress<S> {
    ingress_id: Uuid,
    observation: Observation,
    prior_block_hash: BlockHash,
    state: S,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ImportedIngress {
    imported_by: ActorRef,
    import_policy: ProcedureRef,
    imported_at: RecordedAt,
    import_disposition: ImportDisposition,
    imported_into_lineage: LineageId,
    imported_into_block: BlockId,
    imported_into_height: u64,
    proposed_entry_id: EntryId,
}

/// Data for importing one ingress observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ImportIngress {
    pub(crate) imported_by: ActorRef,
    pub(crate) import_policy: ProcedureRef,
    pub(crate) imported_at: RecordedAt,
    pub(crate) import_disposition: ImportDisposition,
    pub(crate) subject: SubjectRef,
    pub(crate) executor: ActorRef,
}

impl Ingress<ingress::Open> {
    pub(crate) fn observe_late(observation: Observation, prior_block_hash: BlockHash) -> Self {
        Self {
            ingress_id: Uuid::new_v4(),
            observation,
            prior_block_hash,
            state: ingress::Open { _private: Private },
        }
    }

    pub(crate) fn import(
        self,
        block: &Block<block::Open>,
        fields: ImportIngress,
    ) -> (Entry<Proposed>, Ingress<ingress::Imported>) {
        let entry_id = EntryId::new();
        let payload = IngressImportPayload {
            ingress_id: self.ingress_id,
            prior_block_hash: self.prior_block_hash.clone(),
            original_payload_ref: self.observation.payload_ref.clone(),
            original_payload_hash: self.observation.payload_hash.clone(),
            observed_by: self.observation.observer.clone(),
            observed_at: self.observation.observed_at,
            recorded_by: self.observation.recorder.clone(),
            recorded_at: self.observation.recorded_at,
            imported_by: fields.imported_by.clone(),
            import_policy: fields.import_policy.clone(),
            imported_at: fields.imported_at,
            import_disposition: fields.import_disposition.clone(),
            imported_into_lineage: block.lineage_id().clone(),
            imported_into_block: block.block_id(),
            imported_into_height: block.block_height(),
        };
        let proposed = Entry {
            core: EntryCore {
                entry_id,
                entry_kind: EntryKind::Observation,
                subject: fields.subject,
                executor: fields.executor,
                input_refs: vec![EvidenceRef::new(format!("ingress:{}", self.ingress_id))],
                output_refs: Vec::new(),
                occurred_at: self.observation.observed_at,
                payload: EntryPayload::IngressImport(payload),
            },
            state: Draft { _private: Private },
        }
        .observe(self.observation.clone())
        .propose(Proposal {
            proposer: fields.imported_by.clone(),
            procedure_or_policy: fields.import_policy.clone(),
        });
        let imported = ImportedIngress {
            imported_by: fields.imported_by,
            import_policy: fields.import_policy,
            imported_at: fields.imported_at,
            import_disposition: fields.import_disposition,
            imported_into_lineage: block.lineage_id().clone(),
            imported_into_block: block.block_id(),
            imported_into_height: block.block_height(),
            proposed_entry_id: entry_id,
        };

        (
            proposed,
            Ingress {
                ingress_id: self.ingress_id,
                observation: self.observation,
                prior_block_hash: self.prior_block_hash,
                state: ingress::Imported {
                    record: imported,
                    _private: Private,
                },
            },
        )
    }
}

impl Ingress<ingress::Imported> {
    pub(crate) fn imported(&self) -> &ImportedIngress {
        &self.state.record
    }
}

/// History construction and verification errors.
#[derive(Debug, Error)]
pub(crate) enum HistoryError {
    #[error("failed to serialize History value deterministically")]
    StableJson(#[source] serde_json::Error),

    #[error("genesis block cannot have parent block hashes")]
    GenesisHasParents,

    #[error("non-genesis block must cite at least one parent block hash")]
    NonGenesisWithoutParents,

    #[error("entry belongs to another lineage")]
    WrongLineage,

    #[error("entry belongs to another block")]
    WrongBlock,

    #[error("entry belongs to another block height")]
    WrongBlockHeight,

    #[error("duplicate entry id in block: {0:?}")]
    DuplicateEntry(EntryId),

    #[error("sealed block header entry count {header} does not match actual count {actual}")]
    EntryCountMismatch { header: usize, actual: usize },

    #[error("sealed block entries root does not match entries")]
    EntriesRootMismatch,

    #[error("sealed block hash does not match block contents")]
    BlockHashMismatch,

    #[error("sealed block hash does not match expected anchored hash")]
    ExpectedBlockHashMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Private;

#[cfg(test)]
mod tests {
    use super::*;

    fn at(ms: i64) -> RecordedAt {
        RecordedAt(ms)
    }

    fn actor(name: &str) -> ActorRef {
        ActorRef::Process(name.to_string())
    }

    fn env() -> OperationalEnvironment {
        OperationalEnvironment::new()
            .artifact(ArtifactRef::new("artifact:base"))
            .binary(EvidenceRef::new("bin:ploke-eval"))
            .tool_surface(EvidenceRef::new("tool-surface:1"))
            .procedure_version(ProcedureRef::new("procedure:v1"))
            .model("test-model")
            .recorder(EvidenceRef::new("journal:test"))
    }

    fn open_block(height: u64, parents: Vec<BlockHash>) -> Block<block::Open> {
        open_block_with_id(BlockId::new(), height, parents)
    }

    fn open_block_with_id(
        block_id: BlockId,
        height: u64,
        parents: Vec<BlockHash>,
    ) -> Block<block::Open> {
        Block::open_with_block_id(
            block_id,
            OpenBlock {
                lineage_id: LineageId::new("lineage:a"),
                block_height: height,
                parent_block_hashes: parents,
                opened_by: actor("parent"),
                opened_from_artifact: ArtifactRef::new("artifact:base"),
                ruling_authority: actor("ruler"),
                policy_ref: ProcedureRef::new("policy:test"),
                opened_at: at(10),
            },
        )
        .expect("open block")
    }

    fn proposed_entry() -> Entry<Proposed> {
        proposed_entry_with_id(EntryId::new())
    }

    fn proposed_entry_with_id(entry_id: EntryId) -> Entry<Proposed> {
        Entry {
            core: EntryCore {
                entry_id,
                entry_kind: EntryKind::Transition,
                subject: SubjectRef::new("child:ready"),
                executor: actor("child"),
                input_refs: vec![EvidenceRef::new("input:a")],
                output_refs: vec![EvidenceRef::new("output:b")],
                occurred_at: at(20),
                payload: EntryPayload::Direct,
            },
            state: Draft { _private: Private },
        }
        .observe(Observation {
            observer: actor("parent"),
            recorder: actor("journal"),
            operational_environment: env(),
            payload_ref: EvidenceRef::new("payload:child-ready"),
            payload_hash: HistoryHash::of_bytes(b"child-ready"),
            observed_at: at(21),
            recorded_at: at(22),
        })
        .propose(Proposal {
            proposer: actor("parent"),
            procedure_or_policy: ProcedureRef::new("transition:child-ready"),
        })
    }

    fn seal(block: Block<block::Open>) -> Block<block::Sealed> {
        seal_with_transition(block, "transition:crown-lock")
    }

    fn seal_with_transition(
        block: Block<block::Open>,
        transition: &'static str,
    ) -> Block<block::Sealed> {
        block
            .seal(SealBlock {
                crown_lock_transition: EvidenceRef::new(transition),
                selected_successor: SuccessorRef::new(
                    actor("successor"),
                    ArtifactRef::new("artifact:successor"),
                ),
                active_artifact: ArtifactRef::new("artifact:successor"),
                sealed_at: at(30),
            })
            .expect("seal block")
    }

    #[test]
    fn entry_is_admitted_by_open_block_as_one_mutation() {
        let mut block = open_block(0, Vec::new());
        let entry_id = block
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit entry");

        let sealed = seal(block);

        assert_eq!(sealed.entries()[0].entry_id(), entry_id);
        sealed.verify_hash().expect("sealed hash verifies");
        sealed
            .verify_expected_hash(sealed.block_hash())
            .expect("expected hash verifies");
    }

    #[test]
    fn sealed_block_hash_is_deterministic() {
        let block_id = BlockId::new();
        let entry_id = EntryId::new();
        let mut first = open_block_with_id(block_id, 0, Vec::new());
        first
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit first");

        let mut second = open_block_with_id(block_id, 0, Vec::new());
        second
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit second");

        let first = seal(first);
        let second = seal(second);

        assert_eq!(first.block_hash(), second.block_hash());
        assert_eq!(first.header().entries_root, second.header().entries_root);
    }

    #[test]
    fn crown_lock_transition_reference_is_committed_to_block_hash() {
        let block_id = BlockId::new();
        let entry_id = EntryId::new();
        let mut first = open_block_with_id(block_id, 0, Vec::new());
        first
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit first");

        let mut second = open_block_with_id(block_id, 0, Vec::new());
        second
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit second");

        let first = seal_with_transition(first, "transition:crown-lock:a");
        let second = seal_with_transition(second, "transition:crown-lock:b");

        assert_ne!(first.block_hash(), second.block_hash());
        first.verify_hash().expect("first verifies");
        second.verify_hash().expect("second verifies");
    }

    #[test]
    fn non_genesis_block_requires_parent_hash() {
        let err = Block::open(OpenBlock {
            lineage_id: LineageId::new("lineage:a"),
            block_height: 1,
            parent_block_hashes: Vec::new(),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::new("artifact:base"),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            opened_at: at(10),
        })
        .expect_err("non-genesis without parent must fail");

        assert!(matches!(err, HistoryError::NonGenesisWithoutParents));
    }

    #[test]
    fn duplicate_entry_id_is_rejected_by_one_open_block() {
        let entry_id = EntryId::new();
        let mut block = open_block(0, Vec::new());
        block
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("first entry admitted");
        let err = block
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect_err("duplicate entry id must fail");

        assert!(matches!(err, HistoryError::DuplicateEntry(id) if id == entry_id));
    }

    #[test]
    fn successor_block_can_retain_multiple_parent_hashes() {
        let mut genesis = open_block(0, Vec::new());
        genesis
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit");
        let sealed = seal(genesis);

        let merge_parent = BlockHash::from(HistoryHash::of_bytes(b"merge-parent"));
        let successor = sealed
            .open_successor(OpenSuccessorBlock {
                additional_parent_block_hashes: vec![merge_parent.clone()],
                opened_by: actor("successor"),
                opened_from_artifact: ArtifactRef::new("artifact:successor"),
                ruling_authority: actor("successor"),
                policy_ref: ProcedureRef::new("policy:next"),
                opened_at: at(40),
            })
            .expect("open successor");

        assert_eq!(
            successor.state.common.lineage_id,
            LineageId::new("lineage:a")
        );
        assert_eq!(successor.state.common.parent_block_hashes.len(), 2);
        assert_eq!(
            &successor.state.common.parent_block_hashes[0],
            sealed.block_hash()
        );
        assert_eq!(
            &successor.state.common.parent_block_hashes[1],
            &merge_parent
        );
    }

    #[test]
    fn ingress_import_preserves_original_observation_and_policy() {
        let predecessor_hash = BlockHash::from(HistoryHash::of_bytes(b"sealed-predecessor"));
        let ingress = Ingress::observe_late(
            Observation {
                observer: actor("monitor"),
                recorder: actor("ingress-log"),
                operational_environment: env(),
                payload_ref: EvidenceRef::new("payload:late-ready"),
                payload_hash: HistoryHash::of_bytes(b"late-ready"),
                observed_at: at(50),
                recorded_at: at(51),
            },
            predecessor_hash.clone(),
        );
        let block = open_block(1, vec![predecessor_hash.clone()]);

        let (proposed, imported) = ingress.import(
            &block,
            ImportIngress {
                imported_by: actor("successor"),
                import_policy: ProcedureRef::new("policy:late-ready"),
                imported_at: at(60),
                import_disposition: ImportDisposition::AcceptedAsObservation,
                subject: SubjectRef::new("late:ready"),
                executor: actor("monitor"),
            },
        );

        assert_eq!(
            proposed.state.procedure_or_policy,
            ProcedureRef::new("policy:late-ready")
        );
        match &proposed.core.payload {
            EntryPayload::IngressImport(payload) => {
                assert_eq!(payload.prior_block_hash, predecessor_hash);
                assert_eq!(
                    payload.import_disposition,
                    ImportDisposition::AcceptedAsObservation
                );
                assert_eq!(payload.imported_into_height, 1);
            }
            EntryPayload::Direct => panic!("ingress import must be committed in entry payload"),
        }
        assert_eq!(imported.imported().imported_into_height, 1);
        assert_eq!(
            imported.imported().import_policy,
            ProcedureRef::new("policy:late-ready")
        );
    }

    #[test]
    fn ingress_import_cannot_be_admitted_into_a_different_block() {
        let predecessor_hash = BlockHash::from(HistoryHash::of_bytes(b"sealed-predecessor"));
        let ingress = Ingress::observe_late(
            Observation {
                observer: actor("monitor"),
                recorder: actor("ingress-log"),
                operational_environment: env(),
                payload_ref: EvidenceRef::new("payload:late-ready"),
                payload_hash: HistoryHash::of_bytes(b"late-ready"),
                observed_at: at(50),
                recorded_at: at(51),
            },
            predecessor_hash.clone(),
        );
        let target = open_block(1, vec![predecessor_hash.clone()]);
        let mut wrong_block = open_block(1, vec![predecessor_hash]);

        let (proposed, _imported) = ingress.import(
            &target,
            ImportIngress {
                imported_by: actor("successor"),
                import_policy: ProcedureRef::new("policy:late-ready"),
                imported_at: at(60),
                import_disposition: ImportDisposition::AcceptedAsObservation,
                subject: SubjectRef::new("late:ready"),
                executor: actor("monitor"),
            },
        );

        let err = wrong_block
            .admit(proposed, actor("admitter"))
            .expect_err("ingress import must stay bound to its target block");

        assert!(matches!(err, HistoryError::WrongBlock));
    }
}
