#[cfg(test)]
use ploke_records::ids::CampaignId;

/// Ingress chain-of-custody payload that must be sealed with the entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize)]
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

    /// Loaded-only digest recovered from the exact selection-decision payload JSON
    /// committed in the stored entry. Skipped so re-serializing entries and
    /// blocks preserves the sealed wire shape and hash preimages.
    #[serde(skip)]
    selection_hash: Option<HistoryHash>,
}

/// A provenance-bearing fact in one typed History state.
#[derive(Debug, PartialEq, Serialize)]
pub(crate) struct Entry<S> {
    core: EntryCore,
    state: S,
}

impl Entry<Draft> {
    pub(crate) fn draft(fields: DraftEntry) -> Self {
        Self::draft_with_payload(fields, EntryPayload::Direct)
    }

    pub(crate) fn draft_selection_decision(
        fields: DraftEntry,
        payload: SelectionDecisionEntry,
    ) -> Self {
        Self::draft_with_payload(fields, EntryPayload::SelectionDecision(payload))
    }

    fn draft_with_payload(fields: DraftEntry, payload: EntryPayload) -> Self {
        Self {
            core: EntryCore {
                entry_id: EntryId::new(),
                entry_kind: fields.entry_kind,
                subject: fields.subject,
                executor: fields.executor,
                input_refs: fields.input_refs,
                output_refs: fields.output_refs,
                occurred_at: fields.occurred_at,
                payload,
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

    pub(crate) fn entry_kind(&self) -> EntryKind {
        self.core.entry_kind.clone()
    }

    pub(crate) fn subject(&self) -> &SubjectRef {
        &self.core.subject
    }

    pub(crate) fn selection_decision(&self) -> Option<&SelectionDecisionEntry> {
        match &self.core.payload {
            EntryPayload::SelectionDecision(payload) => Some(payload),
            _ => None,
        }
    }

    pub(crate) fn observed_payload_ref(&self) -> &EvidenceRef {
        &self.state.observed.payload_ref
    }

    /// Hash used to verify an observed inline selection decision payload.
    ///
    /// Loaded entries prefer the seal-time digest recovered from the raw stored
    /// payload JSON. Fresh in-memory entries fall back to the current typed
    /// serializer because no stored bytes exist yet.
    pub(crate) fn decision_observation_hash(&self) -> Result<Option<HistoryHash>, HistoryError> {
        let Some(selection) = self.selection_decision() else {
            return Ok(None);
        };
        if let Some(hash) = &self.state.selection_hash {
            return Ok(Some(hash.clone()));
        }
        Ok(Some(selection.decision_hash()?))
    }

    /// When this entry carries a selection decision payload, checks that
    /// [`Self::payload_hash`] matches the seal-time decision payload hash.
    pub(crate) fn verify_selection_decision_observation(
        &self,
    ) -> Result<Option<bool>, HistoryError> {
        let Some(expected) = self.decision_observation_hash()? else {
            return Ok(None);
        };
        Ok(Some(*self.payload_hash() == expected))
    }
}

/// Data required to open a block.
///
/// Implemented now: `block_height` is validated as lineage-local height:
/// genesis opens height 0 with no parents, and predecessor authority opens
/// nonzero heights with parent hashes. Open blocks also carry the local
/// `HistoryStateRoot` they were opened from, so append can reject a block whose
/// store-state observation has gone stale. Not implemented yet: global append
/// position, Merkle/authenticated lineage-head map proofs, artifact manifest
/// digest commitments, or a uniform typed startup/admission carrier. Live
/// successor handoff now commits a backend-derived surface, but block opening
/// still receives that validated material from the current process boundary.
/// The legacy `policy_ref` field below remains a procedure/policy-material
/// label; it is not an independently authoritative `PolicyRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenBlock {
    pub(crate) lineage_id: LineageId,
    pub(crate) block_height: u64,
    pub(crate) parent_block_hashes: Vec<BlockHash>,
    pub(crate) opened_from_state: HistoryStateRoot,
    pub(crate) regime: Regime,
    pub(crate) opening_authority: OpeningAuthority,
    pub(crate) opened_by: ActorRef,
    pub(crate) opened_from_artifact: ArtifactRef,
    pub(crate) ruling_authority: ActorRef,
    pub(crate) policy_ref: ProcedureRef,
    pub(crate) surface: SurfaceCommitment,
    pub(crate) opened_at: RecordedAt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct BlockCommon {
    schema_version: u32,
    block_id: BlockId,
    lineage_id: LineageId,
    block_height: u64,
    parent_block_hashes: Vec<BlockHash>,
    opened_from_state: HistoryStateRoot,
    regime: Regime,
    opening_authority: OpeningAuthority,
    opened_by: ActorRef,
    opened_from_artifact: ArtifactRef,
    ruling_authority: ActorRef,
    policy_ref: ProcedureRef,
    surface: SurfaceCommitment,
    opened_at: RecordedAt,
}

/// Header data committed by `Crown<Locked> -> Block<block::Sealed>`.
///
/// Implementation status updated 2026-04-29: `Crown<Locked>` now carries this
/// material, so the lock transition cannot produce a naked locked Crown without
/// the facts a later seal must commit. The `crown_lock_transition` reference is
/// still header material, not an authority token. Live handoff still needs to
/// pass the carrier into block sealing and persist the sealed block before
/// successor admission can verify it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SealBlock {
    pub(crate) crown_lock_transition: EvidenceRef,
    pub(crate) selected_successor: SuccessorRef,
    pub(crate) selected_parent_identity: ParentIdentity,
    pub(crate) active_artifact: ArtifactRef,
    pub(crate) claims: block::Claims,
    pub(crate) sealed_at: RecordedAt,
}

#[cfg(test)]
fn test_parent_identity() -> ParentIdentity {
    ParentIdentity::from_record_for_test(ParentIdentityRecord {
        schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
        campaign_id: CampaignId::from("campaign:test"),
        parent_id: "node:successor".to_string(),
        node_id: "node:successor".to_string(),
        generation: 0,
        instance_id: Some("instance:successor".to_string()),
        previous_parent_id: None,
        parent_node_id: None,
        branch_id: "branch:successor".to_string(),
        artifact_branch: Some("artifact-branch:successor".to_string()),
        created_at: "2026-05-06T00:00:00Z".to_string(),
    })
}

impl SealBlock {
    /// Compatibility constructor for the live successor handoff seam.
    ///
    /// This is intentionally narrow and should disappear once the parent-side
    /// handoff transition has a real open block and admitted claims in hand.
    /// Until then, callers must still provide the successor and active artifact
    /// identities before they can lock the Crown.
    pub(crate) fn from_handoff(
        crown_lock_transition: EvidenceRef,
        selected_successor: SuccessorRef,
        selected_parent_identity: ParentIdentity,
        active_artifact: ArtifactRef,
        sealed_at: RecordedAt,
    ) -> Self {
        Self {
            crown_lock_transition,
            selected_successor,
            selected_parent_identity,
            active_artifact,
            claims: block::Claims::empty_unchecked(),
            sealed_at,
        }
    }

    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self::from_handoff(
            EvidenceRef::new("transition:crown-lock"),
            SuccessorRef::new(
                ActorRef::Process("successor".to_string()),
                ArtifactRef::from_artifact_id(ArtifactId::new("artifact:successor")),
            ),
            test_parent_identity(),
            ArtifactRef::from_artifact_id(ArtifactId::new("artifact:successor")),
            RecordedAt(30),
        )
    }
}

/// Block typestate payloads.
pub(crate) mod block {
    use serde::{Deserialize, Serialize};

    use super::BlockCommon;
    use super::{
        Admission, Artifact, ArtifactPath, Digest, FlatClaim, Locator, Manifest, Policy, Private,
        RulerWitness, SealedBlockHeader, Verifiable, Witnessed, claim, surface,
    };

    /// Flattened v2 block claims.
    ///
    /// Status recorded 2026-04-29 13:37 PDT; tightened 2026-04-29 21:18 PDT:
    /// this is a storage boundary, not a report object and not an authority
    /// factory. The block stores flat fields for serialization and hashing;
    /// setter/accessor methods consume and reconstruct the nested semantic
    /// shape:
    ///
    /// ```text
    /// claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>
    /// ```
    ///
    /// `Claims` is deliberately in the `block` module and has no public
    /// constructor or `Default` implementation. Outside this module, a caller
    /// should not be able to mint block claim storage from a bare path, digest,
    /// or status string. A field being `None` means the current live code has
    /// not yet supplied that claim. It is not an implicit admission, and it
    /// must not be interpreted as successful verification.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct Claims {
        policy: Option<FlatClaim<ArtifactPath, Digest<Policy>>>,
        surface: Option<FlatClaim<ArtifactPath, Digest<surface::Bounded>>>,
        manifest: Option<FlatClaim<ArtifactPath, Digest<Manifest>>>,
        artifact: Option<FlatClaim<super::TreeKeyHash, Digest<Artifact>>>,
    }

    impl Claims {
        pub(super) fn empty_unchecked() -> Self {
            Self {
                policy: None,
                surface: None,
                manifest: None,
                artifact: None,
            }
        }

        pub(super) fn with_policy<L>(
            mut self,
            claim: claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Policy, L>>>,
        ) -> Self
        where
            L: Locator<Policy, Key = ArtifactPath, Digest = Digest<Policy>>,
        {
            self.policy = Some(FlatClaim::from_admitted(claim));
            self
        }

        pub(crate) fn policy<L>(
            &self,
        ) -> Option<claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Policy, L>>>>
        where
            L: Locator<Policy, Key = ArtifactPath, Digest = Digest<Policy>>,
        {
            self.policy.as_ref().map(FlatClaim::to_admitted)
        }

        pub(super) fn with_surface<L>(
            mut self,
            claim: claim::Admitted<
                Admission,
                Witnessed<RulerWitness, Verifiable<surface::Bounded, L>>,
            >,
        ) -> Self
        where
            L: Locator<surface::Bounded, Key = ArtifactPath, Digest = Digest<surface::Bounded>>,
        {
            self.surface = Some(FlatClaim::from_admitted(claim));
            self
        }

        pub(crate) fn surface<L>(
            &self,
        ) -> Option<
            claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<surface::Bounded, L>>>,
        >
        where
            L: Locator<surface::Bounded, Key = ArtifactPath, Digest = Digest<surface::Bounded>>,
        {
            self.surface.as_ref().map(FlatClaim::to_admitted)
        }

        pub(super) fn with_manifest<L>(
            mut self,
            claim: claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Manifest, L>>>,
        ) -> Self
        where
            L: Locator<Manifest, Key = ArtifactPath, Digest = Digest<Manifest>>,
        {
            self.manifest = Some(FlatClaim::from_admitted(claim));
            self
        }

        pub(crate) fn manifest<L>(
            &self,
        ) -> Option<claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Manifest, L>>>>
        where
            L: Locator<Manifest, Key = ArtifactPath, Digest = Digest<Manifest>>,
        {
            self.manifest.as_ref().map(FlatClaim::to_admitted)
        }

        pub(crate) fn with_artifact<L>(
            mut self,
            claim: claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Artifact, L>>>,
        ) -> Self
        where
            L: Locator<Artifact, Key = super::TreeKeyHash, Digest = Digest<Artifact>>,
        {
            self.artifact = Some(FlatClaim::from_admitted(claim));
            self
        }

        pub(crate) fn artifact<L>(
            &self,
        ) -> Option<claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Artifact, L>>>>
        where
            L: Locator<Artifact, Key = super::TreeKeyHash, Digest = Digest<Artifact>>,
        {
            self.artifact.as_ref().map(FlatClaim::to_admitted)
        }
    }

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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedBlockHeader {
    common: BlockCommon,
    crown_lock_transition: EvidenceRef,
    selected_successor: SuccessorRef,
    selected_parent_identity: ParentIdentity,
    active_artifact: ArtifactRef,
    claims: block::Claims,
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
    selected_parent_identity: ParentIdentity,
    active_artifact: ArtifactRef,
    claims: block::Claims,
    sealed_at: RecordedAt,
    entry_count: usize,
    entries_root: HistoryHash,
}

/// One authority epoch in a lineage-local History chain.
///
/// Draft block-content framing recorded 2026-04-29 08:03 PDT. Current code
/// implements `Entry<Admitted>` plus sealed header material only; the grouping
/// below is the design target for the next type slice, not a completed
/// implementation claim.
///
/// A block should witness the meeting point of several independently useful
/// invariants:
///
/// - chain position: block id/hash, parent block hashes, lineage coordinate,
///   and lineage-local height as an index/projection rather than identity;
/// - local authority: opening authority, ruling authority, Crown lock
///   transition, and the store scope under which this block is authoritative;
/// - procedural environment: `ProcedureRef` currently names the procedure set
///   and runtime contract available to a Runtime built from an Artifact, not
///   merely one narrow function call;
/// - surface commitment: one immutable authority surface root plus mutated and
///   ambient surface deltas, computed statically from Artifacts without running
///   the candidate Runtime;
/// - artifact commitments: active Artifact and selected successor Artifact
///   should be recoverable from the tree and validated through backend tree key
///   commitment plus artifact-local manifest digest/reference;
/// - block claims: flat serialized fields should enter and leave the block
///   through the nested claim boundary
///   `claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>`;
///   a bare digest or path is never itself admitted block evidence;
/// - successor eligibility: selected successor runtime/artifact evidence
///   should be sufficient for startup to validate the immediate sealed head
///   without replaying the entire History hot path;
/// - stochastic evidence: evaluation samples, oracle/eval refs, uncertainty
///   summaries, risk-budget effects, validator/reporter refs, and rejected or
///   failed candidate evidence should be committed by digest/root/reference when
///   policy uses them for admission;
/// - head state: rollback, fork/conflict, admission, and finality status are
///   first-class block/History concerns even when the initial filesystem store
///   can only model them as local projections;
/// - admitted facts: entries remain the implemented admission unit, sealed by
///   entry count and entries root.
///
/// Future consensus or multi-ruler work should add explicit policy/store
/// semantics instead of treating a local Crown block as globally final. The
/// current intended claim is local: under the configured single-ruler policy,
/// this block is a valid authority epoch for one History store and lineage.
/// Human or root authority is intentionally left as future policy work rather
/// than a current block invariant.
#[derive(Debug, Serialize)]
pub(crate) struct Block<S> {
    entries: Vec<Entry<Admitted>>,
    state: S,
    #[serde(skip)]
    stored_entry_hashes: Option<Vec<HistoryHash>>,
}

impl<S: PartialEq> PartialEq for Block<S> {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries && self.state == other.state
    }
}

impl Block<block::Open> {
    fn open(fields: OpenBlock) -> Result<Self, HistoryError> {
        Self::open_with_block_id(BlockId::new(), fields)
    }

