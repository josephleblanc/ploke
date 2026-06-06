/// Read-only History projection over verified sealed blocks.
///
/// This is the narrow `History::candidates(scope)` surface for traversal
/// policies. It reads admitted selection entries from History and returns
/// candidate payloads with their block/entry provenance and candidate-set
/// membership proofs where the sealed decision carried them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct History {
    store: FsBlockStore,
}

impl History {
    pub(crate) fn new(store: FsBlockStore) -> Self {
        Self { store }
    }

    pub(crate) fn for_campaign_manifest(manifest_path: &Path) -> Self {
        Self::new(FsBlockStore::for_campaign_manifest(manifest_path))
    }

    pub(crate) fn candidates(
        &self,
        scope: &SelectionScope,
    ) -> Result<HistoryCandidates, BlockStoreError> {
        let mut candidates = Vec::new();
        for (_line_index, block) in self.store.load_segment_verified_blocks()? {
            let block_hash = *block.block_hash();
            let block_height = block.block_height();
            let lineage_id = block.lineage_id().clone();

            for entry in block.entries() {
                let Some(selection) = entry.selection_decision() else {
                    continue;
                };
                if !selection.contributes_candidates_to_history_projection() {
                    continue;
                }
                if !scope.includes(&selection.scope) {
                    continue;
                }
                if entry.verify_selection_decision_observation()? != Some(true) {
                    return Err(HistoryError::InvalidSelectionDecision {
                        detail: format!(
                            "selection entry {} payload hash does not match decision payload",
                            entry.entry_id()
                        ),
                    }
                    .into());
                }
                if !selection.verify_considered_order_hash()? {
                    return Err(HistoryError::InvalidSelectionDecision {
                        detail: format!(
                            "selection entry {} considered_order_hash mismatch",
                            entry.entry_id()
                        ),
                    }
                    .into());
                }
                if selection.verify_candidate_set_commitment()? == Some(false) {
                    return Err(HistoryError::InvalidSelectionDecision {
                        detail: format!(
                            "selection entry {} candidate_set commitment mismatch",
                            entry.entry_id()
                        ),
                    }
                    .into());
                }

                for (index, payload) in selection.considered.iter().enumerate() {
                    if selection.procedure_or_policy.as_str()
                        == crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
                    {
                        match selection.considered_sources.get(index) {
                            Some(TraversalCandidateSource::CurrentGeneration) => {}
                            _ => continue,
                        }
                    }
                    let payload_hash = payload.payload_hash()?;
                    let candidate_set_membership =
                        selection.candidate_set_membership_for_payload(index, payload)?;
                    candidates.push(HistoryCandidate {
                        source: HistoryCandidateSource {
                            block_hash,
                            block_height,
                            lineage_id: lineage_id.clone(),
                            entry_id: entry.entry_id(),
                        },
                        decision_scope: selection.scope.clone(),
                        selected_by_decision: selection
                            .payload_selected_by_decision(candidate_set_membership, payload),
                        candidate_set_root: selection
                            .candidate_set
                            .as_ref()
                            .map(|commitment| commitment.root.clone()),
                        candidate_set_membership: candidate_set_membership.cloned(),
                        payload_hash,
                        payload: payload.clone(),
                    });
                }
            }
        }

        Ok(HistoryCandidates {
            scope: scope.clone(),
            candidates,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HistoryCandidates {
    pub(crate) scope: SelectionScope,
    pub(crate) candidates: Vec<HistoryCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HistoryCandidate {
    pub(crate) source: HistoryCandidateSource,
    pub(crate) decision_scope: SelectionScope,
    pub(crate) selected_by_decision: bool,
    pub(crate) payload: EvaluationPayload,
    pub(crate) payload_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_set_root: Option<CandidateSetRoot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_set_membership: Option<CandidateSetMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct HistoryCandidateSource {
    pub(crate) block_hash: BlockHash,
    pub(crate) block_height: u64,
    pub(crate) lineage_id: LineageId,
    pub(crate) entry_id: EntryId,
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

    pub(crate) fn as_str(&self) -> &str {
        &self.value
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

    pub(crate) fn as_str(&self) -> &str {
        &self.value
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

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
}

/// Recoverable artifact identity used by History block boundaries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "artifact_ref::Tagged", try_from = "artifact_ref::Repr")]
pub(crate) enum ArtifactRef {
    Artifact {
        id: ArtifactRefId,
        artifact_id: ArtifactId,
    },
    Branch {
        id: ArtifactRefId,
        branch_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct ArtifactRefId(String);

mod artifact_ref {
    use super::{ArtifactId, ArtifactRefId};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) enum Repr {
        Tagged(Tagged),
        Legacy(Legacy),
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "kind")]
    pub(crate) enum Tagged {
        Artifact {
            id: ArtifactRefId,
            artifact_id: ArtifactId,
        },
        Branch {
            id: ArtifactRefId,
            branch_id: String,
        },
    }

    #[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
    pub(crate) struct Legacy {
        pub(crate) value: String,
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

impl ArtifactRef {
    pub(crate) fn from_artifact_id(artifact_id: ArtifactId) -> Self {
        Self::Artifact {
            id: artifact_ref_id("artifact", artifact_id.as_str()),
            artifact_id,
        }
    }

    pub(crate) fn from_branch_id(branch_id: impl Into<String>) -> Self {
        let branch_id = branch_id.into();
        Self::Branch {
            id: artifact_ref_id("branch", branch_id.as_str()),
            branch_id,
        }
    }

    pub(crate) fn id(&self) -> &ArtifactRefId {
        match self {
            Self::Artifact { id, .. } | Self::Branch { id, .. } => id,
        }
    }

    pub(crate) fn artifact_id(&self) -> Option<&ArtifactId> {
        match self {
            Self::Artifact { artifact_id, .. } => Some(artifact_id),
            Self::Branch { .. } => None,
        }
    }

    pub(crate) fn branch_id(&self) -> Option<&str> {
        match self {
            Self::Artifact { .. } => None,
            Self::Branch { branch_id, .. } => Some(branch_id.as_str()),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Artifact { artifact_id, .. } => artifact_id.as_str(),
            Self::Branch { branch_id, .. } => branch_id.as_str(),
        }
    }
}

impl From<ArtifactRef> for artifact_ref::Tagged {
    fn from(value: ArtifactRef) -> Self {
        match value {
            ArtifactRef::Artifact { id, artifact_id } => Self::Artifact { id, artifact_id },
            ArtifactRef::Branch { id, branch_id } => Self::Branch { id, branch_id },
        }
    }
}

impl TryFrom<artifact_ref::Repr> for ArtifactRef {
    type Error = String;

    fn try_from(value: artifact_ref::Repr) -> Result<Self, Self::Error> {
        match value {
            artifact_ref::Repr::Tagged(tagged) => match tagged {
                artifact_ref::Tagged::Artifact { id, artifact_id } => {
                    let expected = artifact_ref_id("artifact", artifact_id.as_str());
                    if id != expected {
                        return Err(format!(
                            "artifact ref id mismatch: expected {}, got {}",
                            expected.as_str(),
                            id.as_str()
                        ));
                    }
                    Ok(Self::Artifact { id, artifact_id })
                }
                artifact_ref::Tagged::Branch { id, branch_id } => {
                    let expected = artifact_ref_id("branch", branch_id.as_str());
                    if id != expected {
                        return Err(format!(
                            "artifact ref id mismatch: expected {}, got {}",
                            expected.as_str(),
                            id.as_str()
                        ));
                    }
                    Ok(Self::Branch { id, branch_id })
                }
            },
            artifact_ref::Repr::Legacy(artifact_ref::Legacy { value }) => {
                if let Some(artifact_id) = value.strip_prefix("artifact:") {
                    return Ok(Self::from_artifact_id(ArtifactId::new(artifact_id)));
                }
                if let Some(branch_id) = value.strip_prefix("branch:") {
                    return Ok(Self::from_branch_id(branch_id));
                }
                Err(format!("unsupported legacy artifact ref value: {value}"))
            }
        }
    }
}

impl ArtifactRefId {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

fn artifact_ref_id(kind: &str, value: &str) -> ArtifactRefId {
    let mut hasher = Sha256::new();
    hasher.update("prototype1.history.artifact_ref.v1");
    hasher.update([0]);
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(value.as_bytes());
    ArtifactRefId(format!("{:x}", hasher.finalize()))
}

/// Marker states for the partitioned Artifact surface committed by a block.
///
/// The markers keep the partition role in the type rather than in names such
/// as `immutable_surface_root_before`. Ordinary succession policy interprets
/// the roots: for now, the policy-bearing immutable surface must stay fixed;
/// the mutated surface is the bounded edit target; and the ambient surface is
/// the rest of the declared Artifact surface that was not held immutable and
/// was not the edit target.
pub(crate) mod surface {
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Immutable;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Mutated;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Ambient;

    /// Marker for a bounded surface named by a claim or policy-interpreted
    /// material locator.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Bounded;
}

/// Root digest for a declared Artifact surface partition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceRoot {
    hash: HistoryHash,
}

impl SurfaceRoot {
    fn new(hash: HistoryHash) -> Self {
        Self { hash }
    }

    pub(crate) fn hash(&self) -> &HistoryHash {
        &self.hash
    }
}

/// Digest commitment to one partition of an Artifact surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub(crate) struct Surface<P> {
    root: SurfaceRoot,
    #[serde(skip)]
    _partition: PhantomData<fn() -> P>,
}

impl<P> Surface<P> {
    fn new(root: SurfaceRoot) -> Self {
        Self {
            root,
            _partition: PhantomData,
        }
    }

    fn root(&self) -> &SurfaceRoot {
        &self.root
    }
}

/// Before/after commitment for a surface partition that policy compares.
///
/// This is a static reconstruction witness. It is meant to be computed by
/// checking out Artifacts and hashing declared surfaces, not by executing the
/// candidate Runtime.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub(crate) struct SurfaceDelta<P> {
    before: Surface<P>,
    after: Surface<P>,
}

impl<P> SurfaceDelta<P> {
    fn new(before: Surface<P>, after: Surface<P>) -> Self {
        Self { before, after }
    }

    fn after(&self) -> &Surface<P> {
        &self.after
    }
}

/// Partitioned commitment to the Artifact transition admitted by a block.
///
/// `immutable` is one root, not a before/after pair. The rule that it must
/// match the expected immutable root is runtime policy, and that rule itself
/// belongs to the immutable authority surface. `mutated` and `ambient` are
/// before/after commitments so a verifier can reconstruct the candidate
/// transition without running the candidate code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceCommitment {
    immutable: Surface<surface::Immutable>,
    mutated: SurfaceDelta<surface::Mutated>,
    ambient: SurfaceDelta<surface::Ambient>,
}

impl SurfaceCommitment {
    fn new(
        immutable: Surface<surface::Immutable>,
        mutated: SurfaceDelta<surface::Mutated>,
        ambient: SurfaceDelta<surface::Ambient>,
    ) -> Self {
        Self {
            immutable,
            mutated,
            ambient,
        }
    }

    pub(crate) fn from_backend_roots(roots: super::backend::SurfaceRoots) -> Self {
        Self::new(
            Surface::new(SurfaceRoot::new(roots.immutable().clone())),
            SurfaceDelta::new(
                Surface::new(SurfaceRoot::new(roots.mutated_before().clone())),
                Surface::new(SurfaceRoot::new(roots.mutated_after().clone())),
            ),
            SurfaceDelta::new(
                Surface::new(SurfaceRoot::new(roots.ambient_before().clone())),
                Surface::new(SurfaceRoot::new(roots.ambient_after().clone())),
            ),
        )
    }

    pub(crate) fn from_artifact_surfaces(
        before: &ArtifactSurface,
        after: &ArtifactSurface,
    ) -> Result<Self, HistoryError> {
        if before.immutable != after.immutable {
            return Err(HistoryError::SurfaceMismatch {
                partition: "immutable",
                expected: before.immutable.root().hash().clone(),
                actual: after.immutable.root().hash().clone(),
            });
        }
        Ok(Self::new(
            before.immutable.clone(),
            SurfaceDelta::new(before.mutated.clone(), after.mutated.clone()),
            SurfaceDelta::new(before.ambient.clone(), after.ambient.clone()),
        ))
    }

    pub(crate) fn verify_current(&self, current: &Self) -> Result<(), HistoryError> {
        verify_surface_root(
            "immutable",
            self.immutable.root().hash(),
            current.immutable.root().hash(),
        )?;
        verify_surface_root(
            "mutated",
            self.mutated.after().root().hash(),
            current.mutated.after().root().hash(),
        )?;
        verify_surface_root(
            "ambient",
            self.ambient.after().root().hash(),
            current.ambient.after().root().hash(),
        )
    }
}

/// Backend-measured identity and surface profile for one recoverable Artifact.
///
/// This is not patch provenance. It is the artifact-local profile used to build
/// an authority transition from the current ruler artifact to a selected
/// successor artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ArtifactSurface {
    pub(crate) schema_version: u32,
    pub(crate) measurement: ProcedureRef,
    tree_key: TreeKeyHash,
    immutable: Surface<surface::Immutable>,
    mutated: Surface<surface::Mutated>,
    ambient: Surface<surface::Ambient>,
}

impl ArtifactSurface {
    pub(crate) fn from_backend_measurement(
        tree_key: TreeKeyHash,
        immutable: HistoryHash,
        mutated: HistoryHash,
        ambient: HistoryHash,
    ) -> Self {
        Self {
            schema_version: 1,
            measurement: ProcedureRef::new("prototype1:artifact-surface:v1"),
            tree_key,
            immutable: Surface::new(SurfaceRoot::new(immutable)),
            mutated: Surface::new(SurfaceRoot::new(mutated)),
            ambient: Surface::new(SurfaceRoot::new(ambient)),
        }
    }

    pub(crate) fn tree_key(&self) -> &TreeKeyHash {
        &self.tree_key
    }

    #[cfg(test)]
    pub(crate) fn test(label: &str) -> Self {
        let tree_key =
            TreeKeyHash::from_serialized_key(&format!("tree:{label}")).expect("test tree key hash");
        let immutable =
            HistoryHash::of_domain_json("prototype1.test.artifact_surface.immutable", &"shared")
                .expect("test immutable surface");
        let mutated =
            HistoryHash::of_domain_json("prototype1.test.artifact_surface.mutated", &label)
                .expect("test mutated surface");
        let ambient =
            HistoryHash::of_domain_json("prototype1.test.artifact_surface.ambient", &label)
                .expect("test ambient surface");
        Self::from_backend_measurement(tree_key, immutable, mutated, ambient)
    }
}

fn verify_surface_root(
    partition: &'static str,
    expected: &HistoryHash,
    actual: &HistoryHash,
) -> Result<(), HistoryError> {
    if expected == actual {
        return Ok(());
    }

    Err(HistoryError::SurfaceMismatch {
        partition,
        expected: expected.clone(),
        actual: actual.clone(),
    })
}

/// Typed content digest for a recoverable History object.
///
/// `Digest<T>` is stored evidence, not a recovery capability. The capability
/// lives in [`Locator<T>`], which defines the key and digest type used by a
/// particular artifact/tree context. Common local implementations can use this
/// generic digest as the associated `Locator<T>::Digest`, while a future backend
/// may use a different associated digest type without changing the conceptual
/// relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound = "")]
pub(crate) struct Digest<T> {
    hash: HistoryHash,
    #[serde(skip)]
    _target: PhantomData<fn() -> T>,
}

impl<T> Digest<T> {
    fn new(hash: HistoryHash) -> Self {
        Self {
            hash,
            _target: PhantomData,
        }
    }

    pub(crate) fn hash(&self) -> &HistoryHash {
        &self.hash
    }
}

/// Fallible recovery contract for an object `T` under a concrete context.
///
/// In Prototype 1 the intended context is a committed Artifact backed by the
/// workspace tree. A `Locator<T>` can locate `T` from its associated `Key` and
/// can compute the associated digest for the object found at that key. This is
/// the code-level form of the block invariant:
///
/// ```text
/// key --locate through Artifact/Tree--> T
/// key --digest through Artifact/Tree--> Digest(T)
/// ```
///
/// The trait is intentionally a capability, not stored block data.
pub(crate) trait Locator<T> {
    type Key;
    type Digest;
    type Error;

    fn locate(&self, key: &Self::Key) -> Result<T, Self::Error>;

    fn digest(&self, key: &Self::Key) -> Result<Self::Digest, Self::Error>;
}

/// Artifact state recoverable through the configured tree/backend boundary.
///
/// This is intentionally not a `*Ref` placeholder. In block claims, `Artifact`
/// is the target of a verifiable claim: the stored block keeps a flat key and
/// digest, while extraction reconstructs
/// `claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Artifact, L>>>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Artifact {
    _private: Private,
}

/// Locator for the current tree-key-backed Artifact commitment.
///
/// This is a small Prototype 1 bridge between the backend-owned clean tree key
/// and the generic `Locator<T>` claim boundary. It does not define the final
/// Artifact identity model; it only says that a `TreeKeyHash` admitted by the
/// current ruling authority can be rechecked as the same tree-key commitment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ArtifactLocator;

impl Locator<Artifact> for ArtifactLocator {
    type Key = TreeKeyHash;
    type Digest = Digest<Artifact>;
    type Error = HistoryError;

    fn locate(&self, _key: &Self::Key) -> Result<Artifact, Self::Error> {
        Ok(Artifact { _private: Private })
    }

    fn digest(&self, key: &Self::Key) -> Result<Self::Digest, Self::Error> {
        Ok(Digest::new(HistoryHash::of_domain_json(
            "prototype1.history.artifact.digest.v1",
            key,
        )?))
    }
}

/// Stored key plus digest for an object that can be checked through `L`.
///
/// This is the innermost envelope for block claims. It says only that `T` is
/// verifiable through locator context `L`; it does not say who observed it, who
/// admitted it, or whether a sealed block has made it durable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verifiable<T, L>
where
    L: Locator<T>,
{
    key: L::Key,
    digest: L::Digest,
    _target: PhantomData<fn() -> T>,
    _locator: PhantomData<fn() -> L>,
}

impl<T, L> Verifiable<T, L>
where
    L: Locator<T>,
{
    fn new(key: L::Key, digest: L::Digest) -> Self {
        Self {
            key,
            digest,
            _target: PhantomData,
            _locator: PhantomData,
        }
    }

    fn from_locator(locator: &L, key: L::Key) -> Result<(T, Self), VerifyError<L::Error>> {
        let item = locator.locate(&key).map_err(VerifyError::Locate)?;
        let digest = locator.digest(&key).map_err(VerifyError::Digest)?;
        Ok((item, Self::new(key, digest)))
    }

    pub(crate) fn key(&self) -> &L::Key {
        &self.key
    }

    pub(crate) fn digest(&self) -> &L::Digest {
        &self.digest
    }

    pub(crate) fn verify_with(&self, locator: &L) -> Result<T, VerifyError<L::Error>>
    where
        L::Digest: PartialEq,
    {
        let item = locator.locate(&self.key).map_err(VerifyError::Locate)?;
        let actual = locator.digest(&self.key).map_err(VerifyError::Digest)?;
        if actual != self.digest {
            return Err(VerifyError::DigestMismatch);
        }
        Ok(item)
    }

    fn into_parts(self) -> (L::Key, L::Digest) {
        (self.key, self.digest)
    }
}

/// Error while checking a [`Verifiable`] object through its locator contract.
#[derive(Debug, Error)]
pub(crate) enum VerifyError<E> {
    #[error("failed to locate verifiable object")]
    Locate(#[source] E),

    #[error("failed to compute verifiable object digest")]
    Digest(#[source] E),

    #[error("located object digest does not match the sealed expectation")]
    DigestMismatch,
}

impl VerifyError<HistoryError> {
    pub(crate) fn into_history_error(self) -> HistoryError {
        match self {
            Self::Locate(source) | Self::Digest(source) => source,
            Self::DigestMismatch => HistoryError::ClaimDigestMismatch,
        }
    }
}

/// A claim observed or produced by a witness.
///
/// This wrapper is justified only because it adds witness information that is
/// not present in the inner value. In the block model, the witness is the actor
/// and environment that produced the key/digest pair before admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Witnessed<W, X> {
    witness: W,
    claim: X,
}

impl<W, X> Witnessed<W, X> {
    fn new(witness: W, claim: X) -> Self {
        Self { witness, claim }
    }

    pub(crate) fn witness(&self) -> &W {
        &self.witness
    }

    pub(crate) fn claim(&self) -> &X {
        &self.claim
    }

    fn into_parts(self) -> (W, X) {
        (self.witness, self.claim)
    }
}

/// Claim-envelope types for block header facts.
///
/// This module avoids colliding with the existing entry-state `Admitted` while
/// preserving the intended nested shape:
///
/// ```text
/// claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>
/// ```
pub(crate) mod claim {
    /// A witnessed claim admitted into a block under authority/policy.
    ///
    /// This wrapper is distinct from sealing. Admission says the claim belongs
    /// in the block; sealing later commits the flattened fields into the block
    /// hash.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Admitted<A, X> {
        pub(super) admission: A,
        pub(super) claim: X,
    }

    impl<A, X> Admitted<A, X> {
        pub(super) fn new(admission: A, claim: X) -> Self {
            Self { admission, claim }
        }

        pub(crate) fn admission(&self) -> &A {
            &self.admission
        }

        pub(crate) fn claim(&self) -> &X {
            &self.claim
        }

        pub(super) fn into_parts(self) -> (A, X) {
            (self.admission, self.claim)
        }
    }
}

/// Witness for a block claim produced under the current ruling authority.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RulerWitness {
    ruler: ActorRef,
    environment: OperationalEnvironment,
    witnessed_at: RecordedAt,
}

impl RulerWitness {
    fn new(ruler: ActorRef, environment: OperationalEnvironment, witnessed_at: RecordedAt) -> Self {
        Self {
            ruler,
            environment,
            witnessed_at,
        }
    }
}

/// Admission decision for a witnessed block claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Admission {
    admitting_authority: ActorRef,
    policy: ProcedureRef,
    admitted_at: RecordedAt,
}

impl Admission {
    fn new(admitting_authority: ActorRef, policy: ProcedureRef, admitted_at: RecordedAt) -> Self {
        Self {
            admitting_authority,
            policy,
            admitted_at,
        }
    }
}

/// Flat stored form for one admitted, witnessed, verifiable claim.
///
/// Blocks store this shape so their serialized representation stays simple:
/// key, digest, witness, admission. Construction and extraction go through the
/// nested type boundary so callers cannot accidentally treat a bare digest as
/// admitted block evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct FlatClaim<Key, Digest> {
    key: Key,
    digest: Digest,
    witness: RulerWitness,
    admission: Admission,
}

impl<Key, Digest> FlatClaim<Key, Digest> {
    fn from_admitted<T, L>(
        admitted: claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>,
    ) -> Self
    where
        L: Locator<T, Key = Key, Digest = Digest>,
    {
        let (admission, witnessed) = admitted.into_parts();
        let (witness, verifiable) = witnessed.into_parts();
        let (key, digest) = verifiable.into_parts();
        Self {
            key,
            digest,
            witness,
            admission,
        }
    }

    fn to_admitted<T, L>(
        &self,
    ) -> claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>
    where
        Key: Clone,
        Digest: Clone,
        L: Locator<T, Key = Key, Digest = Digest>,
    {
        claim::Admitted::new(
            self.admission.clone(),
            Witnessed::new(
                self.witness.clone(),
                Verifiable::new(self.key.clone(), self.digest.clone()),
            ),
        )
    }
}

/// Marker for the policy artifact used to admit a block or block claim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Policy;

/// Strategy context committed by a block authority epoch.
///
/// Status recorded 2026-04-30 03:51 PDT: this is a deliberately small
/// placeholder for the periodic strategy schedule we expect History to witness.
/// It is not external authority and it is not an interpreted policy file. A
/// `Regime` records the runtime-frame strategy context in which a block was
/// opened: an absolute cycle step, a current phase, and a coarse risk profile.
///
/// The design hypothesis is that a long-running self-propagating process should
/// not optimize under one fixed risk posture. It should alternate among phases
/// such as expansion, evaluation, consolidation, and hardening, while allowing
/// dependent risk axes to shift with observed environment and History state.
/// Current code commits this context into block hashes only; it does not yet
/// derive phase transitions, selection pressure, finality, or consensus rules
/// from these fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Regime {
    step: Step,
    phase: Phase,
    risk: Risk,
}

impl Regime {
    pub(crate) fn new(step: Step, phase: Phase, risk: Risk) -> Self {
        Self { step, phase, risk }
    }