    fn open_with_block_id(block_id: BlockId, fields: OpenBlock) -> Result<Self, HistoryError> {
        if fields.block_height == 0 && !fields.parent_block_hashes.is_empty() {
            return Err(HistoryError::GenesisHasParents);
        }
        if fields.block_height > 0 && fields.parent_block_hashes.is_empty() {
            return Err(HistoryError::NonGenesisWithoutParents);
        }
        match (&fields.opening_authority, fields.block_height) {
            (OpeningAuthority::Genesis(_), 0) => {}
            (OpeningAuthority::Genesis(_), _) => return Err(HistoryError::GenesisAuthorityOnChild),
            (OpeningAuthority::Predecessor(_), 0) => {
                return Err(HistoryError::GenesisWithoutBootstrap);
            }
            (OpeningAuthority::Predecessor(predecessor), _) => {
                if !fields
                    .parent_block_hashes
                    .contains(&predecessor.predecessor_block_hash)
                {
                    return Err(HistoryError::OpeningPredecessorNotParent);
                }
            }
        }

        Ok(Self {
            entries: Vec::new(),
            stored_entry_hashes: None,
            state: block::Open {
                common: BlockCommon {
                    schema_version: SCHEMA_VERSION,
                    block_id,
                    lineage_id: fields.lineage_id,
                    block_height: fields.block_height,
                    parent_block_hashes: fields.parent_block_hashes,
                    opened_from_state: fields.opened_from_state,
                    regime: fields.regime,
                    opening_authority: fields.opening_authority,
                    opened_by: fields.opened_by,
                    opened_from_artifact: fields.opened_from_artifact,
                    ruling_authority: fields.ruling_authority,
                    policy_ref: fields.policy_ref,
                    surface: fields.surface,
                    opened_at: fields.opened_at,
                },
                _private: Private,
            },
        })
    }

    fn admit(
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
                selection_hash: None,
            },
        });
        Ok(entry_id)
    }

    fn seal(self, fields: SealBlock) -> Result<Block<block::Sealed>, HistoryError> {
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
            selected_parent_identity: fields.selected_parent_identity,
            active_artifact: fields.active_artifact,
            claims: fields.claims,
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
            selected_parent_identity: preimage.selected_parent_identity,
            active_artifact: preimage.active_artifact,
            claims: preimage.claims,
            sealed_at: preimage.sealed_at,
            entry_count: preimage.entry_count,
            entries_root: preimage.entries_root,
            block_hash,
        };

        Ok(Block {
            entries: self.entries,
            stored_entry_hashes: None,
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

    pub(crate) fn regime(&self) -> &Regime {
        &self.state.common.regime
    }
}

impl Block<block::Sealed> {
    pub(crate) fn header(&self) -> &SealedBlockHeader {
        &self.state.header
    }

    pub(crate) fn block_hash(&self) -> &BlockHash {
        &self.header().block_hash
    }

    pub(crate) fn block_height(&self) -> u64 {
        self.header().common.block_height
    }

    pub(crate) fn lineage_id(&self) -> &LineageId {
        &self.header().common.lineage_id
    }

    pub(crate) fn regime(&self) -> &Regime {
        &self.header().common.regime
    }

    pub(crate) fn entries(&self) -> &[Entry<Admitted>] {
        &self.entries
    }

    fn entry_hashes_for_verification(&self) -> Result<Vec<HistoryHash>, HistoryError> {
        if let Some(hashes) = &self.stored_entry_hashes {
            if hashes.len() != self.entries.len() {
                return Err(HistoryError::EntryCountMismatch {
                    header: hashes.len(),
                    actual: self.entries.len(),
                });
            }
            return Ok(hashes.clone());
        }

        self.entries.iter().map(Entry::entry_hash).collect()
    }

    pub(crate) fn selected_successor(&self) -> &SuccessorRef {
        &self.header().selected_successor
    }

    pub(crate) fn selected_parent_identity(&self) -> &ParentIdentity {
        &self.header().selected_parent_identity
    }

    pub(crate) fn active_artifact(&self) -> &ArtifactRef {
        &self.header().active_artifact
    }

    pub(crate) fn verify_hash(&self) -> Result<(), HistoryError> {
        let header = self.header();
        if header.entry_count != self.entries.len() {
            return Err(HistoryError::EntryCountMismatch {
                header: header.entry_count,
                actual: self.entries.len(),
            });
        }

        let entry_hashes = self.entry_hashes_for_verification()?;
        let entries_root =
            HistoryHash::of_domain_json("prototype1.history.entries_root.v1", &entry_hashes)?;
        if entries_root != header.entries_root {
            return Err(HistoryError::EntriesRootMismatch);
        }

        let preimage = SealedBlockPreimage {
            common: header.common.clone(),
            crown_lock_transition: header.crown_lock_transition.clone(),
            selected_successor: header.selected_successor.clone(),
            selected_parent_identity: header.selected_parent_identity.clone(),
            active_artifact: header.active_artifact.clone(),
            claims: header.claims.clone(),
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

    /// Verify that the sealed head admits the current checkout's Artifact tree.
    ///
    /// This is the successor-side half of the cross-runtime handoff contract: the
    /// predecessor sealed a tree-key-backed Artifact claim, and the incoming
    /// runtime must derive its own backend-owned clean tree key and match it
    /// before entering the ruling Parent path.
    pub(crate) fn verify_current_artifact_tree<L>(
        &self,
        current: &TreeKeyHash,
        locator: &L,
    ) -> Result<(), HistoryError>
    where
        L: Locator<Artifact, Key = TreeKeyHash, Digest = Digest<Artifact>, Error = HistoryError>,
        L::Digest: PartialEq,
    {
        let artifact = self
            .header()
            .claims
            .artifact::<L>()
            .ok_or(HistoryError::MissingArtifactClaim)?;
        let verifiable = artifact.claim().claim();
        if verifiable.key() != current {
            return Err(HistoryError::ArtifactTreeKeyMismatch {
                expected: verifiable.key().clone(),
                actual: current.clone(),
            });
        }
        verifiable
            .verify_with(locator)
            .map_err(VerifyError::into_history_error)?;
        Ok(())
    }

    /// Verify that the current checkout matches the surface admitted by this
    /// sealed head.
    ///
    /// Startup validation intentionally recomputes the current surface from the
    /// checked-out Artifact instead of trusting invocation JSON. The sealed
    /// block supplies the expected roots; the backend supplies the observed
    /// roots at this runtime boundary.
    pub(crate) fn verify_current_surface(
        &self,
        current: &SurfaceCommitment,
    ) -> Result<(), HistoryError> {
        self.header().common.surface.verify_current(current)
    }

    fn open_successor(
        &self,
        fields: OpenSuccessorBlock,
    ) -> Result<Block<block::Open>, HistoryError> {
        let mut parent_block_hashes = vec![*self.block_hash()];
        parent_block_hashes.extend(fields.additional_parent_block_hashes);
        let block_height = self.header().common.block_height + 1;
        Block::<block::Open>::open(OpenBlock {
            lineage_id: self.header().common.lineage_id.clone(),
            block_height,
            opened_from_state: fields.opened_from_state,
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                *self.block_hash(),
            )),
            parent_block_hashes,
            regime: Regime::prototype1_baseline(block_height),
            opened_by: fields.opened_by,
            opened_from_artifact: fields.opened_from_artifact,
            ruling_authority: fields.ruling_authority,
            policy_ref: fields.policy_ref,
            surface: fields.surface,
            opened_at: fields.opened_at,
        })
    }
}

impl super::inner::Crown<super::inner::crown::Ruling> {
    /// Open a History block under the current ruling Crown.
    ///
    /// This is the crate-visible construction boundary for `Block<Open>`.
    /// `Block::open` remains private so sibling modules cannot create an open
    /// authority epoch from a struct literal alone. Current limitation
    /// recorded 2026-04-29: the Crown proves lineage authority but does not
    /// yet carry the ruling actor identity; `OpenBlock` still supplies
    /// `opened_by` and `ruling_authority` as data until `Parent<Ruling>` is
    /// wired into this boundary.
    pub(crate) fn open_block(&self, fields: OpenBlock) -> Result<Block<block::Open>, HistoryError> {
        if !self
            .lineage_key()
            .matches_debug_str(fields.lineage_id.as_str())
        {
            return Err(HistoryError::WrongCrownLineage);
        }

        Block::<block::Open>::open(fields)
    }

    /// Open the next block from a verified sealed predecessor under this Crown.
    ///
    /// The sealed predecessor supplies predecessor authority and lineage-local
    /// height. The ruling Crown supplies the permission to create the next open
    /// epoch for that same lineage.
    pub(crate) fn open_successor(
        &self,
        predecessor: &Block<block::Sealed>,
        fields: OpenSuccessorBlock,
    ) -> Result<Block<block::Open>, HistoryError> {
        if !self
            .lineage_key()
            .matches_debug_str(predecessor.header().common.lineage_id.as_str())
        {
            return Err(HistoryError::WrongCrownLineage);
        }

        predecessor.open_successor(fields)
    }

    /// Admit a proposed entry into an open block under the current Crown.
    ///
    /// Entry admission is a mutation of the block's authority epoch, so it is
    /// routed through the ruling Crown instead of exposed as a free-standing
    /// `Block<Open>` method. The fallible checks inside `Block::admit` still
    /// protect ingress imports from being moved across lineage/block/height.
    pub(crate) fn admit_entry(
        &self,
        block: &mut Block<block::Open>,
        entry: Entry<Proposed>,
        admitting_authority: ActorRef,
    ) -> Result<EntryId, HistoryError> {
        if !self
            .lineage_key()
            .matches_debug_str(block.lineage_id().as_str())
        {
            return Err(HistoryError::WrongCrownLineage);
        }

        block.admit(entry, admitting_authority)
    }

    /// Admit a fallibly located claim under the current ruling Crown.
    ///
    /// This is the construction boundary for the nested claim shape. The
    /// locator call is the quarantined fallible border with the artifact/tree
    /// backend: missing files, digest failures, and backend inconsistencies are
    /// returned here instead of being represented as valid block-internal
    /// facts.
    ///
    /// Current implementation gap recorded 2026-04-29 21:18 PDT: the method
    /// proves possession of `Crown<Ruling>`, but it still accepts the ruler
    /// actor identity as data. A future `Parent<Ruling>` carrier should supply
    /// that identity structurally rather than trusting the caller to pass the
    /// matching `ActorRef`.
    pub(crate) fn admit_claim<T, L>(
        &self,
        locator: &L,
        key: L::Key,
        ruler: ActorRef,
        environment: OperationalEnvironment,
        policy: ProcedureRef,
        at: RecordedAt,
    ) -> Result<
        (
            T,
            claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>,
        ),
        VerifyError<L::Error>,
    >
    where
        L: Locator<T>,
    {
        let (item, verifiable) = Verifiable::from_locator(locator, key)?;
        let witness = RulerWitness::new(ruler.clone(), environment, at.clone());
        let admission = Admission::new(ruler, policy, at);
        Ok((
            item,
            claim::Admitted::new(admission, Witnessed::new(witness, verifiable)),
        ))
    }
}

impl super::inner::Crown<super::inner::crown::Locked> {
    /// Seal a block using a locked Crown carrier for the same lineage.
    ///
    /// This is the public crate boundary for `Block<Open> -> Block<Sealed>`.
    /// The block remains a History object, while the authority to seal it is
    /// carried structurally by `Crown<Locked>`.
    pub(crate) fn seal(
        self,
        block: Block<block::Open>,
    ) -> Result<Block<block::Sealed>, HistoryError> {
        if !self
            .lineage_key()
            .matches_debug_str(block.lineage_id().as_str())
        {
            return Err(HistoryError::WrongCrownLineage);
        }

        block.seal(self.into_seal_fields())
    }
}

/// Data for opening a successor block after verifying a sealed predecessor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenSuccessorBlock {
    pub(crate) additional_parent_block_hashes: Vec<BlockHash>,
    pub(crate) opened_from_state: HistoryStateRoot,
    pub(crate) opened_by: ActorRef,
    pub(crate) opened_from_artifact: ArtifactRef,
    pub(crate) ruling_authority: ActorRef,
    pub(crate) policy_ref: ProcedureRef,
    pub(crate) surface: SurfaceCommitment,
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
            prior_block_hash: self.prior_block_hash,
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

    #[error("genesis block must be opened by bootstrap authority")]
    GenesisWithoutBootstrap,

    #[error("bootstrap authority can only open the genesis block")]
    GenesisAuthorityOnChild,

    #[error("predecessor opening authority must cite one of the parent block hashes")]
    OpeningPredecessorNotParent,

    #[error("entry belongs to another lineage")]
    WrongLineage,

    #[error("entry belongs to another block")]
    WrongBlock,

    #[error("entry belongs to another block height")]
    WrongBlockHeight,

    #[error("locked Crown belongs to another lineage")]
    WrongCrownLineage,

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

    #[error("sealed block claim digest does not match the expected artifact/tree value")]
    ClaimDigestMismatch,

    #[error("invalid selection decision entry: {detail}")]
    InvalidSelectionDecision { detail: String },

    #[error("sealed block is missing the required admitted Artifact claim")]
    MissingArtifactClaim,

    #[error("current checkout tree key does not match sealed Artifact claim")]
    ArtifactTreeKeyMismatch {
        expected: TreeKeyHash,
        actual: TreeKeyHash,
    },
    #[error("current {partition} surface root does not match sealed expectation")]
    SurfaceMismatch {
        partition: &'static str,
        expected: HistoryHash,
        actual: HistoryHash,
    },
}

/// Sealed History block storage errors.
#[derive(Debug, Error)]
pub(crate) enum BlockStoreError {
    #[error("failed to create History store directory '{}'", path.display())]
    CreateDir { path: PathBuf, source: io::Error },

    #[error("failed to open History store file '{}'", path.display())]
    Open { path: PathBuf, source: io::Error },

    #[error("failed to read History store file '{}'", path.display())]
    Read { path: PathBuf, source: io::Error },

    #[error("failed to write History store file '{}'", path.display())]
    Write { path: PathBuf, source: io::Error },

    #[error("failed to sync History store file '{}'", path.display())]
    Sync { path: PathBuf, source: io::Error },

    #[error("failed to serialize History store value")]
    Serialize(#[source] serde_json::Error),

    #[error("failed to deserialize History store value")]
    Deserialize(#[source] serde_json::Error),

    #[error(
        "History head projection for lineage '{lineage_id:?}' points at missing block {block_hash:?}"
    )]
    MissingHeadIndex {
        lineage_id: LineageId,
        block_hash: BlockHash,
    },

    #[error("History heads projection is missing while stored blocks exist: '{}'", path.display())]
    MissingHeadsProjection { path: PathBuf },

    #[error("History lineage index exists without a head projection for lineage '{lineage_id:?}'")]
    MissingLineageHeadProjection { lineage_id: LineageId },

    #[error("History append used stale History state: expected {expected:?}, actual {actual:?}")]
    StaleStoreHead {
        expected: LineageState,
        actual: LineageState,
    },