    pub(crate) fn prototype1_baseline(step: u64) -> Self {
        Self {
            step: Step::new(step),
            phase: Phase::Consolidation,
            risk: Risk::balanced(),
        }
    }

    pub(crate) fn step(&self) -> Step {
        self.step
    }

    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }

    pub(crate) fn risk(&self) -> Risk {
        self.risk
    }
}

/// Absolute cycle coordinate for strategy scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Step(u64);

impl Step {
    pub(crate) fn new(value: u64) -> Self {
        Self(value)
    }

    pub(crate) fn value(self) -> u64 {
        self.0
    }
}

/// Coarse phase of a periodic strategy cycle.
///
/// These phases are placeholders for future admission and reward-shaping
/// policy. They should be interpreted as strategy context, not as a guarantee
/// that the current block performed all work implied by the phase name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Phase {
    Expansion,
    Evaluation,
    Consolidation,
    Hardening,
}

/// Coarse risk profile for the current strategy phase.
///
/// The axes are intentionally minimal. They encode the fact that risk is not
/// one scalar: exploration pressure, mutation tolerance, and finality pressure
/// may move differently as the system alternates between expanding the search
/// space and restoring coherence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Risk {
    exploration: Level,
    mutation: Level,
    finality: Level,
}

impl Risk {
    pub(crate) fn new(exploration: Level, mutation: Level, finality: Level) -> Self {
        Self {
            exploration,
            mutation,
            finality,
        }
    }

    pub(crate) fn balanced() -> Self {
        Self {
            exploration: Level::Medium,
            mutation: Level::Medium,
            finality: Level::Medium,
        }
    }

    pub(crate) fn exploration(self) -> Level {
        self.exploration
    }

    pub(crate) fn mutation(self) -> Level {
        self.mutation
    }

    pub(crate) fn finality(self) -> Level {
        self.finality
    }
}

/// Coarse ordinal level for one risk axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Level {
    Low,
    Medium,
    High,
}

/// Marker for an artifact-local provenance manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Manifest;

/// Artifact-relative path used by the initial History claim shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ArtifactPath {
    value: String,
}

impl ArtifactPath {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

/// Stable commitment to a backend-owned clean tree key.
///
/// The concrete tree key belongs to the workspace backend adapter
/// (`WorkspaceBackend::TreeKey`). History commits a deterministic digest of
/// that typed backend key so this module does not accept caller-authored text
/// as an Artifact identity witness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TreeKeyHash {
    hash: HistoryHash,
}

impl TreeKeyHash {
    fn from_serialized_key<K>(key: &K) -> Result<Self, HistoryError>
    where
        K: Serialize,
    {
        Ok(Self {
            hash: HistoryHash::of_domain_json("prototype1.history.tree_key.v1", key)?,
        })
    }
}

/// Capability for a backend-owned tree key to produce its History commitment.
///
/// The constructor for `TreeKeyHash` stays private. Backends expose a concrete
/// associated `WorkspaceBackend::TreeKey`; only key types that implement this
/// trait can be admitted into History opening authority.
pub(crate) trait TreeKeyCommitment {
    fn tree_key_hash(&self) -> Result<TreeKeyHash, HistoryError>;
}

impl TreeKeyCommitment for super::backend::GitTreeKey {
    fn tree_key_hash(&self) -> Result<TreeKeyHash, HistoryError> {
        TreeKeyHash::from_serialized_key(self)
    }
}

/// Parent identity evidence committed into a parent-capable Artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ParentIdentityRef {
    evidence: EvidenceRef,
}

impl ParentIdentityRef {
    pub(crate) fn new(evidence: EvidenceRef) -> Self {
        Self { evidence }
    }
}

/// Authority that opens the first block for a lineage.
///
/// Gen0 is not authorized by a predecessor block. It is authorized by
/// setup/bootstrap policy, and that base case must be committed explicitly so
/// later successor admission can recurse from a real History head instead of a
/// conceptual hole.
///
/// Update recorded 2026-04-29 10:35 UTC: genesis authority is lineage-local and
/// store-scoped. In the intended startup procedure, genesis is valid only after
/// the configured History store proves or locally validates absence of a valid
/// associated head for this lineage/artifact. This type records bootstrap
/// material; it is not by itself a global absence proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GenesisAuthority {
    bootstrap_policy: ProcedureRef,
    tree_key: TreeKeyHash,
    parent_identity: ParentIdentityRef,
}

impl GenesisAuthority {
    pub(crate) fn new(
        bootstrap_policy: ProcedureRef,
        tree_key: TreeKeyHash,
        parent_identity: ParentIdentityRef,
    ) -> Self {
        Self {
            bootstrap_policy,
            tree_key,
            parent_identity,
        }
    }
}

/// Authority that opens a non-genesis block from a sealed predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PredecessorAuthority {
    predecessor_block_hash: BlockHash,
}

impl PredecessorAuthority {
    pub(crate) fn new(predecessor_block_hash: BlockHash) -> Self {
        Self {
            predecessor_block_hash,
        }
    }
}

/// Authority basis for opening a block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub(crate) enum OpeningAuthority {
    Genesis(GenesisAuthority),
    Predecessor(PredecessorAuthority),
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

    pub(crate) fn runtime(&self) -> &ActorRef {
        &self.runtime
    }

    pub(crate) fn artifact(&self) -> &ArtifactRef {
        &self.artifact
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
enum EntryPayload {
    Direct,
    SelectionDecision(SelectionDecisionEntry),
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

/// Explicit candidate coordinate for sealed selection replay (no path recovery).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateCoordinate {
    pub(crate) node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) plan_index: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) primary_runtime_id: Option<String>,
}

/// Source class for one observed candidate occurrence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CandidateSourceClass {
    History,
    CurrentGeneration,
}

/// Typed preimage for one observed candidate occurrence.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct CandidateOccurrencePreimage<'a> {
    pub(crate) coordinate: &'a CandidateCoordinate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) lineage_id: Option<&'a LineageId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) artifact: Option<&'a ArtifactRef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) runtime: Option<&'a ActorRef>,
    pub(crate) source: CandidateSourceClass,
}

impl<'a> CandidateOccurrencePreimage<'a> {
    pub(crate) fn new(source: CandidateSourceClass, coordinate: &'a CandidateCoordinate) -> Self {
        Self {
            coordinate,
            lineage_id: None,
            artifact: None,
            runtime: None,
            source,
        }
    }
}

/// Durable identity for one observed candidate occurrence.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CandidateOccurrenceId(HistoryHash);

impl CandidateOccurrenceId {
    pub(crate) fn from_preimage(
        preimage: CandidateOccurrencePreimage<'_>,
    ) -> Result<Self, HistoryError> {
        Ok(Self(HistoryHash::of_domain_json(
            "prototype1.history.candidate_occurrence.v1",
            &preimage,
        )?))
    }

    pub(crate) fn hash(&self) -> &HistoryHash {
        &self.0
    }
}

/// Lifecycle outcome labels from the planner and persisted node status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateLifecycle {
    pub(crate) planner_outcome: String,
    pub(crate) node_status: String,
}

/// Citation to a typed evidence source (`ref_id` + optional content hash).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedEvidenceCitation {
    pub(crate) ref_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) content_hash: Option<HistoryHash>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) record_name: Option<String>,
}

/// Compared-run slice carried from child evaluation evidence (metrics + citations).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedComparedRunEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) instance_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_citation: Option<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) treatment_citation: Option<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) treatment_metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_protocol: Option<metric::Protocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) treatment_protocol: Option<metric::Protocol>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_evaluation: Option<crate::mbe::OracleEvaluation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) diagnostics: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_run: Option<SealedRunEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) treatment_run: Option<SealedRunEvidence>,
}

/// Passive run snapshot sealed into candidate evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedRunEvidence {
    pub(crate) run_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) run_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) spec_fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) provider_slug: Option<String>,
    #[serde(default)]
    pub(crate) protocol: SealedRunProtocolEvidence,
}

/// Protocol evidence summary for a sealed run snapshot.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedRunProtocolEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) anchor_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) artifacts: Vec<SealedProtocolArtifactEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedProtocolArtifactEvidence {
    pub(crate) procedure_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) schema_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) subject_id: Option<String>,
}

/// One branch evaluation report worth of sealed material.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedEvaluationEvidence {
    pub(crate) branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluation_procedure_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluator_identity: Option<SealedEvaluatorIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) eval_set_identity: Option<SealedEvalSetIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluation_artifact_citation: Option<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) overall_disposition: Option<String>,
    pub(crate) primary_report_citation: SealedEvidenceCitation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) compared_runs: Vec<SealedComparedRunEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedEvaluatorIdentity {
    pub(crate) id: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedEvalSetIdentity {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) authority: String,
    pub(crate) explicit: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) benchmark_family: Option<String>,
    #[serde(default)]
    pub(crate) dataset_source_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_treatment_instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedRuntimeEvidence {
    pub(crate) runtime_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) document_citations: Vec<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) journal_citations: Vec<SealedEvidenceCitation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedBranchEvidence {
    pub(crate) branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_state_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) branch_evidence_citations: Vec<SealedEvidenceCitation>,
}

/// Snapshot mirroring [`super::evidence::ChildEvidence`] categories for sealed History.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SealedCandidateEvidence {
    pub(crate) schema_version: u32,
    pub(crate) coordinate: CandidateCoordinate,
    pub(crate) lifecycle: CandidateLifecycle,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) evaluations: Vec<SealedEvaluationEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) runtimes: Vec<SealedRuntimeEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) branches: Vec<SealedBranchEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) extra_document_citations: Vec<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) extra_journal_citations: Vec<SealedEvidenceCitation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) child_diagnostics: Vec<String>,
}

/// Serializable evidence for one checked edit-surface candidate.
///
/// This is the durable projection of the backend-owned surface check and apply
/// result. It is intentionally candidate-local: legacy candidates may carry no
/// edit evidence, but deterministic edit-surface candidates must not silently
/// downgrade to a plain text-file branch projection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceEvidence {
    pub(crate) schema_version: u32,
    pub(crate) producer_id: String,
    pub(crate) proposal_id: String,
    pub(crate) run_id: String,
    pub(crate) policy: String,
    pub(crate) target_relpath: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) grant: Option<grant::Evidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) check: Option<SurfaceCheckEvidence>,
    pub(crate) base: SurfaceArtifactRef,
    pub(crate) after: SurfaceArtifactRef,
    pub(crate) patch_id: PatchId,
    pub(crate) source_content_hash: String,
    pub(crate) proposed_content_hash: String,
    #[serde(default)]
    pub(crate) proposal_producer: super::edit_surface::request_policy::ProposalProducer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generator_surface: Option<super::edit_surface::tui::GeneratorSurfaceVersion>,
    pub(crate) touches: Vec<SurfaceTouch>,
    pub(crate) touches_digest: HistoryHash,
    pub(crate) delta_id: String,
    pub(crate) delta_digest: HistoryHash,
    pub(crate) check_status: SurfaceCheckStatus,
    pub(crate) apply_status: SurfaceApplyStatus,
}