    #[error(
        "History append opened from state root {actual:?}, but store was checked at {expected:?}"
    )]
    WrongOpeningStateRoot {
        expected: HistoryStateRoot,
        actual: HistoryStateRoot,
    },

    #[error(
        "History append store-head lineage mismatch: expected '{expected:?}', actual '{actual:?}'"
    )]
    WrongStoreHeadLineage {
        expected: LineageId,
        actual: LineageId,
    },

    #[error(
        "History append for lineage '{lineage_id:?}' tried to write non-genesis block height {block_height} without a verified head"
    )]
    NonGenesisWithoutHead {
        lineage_id: LineageId,
        block_height: u64,
    },

    #[error("History append for lineage '{lineage_id:?}' tried to write another genesis block")]
    DuplicateGenesis { lineage_id: LineageId },

    #[error(
        "History append for lineage '{lineage_id:?}' tried to write genesis with store parents"
    )]
    GenesisWithStoreParents { lineage_id: LineageId },

    #[error(
        "History append for lineage '{lineage_id:?}' expected block height {expected}, got {actual}"
    )]
    NonConsecutiveHeight {
        lineage_id: LineageId,
        expected: u64,
        actual: u64,
    },

    #[error("History append for lineage '{lineage_id:?}' does not cite current head {expected:?}")]
    WrongStoreHeadParent {
        lineage_id: LineageId,
        expected: BlockHash,
    },

    #[error("History block segment '{}' has no line {line_index}", path.display())]
    MissingStoredBlockLine { path: PathBuf, line_index: u64 },

    #[error(
        "stored History block at '{}':{} header declares {entry_count} entries, but its entries array differs",
        path.display(),
        line_index
    )]
    UnsupportedStoredEntries {
        path: PathBuf,
        line_index: u64,
        entry_count: usize,
    },

    #[error("sealed block failed verification before storage: {0}")]
    Verify(#[from] HistoryError),

    #[error("History state map operation failed")]
    StateMap(#[source] sparse_merkle_tree::error::Error),

    #[error("History state root is not a 32-byte digest: {0}")]
    StateRootDigest(String),

    #[error("History state proof does not match the observed lineage head")]
    StateProofMismatch,

    #[error("History state map encoded an occupied lineage as the sparse-tree empty value")]
    StateValueZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
struct Private;

#[cfg(test)]
mod tests {
    use super::super::inner::{Crown, crown};
    use super::*;
    use crate::successor_selection::traversal::StrategyKind;

    fn at(ms: i64) -> RecordedAt {
        RecordedAt(ms)
    }

    fn actor(name: &str) -> ActorRef {
        ActorRef::Process(name.to_string())
    }

    fn env() -> OperationalEnvironment {
        OperationalEnvironment::new()
            .artifact(ArtifactRef::from_artifact_id(ArtifactId::new(
                "artifact:base",
            )))
            .binary(EvidenceRef::new("bin:ploke-eval"))
            .tool_surface(EvidenceRef::new("tool-surface:1"))
            .procedure_version(ProcedureRef::new("procedure:v1"))
            .model("test-model")
            .recorder(EvidenceRef::new("journal:test"))
    }

    #[derive(Serialize)]
    struct TestTreeKey<'a> {
        value: &'a str,
    }

    impl TreeKeyCommitment for TestTreeKey<'_> {
        fn tree_key_hash(&self) -> Result<TreeKeyHash, HistoryError> {
            TreeKeyHash::from_serialized_key(self)
        }
    }

    #[derive(Debug, Error)]
    #[error("test locator error")]
    struct TestLocatorError;

    #[derive(Debug)]
    struct TestLocator;

    impl Locator<Policy> for TestLocator {
        type Key = ArtifactPath;
        type Digest = Digest<Policy>;
        type Error = TestLocatorError;

        fn locate(&self, _key: &Self::Key) -> Result<Policy, Self::Error> {
            Ok(Policy)
        }

        fn digest(&self, key: &Self::Key) -> Result<Self::Digest, Self::Error> {
            Ok(Digest::new(HistoryHash::of_domain_json(
                "test.policy.digest.v1",
                key,
            )?))
        }
    }

    impl From<HistoryError> for TestLocatorError {
        fn from(_value: HistoryError) -> Self {
            Self
        }
    }

    fn tree_key(value: &'static str) -> TreeKeyHash {
        TestTreeKey { value }
            .tree_key_hash()
            .expect("tree key hash")
    }

    fn ruling_crown() -> Crown<crown::Ruling> {
        Crown::test_ruling("lineage:a")
    }

    fn open_block(height: u64, parents: Vec<BlockHash>) -> Block<block::Open> {
        open_block_with_id(BlockId::new(), height, parents)
    }

    fn open_block_fields(lineage: &str, height: u64, parents: Vec<BlockHash>) -> OpenBlock {
        let opening_authority = if height == 0 {
            OpeningAuthority::Genesis(GenesisAuthority::new(
                ProcedureRef::new("policy:bootstrap"),
                tree_key("tree:genesis"),
                ParentIdentityRef::new(EvidenceRef::new("parent-identity:genesis")),
            ))
        } else {
            OpeningAuthority::Predecessor(PredecessorAuthority::new(
                parents.first().expect("non-genesis parent hash").clone(),
            ))
        };
        OpenBlock {
            lineage_id: LineageId::new(lineage),
            block_height: height,
            parent_block_hashes: parents,
            opened_from_state: HistoryStateRoot::test("state:test"),
            regime: Regime::prototype1_baseline(height),
            opening_authority,
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:base")),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            surface: surface_commitment("open-block"),
            opened_at: at(10),
        }
    }

    fn open_block_from_state(
        state: &LineageState,
        height: u64,
        parents: Vec<BlockHash>,
    ) -> Block<block::Open> {
        let mut fields = open_block_fields(state.lineage_id().as_str(), height, parents);
        fields.opened_from_state = state.root().clone();
        Block::open(fields).expect("open block from state")
    }

    fn surface_root(label: &'static str) -> SurfaceRoot {
        SurfaceRoot {
            hash: HistoryHash::of_bytes(label.as_bytes()),
        }
    }

    fn surface<P>(label: &'static str) -> Surface<P> {
        Surface {
            root: surface_root(label),
            _partition: PhantomData,
        }
    }

    fn surface_commitment(label: &'static str) -> SurfaceCommitment {
        SurfaceCommitment {
            immutable: surface::<surface::Immutable>("immutable:prototype1"),
            mutated: SurfaceDelta {
                before: surface::<surface::Mutated>(label),
                after: surface::<surface::Mutated>("mutated:after"),
            },
            ambient: SurfaceDelta {
                before: surface::<surface::Ambient>("ambient:before"),
                after: surface::<surface::Ambient>("ambient:after"),
            },
        }
    }

    fn open_block_with_id(
        block_id: BlockId,
        height: u64,
        parents: Vec<BlockHash>,
    ) -> Block<block::Open> {
        Block::open_with_block_id(block_id, open_block_fields("lineage:a", height, parents))
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

    fn proposed_selection_entry(selection: SelectionDecisionEntry) -> Entry<Proposed> {
        let payload_hash = selection.decision_hash().expect("selection decision hash");
        Entry::draft_selection_decision(
            DraftEntry {
                entry_kind: EntryKind::Decision,
                subject: selection
                    .selected_candidate
                    .clone()
                    .unwrap_or_else(|| SubjectRef::new("candidate:none")),
                executor: actor("selector"),
                input_refs: Vec::new(),
                output_refs: Vec::new(),
                occurred_at: at(20),
            },
            selection,
        )
        .observe(Observation {
            observer: actor("parent"),
            recorder: actor("history"),
            operational_environment: env(),
            payload_ref: EvidenceRef::new("payload:selection-decision"),
            payload_hash,
            observed_at: at(21),
            recorded_at: at(22),
        })
        .propose(Proposal {
            proposer: actor("parent"),
            procedure_or_policy: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        })
    }

    fn selection_decision(
        node_id: &str,
        branch_id: &str,
    ) -> crate::successor_selection::SuccessorDecision {
        crate::successor_selection::SuccessorDecision {
            procedure_id: crate::successor_selection::PROCEDURE_ID.to_string(),
            candidate_node_id: node_id.to_string(),
            selected_branch_id: Some(branch_id.to_string()),
            branch_disposition: "keep".to_string(),
            outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        }
    }

    fn evaluation_payload(node_id: &str, branch_id: &str, plan_index: u32) -> EvaluationPayload {
        let input = crate::successor_selection::SelectionInput::new(
            crate::successor_selection::CandidateRef {
                node_id: node_id.to_string(),
                branch_id: branch_id.to_string(),
                generation: 2,
            },
            crate::BranchDisposition::Keep,
            PathBuf::from(format!("evaluations/{branch_id}.json")),
            vec![crate::successor_selection::RunComparison {
                instance_id: "instance-a".to_string(),
                parent_metrics: Some(test_metrics(false, false, 0)),
                child_metrics: Some(test_metrics(true, true, 0)),
                oracle_evaluation: None,
                status: "compared".to_string(),
            }],
        );
        EvaluationPayload::builder(
            SubjectRef::new(format!("candidate:{node_id}:plan_index={plan_index}")),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input")
        .sealed_candidate_evidence(SealedCandidateEvidence {
            schema_version: 2,
            coordinate: CandidateCoordinate {
                node_id: node_id.to_string(),
                parent_node_id: None,
                branch_id: Some(branch_id.to_string()),
                generation: Some(2),
                plan_index: Some(plan_index),
                primary_runtime_id: Some(format!("runtime:{node_id}")),
            },
            lifecycle: CandidateLifecycle {
                planner_outcome: "done".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: vec![test_sealed_evaluation(branch_id)],
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        })
        .candidate_artifact(test_candidate_artifact(node_id, branch_id, 2))
        .build()
    }

    fn occurrence_coordinate(
        node_id: &str,
        branch_id: &str,
        plan_index: u32,
    ) -> CandidateCoordinate {
        CandidateCoordinate {
            node_id: node_id.to_string(),
            parent_node_id: Some("parent-a".to_string()),
            branch_id: Some(branch_id.to_string()),
            generation: Some(2),
            plan_index: Some(plan_index),
            primary_runtime_id: Some("runtime:primary".to_string()),
        }
    }

    fn runtime_actor(value: u128) -> ActorRef {
        ActorRef::Runtime(RuntimeId(uuid::Uuid::from_u128(value)))
    }

    fn legacy_domain_json_hash<T: Serialize>(
        domain: &'static str,
        value: &T,
    ) -> Result<HistoryHash, HistoryError> {
        let bytes = serde_json::to_vec(&HashPreimage { domain, value })
            .map_err(HistoryError::StableJson)?;
        Ok(HistoryHash::of_bytes(&bytes))
    }

    #[test]
    fn candidate_occurrence_id_is_deterministic_for_same_preimage() {
        let lineage = LineageId::new("lineage:a");
        let coordinate = occurrence_coordinate("node-a", "branch-a", 0);
        let artifact = ArtifactRef::from_artifact_id(ArtifactId::new("artifact:a"));
        let runtime = runtime_actor(1);
        let first = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            lineage_id: Some(&lineage),
            artifact: Some(&artifact),
            runtime: Some(&runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::CurrentGeneration, &coordinate)
        })
        .expect("occurrence id");
        let second = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            lineage_id: Some(&lineage),
            artifact: Some(&artifact),
            runtime: Some(&runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::CurrentGeneration, &coordinate)
        })
        .expect("occurrence id");

        assert_eq!(first, second);
        assert_eq!(first.hash().as_str().len(), 64);
    }

    #[test]
    fn candidate_occurrence_id_changes_with_runtime_or_coordinate() {
        let lineage = LineageId::new("lineage:a");
        let coordinate = occurrence_coordinate("node-a", "branch-a", 0);
        let changed_coordinate = occurrence_coordinate("node-a", "branch-a", 1);
        let runtime = runtime_actor(1);
        let changed_runtime = runtime_actor(2);
        let base = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            lineage_id: Some(&lineage),
            runtime: Some(&runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::History, &coordinate)
        })
        .expect("occurrence id");
        let by_runtime = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            lineage_id: Some(&lineage),
            runtime: Some(&changed_runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::History, &coordinate)
        })
        .expect("occurrence id");
        let by_coordinate = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            lineage_id: Some(&lineage),
            runtime: Some(&runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::History, &changed_coordinate)
        })
        .expect("occurrence id");

        assert_ne!(base, by_runtime);
        assert_ne!(base, by_coordinate);
    }

    #[test]
    fn candidate_membership_id_changes_with_candidate_set_root() {
        let coordinate = occurrence_coordinate("node-a", "branch-a", 0);
        let occurrence_id = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage::new(
            CandidateSourceClass::CurrentGeneration,
            &coordinate,
        ))
        .expect("occurrence id");
        let first_root = CandidateSetRoot(HistoryHash::of_bytes(b"candidate-set:first"));
        let second_root = CandidateSetRoot(HistoryHash::of_bytes(b"candidate-set:second"));
        let first = CandidateMembershipId::new(&occurrence_id, &first_root)
            .expect("candidate membership id");
        let second = CandidateMembershipId::new(&occurrence_id, &second_root)
            .expect("candidate membership id");

        assert_ne!(first, second);
        assert_eq!(first.hash().as_str().len(), 64);
    }

    #[test]
    fn same_subject_ref_can_have_different_runtime_occurrences() {
        let subject = SubjectRef::new("candidate:shared:plan_index=0");
        let coordinate = occurrence_coordinate("node-a", "branch-a", 0);
        let first_runtime = runtime_actor(1);
        let second_runtime = runtime_actor(2);
        let first = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            runtime: Some(&first_runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::CurrentGeneration, &coordinate)
        })
        .expect("occurrence id");
        let second = CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage {
            runtime: Some(&second_runtime),
            ..CandidateOccurrencePreimage::new(CandidateSourceClass::CurrentGeneration, &coordinate)
        })
        .expect("occurrence id");

        assert_eq!(subject, SubjectRef::new("candidate:shared:plan_index=0"));
        assert_ne!(first, second);
    }

    fn test_sealed_evaluation(branch_id: &str) -> SealedEvaluationEvidence {
        SealedEvaluationEvidence {
            branch_id: branch_id.to_string(),
            evaluation_procedure_id: Some(
                super::super::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID.to_string(),
            ),
            evaluator_identity: Some(test_evaluator_identity()),
            eval_set_identity: Some(test_eval_set_identity()),
            evaluation_artifact_citation: None,
            overall_disposition: Some("keep".to_string()),
            primary_report_citation: SealedEvidenceCitation {
                ref_id: format!("report:{branch_id}"),
                content_hash: None,
                record_name: None,
            },
            compared_runs: Vec::new(),
        }
    }

    fn test_evaluator_identity() -> SealedEvaluatorIdentity {
        SealedEvaluatorIdentity {
            id: "test".to_string(),
            version: "1".to_string(),
        }
    }

    fn test_eval_set_identity() -> SealedEvalSetIdentity {
        SealedEvalSetIdentity {
            id: "eval-set".to_string(),
            kind: "test".to_string(),
            authority: "test-suite".to_string(),
            explicit: true,
            benchmark_family: Some("multi_swe_bench_rust".to_string()),
            dataset_source_count: 1,
            instance_ids: vec!["instance-a".to_string()],
            missing_treatment_instance_ids: Vec::new(),
            note: None,
        }
    }

    fn test_runtime_evidence(runtime_id: &str) -> Vec<SealedRuntimeEvidence> {
        vec![SealedRuntimeEvidence {
            runtime_id: runtime_id.to_string(),
            document_citations: vec![SealedEvidenceCitation {
                ref_id: format!("channel:child-to-parent:terminal-result:n1:{runtime_id}"),
                content_hash: Some(
                    HistoryHash::of_domain_json(
                        "prototype1.test.child_channel_terminal_result",
                        &runtime_id,
                    )
                    .expect("terminal hash"),
                ),
                record_name: Some(CHILD_CHANNEL_TERMINAL_RESULT_RECORD.to_string()),
            }],
            journal_citations: Vec::new(),
        }]
    }

    fn test_metrics(
        oracle_eligible: bool,
        convergence: bool,
        failed_tool_calls: usize,
    ) -> OperationalRunMetrics {
        OperationalRunMetrics {
            tool_calls_total: 5,
            tool_calls_failed: failed_tool_calls,
            patch_attempted: true,
            patch_apply_state: if convergence {
                crate::PatchApplyState::Applied
            } else {
                crate::PatchApplyState::No
            },
            submission_artifact_state: if oracle_eligible {
                crate::record::SubmissionArtifactState::Nonempty
            } else {
                crate::record::SubmissionArtifactState::Missing
            },
            patch_projection_check_state: if oracle_eligible {
                ploke_records::evaluation::PatchProjectionCheckState::Passed
            } else {
                ploke_records::evaluation::PatchProjectionCheckState::NotApplicable
            },
            partial_patch_failures: 0,
            same_file_patch_retry_count: 0,
            same_file_patch_max_streak: 0,
            aborted: false,
            aborted_repair_loop: false,
            nonempty_valid_patch: convergence,
            convergence,
            oracle_eligible,
        }
    }

    fn test_candidate_artifact(
        node_id: &str,
        branch_id: &str,
        generation: u32,
    ) -> CandidateArtifact {
        let candidate_id = format!("candidate-{node_id}");
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let node = crate::intervention::Prototype1NodeRecord {
            schema_version: "test-node.v1".to_string(),
            node_id: node_id.to_string(),
            parent_node_id: None,
            generation,
            instance_id: "instance-a".to_string(),
            source_state_id: "source-a".to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: None,
            branch_id: branch_id.to_string(),
            candidate_id: candidate_id.clone(),
            target_relpath: target_relpath.clone(),
            node_dir: PathBuf::from(format!("/tmp/{node_id}")),
            workspace_root: PathBuf::from(format!("/tmp/{node_id}/worktree")),
            binary_path: PathBuf::from(format!("/tmp/{node_id}/target/debug/ploke-eval")),
            runner_request_path: PathBuf::from(format!("/tmp/{node_id}/runner-request.json")),
            runner_result_path: PathBuf::from(format!("/tmp/{node_id}/runner-result.json")),
            status: crate::intervention::Prototype1NodeStatus::Succeeded,
            created_at: "2026-05-06T00:00:00Z".to_string(),
            updated_at: "2026-05-06T00:00:00Z".to_string(),
        };
        let resolved = crate::intervention::ResolvedTreatmentBranch {
            instance_id: node.instance_id.clone(),
            source_state_id: node.source_state_id.clone(),
            parent_branch_id: node.parent_branch_id.clone(),
            target_relpath,
            source_content: "old".to_string(),
            source_content_hash: "old-hash".to_string(),
            selected_branch_id: Some(branch_id.to_string()),
            branch: crate::intervention::TreatmentBranchNode {
                branch_id: branch_id.to_string(),
                candidate_id,
                patch_id: None,
                branch_label: "test".to_string(),
                synthesized_spec_id: "spec".to_string(),
                proposed_content: "new".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: crate::intervention::TreatmentBranchStatus::Selected,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
            },
        };
        CandidateArtifact::new(node, resolved)
    }

    fn test_surface_evidence(
        target_relpath: PathBuf,
        base_artifact_id: &str,
        after_artifact_id: &str,
        patch_id: &str,
    ) -> SurfaceEvidence {
        let base = SurfaceArtifactRef {
            artifact_id: ArtifactId::new(base_artifact_id),
            hash: "base-hash".to_string(),
        };
        let transition = CheckedSurfaceTransition {
            target_relpath: target_relpath.clone(),
            base,
            after: SurfaceArtifactRef {
                artifact_id: ArtifactId::new(after_artifact_id),
                hash: "after-hash".to_string(),
            },
            patch_id: PatchId::new(patch_id),
        };
        let grant = grant::Grant::<grant::Checked>::checked(
            crate::loop_graph::Coordinate {
                runtime_id: crate::loop_graph::RuntimeId(uuid::Uuid::nil()),
                target: crate::loop_graph::OperationTarget::Artifact {
                    artifact_id: transition.base.artifact_id.clone(),
                },
            },
            ProcedureRef::new("policy:surface:test"),
            SurfaceWritable {
                target_relpath: target_relpath.clone(),
            },
            &transition,
        )
        .expect("checked grant");
        SurfaceEvidence::checked(
            "producer-1",
            "proposal-1",
            "run-1",
            CheckedSurface { grant, transition },
            "source-content-hash",
            "proposed-content-hash",
            crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::NonRouter,
            crate::cli::prototype1_state::edit_surface::tui::GeneratorSurfaceVersion {
                projection_id: "projection-1".to_string(),
                projection_hash: "projection-hash".to_string(),
                bounds_digest: "bounds-digest".to_string(),
                source_kind:
                    crate::cli::prototype1_state::edit_surface::tui::GeneratorSourceKind::Named,
                source_id: "generator-source".to_string(),
                source_version: "generator-version".to_string(),
            },
            vec![SurfaceTouch {
                target_relpath: target_relpath.clone(),
                target_name: "target:0".to_string(),
                span_relpath: target_relpath,
                start: 0,
                end: 4,
                base_hash: "base-span-hash".to_string(),
                replacement: "next".to_string(),
                replacement_hash: "replacement-hash".to_string(),
            }],
        )
        .expect("surface evidence")
    }

    fn selection_entry_for_scope(
        scope: SelectionScope,
        selected_node: &str,
        selected_branch: &str,
        considered: Vec<EvaluationPayload>,
    ) -> SelectionDecisionEntry {
        let selected = considered
            .iter()
            .find(|payload| payload.candidate_node_id() == Some(selected_node))
            .expect("selected payload")
            .candidate
            .clone();
        SelectionDecisionEntry::new(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            scope,
            Some(selected),
            considered,
            Vec::new(),
            selection_decision(selected_node, selected_branch),
        )
        .expect("selection entry")
    }

    fn seal(block: Block<block::Open>) -> Block<block::Sealed> {
        seal_with_transition(block, "transition:crown-lock")
    }

    fn seal_with_transition(
        block: Block<block::Open>,
        transition: &'static str,
    ) -> Block<block::Sealed> {
        seal_with_transition_and_claims(block, transition, block::Claims::empty_unchecked())
    }

    fn seal_with_transition_and_claims(
        block: Block<block::Open>,
        transition: &'static str,
        claims: block::Claims,
    ) -> Block<block::Sealed> {
        let seal = SealBlock {
            crown_lock_transition: EvidenceRef::new(transition),
            selected_successor: SuccessorRef::new(
                actor("successor"),
                ArtifactRef::from_artifact_id(ArtifactId::new("artifact:successor")),
            ),
            selected_parent_identity: test_parent_identity(),
            active_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:successor")),
            claims,
            sealed_at: at(30),
        };
        Crown::test_locked_with_seal(block.lineage_id().as_str(), seal)
            .seal(block)
            .expect("seal block")
    }

    fn ruler_witness() -> RulerWitness {
        RulerWitness::new(actor("ruler"), env(), at(25))
    }

    fn admission() -> Admission {
        Admission::new(actor("ruler"), ProcedureRef::new("policy:test"), at(25))
    }

    fn policy_claim(
        path: &'static str,
    ) -> claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Policy, TestLocator>>> {
        let crown: Crown<crown::Ruling> = Crown::test_ruling("lineage:a");
        let (_, claim) = crown
            .admit_claim(
                &TestLocator,
                ArtifactPath::new(path),
                actor("ruler"),
                env(),
                ProcedureRef::new("policy:test"),
                at(25),
            )
            .expect("policy claim");
        claim
    }

    fn artifact_claim(
        key: &'static str,
    ) -> claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Artifact, ArtifactLocator>>>
    {
        let crown: Crown<crown::Ruling> = Crown::test_ruling("lineage:a");
        let (_, claim) = crown
            .admit_claim(
                &ArtifactLocator,
                tree_key(key),
                actor("ruler"),
                env(),
                ProcedureRef::new("policy:test"),
                at(25),
            )
            .expect("artifact claim");
        claim
    }

    #[test]
    fn locked_crown_must_match_block_lineage() {
        let block = open_block(0, Vec::new());
        let err = Crown::test_locked("lineage:other")
            .seal(block)
            .expect_err("wrong lineage must not seal");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn ruling_crown_must_match_open_block_lineage() {
        let err = Crown::test_ruling("lineage:other")
            .open_block(open_block_fields("lineage:a", 0, Vec::new()))
            .expect_err("wrong lineage must not open block");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn ruling_crown_must_match_admitted_block_lineage() {
        let mut block = Crown::test_ruling("lineage:other")
            .open_block(open_block_fields("lineage:other", 0, Vec::new()))
            .expect("open other lineage block");
        let err = ruling_crown()
            .admit_entry(&mut block, proposed_entry(), actor("admitter"))
            .expect_err("wrong lineage must not admit into block");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn ruling_crown_must_match_successor_predecessor_lineage() {
        let predecessor = seal(
            Crown::test_ruling("lineage:other")
                .open_block(open_block_fields("lineage:other", 0, Vec::new()))
                .expect("open other lineage block"),
        );

        let err = ruling_crown()
            .open_successor(
                &predecessor,
                OpenSuccessorBlock {
                    additional_parent_block_hashes: Vec::new(),
                    opened_from_state: HistoryStateRoot::test("state:successor"),
                    opened_by: actor("successor"),
                    opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new(
                        "artifact:successor",
                    )),
                    ruling_authority: actor("successor"),
                    policy_ref: ProcedureRef::new("policy:next"),
                    surface: surface_commitment("wrong-lineage-successor"),
                    opened_at: at(40),
                },
            )
            .expect_err("wrong lineage must not open successor block");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn fs_block_store_appends_block_and_projection_indexes() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let expected_state = store
            .lineage_state(&LineageId::new("lineage:a"))
            .expect("read empty state");
        let mut block = open_block_from_state(&expected_state, 0, Vec::new());
        block
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit");
        let sealed = seal(block);
        let expected_hash = *sealed.block_hash();

        let stored = store
            .append(&expected_state, &sealed)
            .expect("append sealed block");

        assert_eq!(stored.block_hash, expected_hash);
        assert_eq!(
            store
                .lineage_state(&LineageId::new("lineage:a"))
                .expect("read head")
                .head()
                .block_hash(),
            Some(&expected_hash)
        );

        let block_lines = std::fs::read_to_string(
            tmp.path()
                .join("history")
                .join("blocks")
                .join("segment-000000.jsonl"),
        )
        .expect("block segment");
        assert_eq!(block_lines.lines().count(), 1);

        let by_hash = std::fs::read_to_string(
            tmp.path()
                .join("history")
                .join("index")
                .join("by-hash.jsonl"),
        )
        .expect("by hash index");
        assert!(by_hash.contains(&expected_hash.to_hex()));

        let by_lineage = std::fs::read_to_string(
            tmp.path()
                .join("history")
                .join("index")
                .join("by-lineage-height.jsonl"),
        )
        .expect("by lineage index");
        assert!(by_lineage.contains("\"block_height\":0"));
    }

    #[test]
    fn fs_block_store_projection_indexes_deserialize_and_match_sealed_blocks() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");

        let state0 = store.lineage_state(&lineage).expect("read empty state");
        let mut block0 = open_block_from_state(&state0, 0, Vec::new());
        block0
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit block 0");
        let sealed0 = seal(block0);
        let hash0 = *sealed0.block_hash();
        store.append(&state0, &sealed0).expect("append block 0");

        let state1 = store.lineage_state(&lineage).expect("read block 0 state");
        let mut block1 = open_block_from_state(&state1, 1, vec![hash0]);
        block1
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit block 1");
        let sealed1 = seal(block1);
        let hash1 = *sealed1.block_hash();
        store.append(&state1, &sealed1).expect("append block 1");

        let by_hash = std::fs::read_to_string(store.by_hash_path()).expect("by hash index");
        let by_hash_records = by_hash
            .lines()
            .map(|line| serde_json::from_str::<StoredBlock>(line).expect("stored block record"))
            .collect::<Vec<_>>();
        assert_eq!(by_hash_records.len(), 2);
        assert_eq!(by_hash_records[0].lineage_id, lineage);
        assert_eq!(by_hash_records[0].block_height, 0);
        assert_eq!(by_hash_records[0].block_hash, hash0);
        assert_eq!(
            by_hash_records[0].location.segment,
            FsBlockStore::SEGMENT_NAME
        );
        assert_eq!(by_hash_records[0].location.line_index, 0);
        assert_eq!(by_hash_records[1].lineage_id, lineage);
        assert_eq!(by_hash_records[1].block_height, 1);
        assert_eq!(by_hash_records[1].block_hash, hash1);
        assert_eq!(
            by_hash_records[1].location.segment,
            FsBlockStore::SEGMENT_NAME
        );
        assert_eq!(by_hash_records[1].location.line_index, 1);

        let by_lineage =
            std::fs::read_to_string(store.by_lineage_height_path()).expect("lineage index");
        let by_lineage_records = by_lineage
            .lines()
            .map(|line| serde_json::from_str::<LineageHeight>(line).expect("lineage record"))
            .collect::<Vec<_>>();
        assert_eq!(by_lineage_records.len(), 2);
        assert_eq!(by_lineage_records[0].lineage_id, lineage);
        assert_eq!(by_lineage_records[0].block_height, 0);
        assert_eq!(by_lineage_records[0].block_hash, hash0);
        assert_eq!(by_lineage_records[1].lineage_id, lineage);
        assert_eq!(by_lineage_records[1].block_height, 1);
        assert_eq!(by_lineage_records[1].block_hash, hash1);

        let heads: BTreeMap<LineageId, BlockHash> =
            serde_json::from_slice(&std::fs::read(store.heads_path()).expect("heads projection"))
                .expect("typed heads projection");
        assert_eq!(heads.get(&lineage), Some(&hash1));
    }

    #[test]
    fn fs_block_store_history_segment_deserializes_as_passive_record() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let expected_state = store.lineage_state(&lineage).expect("read empty state");
        let mut block = open_block_from_state(&expected_state, 0, Vec::new());
        let considered = vec![evaluation_payload("child-a", "branch-a", 0)];
        let selected = considered[0].candidate.clone();
        let selection = SelectionDecisionEntry::new_with_traversal(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::all_admitted_candidates(),
            Some(selected),
            considered,
            Vec::new(),
            Some(TraversalEvidence {
                seed: 7,
                strategy: StrategyKind::default(),
                selected_source: Some(TraversalCandidateSource::CurrentGeneration),
                child_counts: BTreeMap::new(),
            }),
            selection_decision("child-a", "branch-a"),
        )
        .expect("selection entry with traversal");
        let admitted_entry_id = block
            .admit(proposed_selection_entry(selection), actor("admitter"))
            .expect("admit entry");
        let sealed = seal(block);
        let expected_block_hash = sealed.block_hash().to_hex();
        let expected_block_id =
            serde_json::to_value(sealed.header().common.block_id).expect("serialize eval block id");
        let expected_opened_from_state =
            serde_json::to_value(expected_state.root()).expect("serialize eval state root");

        let stored = store
            .append(&expected_state, &sealed)
            .expect("append sealed block");

        let segment = std::fs::read_to_string(
            tmp.path()
                .join("history")
                .join("blocks")
                .join("segment-000000.jsonl"),
        )
        .expect("block segment");
        let first_line = segment.lines().next().expect("first segment line");
        let passive: ploke_records::history::SealedBlockRecord =
            serde_json::from_str(first_line).expect("deserialize passive sealed block record");

        assert_eq!(stored.location.line_index, 0);
        assert_eq!(passive.state.header.block_hash.0, expected_block_hash);
        assert_eq!(
            serde_json::to_value(&passive.state.header.common.block_id)
                .expect("serialize passive block id"),
            expected_block_id
        );
        assert_eq!(
            serde_json::to_value(&passive.state.header.common.opened_from_state)
                .expect("serialize passive state root"),
            expected_opened_from_state
        );
        assert_eq!(passive.state.header.common.lineage_id.0, lineage.as_str());
        assert_eq!(
            passive.state.header.common.block_height,
            sealed.block_height()
        );
        assert_eq!(passive.state.header.common.parent_block_hashes, Vec::new());
        assert_eq!(passive.state.header.entry_count, 1);
        assert_eq!(passive.entries.len(), 1);
        assert_eq!(
            passive.entries[0].core.entry_id.0,
            admitted_entry_id.to_string()
        );
        let selection = match &passive.entries[0].core.payload {
            ploke_records::history::EntryPayloadRecord::SelectionDecision(selection) => selection,
            other => panic!("expected passive selection decision payload, got {other:?}"),
        };
        assert_eq!(
            selection
                .selected_candidate
                .as_ref()
                .map(|subject| subject.value.as_str()),
            Some("candidate:child-a:plan_index=0")
        );
        assert_eq!(selection.considered.len(), 1);
        assert!(selection.considered[0].selection_input.is_some());
        let traversal = selection
            .traversal
            .as_ref()
            .expect("passive traversal evidence");
        assert_eq!(traversal.seed, 7);
        assert_eq!(
            traversal.selected_source,
            Some(ploke_records::history::TraversalCandidateSourceRecord::CurrentGeneration)
        );
        assert!(matches!(
            traversal.strategy,
            ploke_records::history::TraversalStrategyRecord::FrontierMax { .. }
        ));
        assert_eq!(passive.entries[0].state.lineage_id.0, lineage.as_str());
        assert_eq!(passive.entries[0].state.block_height, sealed.block_height());

        let head = store.lineage_state(&lineage).expect("read stored head");
        let StoreHead::Present(head) = head.head() else {
            panic!("stored selection decision block should be the lineage head");
        };
        let loaded = store
            .sealed_head_block(head)
            .expect("load stored selection decision block as verified sealed head");
        loaded
            .verify_expected_hash(sealed.block_hash())
            .expect("loaded block verifies");
    }

    #[test]
    fn fs_block_store_verifies_loaded_entries_with_stored_json_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let state = store.lineage_state(&lineage).expect("read state");
        let mut block = open_block_from_state(&state, 0, Vec::new());
        block
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit entry");
        let sealed = seal(block);
        store.append(&state, &sealed).expect("append block");

        let segment_path = store.segment_path();
        let segment_text = fs::read_to_string(&segment_path).expect("read segment");
        let mut stored: serde_json::Value =
            serde_json::from_str(segment_text.trim()).expect("parse stored block");
        let entry = stored["entries"][0].clone();
        let state_json = serde_json::to_string(&entry["state"]).expect("entry state json");
        let core_json = serde_json::to_string(&entry["core"]).expect("entry core json");
        let reordered_entry = format!(r#"{{"state":{state_json},"core":{core_json}}}"#);
        let entry_hash = entry_hash_from_raw_json(&reordered_entry);
        assert_ne!(
            entry_hash,
            sealed.entries()[0]
                .entry_hash()
                .expect("current entry hash"),
            "reordered stored JSON must differ from the current in-memory serialization hash"
        );

        let entries_root =
            legacy_domain_json_hash("prototype1.history.entries_root.v1", &vec![entry_hash])
                .expect("entries root");
        let header = sealed.header();
        let block_preimage = SealedBlockPreimage {
            common: header.common.clone(),
            crown_lock_transition: header.crown_lock_transition.clone(),
            selected_successor: header.selected_successor.clone(),
            selected_parent_identity: header.selected_parent_identity.clone(),
            active_artifact: header.active_artifact.clone(),
            claims: header.claims.clone(),
            sealed_at: header.sealed_at,
            entry_count: header.entry_count,
            entries_root: entries_root.clone(),
        };
        let block_hash = BlockHash::from(
            legacy_domain_json_hash("prototype1.history.block.v1", &block_preimage)
                .expect("block hash"),
        );
        stored["state"]["header"]["entries_root"] =
            serde_json::to_value(entries_root).expect("entries root value");
        stored["state"]["header"]["block_hash"] =
            serde_json::to_value(block_hash).expect("block hash value");
        let stored_state_json = serde_json::to_string(&stored["state"]).expect("state json");
        let rewritten = format!(
            r#"{{"entries":[{reordered_entry}],"state":{stored_state_json}}}
"#
        );
        fs::write(&segment_path, rewritten).expect("rewrite segment with stored entry order");

        let loaded = store
            .load_segment_verified_blocks()
            .expect("load segment block using stored entry JSON hashes");
        assert_eq!(loaded.len(), 1);
        loaded[0]
            .1
            .verify_expected_hash(&block_hash)
            .expect("loaded block verifies against rewritten block hash");
    }

    #[test]
    fn history_candidates_verify_selection_payloads_with_stored_json_order() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let state = store.lineage_state(&lineage).expect("read state");
        let mut block = open_block_from_state(&state, 0, Vec::new());
        let selection = selection_entry_for_scope(
            SelectionScope::all_admitted_candidates(),
            "child-a",
            "branch-a",
            vec![evaluation_payload("child-a", "branch-a", 0)],
        );
        block
            .admit(proposed_selection_entry(selection), actor("admitter"))
            .expect("admit selection entry");
        let sealed = seal(block);
        let original_payload_hash = sealed.entries()[0].payload_hash().clone();
        store.append(&state, &sealed).expect("append block");

        let segment_path = store.segment_path();
        let segment_text = fs::read_to_string(&segment_path).expect("read segment");
        let mut stored: serde_json::Value =
            serde_json::from_str(segment_text.trim()).expect("parse stored block");
        let entry = stored["entries"][0].clone();
        let payload = entry["core"]["payload"]
            .as_object()
            .expect("selection payload object");
        let field_order = [
            "decision",
            "formula",
            "metrics",
            "candidate_set",
            "considered_order_hash",
            "considered",
            "selected_candidate",
            "scope",
            "procedure_or_policy",
            "schema_version",
            "selected_occurrence_id",
            "selected_membership_id",
            "considered_sources",
            "projection_failures",
            "traversal",
        ];
        let mut payload_fields = vec![r#""kind":"selection_decision""#.to_string()];
        let mut decision_fields = Vec::new();
        for field in field_order {
            let Some(value) = payload.get(field) else {
                continue;
            };
            let field_json = format!(
                r#""{field}":{}"#,
                serde_json::to_string(value).expect("selection field json")
            );
            payload_fields.push(field_json.clone());
            decision_fields.push(field_json);
        }
        assert_eq!(
            decision_fields.len(),
            payload.len() - 1,
            "test field order must cover every persisted selection field except the enum tag"
        );
        let reordered_payload_json = format!("{{{}}}", payload_fields.join(","));
        let reordered_decision_json = format!("{{{}}}", decision_fields.join(","));
        let reordered_payload_hash = selection_hash_from_raw_json(&reordered_decision_json);
        assert_ne!(
            reordered_payload_hash, original_payload_hash,
            "reordered stored selection payload must differ from current in-memory decision_hash"
        );

        let mut entry_state = entry["state"].clone();
        entry_state["observed"]["payload_hash"] =
            serde_json::to_value(&reordered_payload_hash).expect("payload hash json");
        let core = &entry["core"];
        let core_json = format!(
            r#"{{"entry_id":{},"entry_kind":{},"subject":{},"executor":{},"input_refs":{},"output_refs":{},"occurred_at":{},"payload":{}}}"#,
            serde_json::to_string(&core["entry_id"]).expect("entry id json"),
            serde_json::to_string(&core["entry_kind"]).expect("entry kind json"),
            serde_json::to_string(&core["subject"]).expect("subject json"),
            serde_json::to_string(&core["executor"]).expect("executor json"),
            serde_json::to_string(&core["input_refs"]).expect("input refs json"),
            serde_json::to_string(&core["output_refs"]).expect("output refs json"),
            serde_json::to_string(&core["occurred_at"]).expect("occurred at json"),
            reordered_payload_json,
        );
        let entry_state_json = serde_json::to_string(&entry_state).expect("entry state json");
        let rewritten_entry = format!(r#"{{"core":{core_json},"state":{entry_state_json}}}"#);
        let entry_hash = entry_hash_from_raw_json(&rewritten_entry);
        let entries_root =
            legacy_domain_json_hash("prototype1.history.entries_root.v1", &vec![entry_hash])
                .expect("entries root");
        let header = sealed.header();
        let block_preimage = SealedBlockPreimage {
            common: header.common.clone(),
            crown_lock_transition: header.crown_lock_transition.clone(),
            selected_successor: header.selected_successor.clone(),
            selected_parent_identity: header.selected_parent_identity.clone(),
            active_artifact: header.active_artifact.clone(),
            claims: header.claims.clone(),
            sealed_at: header.sealed_at,
            entry_count: header.entry_count,
            entries_root: entries_root.clone(),
        };
        let block_hash = BlockHash::from(
            legacy_domain_json_hash("prototype1.history.block.v1", &block_preimage)
                .expect("block hash"),
        );
        stored["state"]["header"]["entries_root"] =
            serde_json::to_value(entries_root).expect("entries root value");
        stored["state"]["header"]["block_hash"] =
            serde_json::to_value(block_hash).expect("block hash value");
        let stored_state_json = serde_json::to_string(&stored["state"]).expect("state json");
        let rewritten = format!(
            r#"{{"state":{stored_state_json},"entries":[{rewritten_entry}]}}
"#
        );
        fs::write(&segment_path, rewritten).expect("rewrite segment with stored payload order");

        let loaded = store
            .load_segment_verified_blocks()
            .expect("load segment block using stored entry JSON hashes");
        assert_eq!(loaded.len(), 1);
        let loaded_entry = &loaded[0].1.entries()[0];
        assert_eq!(
            loaded_entry
                .decision_observation_hash()
                .expect("selection observation hash"),
            Some(reordered_payload_hash.clone())
        );
        assert_eq!(
            loaded_entry
                .verify_selection_decision_observation()
                .expect("selection observation verifies"),
            Some(true)
        );

        let candidates = History::new(store)
            .candidates(&SelectionScope::all_admitted_candidates())
            .expect("history candidates use stored selection payload hash");
        assert_eq!(candidates.candidates.len(), 1);
    }

    #[test]
    fn history_candidates_reads_cross_generation_selection_payloads_with_proofs() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");

        let state0 = store.lineage_state(&lineage).expect("read empty state");
        let mut block0 = open_block_from_state(&state0, 0, Vec::new());
        let gen0 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=root;generation=1"),
            "child-a",
            "branch-a",
            vec![
                evaluation_payload("child-a", "branch-a", 0),
                evaluation_payload("child-b", "branch-b", 1),
            ],
        );
        block0
            .admit(proposed_selection_entry(gen0), actor("admitter"))
            .expect("admit gen0 selection");
        let sealed0 = seal(block0);
        let sealed0_hash = *sealed0.block_hash();
        store.append(&state0, &sealed0).expect("append gen0");

        let state1 = store.lineage_state(&lineage).expect("read gen0 state");
        let mut block1 = open_block_from_state(&state1, 1, vec![sealed0_hash]);
        let gen1 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=child-a;generation=2"),
            "child-c",
            "branch-c",
            vec![evaluation_payload("child-c", "branch-c", 0)],
        );
        block1
            .admit(proposed_selection_entry(gen1), actor("admitter"))
            .expect("admit gen1 selection");
        let sealed1 = seal(block1);
        store.append(&state1, &sealed1).expect("append gen1");

        let history = History::new(store);
        let all = history
            .candidates(&SelectionScope::all_admitted_candidates())
            .expect("history candidates");
        assert_eq!(all.candidates.len(), 3);
        assert!(
            all.candidates
                .iter()
                .all(|candidate| candidate.candidate_set_root.is_some()
                    && candidate.candidate_set_membership.is_some())
        );

        let exact = history
            .candidates(&SelectionScope::new(
                "generation_local:parent_node_id=child-a;generation=2",
            ))
            .expect("exact-scope candidates");
        assert_eq!(exact.candidates.len(), 1);
        assert_eq!(
            exact.candidates[0].payload.candidate.as_str(),
            "candidate:child-c:plan_index=0"
        );
    }

    #[test]
    fn traversal_selector_seals_cross_generation_considered_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");

        let state0 = store.lineage_state(&lineage).expect("read empty state");
        let mut block0 = open_block_from_state(&state0, 0, Vec::new());
        let gen0 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=root;generation=1"),
            "child-a",
            "branch-a",
            vec![evaluation_payload("child-a", "branch-a", 0)],
        );
        block0
            .admit(proposed_selection_entry(gen0), actor("admitter"))
            .expect("admit gen0 selection");
        let sealed0 = seal(block0);
        let sealed0_hash = *sealed0.block_hash();
        store.append(&state0, &sealed0).expect("append gen0");

        let state1 = store.lineage_state(&lineage).expect("read gen0 state");
        let mut block1 = open_block_from_state(&state1, 1, vec![sealed0_hash]);
        let gen1 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=child-a;generation=2"),
            "child-b",
            "branch-b",
            vec![evaluation_payload("child-b", "branch-b", 0)],
        );
        block1
            .admit(proposed_selection_entry(gen1), actor("admitter"))
            .expect("admit gen1 selection");
        let sealed1 = seal(block1);
        store.append(&state1, &sealed1).expect("append gen1");

        let history = History::new(store);
        let scope = SelectionScope::all_admitted_candidates();
        let traversal = crate::successor_selection::traversal::select_from_history(
            history.candidates(&scope).expect("history candidates"),
            7,
            StrategyKind::default(),
        )
        .expect("traversal decision")
        .expect("selected candidate");

        let entry = SelectionDecisionEntry::new_with_traversal(
            ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
            scope,
            Some(traversal.selected_payload.candidate.clone()),
            traversal.considered,
            traversal.projection_failures,
            Some(TraversalEvidence {
                seed: 7,
                strategy: StrategyKind::default(),
                selected_source: None,
                child_counts: traversal.child_counts,
            }),
            traversal.decision,
        )
        .expect("history traversal entry");

        assert_eq!(entry.considered.len(), 2);
        assert_eq!(
            entry.procedure_or_policy.as_str(),
            crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
        );
        assert_eq!(
            entry.traversal.as_ref().map(|evidence| evidence.seed),
            Some(7)
        );
        assert_eq!(
            entry
                .verify_candidate_set_commitment()
                .expect("candidate set verifies"),
            Some(true)
        );
        assert!(
            entry
                .candidate_set_membership(entry.selected_candidate.as_ref().expect("selected"))
                .is_some()
        );
    }

    #[test]
    fn history_candidates_do_not_reingest_prior_traversal_considered_set() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");

        let state0 = store.lineage_state(&lineage).expect("read empty state");
        let mut block0 = open_block_from_state(&state0, 0, Vec::new());
        let gen0 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=root;generation=1"),
            "child-a",
            "branch-a",
            vec![evaluation_payload("child-a", "branch-a", 0)],
        );
        block0
            .admit(proposed_selection_entry(gen0), actor("admitter"))
            .expect("admit gen0 selection");
        let sealed0 = seal(block0);
        let sealed0_hash = *sealed0.block_hash();
        store.append(&state0, &sealed0).expect("append gen0");

        let state1 = store.lineage_state(&lineage).expect("read gen0 state");
        let mut block1 = open_block_from_state(&state1, 1, vec![sealed0_hash]);
        let gen1 = selection_entry_for_scope(
            SelectionScope::new("generation_local:parent_node_id=child-a;generation=2"),
            "child-b",
            "branch-b",
            vec![evaluation_payload("child-b", "branch-b", 0)],
        );
        block1
            .admit(proposed_selection_entry(gen1), actor("admitter"))
            .expect("admit gen1 selection");
        let sealed1 = seal(block1);
        let sealed1_hash = *sealed1.block_hash();
        store.append(&state1, &sealed1).expect("append gen1");

        let scope = SelectionScope::all_admitted_candidates();
        let first_traversal = crate::successor_selection::traversal::select_from_history(
            History::new(store.clone())
                .candidates(&scope)
                .expect("initial history candidates"),
            7,
            StrategyKind::default(),
        )
        .expect("first traversal decision")
        .expect("first selected candidate");
        assert_eq!(first_traversal.considered.len(), 2);

        let traversal_entry = SelectionDecisionEntry::new_with_traversal(
            ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
            scope.clone(),
            Some(first_traversal.selected_payload.candidate.clone()),
            first_traversal.considered.clone(),
            first_traversal.projection_failures.clone(),
            Some(TraversalEvidence {
                seed: 7,
                strategy: StrategyKind::default(),
                selected_source: None,
                child_counts: first_traversal.child_counts.clone(),
            }),
            first_traversal.decision.clone(),
        )
        .expect("first traversal entry");

        let state2 = store
            .lineage_state(&lineage)
            .expect("read state before traversal");
        let mut block2 = open_block_from_state(&state2, 2, vec![sealed1_hash]);
        block2
            .admit(proposed_selection_entry(traversal_entry), actor("admitter"))
            .expect("admit traversal selection");
        let sealed2 = seal(block2);
        store
            .append(&state2, &sealed2)
            .expect("append traversal selection");

        let candidates_after_traversal = History::new(store)
            .candidates(&scope)
            .expect("candidates after traversal");
        assert_eq!(
            candidates_after_traversal.candidates.len(),
            2,
            "traversal decisions must not replay their considered set as new candidates"
        );

        let second_traversal = crate::successor_selection::traversal::select_from_history(
            candidates_after_traversal,
            7,
            StrategyKind::default(),
        )
        .expect("second traversal decision")
        .expect("second selected candidate");

        let second_entry = SelectionDecisionEntry::new_with_traversal(
            ProcedureRef::new(crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID),
            scope,
            Some(second_traversal.selected_payload.candidate.clone()),
            second_traversal.considered,
            second_traversal.projection_failures,
            Some(TraversalEvidence {
                seed: 7,
                strategy: StrategyKind::default(),
                selected_source: None,
                child_counts: second_traversal.child_counts,
            }),
            second_traversal.decision,
        )
        .expect("second traversal entry should not see duplicate candidate-set keys");
        assert_eq!(second_entry.considered.len(), 2);
    }

    #[test]
    fn fs_block_store_rejects_duplicate_genesis() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let first_state = store.lineage_state(&lineage).expect("read empty state");
        let first = seal(open_block_from_state(&first_state, 0, Vec::new()));
        store
            .append(&first_state, &first)
            .expect("append first genesis");

        let current = store.lineage_state(&lineage).expect("read current state");
        let second = seal(open_block_from_state(&current, 0, Vec::new()));
        let err = store
            .append(&current, &second)
            .expect_err("duplicate genesis must fail");

        assert!(matches!(err, BlockStoreError::DuplicateGenesis { .. }));
    }

    #[test]
    fn fs_block_store_rejects_non_genesis_without_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let missing_parent = BlockHash::from(HistoryHash::of_bytes(b"missing-parent"));
        let state = store.lineage_state(&lineage).expect("read empty state");
        let block = seal(open_block_from_state(&state, 1, vec![missing_parent]));
        let err = store
            .append(&state, &block)
            .expect_err("non-genesis without head must fail");

        assert!(matches!(err, BlockStoreError::NonGenesisWithoutHead { .. }));
    }

    #[test]
    fn fs_block_store_rejects_child_that_does_not_extend_current_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let genesis_state = store.lineage_state(&lineage).expect("read empty state");
        let genesis = seal(open_block_from_state(&genesis_state, 0, Vec::new()));
        store
            .append(&genesis_state, &genesis)
            .expect("append genesis");

        let wrong_parent = BlockHash::from(HistoryHash::of_bytes(b"wrong-parent"));
        let current = store.lineage_state(&lineage).expect("read current state");
        let child = seal(open_block_from_state(&current, 1, vec![wrong_parent]));
        let err = store
            .append(&current, &child)
            .expect_err("child must extend current head");

        assert!(matches!(err, BlockStoreError::WrongStoreHeadParent { .. }));
    }

    #[test]
    fn fs_block_store_rejects_stale_expected_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let stale = store.lineage_state(&lineage).expect("read empty state");
        let genesis = seal(open_block_from_state(&stale, 0, Vec::new()));
        store.append(&stale, &genesis).expect("append genesis");

        let child = seal(open_block_from_state(
            &stale,
            1,
            vec![*genesis.block_hash()],
        ));
        let err = store
            .append(&stale, &child)
            .expect_err("stale expected head must fail");

        assert!(matches!(err, BlockStoreError::StaleStoreHead { .. }));
    }

    #[test]
    fn fs_block_store_rejects_block_opened_from_different_state_root() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let state = store.lineage_state(&lineage).expect("read empty state");
        let sealed = seal(open_block(0, Vec::new()));

        let err = store
            .append(&state, &sealed)
            .expect_err("append must reject a mismatched opening state root");

        assert!(matches!(err, BlockStoreError::WrongOpeningStateRoot { .. }));
    }

    #[test]
    fn fs_block_store_rejects_missing_heads_projection_when_blocks_exist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let state = store.lineage_state(&lineage).expect("read empty state");
        let genesis = seal(open_block_from_state(&state, 0, Vec::new()));
        store.append(&state, &genesis).expect("append genesis");
        std::fs::remove_file(store.heads_path()).expect("remove heads projection");

        let err = store
            .lineage_state(&lineage)
            .expect_err("missing heads projection must not become genesis absence");

        assert!(matches!(
            err,
            BlockStoreError::MissingHeadsProjection { .. }
        ));
    }

    #[test]
    fn fs_block_store_rejects_missing_lineage_head_projection() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let state = store.lineage_state(&lineage).expect("read empty state");
        let genesis = seal(open_block_from_state(&state, 0, Vec::new()));
        store.append(&state, &genesis).expect("append genesis");
        std::fs::write(store.heads_path(), "{}").expect("clear heads projection");

        let err = store
            .lineage_state(&lineage)
            .expect_err("lineage index without head must not become genesis absence");

        assert!(matches!(
            err,
            BlockStoreError::MissingLineageHeadProjection { .. }
        ));
    }

    #[test]
    fn fs_block_store_loads_zero_entry_sealed_head_for_startup_admission() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let claims =
            block::Claims::empty_unchecked().with_artifact(artifact_claim("tree:successor"));
        let expected_state = store.lineage_state(&lineage).expect("read empty state");
        let sealed = seal_with_transition_and_claims(
            open_block_from_state(&expected_state, 0, Vec::new()),
            "transition:crown-lock",
            claims,
        );
        let expected_hash = *sealed.block_hash();
        store
            .append(&expected_state, &sealed)
            .expect("append sealed block");
        let StoreHead::Present(head) = store
            .lineage_state(&lineage)
            .expect("read head")
            .head()
            .clone()
        else {
            panic!("appended block should produce present head");
        };

        let loaded = store.sealed_head_block(&head).expect("load sealed head");

        loaded
            .verify_expected_hash(&expected_hash)
            .expect("loaded block hash verifies");
        loaded
            .verify_current_artifact_tree(&tree_key("tree:successor"), &ArtifactLocator)
            .expect("current successor tree is admitted by sealed head");
    }

    #[test]
    fn sealed_head_artifact_verification_rejects_tree_mismatch() {
        let claims =
            block::Claims::empty_unchecked().with_artifact(artifact_claim("tree:successor"));
        let sealed = seal_with_transition_and_claims(
            open_block(0, Vec::new()),
            "transition:crown-lock",
            claims,
        );

        let err = sealed
            .verify_current_artifact_tree(&tree_key("tree:other"), &ArtifactLocator)
            .expect_err("mismatched current tree key must not enter parent path");

        assert!(matches!(err, HistoryError::ArtifactTreeKeyMismatch { .. }));
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
    fn regime_is_committed_to_block_hash() {
        let block_id = BlockId::new();
        let entry_id = EntryId::new();
        let expansion = Regime::new(
            Step::new(7),
            Phase::Expansion,
            Risk::new(Level::High, Level::High, Level::Low),
        );
        let hardening = Regime::new(
            Step::new(7),
            Phase::Hardening,
            Risk::new(Level::Low, Level::Low, Level::High),
        );

        let mut first_fields = open_block_fields("lineage:a", 0, Vec::new());
        first_fields.regime = expansion.clone();
        let mut first =
            Block::open_with_block_id(block_id, first_fields).expect("first block opens");
        first
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit first");

        let mut second_fields = open_block_fields("lineage:a", 0, Vec::new());
        second_fields.regime = hardening;
        let mut second =
            Block::open_with_block_id(block_id, second_fields).expect("second block opens");
        second
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit second");

        let first = seal(first);
        let second = seal(second);

        assert_eq!(first.regime(), &expansion);
        assert_eq!(first.regime().step().value(), 7);
        assert_eq!(first.regime().phase(), Phase::Expansion);
        assert_eq!(first.regime().risk().exploration(), Level::High);
        assert_eq!(first.regime().risk().mutation(), Level::High);
        assert_eq!(first.regime().risk().finality(), Level::Low);
        let evaluation = Regime::new(
            Step::new(8),
            Phase::Evaluation,
            Risk::new(Level::Medium, Level::Low, Level::Medium),
        );
        assert_eq!(evaluation.phase(), Phase::Evaluation);
        assert_ne!(first.block_hash(), second.block_hash());
    }

    #[test]
    fn surface_commitment_is_committed_to_block_hash() {
        let block_id = BlockId::new();
        let entry_id = EntryId::new();
        let mut first_fields = open_block_fields("lineage:a", 0, Vec::new());
        first_fields.surface = surface_commitment("mutated:before:a");
        let mut first =
            Block::open_with_block_id(block_id, first_fields).expect("first block opens");
        first
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit first");

        let mut second_fields = open_block_fields("lineage:a", 0, Vec::new());
        second_fields.surface = surface_commitment("mutated:before:b");
        let mut second =
            Block::open_with_block_id(block_id, second_fields).expect("second block opens");
        second
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit second");

        let first = seal(first);
        let second = seal(second);

        assert_eq!(
            first.header().common.surface.immutable.root.hash,
            second.header().common.surface.immutable.root.hash
        );
        assert_ne!(first.block_hash(), second.block_hash());
    }

    #[test]
    fn sealed_head_surface_verification_rejects_current_mismatch() {
        let sealed = seal(open_block(0, Vec::new()));
        let mut current = sealed.header().common.surface.clone();
        current.mutated.after = surface::<surface::Mutated>("mutated:after:changed");

        let err = sealed
            .verify_current_surface(&current)
            .expect_err("current surface mismatch must reject startup admission");
        assert!(matches!(
            err,
            HistoryError::SurfaceMismatch {
                partition: "mutated",
                ..
            }
        ));
    }

    #[test]
    fn surface_commitment_from_artifacts_uses_selected_successor_after_roots() {
        let current_parent = ArtifactSurface::test("artifact:f");
        let selected_successor = ArtifactSurface::test("artifact:b");
        let handoff =
            SurfaceCommitment::from_artifact_surfaces(&current_parent, &selected_successor)
                .expect("same immutable surface admits authority transition");

        let selected_current =
            SurfaceCommitment::from_artifact_surfaces(&selected_successor, &selected_successor)
                .expect("selected current surface");
        handoff
            .verify_current(&selected_current)
            .expect("handoff after-roots describe selected successor");

        let stale_parent_current =
            SurfaceCommitment::from_artifact_surfaces(&current_parent, &current_parent)
                .expect("parent current surface");
        let err = handoff
            .verify_current(&stale_parent_current)
            .expect_err("handoff must not verify against stale parent after-roots");
        assert!(matches!(
            err,
            HistoryError::SurfaceMismatch {
                partition: "mutated",
                ..
            }
        ));
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
    fn selection_decision_entry_commits_to_considered_order() {
        let decision = crate::successor_selection::SuccessorDecision {
            procedure_id: crate::successor_selection::PROCEDURE_ID.to_string(),
            candidate_node_id: "child-a".to_string(),
            selected_branch_id: Some("branch-a".to_string()),
            branch_disposition: "keep".to_string(),
            outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        };
        let a = EvaluationPayload {
            schema_version: 2,
            candidate: SubjectRef::new("candidate:child-a:branch-a"),
            procedure: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            selection_input: None,
            selection_input_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.selection_input.v1",
                    &serde_json::json!({"node":"child-a"}),
                )
                .expect("hash"),
            ),
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: Some(SealedCandidateEvidence {
                schema_version: 2,
                coordinate: CandidateCoordinate {
                    node_id: "child-a".to_string(),
                    parent_node_id: None,
                    branch_id: Some("branch-a".to_string()),
                    generation: Some(2),
                    plan_index: Some(0),
                    primary_runtime_id: None,
                },
                lifecycle: CandidateLifecycle {
                    planner_outcome: "done".to_string(),
                    node_status: "completed".to_string(),
                },
                evaluations: Vec::new(),
                runtimes: Vec::new(),
                branches: Vec::new(),
                extra_document_citations: Vec::new(),
                extra_journal_citations: Vec::new(),
                child_diagnostics: Vec::new(),
            }),
            artifact: None,
            surface_attempt: None,
        };
        let b = EvaluationPayload {
            schema_version: 2,
            candidate: SubjectRef::new("candidate:child-b:branch-b"),
            procedure: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            selection_input: None,
            selection_input_hash: Some(
                HistoryHash::of_domain_json(
                    "prototype1.history.selection_input.v1",
                    &serde_json::json!({"node":"child-b"}),
                )
                .expect("hash"),
            ),
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: Some(SealedCandidateEvidence {
                schema_version: 2,
                coordinate: CandidateCoordinate {
                    node_id: "child-b".to_string(),
                    parent_node_id: None,
                    branch_id: Some("branch-b".to_string()),
                    generation: Some(2),
                    plan_index: Some(1),
                    primary_runtime_id: None,
                },
                lifecycle: CandidateLifecycle {
                    planner_outcome: "done".to_string(),
                    node_status: "completed".to_string(),
                },
                evaluations: Vec::new(),
                runtimes: Vec::new(),
                branches: Vec::new(),
                extra_document_citations: Vec::new(),
                extra_journal_citations: Vec::new(),
                child_diagnostics: Vec::new(),
            }),
            artifact: None,
            surface_attempt: None,
        };

        let first = SelectionDecisionEntry::new(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(SubjectRef::new("candidate:child-a:branch-a")),
            vec![a.clone(), b.clone()],
            Vec::new(),
            decision.clone(),
        )
        .expect("selection entry");
        let second = SelectionDecisionEntry::new(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(SubjectRef::new("candidate:child-a:branch-a")),
            vec![b, a],
            Vec::new(),
            decision,
        )
        .expect("selection entry");

        assert_ne!(first.considered_order_hash, second.considered_order_hash);
        assert_eq!(
            first.candidate_set.as_ref().expect("candidate set").root,
            second.candidate_set.as_ref().expect("candidate set").root
        );
        assert_eq!(
            first
                .verify_candidate_set_commitment()
                .expect("candidate set verifies"),
            Some(true)
        );
        assert_ne!(
            first.decision_hash().expect("hash"),
            second.decision_hash().expect("hash")
        );
    }

    #[test]
    fn selection_decision_entry_commits_candidate_set_membership_proofs() {
        let entry = selection_entry_for_scope(
            SelectionScope::new("generation_local:test"),
            "child-a",
            "branch-a",
            vec![
                evaluation_payload("child-a", "branch-a", 0),
                evaluation_payload("child-b", "branch-b", 1),
            ],
        );

        let candidate_set = entry.candidate_set.as_ref().expect("candidate set");
        assert_eq!(candidate_set.memberships.len(), 2);
        assert_eq!(
            entry
                .verify_candidate_set_commitment()
                .expect("candidate set verifies"),
            Some(true)
        );
        for membership in &candidate_set.memberships {
            assert!(
                membership
                    .proof
                    .verify(&candidate_set.root)
                    .expect("membership proof verifies")
            );
        }
    }

    #[test]
    fn selection_decision_entry_allows_same_subject_ref_with_selected_occurrence() {
        let subject = SubjectRef::new("candidate:child-a:plan_index=0");
        let mut first = evaluation_payload("child-a", "branch-a", 0);
        first.candidate = subject.clone();
        first
            .sealed_evidence
            .as_mut()
            .expect("sealed evidence")
            .coordinate
            .primary_runtime_id = Some("runtime:first".to_string());
        let mut second = evaluation_payload("child-a", "branch-a", 0);
        second.candidate = subject.clone();
        second
            .sealed_evidence
            .as_mut()
            .expect("sealed evidence")
            .coordinate
            .primary_runtime_id = Some("runtime:second".to_string());
        let selected_occurrence = second
            .occurrence_id(CandidateSourceClass::CurrentGeneration)
            .expect("occurrence id")
            .expect("selected occurrence");

        let entry = SelectionDecisionEntry::new_with_traversal_identity(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(subject.clone()),
            Some(selected_occurrence.clone()),
            None,
            vec![first, second],
            vec![
                TraversalCandidateSource::CurrentGeneration,
                TraversalCandidateSource::CurrentGeneration,
            ],
            Vec::new(),
            None,
            selection_decision("child-a", "branch-a"),
        )
        .expect("selection entry should use selected occurrence to disambiguate");

        assert_eq!(entry.selected_candidate, Some(subject));
        assert_eq!(entry.selected_occurrence_id, Some(selected_occurrence));
        assert_eq!(
            entry
                .verify_candidate_set_commitment()
                .expect("candidate set verifies"),
            Some(true)
        );
    }

    #[test]
    fn selection_decision_entry_rejects_duplicate_occurrence_ids() {
        let first = evaluation_payload("child-a", "branch-a", 0);
        let mut second = evaluation_payload("child-b", "branch-b", 1);
        second.sealed_evidence = first.sealed_evidence.clone();

        let err = SelectionDecisionEntry::new_with_traversal_identity(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(first.candidate.clone()),
            first
                .occurrence_id(CandidateSourceClass::CurrentGeneration)
                .expect("occurrence id"),
            None,
            vec![first, second],
            vec![
                TraversalCandidateSource::CurrentGeneration,
                TraversalCandidateSource::CurrentGeneration,
            ],
            Vec::new(),
            None,
            selection_decision("child-a", "branch-a"),
        )
        .expect_err("duplicate occurrence ids should be rejected");

        assert!(matches!(err, HistoryError::InvalidSelectionDecision { .. }));
        assert!(
            err.to_string()
                .contains("duplicate candidate occurrence id")
        );
    }

    #[test]
    fn selection_decision_entry_rejects_selected_membership_for_wrong_root() {
        let considered = vec![evaluation_payload("child-a", "branch-a", 0)];
        let occurrence = considered[0]
            .occurrence_id(CandidateSourceClass::CurrentGeneration)
            .expect("occurrence id")
            .expect("occurrence");
        let wrong_root = CandidateSetRoot(HistoryHash::of_bytes(b"wrong-candidate-set-root"));
        let wrong_membership =
            CandidateMembershipId::new(&occurrence, &wrong_root).expect("wrong membership id");

        let err = SelectionDecisionEntry::new_with_traversal_identity(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(considered[0].candidate.clone()),
            Some(occurrence),
            Some(wrong_membership),
            considered,
            vec![TraversalCandidateSource::CurrentGeneration],
            Vec::new(),
            None,
            selection_decision("child-a", "branch-a"),
        )
        .expect_err("wrong-root selected membership should be rejected");

        assert!(matches!(err, HistoryError::InvalidSelectionDecision { .. }));
        assert!(err.to_string().contains("selected membership is absent"));
    }

    #[test]
    fn selection_decision_entry_rejects_selected_candidate_outside_considered() {
        let decision = crate::successor_selection::SuccessorDecision {
            procedure_id: crate::successor_selection::PROCEDURE_ID.to_string(),
            candidate_node_id: "child-a".to_string(),
            selected_branch_id: Some("branch-a".to_string()),
            branch_disposition: "keep".to_string(),
            outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        };
        let considered = vec![EvaluationPayload {
            schema_version: 1,
            candidate: SubjectRef::new("candidate:child-b:plan_index=1"),
            procedure: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            selection_input: None,
            selection_input_hash: None,
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: None,
            artifact: None,
            surface_attempt: None,
        }];

        let err = SelectionDecisionEntry::new(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(SubjectRef::new("candidate:child-a:plan_index=0")),
            considered,
            Vec::new(),
            decision,
        )
        .expect_err("selection entry should reject unconsidered candidate");

        assert!(matches!(err, HistoryError::InvalidSelectionDecision { .. }));
    }

    #[test]
    fn selection_decision_entry_rejects_selected_candidate_node_mismatch() {
        let decision = crate::successor_selection::SuccessorDecision {
            procedure_id: crate::successor_selection::PROCEDURE_ID.to_string(),
            candidate_node_id: "child-a".to_string(),
            selected_branch_id: Some("branch-a".to_string()),
            branch_disposition: "keep".to_string(),
            outcome: crate::successor_selection::decision::SuccessorOutcome::Accepted,
            findings: Vec::new(),
            rationale: Vec::new(),
        };
        let selected = SubjectRef::new("candidate:selected");
        let considered = vec![EvaluationPayload {
            schema_version: 2,
            candidate: selected.clone(),
            procedure: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            selection_input: None,
            selection_input_hash: None,
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: Some(SealedCandidateEvidence {
                schema_version: 2,
                coordinate: CandidateCoordinate {
                    node_id: "child-b".to_string(),
                    parent_node_id: None,
                    branch_id: Some("branch-a".to_string()),
                    generation: Some(2),
                    plan_index: Some(0),
                    primary_runtime_id: None,
                },
                lifecycle: CandidateLifecycle {
                    planner_outcome: "done".to_string(),
                    node_status: "completed".to_string(),
                },
                evaluations: Vec::new(),
                runtimes: Vec::new(),
                branches: Vec::new(),
                extra_document_citations: Vec::new(),
                extra_journal_citations: Vec::new(),
                child_diagnostics: Vec::new(),
            }),
            artifact: None,
            surface_attempt: None,
        }];

        let err = SelectionDecisionEntry::new(
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            SelectionScope::new("generation_local:test"),
            Some(selected),
            considered,
            Vec::new(),
            decision,
        )
        .expect_err("selection entry should reject selected node mismatch");

        assert!(matches!(err, HistoryError::InvalidSelectionDecision { .. }));
    }

    #[test]
    fn evaluation_payload_without_bounded_selection_input_is_not_decision_grade() {
        let payload = EvaluationPayload {
            schema_version: 1,
            candidate: SubjectRef::new("candidate:n1:plan_index=0"),
            procedure: ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
            selection_input: None,
            selection_input_hash: None,
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: None,
            artifact: None,
            surface_attempt: None,
        };
        let grade = payload.decision_grade_eligibility();
        assert!(!grade.eligible);
        assert!(
            grade
                .identity_gaps
                .iter()
                .any(|gap| gap == "missing_selection_input_or_hash")
        );
    }

    #[test]
    fn evaluation_payload_decision_grade_when_identity_surface_complete() {
        use std::path::PathBuf;

        use crate::BranchDisposition;
        use crate::successor_selection::{CandidateRef, SelectionInput};

        let candidate = CandidateRef {
            node_id: "n1".to_string(),
            branch_id: "b1".to_string(),
            generation: 2,
        };
        let input = SelectionInput::new(
            candidate,
            BranchDisposition::Keep,
            PathBuf::from("evaluations/b1.json"),
            Vec::new(),
        );
        let sealed = SealedCandidateEvidence {
            schema_version: 2,
            coordinate: CandidateCoordinate {
                node_id: "n1".to_string(),
                parent_node_id: None,
                branch_id: Some("b1".to_string()),
                generation: Some(2),
                plan_index: Some(0),
                primary_runtime_id: Some("rt-a".to_string()),
            },
            lifecycle: CandidateLifecycle {
                planner_outcome: "done".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: vec![SealedEvaluationEvidence {
                branch_id: "b1".to_string(),
                evaluation_procedure_id: Some(
                    crate::cli::prototype1_state::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID
                        .to_string(),
                ),
                evaluator_identity: Some(test_evaluator_identity()),
                eval_set_identity: Some(test_eval_set_identity()),
                evaluation_artifact_citation: None,
                overall_disposition: Some("keep".to_string()),
                primary_report_citation: SealedEvidenceCitation {
                    ref_id: "file:eval.json".to_string(),
                    content_hash: None,
                    record_name: Some("eval".to_string()),
                },
                compared_runs: Vec::new(),
            }],
            runtimes: test_runtime_evidence("rt-a"),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        };
        let payload = EvaluationPayload::builder(
            SubjectRef::new("candidate:n1:plan_index=0"),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input hash")
        .sealed_candidate_evidence(sealed)
        .candidate_artifact(test_candidate_artifact("n1", "b1", 2))
        .build();

        let grade = payload.decision_grade_eligibility();
        assert!(grade.eligible, "unexpected gaps: {:?}", grade.identity_gaps);
    }

    #[test]
    fn evaluation_payload_rejects_decision_grade_when_evaluation_procedure_mismatches() {
        use std::path::PathBuf;

        use crate::BranchDisposition;
        use crate::successor_selection::{CandidateRef, SelectionInput};

        let input = SelectionInput::new(
            CandidateRef {
                node_id: "n1".to_string(),
                branch_id: "b1".to_string(),
                generation: 2,
            },
            BranchDisposition::Keep,
            PathBuf::from("evaluations/b1.json"),
            Vec::new(),
        );
        let sealed = SealedCandidateEvidence {
            schema_version: 2,
            coordinate: CandidateCoordinate {
                node_id: "n1".to_string(),
                parent_node_id: None,
                branch_id: Some("b1".to_string()),
                generation: Some(2),
                plan_index: Some(0),
                primary_runtime_id: Some("rt-a".to_string()),
            },
            lifecycle: CandidateLifecycle {
                planner_outcome: "done".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: vec![SealedEvaluationEvidence {
                branch_id: "b1".to_string(),
                evaluation_procedure_id: Some("other-procedure".to_string()),
                evaluator_identity: Some(test_evaluator_identity()),
                eval_set_identity: Some(test_eval_set_identity()),
                evaluation_artifact_citation: None,
                overall_disposition: Some("keep".to_string()),
                primary_report_citation: SealedEvidenceCitation {
                    ref_id: "file:eval.json".to_string(),
                    content_hash: None,
                    record_name: Some("eval".to_string()),
                },
                compared_runs: Vec::new(),
            }],
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        };
        let payload = EvaluationPayload::builder(
            SubjectRef::new("candidate:n1:plan_index=0"),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input hash")
        .sealed_candidate_evidence(sealed)
        .candidate_artifact(test_candidate_artifact("n1", "b1", 2))
        .build();

        let grade = payload.decision_grade_eligibility();
        assert!(!grade.eligible);
        assert!(
            grade.identity_gaps.iter().any(
                |gap| gap.starts_with("sealed_evaluations[0].evaluation_procedure_id_mismatch")
            ),
            "unexpected gaps: {:?}",
            grade.identity_gaps
        );
    }

    #[test]
    fn evaluation_payload_rejects_decision_grade_when_evaluation_branch_does_not_bind() {
        use std::path::PathBuf;

        use crate::BranchDisposition;
        use crate::successor_selection::{CandidateRef, SelectionInput};

        let input = SelectionInput::new(
            CandidateRef {
                node_id: "n1".to_string(),
                branch_id: "b1".to_string(),
                generation: 2,
            },
            BranchDisposition::Keep,
            PathBuf::from("evaluations/b1.json"),
            Vec::new(),
        );
        let sealed = SealedCandidateEvidence {
            schema_version: 2,
            coordinate: CandidateCoordinate {
                node_id: "n1".to_string(),
                parent_node_id: None,
                branch_id: Some("b1".to_string()),
                generation: Some(2),
                plan_index: Some(0),
                primary_runtime_id: Some("rt-a".to_string()),
            },
            lifecycle: CandidateLifecycle {
                planner_outcome: "done".to_string(),
                node_status: "completed".to_string(),
            },
            evaluations: vec![SealedEvaluationEvidence {
                branch_id: "b2".to_string(),
                evaluation_procedure_id: Some(
                    crate::cli::prototype1_state::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID
                        .to_string(),
                ),
                evaluator_identity: Some(test_evaluator_identity()),
                eval_set_identity: Some(test_eval_set_identity()),
                evaluation_artifact_citation: None,
                overall_disposition: Some("keep".to_string()),
                primary_report_citation: SealedEvidenceCitation {
                    ref_id: "file:eval.json".to_string(),
                    content_hash: None,
                    record_name: Some("eval".to_string()),
                },
                compared_runs: Vec::new(),
            }],
            runtimes: Vec::new(),
            branches: Vec::new(),
            extra_document_citations: Vec::new(),
            extra_journal_citations: Vec::new(),
            child_diagnostics: Vec::new(),
        };
        let payload = EvaluationPayload::builder(
            SubjectRef::new("candidate:n1:plan_index=0"),
            ProcedureRef::new(crate::successor_selection::PROCEDURE_ID),
        )
        .selection_input(input)
        .expect("selection input hash")
        .sealed_candidate_evidence(sealed)
        .candidate_artifact(test_candidate_artifact("n1", "b1", 2))
        .build();

        let grade = payload.decision_grade_eligibility();
        assert!(!grade.eligible);
        assert!(
            grade.identity_gaps.iter().any(|gap| {
                gap == "sealed_evaluation_for_candidate_branch_missing:branch_id=b1"
            }),
            "unexpected gaps: {:?}",
            grade.identity_gaps
        );
    }

    #[test]
    fn surface_evidence_checked_persists_typed_check_evidence() {
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let evidence = test_surface_evidence(
            target_relpath.clone(),
            "artifact:base",
            "artifact:after",
            "patch:surface",
        );

        assert_eq!(evidence.schema_version, 5);
        let grant = evidence.grant.as_ref().expect("typed grant evidence");
        assert_eq!(grant.policy, ProcedureRef::new("policy:surface:test"));
        match &grant.coordinate {
            grant::AnyCoordinate::Checked(coordinate) => {
                assert_eq!(
                    coordinate.runtime_id(),
                    crate::loop_graph::RuntimeId(uuid::Uuid::nil())
                );
                assert_eq!(
                    coordinate.target_artifact_id(),
                    &ArtifactId::new("artifact:base")
                );
            }
            grant::AnyCoordinate::Admitted(_) => {
                panic!("checked surface evidence should keep checked grant coordinate")
            }
        }
        let check = evidence.check.as_ref().expect("typed check evidence");
        assert_eq!(check.base.artifact_id, ArtifactId::new("artifact:base"));
        assert_eq!(check.after.artifact_id, ArtifactId::new("artifact:after"));
        assert_eq!(check.patch_id, PatchId::new("patch:surface"));
        assert_eq!(check.touches_digest, evidence.touches_digest);
        assert_eq!(check.delta_digest, evidence.delta_digest);
        assert_eq!(check.delta_id, evidence.delta_id);
        assert_eq!(target_relpath, evidence.target_relpath);
        evidence
            .verify_integrity()
            .expect("surface evidence integrity");
    }

    #[test]
    fn candidate_artifact_with_surface_binds_grant_authority_from_admitted_candidate() {
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let mut artifact = test_candidate_artifact("node-a", "branch-a", 2);
        artifact.node.instance_id = crate::loop_graph::RuntimeId(uuid::Uuid::nil()).to_string();
        artifact.node.operation_target = Some(crate::loop_graph::OperationTarget::Artifact {
            artifact_id: ArtifactId::new("artifact:base"),
        });
        artifact.node.base_artifact_id = Some(ArtifactId::new("artifact:base"));
        artifact.node.patch_id = Some(PatchId::new("patch:surface"));
        artifact.node.derived_artifact_id = Some(ArtifactId::new("artifact:after"));
        artifact.resolved.branch.patch_id = Some(PatchId::new("patch:surface"));
        artifact.resolved.branch.derived_artifact_id = Some(ArtifactId::new("artifact:after"));

        let artifact = artifact.with_surface(test_surface_evidence(
            target_relpath.clone(),
            "artifact:base",
            "artifact:after",
            "patch:surface",
        ));
        let surface = artifact.surface.as_ref().expect("surface evidence");
        let grant = surface.grant.as_ref().expect("surface grant evidence");

        assert_eq!(surface.schema_version, 5);
        assert_eq!(grant.policy, ProcedureRef::new("policy:surface:test"));
        match &grant.coordinate {
            grant::AnyCoordinate::Checked(_) => {
                panic!("candidate binding should refine the grant to admitted coordinates")
            }
            grant::AnyCoordinate::Admitted(coordinate) => {
                let expected_runtime = crate::loop_graph::RuntimeId(uuid::Uuid::nil()).to_string();
                assert_eq!(
                    coordinate.target_artifact_id(),
                    &ArtifactId::new("artifact:base")
                );
                assert_eq!(coordinate.candidate().node_id, "node-a");
                assert_eq!(
                    coordinate.candidate().primary_runtime_id.as_deref(),
                    Some(expected_runtime.as_str())
                );
            }
        }
        assert_eq!(grant.writable.target_relpath, target_relpath);
        surface
            .verify_integrity()
            .expect("surface evidence integrity");
    }

    #[test]
    #[should_panic(expected = "checked surface grant runtime")]
    fn candidate_artifact_with_surface_rejects_checked_admitted_runtime_mismatch() {
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let mut artifact = test_candidate_artifact("node-a", "branch-a", 2);
        artifact.node.instance_id =
            crate::loop_graph::RuntimeId(uuid::Uuid::from_u128(1)).to_string();
        artifact.node.operation_target = Some(crate::loop_graph::OperationTarget::Artifact {
            artifact_id: ArtifactId::new("artifact:base"),
        });
        artifact.node.base_artifact_id = Some(ArtifactId::new("artifact:base"));
        artifact.node.patch_id = Some(PatchId::new("patch:surface"));
        artifact.node.derived_artifact_id = Some(ArtifactId::new("artifact:after"));
        artifact.resolved.branch.patch_id = Some(PatchId::new("patch:surface"));
        artifact.resolved.branch.derived_artifact_id = Some(ArtifactId::new("artifact:after"));

        let _ = artifact.with_surface(test_surface_evidence(
            target_relpath,
            "artifact:base",
            "artifact:after",
            "patch:surface",
        ));
    }

    #[test]
    fn candidate_artifact_with_surface_preserves_checked_grant_without_admitted_binding() {
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/read_file.md");
        let artifact =
            test_candidate_artifact("node-a", "branch-a", 2).with_surface(test_surface_evidence(
                target_relpath.clone(),
                "artifact:base",
                "artifact:after",
                "patch:surface",
            ));
        let surface = artifact.surface.as_ref().expect("surface evidence");

        assert_eq!(surface.schema_version, 5);
        let grant = surface.grant.as_ref().expect("surface grant evidence");
        match &grant.coordinate {
            grant::AnyCoordinate::Checked(coordinate) => {
                assert_eq!(
                    coordinate.runtime_id(),
                    crate::loop_graph::RuntimeId(uuid::Uuid::nil())
                );
                assert_eq!(
                    coordinate.target_artifact_id(),
                    &ArtifactId::new("artifact:base")
                );
            }
            grant::AnyCoordinate::Admitted(_) => {
                panic!("candidate facts should not invent admitted grant coordinates")
            }
        }
        assert_eq!(surface.target_relpath, target_relpath);
        surface
            .verify_integrity()
            .expect("surface evidence integrity");
    }

    #[test]
    fn block_claims_store_flat_fields_but_extract_nested_policy_claim() {
        let claims = block::Claims::empty_unchecked().with_policy(policy_claim("policy.toml"));
        let extracted = claims
            .policy::<TestLocator>()
            .expect("policy claim extracts");

        assert_eq!(extracted.admission(), &admission());
        assert_eq!(extracted.claim().witness(), &ruler_witness());
        assert_eq!(
            extracted.claim().claim().key(),
            &ArtifactPath::new("policy.toml")
        );
        extracted
            .claim()
            .claim()
            .verify_with(&TestLocator)
            .expect("policy claim verifies through locator");
    }

    #[test]
    fn block_claims_store_flat_fields_but_extract_nested_artifact_claim() {
        let claims =
            block::Claims::empty_unchecked().with_artifact(artifact_claim("tree:successor"));
        let extracted = claims
            .artifact::<ArtifactLocator>()
            .expect("artifact claim extracts");

        assert_eq!(extracted.admission(), &admission());
        assert_eq!(extracted.claim().witness(), &ruler_witness());
        assert_eq!(extracted.claim().claim().key(), &tree_key("tree:successor"));
        extracted
            .claim()
            .claim()
            .verify_with(&ArtifactLocator)
            .expect("artifact claim verifies through locator");
    }

    #[test]
    fn block_claims_are_committed_to_sealed_block_hash() {
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

        let first_claims =
            block::Claims::empty_unchecked().with_policy(policy_claim("policy-a.toml"));
        let second_claims =
            block::Claims::empty_unchecked().with_policy(policy_claim("policy-b.toml"));
        let first = seal_with_transition_and_claims(first, "transition:crown-lock", first_claims);
        let second =
            seal_with_transition_and_claims(second, "transition:crown-lock", second_claims);

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
            opened_from_state: HistoryStateRoot::test("state:non-genesis-without-parents"),
            regime: Regime::prototype1_baseline(1),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                BlockHash::from(HistoryHash::of_bytes(b"parent")),
            )),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:base")),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            surface: surface_commitment("non-genesis-without-parents"),
            opened_at: at(10),
        })
        .expect_err("non-genesis without parent must fail");

        assert!(matches!(err, HistoryError::NonGenesisWithoutParents));
    }

    #[test]
    fn genesis_block_requires_bootstrap_authority() {
        let err = Block::open(OpenBlock {
            lineage_id: LineageId::new("lineage:a"),
            block_height: 0,
            parent_block_hashes: Vec::new(),
            opened_from_state: HistoryStateRoot::test("state:genesis-without-bootstrap"),
            regime: Regime::prototype1_baseline(0),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                BlockHash::from(HistoryHash::of_bytes(b"parent")),
            )),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:base")),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            surface: surface_commitment("genesis-without-bootstrap"),
            opened_at: at(10),
        })
        .expect_err("genesis without bootstrap authority must fail");

        assert!(matches!(err, HistoryError::GenesisWithoutBootstrap));
    }

    #[test]
    fn child_block_rejects_bootstrap_authority() {
        let parent_hash = BlockHash::from(HistoryHash::of_bytes(b"parent"));
        let err = Block::open(OpenBlock {
            lineage_id: LineageId::new("lineage:a"),
            block_height: 1,
            parent_block_hashes: vec![parent_hash],
            opened_from_state: HistoryStateRoot::test("state:bootstrap-on-child"),
            regime: Regime::prototype1_baseline(1),
            opening_authority: OpeningAuthority::Genesis(GenesisAuthority::new(
                ProcedureRef::new("policy:bootstrap"),
                tree_key("tree:genesis"),
                ParentIdentityRef::new(EvidenceRef::new("parent-identity:genesis")),
            )),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:base")),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            surface: surface_commitment("bootstrap-on-child"),
            opened_at: at(10),
        })
        .expect_err("non-genesis block must not use bootstrap authority");

        assert!(matches!(err, HistoryError::GenesisAuthorityOnChild));
    }

    #[test]
    fn predecessor_authority_must_cite_parent_hash() {
        let parent_hash = BlockHash::from(HistoryHash::of_bytes(b"parent"));
        let other_hash = BlockHash::from(HistoryHash::of_bytes(b"other"));
        let err = Block::open(OpenBlock {
            lineage_id: LineageId::new("lineage:a"),
            block_height: 1,
            parent_block_hashes: vec![parent_hash],
            opened_from_state: HistoryStateRoot::test("state:wrong-predecessor"),
            regime: Regime::prototype1_baseline(1),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(other_hash)),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new("artifact:base")),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            surface: surface_commitment("wrong-predecessor"),
            opened_at: at(10),
        })
        .expect_err("predecessor authority must match a cited parent hash");

        assert!(matches!(err, HistoryError::OpeningPredecessorNotParent));
    }

    #[test]
    fn genesis_tree_key_is_committed_to_block_hash() {
        let block_id = BlockId::new();
        let entry_id = EntryId::new();
        let mut first = Block::open_with_block_id(
            block_id,
            OpenBlock {
                lineage_id: LineageId::new("lineage:a"),
                block_height: 0,
                parent_block_hashes: Vec::new(),
                opened_from_state: HistoryStateRoot::test("state:genesis-tree"),
                regime: Regime::prototype1_baseline(0),
                opening_authority: OpeningAuthority::Genesis(GenesisAuthority::new(
                    ProcedureRef::new("policy:bootstrap"),
                    tree_key("tree:a"),
                    ParentIdentityRef::new(EvidenceRef::new("parent-identity:genesis")),
                )),
                opened_by: actor("parent"),
                opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new(
                    "artifact:base",
                )),
                ruling_authority: actor("ruler"),
                policy_ref: ProcedureRef::new("policy:test"),
                surface: surface_commitment("genesis-tree-a"),
                opened_at: at(10),
            },
        )
        .expect("first genesis");
        first
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit first");

        let mut second = Block::open_with_block_id(
            block_id,
            OpenBlock {
                lineage_id: LineageId::new("lineage:a"),
                block_height: 0,
                parent_block_hashes: Vec::new(),
                opened_from_state: HistoryStateRoot::test("state:genesis-tree"),
                regime: Regime::prototype1_baseline(0),
                opening_authority: OpeningAuthority::Genesis(GenesisAuthority::new(
                    ProcedureRef::new("policy:bootstrap"),
                    tree_key("tree:b"),
                    ParentIdentityRef::new(EvidenceRef::new("parent-identity:genesis")),
                )),
                opened_by: actor("parent"),
                opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new(
                    "artifact:base",
                )),
                ruling_authority: actor("ruler"),
                policy_ref: ProcedureRef::new("policy:test"),
                surface: surface_commitment("genesis-tree-b"),
                opened_at: at(10),
            },
        )
        .expect("second genesis");
        second
            .admit(proposed_entry_with_id(entry_id), actor("admitter"))
            .expect("admit second");

        let first = seal(first);
        let second = seal(second);

        assert_ne!(first.block_hash(), second.block_hash());
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
                additional_parent_block_hashes: vec![merge_parent],
                opened_from_state: HistoryStateRoot::test("state:merge"),
                opened_by: actor("successor"),
                opened_from_artifact: ArtifactRef::from_artifact_id(ArtifactId::new(
                    "artifact:successor",
                )),
                ruling_authority: actor("successor"),
                policy_ref: ProcedureRef::new("policy:next"),
                surface: surface_commitment("successor-merge"),
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
            predecessor_hash,
        );
        let block = open_block(1, vec![predecessor_hash]);

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
            EntryPayload::SelectionDecision(_) => {
                panic!("ingress import must be committed in entry payload");
            }
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
            predecessor_hash,
        );
        let target = open_block(1, vec![predecessor_hash]);
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