/// History-owned authority projection for one checked or admitted surface grant.
///
/// `SurfaceEvidence` keeps proposal/check/apply facts, but History needs an
/// explicit carrier that preserves checked runtime authority immediately and
/// can later refine that authority to an admitted candidate coordinate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GrantEvidence {
    pub(crate) coordinate: grant::AnyCoordinate,
    pub(crate) policy: ProcedureRef,
    pub(crate) writable: SurfaceWritable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedSurface {
    pub(crate) grant: grant::Grant<grant::Checked>,
    pub(crate) transition: CheckedSurfaceTransition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CheckedSurfaceTransition {
    pub(crate) target_relpath: PathBuf,
    pub(crate) base: SurfaceArtifactRef,
    pub(crate) after: SurfaceArtifactRef,
    pub(crate) patch_id: PatchId,
}

pub(crate) mod grant {
    use super::*;
    use std::marker::PhantomData;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Checked;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) struct Admitted;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Coordinate<S> {
        record: record::Coordinate,
        state: PhantomData<S>,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub(crate) struct Grant<S> {
        coordinate: Coordinate<S>,
        policy: ProcedureRef,
        writable: SurfaceWritable,
    }

    pub(crate) type Evidence = super::GrantEvidence;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(untagged)]
    pub(crate) enum AnyCoordinate {
        Checked(Coordinate<Checked>),
        Admitted(Coordinate<Admitted>),
    }

    pub(crate) mod record {
        use super::*;

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub(crate) struct Checked {
            pub(crate) runtime_id: crate::loop_graph::RuntimeId,
            pub(crate) target_artifact_id: ArtifactId,
        }

        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
        pub(crate) struct Admitted {
            pub(crate) candidate: CandidateCoordinate,
            pub(crate) target_artifact_id: ArtifactId,
        }

        #[derive(Debug, Clone, PartialEq, Eq)]
        pub(crate) enum Coordinate {
            Checked(Checked),
            Admitted(Admitted),
        }
    }

    impl Coordinate<Checked> {
        fn from_operation(
            coordinate: &crate::loop_graph::Coordinate,
        ) -> Result<Self, HistoryError> {
            let crate::loop_graph::OperationTarget::Artifact { artifact_id } = &coordinate.target
            else {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant requires OperationTarget::Artifact, got {:?}",
                        coordinate.target
                    ),
                });
            };
            Ok(Self {
                record: record::Coordinate::Checked(record::Checked {
                    runtime_id: coordinate.runtime_id,
                    target_artifact_id: artifact_id.clone(),
                }),
                state: PhantomData,
            })
        }

        pub(crate) fn runtime_id(&self) -> crate::loop_graph::RuntimeId {
            match &self.record {
                record::Coordinate::Checked(coordinate) => coordinate.runtime_id,
                record::Coordinate::Admitted(_) => unreachable!("checked coordinate state"),
            }
        }

        pub(crate) fn operation(&self) -> crate::loop_graph::Coordinate {
            crate::loop_graph::Coordinate {
                runtime_id: self.runtime_id(),
                target: crate::loop_graph::OperationTarget::Artifact {
                    artifact_id: self.target_artifact_id().clone(),
                },
            }
        }
    }

    impl Coordinate<Admitted> {
        fn from_candidate(candidate: CandidateCoordinate, target_artifact_id: ArtifactId) -> Self {
            Self {
                record: record::Coordinate::Admitted(record::Admitted {
                    candidate,
                    target_artifact_id,
                }),
                state: PhantomData,
            }
        }

        pub(crate) fn candidate(&self) -> &CandidateCoordinate {
            match &self.record {
                record::Coordinate::Checked(_) => unreachable!("admitted coordinate state"),
                record::Coordinate::Admitted(coordinate) => &coordinate.candidate,
            }
        }
    }

    impl<S> Coordinate<S> {
        pub(crate) fn target_artifact_id(&self) -> &ArtifactId {
            match &self.record {
                record::Coordinate::Checked(coordinate) => &coordinate.target_artifact_id,
                record::Coordinate::Admitted(coordinate) => &coordinate.target_artifact_id,
            }
        }

        fn runtime_id_opt(&self) -> Option<crate::loop_graph::RuntimeId> {
            match &self.record {
                record::Coordinate::Checked(coordinate) => Some(coordinate.runtime_id),
                record::Coordinate::Admitted(coordinate) => coordinate
                    .candidate
                    .primary_runtime_id
                    .as_deref()
                    .and_then(|runtime_id| runtime_id.parse().ok()),
            }
        }
    }

    impl Grant<Checked> {
        pub(crate) fn checked(
            coordinate: crate::loop_graph::Coordinate,
            policy: ProcedureRef,
            writable: SurfaceWritable,
            transition: &CheckedSurfaceTransition,
        ) -> Result<Self, HistoryError> {
            if writable.target_relpath != transition.target_relpath {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant target '{}' did not match checked target '{}'",
                        writable.target_relpath.display(),
                        transition.target_relpath.display()
                    ),
                });
            }
            let coordinate = Coordinate::<Checked>::from_operation(&coordinate)?;
            if coordinate.target_artifact_id() != &transition.base.artifact_id {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant target artifact '{}' did not match checked base artifact '{}'",
                        coordinate.target_artifact_id(),
                        transition.base.artifact_id
                    ),
                });
            }
            Ok(Self {
                coordinate,
                policy,
                writable,
            })
        }

        pub(super) fn admit(
            self,
            admitted: Grant<Admitted>,
        ) -> Result<Grant<Admitted>, HistoryError> {
            if self.policy != admitted.policy {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant policy '{}' did not match admitted policy '{}'",
                        self.policy.as_str(),
                        admitted.policy.as_str()
                    ),
                });
            }
            if self.writable != admitted.writable {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant target '{}' did not match admitted target '{}'",
                        self.writable.target_relpath.display(),
                        admitted.writable.target_relpath.display()
                    ),
                });
            }
            if self.coordinate.target_artifact_id() != admitted.coordinate.target_artifact_id() {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant target artifact '{}' did not match admitted target artifact '{}'",
                        self.coordinate.target_artifact_id(),
                        admitted.coordinate.target_artifact_id()
                    ),
                });
            }
            let expected_runtime = self.coordinate.runtime_id();
            let actual_runtime = admitted.coordinate.runtime_id_opt().ok_or_else(|| {
                HistoryError::InvalidSelectionDecision {
                    detail: "admitted surface grant is missing runtime identity".to_string(),
                }
            })?;
            if expected_runtime != actual_runtime {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "checked surface grant runtime '{}' did not match admitted runtime '{}'",
                        expected_runtime, actual_runtime
                    ),
                });
            }
            Ok(admitted)
        }
    }

    impl Grant<Admitted> {
        pub(super) fn from_candidate_artifact(
            node: &crate::intervention::Prototype1NodeRecord,
            resolved: &crate::intervention::ResolvedTreatmentBranch,
            surface: &SurfaceEvidence,
        ) -> Result<Option<Self>, HistoryError> {
            let operation_target_artifact = node
                .operation_target
                .as_ref()
                .and_then(crate::intervention::operation_target_artifact_id);
            if let (Some(operation_target_artifact), Some(base_artifact_id)) =
                (operation_target_artifact, node.base_artifact_id.as_ref())
                && operation_target_artifact != base_artifact_id
            {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "candidate operation target '{}' did not match base artifact '{}'",
                        operation_target_artifact, base_artifact_id
                    ),
                });
            }
            let Some(target_artifact_id) = operation_target_artifact
                .cloned()
                .or_else(|| node.base_artifact_id.clone())
            else {
                return Ok(None);
            };
            if target_artifact_id != surface.base.artifact_id {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "grant target artifact '{}' did not match checked base artifact '{}'",
                        target_artifact_id, surface.base.artifact_id
                    ),
                });
            }
            if resolved.target_relpath != surface.target_relpath {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "grant target '{}' did not match checked target '{}'",
                        resolved.target_relpath.display(),
                        surface.target_relpath.display()
                    ),
                });
            }
            Ok(Some(Self {
                coordinate: Coordinate::<Admitted>::from_candidate(
                    CandidateCoordinate {
                        node_id: node.node_id.clone(),
                        parent_node_id: node.parent_node_id.clone(),
                        branch_id: Some(node.branch_id.clone()),
                        generation: Some(node.generation),
                        plan_index: None,
                        primary_runtime_id: Some(node.instance_id.clone()),
                    },
                    target_artifact_id,
                ),
                policy: ProcedureRef::new(surface.policy.clone()),
                writable: SurfaceWritable {
                    target_relpath: surface.target_relpath.clone(),
                },
            }))
        }
    }

    impl<S> Grant<S> {
        pub(crate) fn coordinate(&self) -> &Coordinate<S> {
            &self.coordinate
        }

        pub(crate) fn policy(&self) -> &ProcedureRef {
            &self.policy
        }

        pub(crate) fn writable(&self) -> &SurfaceWritable {
            &self.writable
        }

        pub(super) fn into_evidence(self) -> Evidence
        where
            AnyCoordinate: From<Coordinate<S>>,
        {
            Evidence {
                coordinate: self.coordinate.into(),
                policy: self.policy,
                writable: self.writable,
            }
        }
    }

    impl TryFrom<Evidence> for Grant<Checked> {
        type Error = HistoryError;

        fn try_from(evidence: Evidence) -> Result<Self, Self::Error> {
            let AnyCoordinate::Checked(coordinate) = evidence.coordinate else {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: "surface grant is not checked authority".to_string(),
                });
            };
            Ok(Self {
                coordinate,
                policy: evidence.policy,
                writable: evidence.writable,
            })
        }
    }

    impl TryFrom<Evidence> for Grant<Admitted> {
        type Error = HistoryError;

        fn try_from(evidence: Evidence) -> Result<Self, Self::Error> {
            let AnyCoordinate::Admitted(coordinate) = evidence.coordinate else {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: "surface grant is not admitted authority".to_string(),
                });
            };
            Ok(Self {
                coordinate,
                policy: evidence.policy,
                writable: evidence.writable,
            })
        }
    }

    impl From<Coordinate<Checked>> for AnyCoordinate {
        fn from(coordinate: Coordinate<Checked>) -> Self {
            Self::Checked(coordinate)
        }
    }

    impl From<Coordinate<Admitted>> for AnyCoordinate {
        fn from(coordinate: Coordinate<Admitted>) -> Self {
            Self::Admitted(coordinate)
        }
    }

    impl AnyCoordinate {
        pub(crate) fn target_artifact_id(&self) -> &ArtifactId {
            match self {
                Self::Checked(coordinate) => coordinate.target_artifact_id(),
                Self::Admitted(coordinate) => coordinate.target_artifact_id(),
            }
        }

        pub(super) fn runtime_id(&self) -> Option<crate::loop_graph::RuntimeId> {
            match self {
                Self::Checked(coordinate) => Some(coordinate.runtime_id()),
                Self::Admitted(coordinate) => coordinate.runtime_id_opt(),
            }
        }

        pub(super) fn candidate(&self) -> Option<&CandidateCoordinate> {
            match self {
                Self::Checked(_) => None,
                Self::Admitted(coordinate) => Some(coordinate.candidate()),
            }
        }
    }

    impl Serialize for Coordinate<Checked> {
        fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
        where
            T: serde::Serializer,
        {
            match &self.record {
                record::Coordinate::Checked(coordinate) => coordinate.serialize(serializer),
                record::Coordinate::Admitted(_) => unreachable!("checked coordinate state"),
            }
        }
    }

    impl<'de> Deserialize<'de> for Coordinate<Checked> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Self {
                record: record::Coordinate::Checked(record::Checked::deserialize(deserializer)?),
                state: PhantomData,
            })
        }
    }

    impl Serialize for Coordinate<Admitted> {
        fn serialize<T>(&self, serializer: T) -> Result<T::Ok, T::Error>
        where
            T: serde::Serializer,
        {
            match &self.record {
                record::Coordinate::Checked(_) => unreachable!("admitted coordinate state"),
                record::Coordinate::Admitted(coordinate) => coordinate.serialize(serializer),
            }
        }
    }

    impl<'de> Deserialize<'de> for Coordinate<Admitted> {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            Ok(Self {
                record: record::Coordinate::Admitted(record::Admitted::deserialize(deserializer)?),
                state: PhantomData,
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceWritable {
    pub(crate) target_relpath: PathBuf,
}

/// Durable check/apply projection for one checked surface transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceCheckEvidence {
    pub(crate) base: SurfaceArtifactRef,
    pub(crate) after: SurfaceArtifactRef,
    pub(crate) patch_id: PatchId,
    pub(crate) touches_digest: HistoryHash,
    pub(crate) delta_id: String,
    pub(crate) delta_digest: HistoryHash,
    pub(crate) check_status: SurfaceCheckStatus,
    pub(crate) apply_status: SurfaceApplyStatus,
}

/// Durable attempt evidence for one edit-surface proposal, including rejected
/// apply/write outcomes that do not produce an Artifact delta.
pub(crate) mod surface_attempt {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct Evidence {
        pub(crate) schema_version: u32,
        pub(crate) producer_id: String,
        pub(crate) proposal_id: String,
        pub(crate) run_id: String,
        pub(crate) policy: String,
        pub(crate) target_relpath: PathBuf,
        pub(crate) outcome: Outcome,
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    pub(crate) enum Outcome {
        Applied,
        Rejected { reason: String },
    }

    impl Evidence {
        pub(crate) fn applied(
            producer_id: impl Into<String>,
            proposal_id: impl Into<String>,
            run_id: impl Into<String>,
            policy: impl Into<String>,
            target_relpath: PathBuf,
        ) -> Self {
            Self {
                schema_version: 1,
                producer_id: producer_id.into(),
                proposal_id: proposal_id.into(),
                run_id: run_id.into(),
                policy: policy.into(),
                target_relpath,
                outcome: Outcome::Applied,
            }
        }

        pub(crate) fn rejected(
            producer_id: impl Into<String>,
            proposal_id: impl Into<String>,
            run_id: impl Into<String>,
            policy: impl Into<String>,
            target_relpath: PathBuf,
            reason: impl Into<String>,
        ) -> Self {
            Self {
                schema_version: 1,
                producer_id: producer_id.into(),
                proposal_id: proposal_id.into(),
                run_id: run_id.into(),
                policy: policy.into(),
                target_relpath,
                outcome: Outcome::Rejected {
                    reason: reason.into(),
                },
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceArtifactRef {
    pub(crate) artifact_id: ArtifactId,
    pub(crate) hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SurfaceTouch {
    pub(crate) target_relpath: PathBuf,
    pub(crate) target_name: String,
    pub(crate) span_relpath: PathBuf,
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) base_hash: String,
    pub(crate) replacement: String,
    pub(crate) replacement_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SurfaceCheckStatus {
    Checked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SurfaceApplyStatus {
    Applied,
}

impl SurfaceEvidence {
    pub(crate) fn checked(
        producer_id: impl Into<String>,
        proposal_id: impl Into<String>,
        run_id: impl Into<String>,
        checked: CheckedSurface,
        source_content_hash: impl Into<String>,
        proposed_content_hash: impl Into<String>,
        proposal_producer: super::edit_surface::request_policy::ProposalProducer,
        generator_surface: super::edit_surface::tui::GeneratorSurfaceVersion,
        touches: Vec<SurfaceTouch>,
    ) -> Result<Self, HistoryError> {
        let grant = grant::Evidence::from_checked(&checked.grant, &checked.transition)?;
        let touches_digest = HistoryHash::of_domain_json(
            "prototype1.history.surface_evidence.touches.v1",
            &touches,
        )?;
        let delta = SurfaceDeltaPreimage {
            base: &checked.transition.base,
            after: &checked.transition.after,
            patch_id: &checked.transition.patch_id,
            touches_digest: &touches_digest,
        };
        let delta_digest =
            HistoryHash::of_domain_json("prototype1.history.surface_evidence.delta.v1", &delta)?;
        let delta_id = format!("surface-delta:{}", delta_digest.as_str());
        Ok(Self {
            schema_version: 5,
            producer_id: producer_id.into(),
            proposal_id: proposal_id.into(),
            run_id: run_id.into(),
            policy: checked.grant.policy().as_str().to_string(),
            target_relpath: checked.transition.target_relpath.clone(),
            grant: Some(grant),
            check: None,
            base: checked.transition.base,
            after: checked.transition.after,
            patch_id: checked.transition.patch_id,
            source_content_hash: source_content_hash.into(),
            proposed_content_hash: proposed_content_hash.into(),
            proposal_producer,
            generator_surface: Some(generator_surface),
            touches,
            touches_digest,
            delta_id,
            delta_digest,
            check_status: SurfaceCheckStatus::Checked,
            apply_status: SurfaceApplyStatus::Applied,
        })
        .map(|mut evidence| {
            evidence.check = Some(SurfaceCheckEvidence::from_surface(&evidence));
            evidence
        })
    }

    pub(crate) fn verify_integrity(&self) -> Result<(), HistoryError> {
        if !matches!(self.schema_version, 1 | 2 | 3 | 4 | 5) {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "surface evidence has unsupported schema_version {}",
                    self.schema_version
                ),
            });
        }
        let touches_digest = HistoryHash::of_domain_json(
            "prototype1.history.surface_evidence.touches.v1",
            &self.touches,
        )?;
        if touches_digest != self.touches_digest {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface evidence touches_digest does not match touches".to_string(),
            });
        }
        let delta = SurfaceDeltaPreimage {
            base: &self.base,
            after: &self.after,
            patch_id: &self.patch_id,
            touches_digest: &self.touches_digest,
        };
        let delta_digest =
            HistoryHash::of_domain_json("prototype1.history.surface_evidence.delta.v1", &delta)?;
        if delta_digest != self.delta_digest {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface evidence delta_digest does not match delta preimage".to_string(),
            });
        }
        let delta_id = format!("surface-delta:{}", self.delta_digest.as_str());
        if delta_id != self.delta_id {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface evidence delta_id does not match delta_digest".to_string(),
            });
        }
        self.proposal_producer
            .verify_complete(&self.base.artifact_id, &self.proposal_id, &self.run_id)
            .map_err(|detail| HistoryError::InvalidSelectionDecision { detail })?;
        if self.schema_version >= 2 && self.generator_surface.is_none() {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface evidence is missing generator_surface provenance".to_string(),
            });
        }
        if self.schema_version >= 3 {
            let Some(check) = self.check.as_ref() else {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: "surface evidence is missing check evidence".to_string(),
                });
            };
            check.verify(self)?;
        }
        if self.schema_version >= 4 {
            let Some(grant) = self.grant.as_ref() else {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: "surface evidence is missing grant evidence".to_string(),
                });
            };
            grant.verify(self)?;
        }
        Ok(())
    }

    fn bind_candidate_artifact(
        mut self,
        node: &crate::intervention::Prototype1NodeRecord,
        resolved: &crate::intervention::ResolvedTreatmentBranch,
    ) -> Result<Self, HistoryError> {
        self.check = Some(SurfaceCheckEvidence::from_surface(&self));
        self.bind_transition(node, resolved)?;
        match (
            self.grant.take(),
            grant::Evidence::from_candidate_artifact(node, resolved, &self)?,
        ) {
            (Some(grant), Some(admitted)) => {
                self.grant = Some(grant.admit(admitted)?);
                self.schema_version = self.schema_version.max(5);
            }
            (Some(grant), None) => {
                self.grant = Some(grant);
                self.schema_version = self.schema_version.max(5);
            }
            (None, Some(admitted)) => {
                self.grant = Some(admitted);
                self.schema_version = self.schema_version.max(4);
            }
            (None, None) => {
                self.schema_version = self.schema_version.max(3);
            }
        }
        Ok(self)
    }

    fn bind_transition(
        &self,
        node: &crate::intervention::Prototype1NodeRecord,
        resolved: &crate::intervention::ResolvedTreatmentBranch,
    ) -> Result<(), HistoryError> {
        if node.target_relpath != self.target_relpath {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "candidate node target '{}' did not match surface evidence target '{}'",
                    node.target_relpath.display(),
                    self.target_relpath.display()
                ),
            });
        }
        if resolved.target_relpath != self.target_relpath {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "resolved branch target '{}' did not match surface evidence target '{}'",
                    resolved.target_relpath.display(),
                    self.target_relpath.display()
                ),
            });
        }
        if let Some(base_artifact_id) = node.base_artifact_id.as_ref()
            && base_artifact_id != &self.base.artifact_id
        {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "candidate base artifact '{}' did not match surface evidence base '{}'",
                    base_artifact_id, self.base.artifact_id
                ),
            });
        }
        if let Some(patch_id) = node.patch_id.as_ref()
            && patch_id != &self.patch_id
        {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "candidate patch '{}' did not match surface evidence patch '{}'",
                    patch_id, self.patch_id
                ),
            });
        }
        if let Some(derived_artifact_id) = node.derived_artifact_id.as_ref()
            && derived_artifact_id != &self.after.artifact_id
        {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "candidate derived artifact '{}' did not match surface evidence after '{}'",
                    derived_artifact_id, self.after.artifact_id
                ),
            });
        }
        if let Some(patch_id) = resolved.branch.patch_id.as_ref()
            && patch_id != &self.patch_id
        {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "resolved branch patch '{}' did not match surface evidence patch '{}'",
                    patch_id, self.patch_id
                ),
            });
        }
        if let Some(derived_artifact_id) = resolved.branch.derived_artifact_id.as_ref()
            && derived_artifact_id != &self.after.artifact_id
        {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "resolved branch derived artifact '{}' did not match surface evidence after '{}'",
                    derived_artifact_id, self.after.artifact_id
                ),
            });
        }
        Ok(())
    }
}

impl GrantEvidence {
    fn from_checked(
        grant: &grant::Grant<grant::Checked>,
        transition: &CheckedSurfaceTransition,
    ) -> Result<Self, HistoryError> {
        if grant.writable().target_relpath != transition.target_relpath {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "checked surface grant target '{}' did not match checked target '{}'",
                    grant.writable().target_relpath.display(),
                    transition.target_relpath.display()
                ),
            });
        }
        Ok(grant.clone().into_evidence())
    }

    fn from_candidate_artifact(
        node: &crate::intervention::Prototype1NodeRecord,
        resolved: &crate::intervention::ResolvedTreatmentBranch,
        surface: &SurfaceEvidence,
    ) -> Result<Option<Self>, HistoryError> {
        grant::Grant::<grant::Admitted>::from_candidate_artifact(node, resolved, surface)
            .map(|grant| grant.map(grant::Grant::into_evidence))
    }

    fn admit(self, admitted: grant::Evidence) -> Result<Self, HistoryError> {
        let checked = grant::Grant::<grant::Checked>::try_from(self)?;
        let admitted = grant::Grant::<grant::Admitted>::try_from(admitted)?;
        checked
            .admit(admitted)
            .map(grant::Grant::<grant::Admitted>::into_evidence)
    }

    fn verify(&self, surface: &SurfaceEvidence) -> Result<(), HistoryError> {
        if self.policy.as_str() != surface.policy {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "surface grant policy '{}' did not match stored policy '{}'",
                    self.policy.as_str(),
                    surface.policy
                ),
            });
        }
        if self.writable.target_relpath != surface.target_relpath {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "surface grant target '{}' did not match stored target '{}'",
                    self.writable.target_relpath.display(),
                    surface.target_relpath.display()
                ),
            });
        }
        if self.coordinate.target_artifact_id() != &surface.base.artifact_id {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "surface grant target artifact '{}' did not match checked base artifact '{}'",
                    self.coordinate.target_artifact_id(),
                    surface.base.artifact_id
                ),
            });
        }
        if self.coordinate.runtime_id().is_none() {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface grant coordinate is missing runtime identity".to_string(),
            });
        }
        if let Some(candidate) = self.coordinate.candidate() {
            if candidate.node_id.is_empty() {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: "surface grant coordinate is missing candidate node_id".to_string(),
                });
            }
        }
        for touch in &surface.touches {
            if touch.target_relpath != self.writable.target_relpath
                || touch.span_relpath != self.writable.target_relpath
            {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "surface touch '{}'/'{}' escaped writable target '{}'",
                        touch.target_relpath.display(),
                        touch.span_relpath.display(),
                        self.writable.target_relpath.display()
                    ),
                });
            }
        }
        Ok(())
    }
}

impl SurfaceCheckEvidence {
    fn from_surface(surface: &SurfaceEvidence) -> Self {
        Self {
            base: surface.base.clone(),
            after: surface.after.clone(),
            patch_id: surface.patch_id.clone(),
            touches_digest: surface.touches_digest.clone(),
            delta_id: surface.delta_id.clone(),
            delta_digest: surface.delta_digest.clone(),
            check_status: surface.check_status,
            apply_status: surface.apply_status,
        }
    }

    fn verify(&self, surface: &SurfaceEvidence) -> Result<(), HistoryError> {
        let expected = Self::from_surface(surface);
        if self != &expected {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "surface check evidence did not match the stored transition projection"
                    .to_string(),
            });
        }
        Ok(())
    }
}

#[derive(Serialize)]
struct SurfaceDeltaPreimage<'a> {
    base: &'a SurfaceArtifactRef,
    after: &'a SurfaceArtifactRef,
    patch_id: &'a PatchId,
    touches_digest: &'a HistoryHash,
}

/// Candidate-local Artifact payload sealed with selection evidence.
///
/// This is the handoff material needed to reconstruct a typed
/// `Selection<Artifact>` from History without reading mutable node or branch
/// projections. The Artifact backend still verifies the material against the
/// git worktree/branch before install.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateArtifact {
    pub(crate) schema_version: u32,
    pub(crate) node: crate::intervention::Prototype1NodeRecord,
    pub(crate) resolved: crate::intervention::ResolvedTreatmentBranch,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface: Option<SurfaceEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifact_surface: Option<ArtifactSurface>,
}

impl CandidateArtifact {
    pub(crate) fn new(
        node: crate::intervention::Prototype1NodeRecord,
        resolved: crate::intervention::ResolvedTreatmentBranch,
    ) -> Self {
        Self {
            schema_version: 1,
            node,
            resolved,
            surface: None,
            artifact_surface: None,
        }
    }

    pub(crate) fn with_surface(mut self, evidence: SurfaceEvidence) -> Self {
        self.schema_version = self.schema_version.max(2);
        let evidence = evidence
            .bind_candidate_artifact(&self.node, &self.resolved)
            .unwrap_or_else(|err| {
                panic!(
                    "candidate artifact '{}' carried contradictory surface evidence: {}",
                    self.node.node_id, err
                )
            });
        self.surface = Some(evidence);
        self
    }

    pub(crate) fn with_artifact_surface(mut self, surface: ArtifactSurface) -> Self {
        self.schema_version = self.schema_version.max(3);
        self.artifact_surface = Some(surface);
        self
    }

    pub(crate) fn node(&self) -> &crate::intervention::Prototype1NodeRecord {
        &self.node
    }

    pub(crate) fn resolved(&self) -> &crate::intervention::ResolvedTreatmentBranch {
        &self.resolved
    }
}

/// Inline-first per-candidate evaluation payload suitable for sealing in History.
///
/// Carries the projected [`crate::successor_selection::SelectionInput`] when present, and
/// optionally a [`SealedCandidateEvidence`] snapshot so a future ruler can justify the
/// candidate without re-reading scheduler or mutable filesystem projections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EvaluationPayload {
    pub(crate) schema_version: u32,
    pub(crate) candidate: SubjectRef,
    pub(crate) procedure: ProcedureRef,

    /// Exact selection input used (or considered) for this candidate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selection_input: Option<crate::successor_selection::SelectionInput>,

    /// Domain-separated hash of `selection_input` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selection_input_hash: Option<HistoryHash>,

    /// Conservative diagnostics when we could not project a selection input.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) projection_failures: Vec<SelectionProjectionFailure>,

    /// Provenance pointers/hashes for later audit (first slice is minimal).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) source_refs: Vec<EvidenceRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) source_hashes: Vec<HistoryHash>,

    /// Rich sealed mirror of grouped child/evaluation evidence (schema ≥ 2 when present).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) sealed_evidence: Option<SealedCandidateEvidence>,

    /// Sealed material required to hydrate a successor Artifact selection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) artifact: Option<CandidateArtifact>,

    /// Durable edit-surface attempt evidence visible to parent-time selection
    /// and diagnosis, including rejected/no-artifact apply outcomes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) surface_attempt: Option<surface_attempt::Evidence>,
}

impl EvaluationPayload {
    pub(crate) fn builder(
        candidate: SubjectRef,
        procedure: ProcedureRef,
    ) -> EvaluationPayloadBuilder {
        EvaluationPayloadBuilder {
            schema_version: 1,
            candidate,
            procedure,
            selection_input: None,
            selection_input_hash: None,
            projection_failures: Vec::new(),
            source_refs: Vec::new(),
            source_hashes: Vec::new(),
            sealed_evidence: None,
            artifact: None,
            surface_attempt: None,
        }
    }

    pub(crate) fn payload_hash(&self) -> Result<HistoryHash, HistoryError> {
        HistoryHash::of_domain_json("prototype1.history.evaluation_payload.v1", self)
    }

    pub(crate) fn occurrence_id(
        &self,
        source: CandidateSourceClass,
    ) -> Result<Option<CandidateOccurrenceId>, HistoryError> {
        let Some(sealed) = self.sealed_evidence.as_ref() else {
            return Ok(None);
        };
        CandidateOccurrenceId::from_preimage(CandidateOccurrencePreimage::new(
            source,
            &sealed.coordinate,
        ))
        .map(Some)
    }

    /// Confirms `selection_input_hash` matches a recomputed digest of
    /// `selection_input` when both are present.
    pub(crate) fn verify_selection_input_binding(&self) -> Result<bool, HistoryError> {
        match (&self.selection_input, &self.selection_input_hash) {
            (Some(input), Some(stored)) => {
                let h =
                    HistoryHash::of_domain_json("prototype1.history.selection_input.v1", input)?;
                Ok(h == *stored)
            }
            (None, None) => Ok(true),
            _ => Ok(false),
        }
    }

    /// Policy for whether this payload is sufficient for **decision-grade** successor selection
    /// replay (distinct from structural digest checks).
    pub(crate) fn decision_grade_eligibility(&self) -> DecisionGradeEligibility {
        let mut gaps = Vec::<String>::new();
        let binding_ok = self.verify_selection_input_binding().unwrap_or(false);

        match (&self.selection_input, &self.selection_input_hash) {
            (Some(_), Some(_)) => {
                if !binding_ok {
                    gaps.push("selection_input_hash_binding_invalid".into());
                }
            }
            _ => gaps.push("missing_selection_input_or_hash".into()),
        }

        let expected = crate::successor_selection::PROCEDURE_ID;
        if self.procedure.as_str() != expected {
            gaps.push(format!(
                "selection_procedure_mismatch:want={expected},got={}",
                self.procedure.as_str()
            ));
        }

        let Some(ref sealed) = self.sealed_evidence else {
            gaps.push("missing_sealed_candidate_evidence".into());
            return DecisionGradeEligibility {
                eligible: false,
                identity_gaps: gaps,
            };
        };

        if sealed.coordinate.node_id.is_empty() {
            gaps.push("coordinate.node_id_empty".into());
        }
        if sealed
            .coordinate
            .branch_id
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            gaps.push("coordinate.branch_id_missing".into());
        }
        if sealed.coordinate.generation.is_none() {
            gaps.push("coordinate.generation_missing".into());
        }
        if sealed
            .coordinate
            .primary_runtime_id
            .as_deref()
            .map(str::is_empty)
            .unwrap_or(true)
        {
            gaps.push("coordinate.primary_runtime_id_missing".into());
        }

        if sealed.evaluations.is_empty() {
            gaps.push("sealed_evaluations_empty".into());
        }
        let expected_evaluation_procedure =
            super::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID;
        for (index, ev) in sealed.evaluations.iter().enumerate() {
            let prefix = format!("sealed_evaluations[{index}]");
            if ev.branch_id.is_empty() {
                gaps.push(format!("{prefix}.branch_id_empty"));
            }
            match ev.evaluation_procedure_id.as_deref() {
                Some(id) if id == expected_evaluation_procedure => {}
                Some(id) if id.is_empty() => {
                    gaps.push(format!("{prefix}.evaluation_procedure_id_missing"));
                }
                Some(id) => gaps.push(format!(
                    "{prefix}.evaluation_procedure_id_mismatch:want={expected_evaluation_procedure},got={id}"
                )),
                None => gaps.push(format!("{prefix}.evaluation_procedure_id_missing")),
            }
            if ev.evaluator_identity.is_none() {
                gaps.push(format!("{prefix}.evaluator_identity_missing"));
            }
            if ev.eval_set_identity.is_none() {
                gaps.push(format!("{prefix}.eval_set_identity_missing"));
            }
        }

        let candidate_branch = self
            .selection_input
            .as_ref()
            .map(|input| input.candidate.branch_id.as_str())
            .or(sealed.coordinate.branch_id.as_deref());
        if let Some(branch_id) = candidate_branch {
            let matching_evaluations = sealed
                .evaluations
                .iter()
                .filter(|evaluation| evaluation.branch_id == branch_id)
                .count();
            match matching_evaluations {
                0 => gaps.push(format!(
                    "sealed_evaluation_for_candidate_branch_missing:branch_id={branch_id}"
                )),
                1 => {}
                count => gaps.push(format!(
                    "sealed_evaluation_for_candidate_branch_ambiguous:branch_id={branch_id},count={count}"
                )),
            }
        }

        if let Some(ref input) = self.selection_input {
            if sealed.coordinate.node_id != input.candidate.node_id {
                gaps.push(format!(
                    "selection_input_node_id_mismatch:selection={}:sealed={}",
                    input.candidate.node_id, sealed.coordinate.node_id
                ));
            }
            match &sealed.coordinate.branch_id {
                Some(b) if b == &input.candidate.branch_id => {}
                _ => gaps.push("selection_input_branch_id_mismatch".into()),
            }
            match sealed.coordinate.generation {
                Some(g) if g == input.candidate.generation => {}
                Some(_) => gaps.push("selection_input_generation_mismatch".into()),
                None => {}
            }
        }

        let Some(ref artifact) = self.artifact else {
            gaps.push("missing_candidate_artifact".into());
            return DecisionGradeEligibility {
                eligible: false,
                identity_gaps: gaps,
            };
        };

        {
            if artifact.node.node_id != sealed.coordinate.node_id {
                gaps.push(format!(
                    "artifact_node_id_mismatch:artifact={}:sealed={}",
                    artifact.node.node_id, sealed.coordinate.node_id
                ));
            }
            match sealed.coordinate.branch_id.as_deref() {
                Some(branch_id) if branch_id == artifact.node.branch_id => {}
                _ => gaps.push("artifact_branch_id_mismatch".into()),
            }
            if let Some(generation) = sealed.coordinate.generation
                && generation != artifact.node.generation
            {
                gaps.push("artifact_generation_mismatch".into());
            }
            if let Some(ref input) = self.selection_input {
                if artifact.node.node_id != input.candidate.node_id {
                    gaps.push(format!(
                        "artifact_selection_input_node_id_mismatch:artifact={},selection={}",
                        artifact.node.node_id, input.candidate.node_id
                    ));
                }
                if artifact.node.branch_id != input.candidate.branch_id {
                    gaps.push(format!(
                        "artifact_selection_input_branch_id_mismatch:artifact={},selection={}",
                        artifact.node.branch_id, input.candidate.branch_id
                    ));
                }
                if artifact.node.generation != input.candidate.generation {
                    gaps.push(format!(
                        "artifact_selection_input_generation_mismatch:artifact={},selection={}",
                        artifact.node.generation, input.candidate.generation
                    ));
                }
            }
            if artifact.node.branch_id != artifact.resolved.branch.branch_id {
                gaps.push(format!(
                    "artifact_resolved_branch_mismatch:node={},resolved={}",
                    artifact.node.branch_id, artifact.resolved.branch.branch_id
                ));
            }
            if artifact.node.candidate_id != artifact.resolved.branch.candidate_id {
                gaps.push(format!(
                    "artifact_resolved_candidate_mismatch:node={},resolved={}",
                    artifact.node.candidate_id, artifact.resolved.branch.candidate_id
                ));
            }
            if artifact.node.source_state_id != artifact.resolved.source_state_id {
                gaps.push(format!(
                    "artifact_resolved_source_state_mismatch:node={},resolved={}",
                    artifact.node.source_state_id, artifact.resolved.source_state_id
                ));
            }
            if artifact.node.target_relpath != artifact.resolved.target_relpath {
                gaps.push(format!(
                    "artifact_resolved_target_mismatch:node={},resolved={}",
                    artifact.node.target_relpath.display(),
                    artifact.resolved.target_relpath.display()
                ));
            }
        }

        DecisionGradeEligibility {
            eligible: gaps.is_empty(),
            identity_gaps: gaps,
        }
    }

    fn candidate_node_id(&self) -> Option<&str> {
        self.selection_input
            .as_ref()
            .map(|input| input.candidate.node_id.as_str())
            .or_else(|| {
                self.sealed_evidence
                    .as_ref()
                    .map(|sealed| sealed.coordinate.node_id.as_str())
            })
    }

    fn candidate_branch_id(&self) -> Option<&str> {
        self.selection_input
            .as_ref()
            .map(|input| input.candidate.branch_id.as_str())
            .or_else(|| {
                self.sealed_evidence
                    .as_ref()
                    .and_then(|sealed| sealed.coordinate.branch_id.as_deref())
            })
    }

    pub(crate) fn has_parent_readable_surface_attempt(&self) -> bool {
        self.surface_attempt.is_some()
    }
}

/// Outcome of [`EvaluationPayload::decision_grade_eligibility`]: separate from per-field digest checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DecisionGradeEligibility {
    pub(crate) eligible: bool,
    pub(crate) identity_gaps: Vec<String>,
}

/// Root of the authenticated candidate map committed by a selection decision.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CandidateSetRoot(HistoryHash);

impl CandidateSetRoot {
    pub(crate) fn as_str(&self) -> &str {
        self.0.as_str()
    }

    fn to_h256(&self) -> Result<H256, HistoryError> {
        self.0.to_digest_bytes().map(H256::from).map_err(|detail| {
            HistoryError::InvalidSelectionDecision {
                detail: format!("candidate_set_root is not a 32-byte digest: {detail}"),
            }
        })
    }
}

/// Typed preimage binding a candidate occurrence to one candidate-set root.
#[derive(Debug, Clone, Copy, Serialize)]
pub(crate) struct CandidateMembershipPreimage<'a> {
    pub(crate) occurrence_id: &'a HistoryHash,
    pub(crate) candidate_set_root: &'a CandidateSetRoot,
}

/// Durable identity for one occurrence's membership in a sealed candidate set.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct CandidateMembershipId(HistoryHash);

impl CandidateMembershipId {
    pub(crate) fn new(
        occurrence_id: &CandidateOccurrenceId,
        candidate_set_root: &CandidateSetRoot,
    ) -> Result<Self, HistoryError> {
        let preimage = CandidateMembershipPreimage {
            occurrence_id: occurrence_id.hash(),
            candidate_set_root,
        };
        Ok(Self(HistoryHash::of_domain_json(
            "prototype1.history.candidate_membership.v1",
            &preimage,
        )?))
    }

    pub(crate) fn hash(&self) -> &HistoryHash {
        &self.0
    }
}

/// Sparse-Merkle membership proof for one candidate payload under a candidate-set root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateSetProof {
    key: [u8; 32],
    value: [u8; 32],
    program: Vec<u8>,
}

impl CandidateSetProof {
    pub(crate) fn verify(&self, root: &CandidateSetRoot) -> Result<bool, HistoryError> {
        let root = root.to_h256()?;
        let key = H256::from(self.key);
        let value = H256::from(self.value);
        CompiledMerkleProof(self.program.clone())
            .verify::<CandidateSetHasher>(&root, vec![(key, value)])
            .map_err(|source| HistoryError::InvalidSelectionDecision {
                detail: format!("candidate-set proof verification failed: {source}"),
            })
    }
}

/// One committed candidate payload and its proof under [`CandidateSetCommitment::root`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateSetMembership {
    pub(crate) candidate: SubjectRef,
    pub(crate) payload_hash: HistoryHash,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) occurrence_id: Option<CandidateOccurrenceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) membership_id: Option<CandidateMembershipId>,
    pub(crate) proof: CandidateSetProof,
}

/// Authenticated candidate map for the universe considered by one selection decision.
///
/// The map key is a domain-separated candidate coordinate. The value is the
/// domain-separated digest of the full [`EvaluationPayload`]. This keeps the
/// selector's universe set-addressable for cross-generation traversal while
/// preserving the ordered-list commitment used to replay the exact local
/// selector input order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CandidateSetCommitment {
    pub(crate) root: CandidateSetRoot,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) memberships: Vec<CandidateSetMembership>,
}

impl CandidateSetCommitment {
    pub(crate) fn from_payloads(considered: &[EvaluationPayload]) -> Result<Self, HistoryError> {
        candidate_set::commit(considered, None)
    }

    pub(crate) fn from_payloads_with_sources(
        considered: &[EvaluationPayload],
        sources: &[TraversalCandidateSource],
    ) -> Result<Self, HistoryError> {
        if considered.len() != sources.len() {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "candidate-set source count mismatch: considered={}, sources={}",
                    considered.len(),
                    sources.len()
                ),
            });
        }
        let source_classes = sources
            .iter()
            .map(|source| source.candidate_source_class())
            .collect::<Vec<_>>();
        candidate_set::commit(considered, Some(&source_classes))
    }

    pub(crate) fn verify(&self) -> Result<bool, HistoryError> {
        for membership in &self.memberships {
            if !membership.proof.verify(&self.root)? {
                return Ok(false);
            }
            match (&membership.occurrence_id, &membership.membership_id) {
                (Some(occurrence_id), Some(membership_id)) => {
                    let expected = CandidateMembershipId::new(occurrence_id, &self.root)?;
                    if expected.hash() != membership_id.hash() {
                        return Ok(false);
                    }
                }
                (None, None) => {}
                _ => return Ok(false),
            }
        }
        Ok(true)
    }

    pub(crate) fn membership(&self, candidate: &SubjectRef) -> Option<&CandidateSetMembership> {
        self.memberships
            .iter()
            .find(|membership| &membership.candidate == candidate)
    }

    pub(crate) fn membership_by_occurrence_id(
        &self,
        occurrence_id: &CandidateOccurrenceId,
    ) -> Option<&CandidateSetMembership> {
        self.memberships
            .iter()
            .find(|membership| membership.occurrence_id.as_ref() == Some(occurrence_id))
    }

    pub(crate) fn membership_by_membership_id(
        &self,
        membership_id: &CandidateMembershipId,
    ) -> Option<&CandidateSetMembership> {
        self.memberships
            .iter()
            .find(|membership| membership.membership_id.as_ref() == Some(membership_id))
    }

    pub(crate) fn membership_for_payload(
        &self,
        payload: &EvaluationPayload,
        source: CandidateSourceClass,
    ) -> Result<Option<&CandidateSetMembership>, HistoryError> {
        if let Some(occurrence_id) = payload.occurrence_id(source)? {
            if let Some(membership) = self.membership_by_occurrence_id(&occurrence_id) {
                return Ok(Some(membership));
            }
        }
        Ok(self.membership(&payload.candidate))
    }
}

#[derive(Default)]
struct CandidateSetHasher {
    bytes: Vec<u8>,
}

impl Hasher for CandidateSetHasher {
    fn write_h256(&mut self, h: &H256) {
        self.bytes.extend_from_slice(h.as_slice());
    }

    fn write_byte(&mut self, b: u8) {
        self.bytes.push(b);
    }

    fn finish(self) -> H256 {
        let digest = Sha256::digest(&self.bytes);
        let mut bytes = [0_u8; 32];
        bytes.copy_from_slice(&digest);
        H256::from(bytes)
    }
}

mod candidate_set {
    use super::*;

    type Tree = SparseMerkleTree<CandidateSetHasher, H256, DefaultStore<H256>>;

    #[derive(Serialize)]
    struct KeyPreimage<'a> {
        candidate: &'a SubjectRef,
        #[serde(skip_serializing_if = "Option::is_none")]
        occurrence_id: Option<&'a HistoryHash>,
        #[serde(skip_serializing_if = "Option::is_none")]
        node_id: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        branch_id: Option<&'a str>,
        #[serde(skip_serializing_if = "Option::is_none")]
        generation: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        plan_index: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        primary_runtime_id: Option<&'a str>,
    }

    struct MemberInput {
        candidate: SubjectRef,
        key: H256,
        key_bytes: [u8; 32],
        value: H256,
        value_hash: HistoryHash,
        occurrence_id: Option<CandidateOccurrenceId>,
    }

    pub(super) fn commit(
        considered: &[EvaluationPayload],
        source_classes: Option<&[CandidateSourceClass]>,
    ) -> Result<CandidateSetCommitment, HistoryError> {
        let mut tree = Tree::default();
        let mut seen_keys = BTreeSet::<[u8; 32]>::new();
        let mut seen_occurrences = BTreeSet::<CandidateOccurrenceId>::new();
        let mut inputs = Vec::with_capacity(considered.len());

        for (index, payload) in considered.iter().enumerate() {
            let source = source_classes
                .and_then(|sources| sources.get(index).copied())
                .unwrap_or_else(|| source_class(payload));
            let occurrence_id = payload.occurrence_id(source)?;
            if let Some(occurrence_id) = occurrence_id.as_ref()
                && !seen_occurrences.insert(occurrence_id.clone())
            {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "duplicate candidate occurrence id for candidate {}",
                        payload.candidate.as_str()
                    ),
                });
            }
            let key_hash = key_hash(payload, occurrence_id.as_ref())?;
            let key_bytes = key_hash.to_digest_bytes().map_err(|detail| {
                HistoryError::InvalidSelectionDecision {
                    detail: format!("candidate-set key is not a 32-byte digest: {detail}"),
                }
            })?;
            if !seen_keys.insert(key_bytes) {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "duplicate candidate-set key for candidate {}",
                        payload.candidate.as_str()
                    ),
                });
            }
            let value_hash = payload.payload_hash()?;
            let value = value_hash
                .to_digest_bytes()
                .map(H256::from)
                .map_err(|detail| HistoryError::InvalidSelectionDecision {
                    detail: format!("candidate-set value is not a 32-byte digest: {detail}"),
                })?;
            if value.is_zero() {
                return Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "candidate-set value for {} is sparse-tree empty value",
                        payload.candidate.as_str()
                    ),
                });
            }
            inputs.push(MemberInput {
                candidate: payload.candidate.clone(),
                key: H256::from(key_bytes),
                key_bytes,
                value,
                value_hash,
                occurrence_id,
            });
        }

        tree.update_all(
            inputs
                .iter()
                .map(|input| (input.key, input.value))
                .collect(),
        )
        .map_err(|source| HistoryError::InvalidSelectionDecision {
            detail: format!("candidate-set map update failed: {source}"),
        })?;
        let root = CandidateSetRoot(HistoryHash::from_digest_bytes((*tree.root()).into()));

        let mut memberships = Vec::with_capacity(inputs.len());
        for input in inputs {
            let proof = tree
                .merkle_proof(vec![input.key])
                .and_then(|proof| proof.compile(vec![input.key]))
                .map_err(|source| HistoryError::InvalidSelectionDecision {
                    detail: format!("candidate-set proof construction failed: {source}"),
                })?;
            let membership_id = input
                .occurrence_id
                .as_ref()
                .map(|occurrence_id| CandidateMembershipId::new(occurrence_id, &root))
                .transpose()?;
            if let Some(membership_id) = &membership_id {
                debug_assert_eq!(membership_id.hash().as_str().len(), 64);
            }
            memberships.push(CandidateSetMembership {
                candidate: input.candidate,
                payload_hash: input.value_hash,
                membership_id,
                occurrence_id: input.occurrence_id,
                proof: CandidateSetProof {
                    key: input.key_bytes,
                    value: input.value.into(),
                    program: proof.into(),
                },
            });
        }

        Ok(CandidateSetCommitment { root, memberships })
    }

    fn key_hash(
        payload: &EvaluationPayload,
        occurrence_id: Option<&CandidateOccurrenceId>,
    ) -> Result<HistoryHash, HistoryError> {
        let coordinate = payload
            .sealed_evidence
            .as_ref()
            .map(|sealed| &sealed.coordinate);
        let preimage = KeyPreimage {
            candidate: &payload.candidate,
            occurrence_id: occurrence_id.map(|id| id.hash()),
            node_id: coordinate.map(|coord| coord.node_id.as_str()),
            branch_id: coordinate.and_then(|coord| coord.branch_id.as_deref()),
            generation: coordinate.and_then(|coord| coord.generation),
            plan_index: coordinate.and_then(|coord| coord.plan_index),
            primary_runtime_id: coordinate.and_then(|coord| coord.primary_runtime_id.as_deref()),
        };
        HistoryHash::of_domain_json("prototype1.history.candidate_set.key.v1", &preimage)
    }

    pub(super) fn source_class(payload: &EvaluationPayload) -> CandidateSourceClass {
        if payload.artifact.is_some() || payload.surface_attempt.is_some() {
            CandidateSourceClass::CurrentGeneration
        } else {
            CandidateSourceClass::History
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EvaluationPayloadBuilder {
    schema_version: u32,
    candidate: SubjectRef,
    procedure: ProcedureRef,
    selection_input: Option<crate::successor_selection::SelectionInput>,
    selection_input_hash: Option<HistoryHash>,
    projection_failures: Vec<SelectionProjectionFailure>,
    source_refs: Vec<EvidenceRef>,
    source_hashes: Vec<HistoryHash>,
    sealed_evidence: Option<SealedCandidateEvidence>,
    artifact: Option<CandidateArtifact>,
    surface_attempt: Option<surface_attempt::Evidence>,
}

impl EvaluationPayloadBuilder {
    pub(crate) fn selection_input(
        mut self,
        input: crate::successor_selection::SelectionInput,
    ) -> Result<Self, HistoryError> {
        let hash = HistoryHash::of_domain_json("prototype1.history.selection_input.v1", &input)?;
        self.selection_input = Some(input);
        self.selection_input_hash = Some(hash);
        Ok(self)
    }

    pub(crate) fn projection_failure(mut self, failure: SelectionProjectionFailure) -> Self {
        self.projection_failures.push(failure);
        self
    }

    pub(crate) fn source_ref(mut self, reference: EvidenceRef) -> Self {
        self.source_refs.push(reference);
        self
    }

    pub(crate) fn source_hash(mut self, hash: HistoryHash) -> Self {
        self.source_hashes.push(hash);
        self
    }

    pub(crate) fn sealed_candidate_evidence(mut self, body: SealedCandidateEvidence) -> Self {
        self.sealed_evidence = Some(body);
        self
    }

    pub(crate) fn candidate_artifact(mut self, artifact: CandidateArtifact) -> Self {
        self.artifact = Some(artifact);
        self
    }

    pub(crate) fn surface_attempt_evidence(mut self, evidence: surface_attempt::Evidence) -> Self {
        self.surface_attempt = Some(evidence);
        self
    }

    pub(crate) fn build(mut self) -> EvaluationPayload {
        let mut schema_version = self.schema_version;
        if self.sealed_evidence.is_some() {
            schema_version = schema_version.max(2);
        }
        if self.artifact.is_some() {
            schema_version = schema_version.max(3);
        }
        if self.surface_attempt.is_some() {
            schema_version = schema_version.max(4);
        }

        if let Some(ref sealed) = self.sealed_evidence {
            append_sealed_evidence_citation_pairs(
                &mut self.source_refs,
                &mut self.source_hashes,
                sealed,
            );
        }

        EvaluationPayload {
            schema_version,
            candidate: self.candidate,
            procedure: self.procedure,
            selection_input: self.selection_input,
            selection_input_hash: self.selection_input_hash,
            projection_failures: self.projection_failures,
            source_refs: self.source_refs,
            source_hashes: self.source_hashes,
            sealed_evidence: self.sealed_evidence,
            artifact: self.artifact,
            surface_attempt: self.surface_attempt,
        }
    }
}

fn append_sealed_evidence_citation_pairs(
    refs: &mut Vec<EvidenceRef>,
    hashes: &mut Vec<HistoryHash>,
    sealed: &SealedCandidateEvidence,
) {
    fn push(
        refs: &mut Vec<EvidenceRef>,
        hashes: &mut Vec<HistoryHash>,
        c: &SealedEvidenceCitation,
    ) {
        if let Some(hash) = c.content_hash.clone() {
            refs.push(EvidenceRef::new(c.ref_id.clone()));
            hashes.push(hash);
        }
    }

    for ev in &sealed.evaluations {
        push(refs, hashes, &ev.primary_report_citation);
        if let Some(ref ac) = ev.evaluation_artifact_citation {
            push(refs, hashes, ac);
        }
        for row in &ev.compared_runs {
            if let Some(ref b) = row.baseline_citation {
                push(refs, hashes, b);
            }
            if let Some(ref t) = row.treatment_citation {
                push(refs, hashes, t);
            }
        }
    }
    for rt in &sealed.runtimes {
        for c in &rt.document_citations {
            push(refs, hashes, c);
        }
        for c in &rt.journal_citations {
            push(refs, hashes, c);
        }
    }
    for br in &sealed.branches {
        for c in &br.branch_evidence_citations {
            push(refs, hashes, c);
        }
    }
    for c in &sealed.extra_document_citations {
        push(refs, hashes, c);
    }
    for c in &sealed.extra_journal_citations {
        push(refs, hashes, c);
    }
}

/// Block-sealable selection decision payload.
///
/// This is intended to be committed into an `EntryKind::Decision` entry, using
/// the existing `Entry` observation/proposal/admission chain-of-custody.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct SelectionDecisionEntry {
    pub(crate) schema_version: u32,

    /// Procedure/policy identity for the selection rule (e.g. successor-selection v1).
    pub(crate) procedure_or_policy: ProcedureRef,

    /// Explicit candidate universe description. This is a first increment:
    /// it is carried as an opaque string so selection replay does not depend
    /// on interpreting filesystem layout.
    pub(crate) scope: SelectionScope,

    /// Selected candidate coordinate, or `None` when no admissible candidate exists.
    pub(crate) selected_candidate: Option<SubjectRef>,

    /// Selected concrete candidate occurrence, when sealed by occurrence-aware traversal.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selected_occurrence_id: Option<CandidateOccurrenceId>,

    /// Selected membership in this decision's candidate set, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selected_membership_id: Option<CandidateMembershipId>,

    /// The ordered evaluation payloads the decision considered.
    ///
    /// Inline-first behavior can include *all* candidates considered under the scope.
    pub(crate) considered: Vec<EvaluationPayload>,

    /// Source of each considered payload in the traversal candidate universe.
    ///
    /// For History traversal entries, only `CurrentGeneration` payloads are
    /// projected back out as newly admitted History candidates. Previously
    /// admitted History payloads remain sealed for replay but must not be
    /// re-ingested as fresh candidates on every generation.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) considered_sources: Vec<TraversalCandidateSource>,

    /// Domain-separated commitment to the ordered considered list.
    pub(crate) considered_order_hash: HistoryHash,

    /// Authenticated map commitment for membership proofs over the considered candidate universe.
    ///
    /// Missing only for older stored entries sealed before this field existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate_set: Option<CandidateSetCommitment>,

    /// Gaps or load errors while assembling seal-time selection material (stored in the sealed entry).
    ///
    /// Must not be treated as members of the ordered considered list used by the selector.
    /// Each [`SelectionProjectionFailure`] may carry a `committed_message` field bound into its [`SelectionProjectionFailureId`] preimage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) projection_failures: Vec<SelectionProjectionFailure>,

    /// Replay parameters for History-backed traversal policies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) traversal: Option<TraversalEvidence>,

    /// Selection-time metric evidence bound to the ordered candidate set.
    pub(crate) metrics: selection_metrics::Set,

    /// Selection-entry-scoped selector formula values persisted for debugger replay.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) formula: Option<crate::successor_selection::traversal::SelectionFormula>,

    /// Decision result under `procedure_or_policy`.
    pub(crate) decision: crate::successor_selection::SuccessorDecision,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TraversalEvidence {
    pub(crate) seed: u64,
    #[serde(default)]
    pub(crate) strategy: crate::successor_selection::traversal::StrategyKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) selected_source: Option<TraversalCandidateSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum TraversalCandidateSource {
    History,
    CurrentGeneration,
}

impl TraversalCandidateSource {
    pub(crate) fn candidate_source_class(self) -> CandidateSourceClass {
        match self {
            Self::History => CandidateSourceClass::History,
            Self::CurrentGeneration => CandidateSourceClass::CurrentGeneration,
        }
    }
}

impl SelectionDecisionEntry {
    pub(crate) fn new(
        procedure_or_policy: ProcedureRef,
        scope: SelectionScope,
        selected_candidate: Option<SubjectRef>,
        considered: Vec<EvaluationPayload>,
        projection_failures: Vec<SelectionProjectionFailure>,
        decision: crate::successor_selection::SuccessorDecision,
    ) -> Result<Self, HistoryError> {
        Self::new_with_traversal(
            procedure_or_policy,
            scope,
            selected_candidate,
            considered,
            projection_failures,
            None,
            decision,
        )
    }

    pub(crate) fn new_with_traversal(
        procedure_or_policy: ProcedureRef,
        scope: SelectionScope,
        selected_candidate: Option<SubjectRef>,
        considered: Vec<EvaluationPayload>,
        projection_failures: Vec<SelectionProjectionFailure>,
        traversal: Option<TraversalEvidence>,
        decision: crate::successor_selection::SuccessorDecision,
    ) -> Result<Self, HistoryError> {
        Self::new_with_traversal_identity(
            procedure_or_policy,
            scope,
            selected_candidate,
            None,
            None,
            considered,
            Vec::new(),
            projection_failures,
            traversal,
            decision,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_traversal_identity(
        procedure_or_policy: ProcedureRef,
        scope: SelectionScope,
        selected_candidate: Option<SubjectRef>,
        selected_occurrence_id: Option<CandidateOccurrenceId>,
        selected_membership_id: Option<CandidateMembershipId>,
        considered: Vec<EvaluationPayload>,
        considered_sources: Vec<TraversalCandidateSource>,
        projection_failures: Vec<SelectionProjectionFailure>,
        traversal: Option<TraversalEvidence>,
        decision: crate::successor_selection::SuccessorDecision,
    ) -> Result<Self, HistoryError> {
        let metrics = selection_metrics::Set::from_considered(
            selection_metrics::Policy::default(),
            &considered,
            &considered_sources,
        )?;
        Self::new_with_traversal_identity_metrics(
            procedure_or_policy,
            scope,
            selected_candidate,
            selected_occurrence_id,
            selected_membership_id,
            considered,
            considered_sources,
            projection_failures,
            traversal,
            metrics,
            decision,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new_with_traversal_identity_metrics(
        procedure_or_policy: ProcedureRef,
        scope: SelectionScope,
        selected_candidate: Option<SubjectRef>,
        selected_occurrence_id: Option<CandidateOccurrenceId>,
        selected_membership_id: Option<CandidateMembershipId>,
        considered: Vec<EvaluationPayload>,
        considered_sources: Vec<TraversalCandidateSource>,
        projection_failures: Vec<SelectionProjectionFailure>,
        traversal: Option<TraversalEvidence>,
        metrics: selection_metrics::Set,
        decision: crate::successor_selection::SuccessorDecision,
    ) -> Result<Self, HistoryError> {
        let candidate_set = Some(Self::candidate_set_for_considered(
            &considered,
            &considered_sources,
        )?);
        let selected_candidate = Self::selected_candidate_projection(
            selected_candidate,
            selected_occurrence_id.as_ref(),
            selected_membership_id.as_ref(),
            candidate_set.as_ref().expect("candidate set"),
            &considered,
        )?;
        Self::validate_decision(
            &procedure_or_policy,
            selected_candidate.as_ref(),
            selected_occurrence_id.as_ref(),
            selected_membership_id.as_ref(),
            candidate_set.as_ref().expect("candidate set"),
            &considered,
            &decision,
        )?;
        let considered_order_hash = HistoryHash::of_domain_json(
            "prototype1.history.selection_considered_order.v1",
            &Self::considered_order_preimage(&considered)?,
        )?;
        Self::validate_metrics(
            &metrics,
            &considered_order_hash,
            candidate_set.as_ref().map(|set| &set.root),
        )?;
        let mut entry = Self {
            schema_version: 4,
            procedure_or_policy,
            scope,
            selected_candidate,
            selected_occurrence_id,
            selected_membership_id,
            considered,
            considered_sources,
            considered_order_hash,
            candidate_set,
            projection_failures,
            traversal,
            metrics,
            formula: None,
            decision,
        };
        entry.formula = crate::successor_selection::traversal::score_child_prop_formula(&entry)?;
        Ok(entry)
    }

    fn contributes_candidates_to_history_projection(&self) -> bool {
        matches!(
            self.procedure_or_policy.as_str(),
            crate::successor_selection::PROCEDURE_ID
                | crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
        )
    }

    fn validate_decision(
        procedure_or_policy: &ProcedureRef,
        selected_candidate: Option<&SubjectRef>,
        selected_occurrence_id: Option<&CandidateOccurrenceId>,
        selected_membership_id: Option<&CandidateMembershipId>,
        candidate_set: &CandidateSetCommitment,
        considered: &[EvaluationPayload],
        decision: &crate::successor_selection::SuccessorDecision,
    ) -> Result<(), HistoryError> {
        fn invalid(detail: impl Into<String>) -> HistoryError {
            HistoryError::InvalidSelectionDecision {
                detail: detail.into(),
            }
        }

        if procedure_or_policy.as_str() != decision.procedure_id {
            return Err(invalid(format!(
                "procedure mismatch: entry={}, decision={}",
                procedure_or_policy.as_str(),
                decision.procedure_id
            )));
        }

        let expected_payload_procedure = if procedure_or_policy.as_str()
            == crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID
        {
            crate::successor_selection::PROCEDURE_ID
        } else {
            procedure_or_policy.as_str()
        };

        for payload in considered {
            if payload.procedure.as_str() != expected_payload_procedure {
                return Err(invalid(format!(
                    "considered payload procedure mismatch: candidate={}, expected={}, payload={}",
                    payload.candidate.as_str(),
                    expected_payload_procedure,
                    payload.procedure.as_str()
                )));
            }
        }

        let selected_payload_by_identity = Self::selected_payload_by_identity(
            selected_occurrence_id,
            selected_membership_id,
            candidate_set,
            considered,
        )?;

        let selected_payload = match (selected_payload_by_identity, selected_candidate) {
            (Some(payload), Some(selected_candidate)) => {
                if &payload.candidate != selected_candidate {
                    return Err(invalid(format!(
                        "selected_candidate projection mismatch: occurrence={}, selected_candidate={}",
                        payload.candidate.as_str(),
                        selected_candidate.as_str()
                    )));
                }
                payload
            }
            (Some(payload), None) => payload,
            (None, Some(selected_candidate)) => {
                let selected_payloads = considered
                    .iter()
                    .filter(|payload| &payload.candidate == selected_candidate)
                    .collect::<Vec<_>>();
                match selected_payloads.as_slice() {
                    [payload] => *payload,
                    [] => {
                        return Err(invalid(format!(
                            "selected_candidate {} is absent from considered payloads",
                            selected_candidate.as_str()
                        )));
                    }
                    many => {
                        return Err(invalid(format!(
                            "selected_candidate {} is ambiguous in considered payloads: count={}",
                            selected_candidate.as_str(),
                            many.len()
                        )));
                    }
                }
            }
            (None, None) => {
                if matches!(
                    decision.outcome,
                    crate::successor_selection::decision::SuccessorOutcome::Accepted
                        | crate::successor_selection::decision::SuccessorOutcome::ExploreFrom
                ) {
                    return Err(invalid(format!(
                        "decision outcome {:?} requires selected occurrence, membership, or selected_candidate",
                        decision.outcome
                    )));
                }
                return Ok(());
            }
        };

        match selected_payload.candidate_node_id() {
            Some(node_id) if node_id == decision.candidate_node_id => {}
            Some(node_id) => {
                return Err(invalid(format!(
                    "selected candidate node mismatch: selected={}, decision={}",
                    node_id, decision.candidate_node_id
                )));
            }
            None => {
                return Err(invalid("selected_candidate requires payload node identity"));
            }
        }

        match (
            &decision.selected_branch_id,
            selected_payload.candidate_branch_id(),
        ) {
            (Some(decision_branch), Some(payload_branch)) if decision_branch == payload_branch => {}
            (Some(decision_branch), Some(payload_branch)) => {
                return Err(invalid(format!(
                    "selected candidate branch mismatch: selected={}, decision={}",
                    payload_branch, decision_branch
                )));
            }
            (Some(_), None) => {
                return Err(invalid(
                    "selected_candidate requires payload branch identity",
                ));
            }
            (None, _) => {
                return Err(invalid(
                    "selected_candidate requires decision.selected_branch_id",
                ));
            }
        }

        Ok(())
    }

    fn validate_metrics(
        metrics: &selection_metrics::Set,
        considered_order_hash: &HistoryHash,
        candidate_set_root: Option<&CandidateSetRoot>,
    ) -> Result<(), HistoryError> {
        if &metrics.considered_order_hash != considered_order_hash {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "selection metrics considered_order_hash does not match decision input"
                    .to_string(),
            });
        }
        if metrics.candidate_set_root.as_ref() != candidate_set_root {
            return Err(HistoryError::InvalidSelectionDecision {
                detail: "selection metrics candidate_set_root does not match decision input"
                    .to_string(),
            });
        }
        Ok(())
    }

    fn candidate_set_for_considered(
        considered: &[EvaluationPayload],
        considered_sources: &[TraversalCandidateSource],
    ) -> Result<CandidateSetCommitment, HistoryError> {
        if considered_sources.is_empty() {
            CandidateSetCommitment::from_payloads(considered)
        } else {
            CandidateSetCommitment::from_payloads_with_sources(considered, considered_sources)
        }
    }

    fn selected_candidate_projection(
        selected_candidate: Option<SubjectRef>,
        selected_occurrence_id: Option<&CandidateOccurrenceId>,
        selected_membership_id: Option<&CandidateMembershipId>,
        candidate_set: &CandidateSetCommitment,
        considered: &[EvaluationPayload],
    ) -> Result<Option<SubjectRef>, HistoryError> {
        let selected_payload = Self::selected_payload_by_identity(
            selected_occurrence_id,
            selected_membership_id,
            candidate_set,
            considered,
        )?;
        match (selected_candidate, selected_payload) {
            (Some(candidate), Some(payload)) if candidate != payload.candidate => {
                Err(HistoryError::InvalidSelectionDecision {
                    detail: format!(
                        "selected_candidate projection mismatch: occurrence={}, selected_candidate={}",
                        payload.candidate.as_str(),
                        candidate.as_str()
                    ),
                })
            }
            (Some(candidate), _) => Ok(Some(candidate)),
            (None, Some(payload)) => Ok(Some(payload.candidate.clone())),
            (None, None) => Ok(None),
        }
    }

    fn selected_payload_by_identity<'a>(
        selected_occurrence_id: Option<&CandidateOccurrenceId>,
        selected_membership_id: Option<&CandidateMembershipId>,
        candidate_set: &CandidateSetCommitment,
        considered: &'a [EvaluationPayload],
    ) -> Result<Option<&'a EvaluationPayload>, HistoryError> {
        let membership = match (selected_occurrence_id, selected_membership_id) {
            (Some(occurrence_id), Some(membership_id)) => {
                let occurrence_member = candidate_set
                    .membership_by_occurrence_id(occurrence_id)
                    .ok_or_else(|| HistoryError::InvalidSelectionDecision {
                        detail: "selected occurrence is absent from candidate set".to_string(),
                    })?;
                let membership_member = candidate_set
                    .membership_by_membership_id(membership_id)
                    .ok_or_else(|| HistoryError::InvalidSelectionDecision {
                        detail: "selected membership is absent from candidate set".to_string(),
                    })?;
                if occurrence_member.membership_id != membership_member.membership_id {
                    return Err(HistoryError::InvalidSelectionDecision {
                        detail: "selected occurrence and membership refer to different candidates"
                            .to_string(),
                    });
                }
                Some(occurrence_member)
            }
            (Some(occurrence_id), None) => Some(
                candidate_set
                    .membership_by_occurrence_id(occurrence_id)
                    .ok_or_else(|| HistoryError::InvalidSelectionDecision {
                        detail: "selected occurrence is absent from candidate set".to_string(),
                    })?,
            ),
            (None, Some(membership_id)) => Some(
                candidate_set
                    .membership_by_membership_id(membership_id)
                    .ok_or_else(|| HistoryError::InvalidSelectionDecision {
                        detail: "selected membership is absent from candidate set".to_string(),
                    })?,
            ),
            (None, None) => None,
        };

        let Some(membership) = membership else {
            return Ok(None);
        };
        let payloads = considered
            .iter()
            .filter_map(|payload| {
                payload
                    .payload_hash()
                    .ok()
                    .filter(|hash| hash == &membership.payload_hash)
                    .map(|_| payload)
            })
            .collect::<Vec<_>>();
        match payloads.as_slice() {
            [payload] => Ok(Some(*payload)),
            [] => Err(HistoryError::InvalidSelectionDecision {
                detail: "selected candidate-set member has no considered payload".to_string(),
            }),
            many => Err(HistoryError::InvalidSelectionDecision {
                detail: format!(
                    "selected candidate-set member resolves to multiple payloads: count={}",
                    many.len()
                ),
            }),
        }
    }

    fn considered_order_preimage(
        considered: &[EvaluationPayload],
    ) -> Result<Vec<HistoryHash>, HistoryError> {
        considered
            .iter()
            .map(|payload| payload.payload_hash())
            .collect()
    }

    pub(crate) fn decision_hash(&self) -> Result<HistoryHash, HistoryError> {
        HistoryHash::of_domain_json("prototype1.history.selection_decision_entry.v1", self)
    }

    pub(crate) fn verify_considered_order_hash(&self) -> Result<bool, HistoryError> {
        let preimage = Self::considered_order_preimage(&self.considered)?;
        let h = HistoryHash::of_domain_json(
            "prototype1.history.selection_considered_order.v1",
            &preimage,
        )?;
        Ok(h == self.considered_order_hash)
    }

    pub(crate) fn verify_candidate_set_commitment(&self) -> Result<Option<bool>, HistoryError> {
        let Some(candidate_set) = &self.candidate_set else {
            return Ok(None);
        };
        let expected =
            Self::candidate_set_for_considered(&self.considered, &self.considered_sources)?;
        if expected.root != candidate_set.root
            || expected.memberships.len() != candidate_set.memberships.len()
        {
            return Ok(Some(false));
        }
        for expected_member in expected.memberships {
            let Some(member) = Self::matching_candidate_set_member(candidate_set, &expected_member)
            else {
                return Ok(Some(false));
            };
            if member.payload_hash != expected_member.payload_hash {
                return Ok(Some(false));
            }
            if !member.proof.verify(&candidate_set.root)? {
                return Ok(Some(false));
            }
        }
        Ok(Some(true))
    }

    pub(crate) fn candidate_set_membership(
        &self,
        candidate: &SubjectRef,
    ) -> Option<&CandidateSetMembership> {
        self.candidate_set.as_ref()?.membership(candidate)
    }

    pub(crate) fn candidate_set_membership_for_payload(
        &self,
        index: usize,
        payload: &EvaluationPayload,
    ) -> Result<Option<&CandidateSetMembership>, HistoryError> {
        let Some(candidate_set) = self.candidate_set.as_ref() else {
            return Ok(None);
        };
        if let Some(source) = self.considered_sources.get(index).copied() {
            return candidate_set.membership_for_payload(payload, source.candidate_source_class());
        }
        if let Some(occurrence_id) = payload.occurrence_id(candidate_set::source_class(payload))?
            && let Some(membership) = candidate_set.membership_by_occurrence_id(&occurrence_id)
        {
            return Ok(Some(membership));
        }
        Ok(candidate_set.membership(&payload.candidate))
    }

    fn matching_candidate_set_member<'a>(
        candidate_set: &'a CandidateSetCommitment,
        expected: &CandidateSetMembership,
    ) -> Option<&'a CandidateSetMembership> {
        if let Some(membership_id) = expected.membership_id.as_ref() {
            return candidate_set.membership_by_membership_id(membership_id);
        }
        if let Some(occurrence_id) = expected.occurrence_id.as_ref() {
            return candidate_set.membership_by_occurrence_id(occurrence_id);
        }
        candidate_set.membership(&expected.candidate)
    }

    fn payload_selected_by_decision(
        &self,
        membership: Option<&CandidateSetMembership>,
        payload: &EvaluationPayload,
    ) -> bool {
        if let Some(selected_membership_id) = self.selected_membership_id.as_ref() {
            return membership.and_then(|member| member.membership_id.as_ref())
                == Some(selected_membership_id);
        }
        if let Some(selected_occurrence_id) = self.selected_occurrence_id.as_ref() {
            return membership.and_then(|member| member.occurrence_id.as_ref())
                == Some(selected_occurrence_id);
        }
        self.selected_candidate.as_ref() == Some(&payload.candidate)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SelectionProjectionFailureId(pub(crate) HistoryHash);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionProjectionFailureKind {
    MissingSelectionInput,
    ChildEvidenceStoreLoadFailed,
    SelectionInputBindingInvalid,
    SelectionProcedureMismatch,
    CandidateSetMembershipMissing,
    CandidateSetPayloadHashMismatch,
    CandidateSetProofInvalid,
    DecisionGradeIneligible,
}

/// [`SelectionProjectionFailureId`] preimage: only fields that are also present on the serialized
/// [`SelectionProjectionFailure`] (so the id is explainable from the record). Domain
/// `prototype1.history.selection_projection_failure.v3`.
#[derive(Serialize)]
struct SelectionProjectionFailureIdPreimage<'a> {
    kind: &'a SelectionProjectionFailureKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_subject: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    committed_message: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SelectionProjectionFailure {
    pub(crate) id: SelectionProjectionFailureId,
    pub(crate) kind: SelectionProjectionFailureKind,
    /// Single-candidate coordinate when this failure is local to one considered child; absent when it applies to the whole considered set (e.g. store assembly).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) candidate: Option<SubjectRef>,
    /// Text committed beside the stable `id`; always included in the id preimage when present (
    /// e.g. filesystem load error, or `MissingSelectionInput` child outcome classification).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) committed_message: Option<String>,
}

impl SelectionProjectionFailure {
    pub(crate) fn committed(
        kind: SelectionProjectionFailureKind,
        candidate: Option<SubjectRef>,
        committed_message: Option<String>,
    ) -> Result<Self, HistoryError> {
        let id = Self::identity_hash(&kind, candidate.as_ref(), committed_message.as_deref())?;
        Ok(Self {
            id: SelectionProjectionFailureId(id),
            kind,
            candidate,
            committed_message,
        })
    }

    fn identity_hash(
        kind: &SelectionProjectionFailureKind,
        candidate: Option<&SubjectRef>,
        committed_message: Option<&str>,
    ) -> Result<HistoryHash, HistoryError> {
        let preimage = SelectionProjectionFailureIdPreimage {
            kind,
            candidate_subject: candidate.map(SubjectRef::as_str),
            committed_message,
        };
        HistoryHash::of_domain_json(
            "prototype1.history.selection_projection_failure.v3",
            &preimage,
        )
    }
}

/// Candidate-universe description for a selection decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SelectionScope {
    value: String,
}

/// Typed source for a [`SelectionScope`] projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope<K> {
    value: SelectionScope,
    _kind: PhantomData<fn() -> K>,
}

/// Marker for a generation-local selection universe under one parent node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Generation {}

pub(crate) trait ScopeFor<K> {
    type Coordinate;

    fn scope_for(&self, coordinate: Self::Coordinate) -> Scope<K>;
}

impl<K> Scope<K> {
    fn new(value: SelectionScope) -> Self {
        Self {
            value,
            _kind: PhantomData,
        }
    }

    pub(crate) fn into_selection_scope(self) -> SelectionScope {
        self.value
    }
}

impl Scope<Generation> {
    pub(crate) fn local(parent_node_id: impl AsRef<str>, generation: u32) -> Self {
        Self::new(SelectionScope::new(format!(
            "generation_local:parent_node_id={};generation={}",
            parent_node_id.as_ref(),
            generation
        )))
    }
}

impl SelectionScope {
    const ALL_ADMITTED_CANDIDATES: &'static str = "history:all_admitted_candidates";

    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub(crate) fn all_admitted_candidates() -> Self {
        Self::new(Self::ALL_ADMITTED_CANDIDATES)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }

    pub(crate) fn includes(&self, decision_scope: &SelectionScope) -> bool {
        self.value == Self::ALL_ADMITTED_CANDIDATES || self == decision_scope
    }
}
