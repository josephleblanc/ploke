//! Prototype 1 History invariant framework.
//!
//! Status recorded 2026-04-27 14:32 PDT. Implementation started from this
//! specification on 2026-04-27.
//!
//! This module defines the local invariant core for Prototype 1 History.
//! Current live handoff seals and appends a minimal History block before
//! launching the successor runtime. The live loop still uses typed transition
//! scaffolding, a transition journal, invocation files, successor-ready files,
//! and mutable scheduler/branch projections. Those records can be imported as
//! evidence later; they are not themselves sealed History authority.
//!
//! Update recorded 2026-04-29 22:05 PDT: in-code `Block<Open>` construction,
//! successor opening, and entry admission are now routed through
//! `Crown<Ruling>` methods. The live successor handoff also now routes block
//! sealing through the Crown transition and appends the sealed block before
//! successor launch.
//!
//! Update recorded 2026-04-30 10:13 PDT: the live successor handoff path now
//! checks the current clean Artifact tree against the current sealed History
//! head before entering the next parent path.
//!
//! Update recorded 2026-04-30 17:17 PDT: open blocks now commit to the
//! `HistoryStateRoot` observed when the lineage state was read, and append
//! rejects blocks opened from a different state root.
//!
//! Update recorded 2026-05-01 10:57 PDT: `HistoryStateRoot` is now derived from
//! a sparse Merkle map over the local lineage-head projection, and
//! `LineageState` carries the sparse proof for the observed lineage key.
//! `StoreHead::Absent` is therefore a local sparse-proof absence observation
//! under the current root. This is still not distributed consensus, a global
//! fork-choice rule, or OS-process uniqueness. Bootstrap admission remains
//! weaker than the full genesis authority model.
//!
//! The weekly review policy in `AGENTS.md` applies here: reviewers must compare
//! these claims against the actual code at least once per week while this
//! architecture is active, and must either narrow the claims or fix the
//! implementation when the code does not enforce them.
//!
//! ## Authoritative History model
//!
//! Treat these module docs as the local authority for Prototype 1 History while
//! this type stabilizes. Older design notes and audit reports may explain how
//! we got here, but this module should state the claims we are willing to make
//! about the current code.
//!
//! Update recorded 2026-04-29 10:35 UTC: the next design slice treats History
//! as a global authenticated store over lineage-local authority chains. A block
//! height is lineage-local; it is not the same as a global storage offset or
//! append position.
//!
//! History is the durable authority surface for admitted lineage facts. It is
//! not a scheduler snapshot, branch registry, CLI report, metrics dashboard,
//! preview aggregate, or database side table. Those are projections, caches, or
//! evidence sources. The semantic object is:
//!
//! ```text
//! History     = authenticated store over sealed lineage-local blocks
//! Lineage     = policy-governed projection over admitted Artifact continuity
//! Block       = one authority epoch for one lineage
//! Entry       = provenance-bearing fact admitted inside an epoch
//! Ingress     = append-only late/backchannel observation outside a sealed epoch
//! Regime      = phase/risk strategy context witnessed by a block
//! Projection  = disposable view or index derived from History or evidence
//! ```
//!
//! Terminology status recorded 2026-04-29 10:35 UTC: the terms
//! "transaction", "relation", "intervention", "policy", and "lineage
//! projection" are intentionally not fully formalized in this module yet.
//! Current code uses `Entry` as the implemented admission unit. Future block
//! contents should likely be expressed as admitted transactions or relations
//! over typed references, but that vocabulary needs its own definition before
//! it becomes an implementation claim.
//!
//! Intended, not fully implemented as of 2026-04-29 10:35 UTC: startup should
//! become an explicit admission procedure. A Runtime must first establish
//! `ProducedBy(SelfRuntime, CurrentArtifact)`, then establish
//! `AdmittedBy(CurrentArtifact, Lineage, Policy, History)` before it may enter
//! the ruling Parent path.
//!
//! The intended live authority sequence is:
//!
//! ```text
//! Startup<Observed>
//!   -> Startup<Genesis> | Startup<Predecessor>
//!   -> Startup<Validated>
//!   -> Parent<Ruling>
//!
//! BootstrapPolicy admits clean Tree<Key> for a lineage with no valid
//! associated History head in the configured store
//! BootstrapPolicy opens genesis Block<Open> for lineage-local height 0
//! Parent<Ruling> records entries while it has the Crown
//! Parent<Ruling> installs the selected Artifact
//! Parent<Ruling> locks Crown<Locked>
//! Crown<Locked> seals Block<Sealed>
//! incoming Runtime derives clean Tree<Key>
//! incoming Runtime verifies Tree<Key> against current Block<Sealed>
//! incoming Runtime imports admissible Ingress
//! incoming Runtime becomes Parent<Ruling>
//! Parent<Ruling> opens the next Block<Open> from predecessor authority
//! ```
//!
//! The genesis absence claim is local and store-scoped. It means "no valid
//! associated authority for this lineage/artifact is present in the configured
//! History store/root", not "no such authority exists anywhere". If the
//! configured store is unreadable, ambiguous, or inconsistent with the local
//! checkout, startup must reject rather than silently bootstrap.
//!
//! This is a cross-runtime contract, not merely an in-process state machine. The
//! outgoing Parent runtime locks the handoff material at the end of its rule.
//! A later successor runtime, built from the selected Artifact, verifies that
//! sealed material before it may become the next Parent. The type system keeps
//! both runtimes aligned to the same protocol even though the transition is
//! observed across process and artifact boundaries.
//!
//! This is the subtle part: the Crown is not locked because a single in-memory
//! object survives across both runtimes. The Crown is locked because, within the
//! shared contract compiled into both artifacts, the only valid way to make the
//! successor runtime executable is for the predecessor to cross the move-only
//! handoff transition and produce the sealed handoff material. If that contract
//! is preserved by the successor Artifact, a valid successor execution implies
//! that the predecessor has already moved out of the state with ruling access.
//!
//! Therefore the core invariant is:
//!
//! ```text
//! For one lineage, at most one valid typestate carrier may hold
//! Crown<Ruling>.
//!
//! During handoff, there may be zero rulers:
//! Parent<Ruling> has moved to a retired/non-ruling state, Crown<Locked> exists
//! as handoff evidence, and the successor runtime has not yet verified that
//! evidence into the next Parent<Ruling>.
//! ```
//!
//! Multiple runtimes may execute, especially around handoff. Execution is not
//! the same as Crown authority. The invariant is about which typed carriers can
//! mutate the lineage as ruling Parent.
//!
//! Intended local authority claim updated 2026-04-29 11:58 PDT: Crown/History
//! should gate artifact eligibility, not OS-process uniqueness. For one
//! lineage, the target startup gate is that a Runtime may enter the ruling
//! parent path only if its current clean Artifact tree matches the Artifact
//! committed by the current sealed History head. The live successor handoff
//! path now enforces this sealed-head Artifact check before the next runtime
//! enters the parent path; non-handoff/bootstrap startup still needs an
//! explicit admission carrier. Even with the sealed-head gate, this does not
//! prove that only one OS process is running from that Artifact; process
//! uniqueness requires a later lease, lock, or consensus mechanism.
//!
//! Update recorded 2026-04-30 10:13 PDT: the policy-bearing surface should be
//! modeled as part of the Artifact/Runtime contract, not as an independently
//! authoritative external `PolicyRef`. The current intended surface is the
//! `ploke-eval` code surface that contains the parent-creation, spawn,
//! History-admission, Crown, and handoff rules. Let `D` be the digest of that
//! surface. The invariant is closed over the admitted transition system:
//!
//! ```text
//! Base:
//!   the configured first Parent is admitted with policy-surface digest D.
//!
//! Step:
//!   a Parent whose Artifact has digest D may execute a child/successor
//!   runtime only after proving the child/successor Artifact also has digest D.
//!
//! Therefore:
//!   every executed child or descendant produced by this transition system has
//!   policy-surface digest D.
//! ```
//!
//! This is not a claim that no external process can compile or run incompatible
//! code. External processes are outside the quantified domain. The claim is
//! that incompatible code is not an admitted descendant and is not admissible to
//! the History/Crown mutation path under this digest-preserving transition
//! system. Changing the policy-bearing surface is therefore a protocol
//! upgrade/fork candidate, not an ordinary successor transition, unless a later
//! explicit upgrade rule admits it.
//!
//! Current bounded-surface policy implication: ordinary Prototype 1
//! self-improvement must not include this policy-bearing `ploke-eval` surface.
//! We do intend to allow that surface into the bounded edit scope later, but
//! only after defining a protocol-upgrade transition that replaces the
//! digest-preservation invariant with an explicit upgrade invariant. Until
//! then, any ordinary child/successor that changes the policy-bearing surface
//! must fail admission or execution before it can become an admitted descendant.
//!
//! The Crown is the one-at-a-time lineage authority. Parent is a role a Runtime
//! may hold; the Crown is the capability that prevents two Parents from
//! mutating the same lineage as if both were ruling. Future multi-parent or
//! consensus work must make the lineage coordinate explicit, so "one Crown"
//! means one Crown per lineage, not one global singleton for the whole tree.
//!
//! Update recorded 2026-05-01 16:26 PDT: Artifact identity is not exclusive to
//! one lineage. Multiple lineage heads may reference the same Artifact/tree key
//! under policy. Lineage authority is keyed by the History lineage coordinate,
//! not by git branch, worktree path, process id, runtime id, or Artifact
//! identity alone. Therefore a sealed head should not accidentally turn
//! `selected_successor` transport/debug identity into the authority source. The
//! authority question is whether the incoming Runtime satisfies
//! `MayEnterRuling(H, L, P, R_i)` for the lineage `L` under the current
//! policy-bearing runtime surface and sealed Artifact/surface commitments.
//!
//! A sealed block must be a projection of the authority transition, not a caller
//! assembled status blob. Mutable files such as `scheduler.json`,
//! `branches.json`, node records, invocation files, ready/completion files, and
//! monitor reports may be cited as evidence or projections, but they do not
//! become History authority until admitted into a sealed block or imported as
//! ingress under an explicit policy.
//!
//! Update recorded 2026-04-30 03:51 PDT: the block header now reserves a
//! `Regime` slot for the phase/risk strategy context in effect when an
//! authority epoch opens. This is not an external `Policy` object and should
//! not be read as a claim that a policy file has authority. It records the
//! current runtime-frame strategy hypothesis: long-horizon coherence and
//! improvement are expected to require a variable strategy that changes across
//! an absolute step axis and dependent environment axes, such as expansion,
//! evaluation, consolidation, and hardening. Current code only commits the
//! placeholder context into block hashes; it does not yet compute risk budgets,
//! phase transitions, or consensus/finality effects from it.
//!
//! Update recorded 2026-04-30 10:58 PDT: the block header now carries
//! `SurfaceCommitment` as the first structural carrier for the partitioned
//! Artifact surface:
//!
//! ```text
//! ArtifactSurface = Immutable + Mutated + Ambient
//! ```
//!
//! `Immutable` is stored as one root. The rule that ordinary succession must
//! preserve that root is runtime policy, and the rule itself must be inside the
//! immutable authority surface. Current Prototype 1 policy treats
//! `crates/ploke-eval` as immutable and all tool-description text files as the
//! mutated surface. `Mutated` and `Ambient` are before/after commitments so a
//! verifier can reconstruct the candidate Artifact transition without
//! executing the candidate runtime. Current live code computes this commitment
//! before successor execution; child execution validates the same surface
//! before build/hydration and after persisted Artifact commit.
//!
//! Partially implemented as of 2026-04-30 17:17 PDT: the store computes and
//! carries a root digest for the current local lineage-head map, and sealed
//! blocks commit to the root they were opened from. Intended, not yet
//! implemented: this should become an authenticated lineage-head map, likely
//! using a Merkle-Patricia trie or equivalent authenticated map rather than a
//! hand-rolled directory scan. That map should support present and absent
//! lineage-head proofs. The current filesystem `heads.json` projection is not
//! such a proof.
//!
//! Intended, not implemented as of 2026-04-29 10:35 UTC: admitted Artifacts
//! should carry an artifact-local provenance manifest committed by the Artifact
//! tree. History should admit the Artifact by committing to its backend tree key
//! plus manifest digest, leaving large evidence such as self-evaluations,
//! intervention details, and build/runtime records in the Artifact or external
//! content-addressed locations when policy permits.
//!
//! The current implementation enforces only part of this model:
//!
//! - `Block<Open>::seal` is private to this module.
//! - sealing requires a lineage-bound `Crown<Locked>` carrier.
//! - sealed blocks carry deterministic hashes and can be locally verified.
//! - live handoff appends a minimal sealed block before successor launch.
//! - a filesystem `BlockStore` can append sealed blocks and maintain
//!   rebuildable indexes.
//!
//! The current implementation does not yet enforce:
//!
//! - live `Parent<Ruling>` as the only writer of open block entries;
//! - a uniform bootstrap/predecessor admission carrier for every startup path;
//! - structural type-state representation of the child/successor surface gate;
//! - ingress capture/import while the Crown is locked;
//! - cryptographic signatures or distributed consensus.
//!
//! Therefore the current claim is local and narrow: this module defines and
//! partially enforces tamper-evident, lineage-scoped, transition-checked
//! History. It does not make the whole execution environment trustworthy, and
//! it does not upgrade existing Prototype 1 JSON records into authority by
//! reading or previewing them.
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
//! The next live integration still needs complete authority carriers outside
//! this module. The intended shape is:
//!
//! ```ignore
//! // Blocked until Prototype 1 has real live authority carriers and tree keys:
//! //
//! // BootstrapPolicy
//! //   .admit(tree_key, parent_identity)
//! //   -> Parent<Ruling>
//! //
//! // Parent<Ruling>
//! //   .seal_block_with_artifact(open_block, selected_successor, admitted_artifact)
//! //   -> (Parent<Retired>, Block<Sealed>)
//! //
//! // Runtime<Checked>
//! //   .verify(sealed_head, tree_key, parent_identity)
//! //   -> Parent<Ruling>
//! ```
//!
//! Blocking reasons:
//!
//! - `Parent<Ruling>` is not yet represented as `Startup<Validated>` in the
//!   type system. Update recorded 2026-04-30 12:20 PDT: the live successor
//!   startup path now validates the current clean Artifact tree and surface
//!   commitment against the sealed History head before entering the parent
//!   path; bootstrap startup still needs a uniform admission carrier.
//! - `Block<Open>` construction and entry admission are locally gated by
//!   `Crown<Ruling>`, but the current methods still accept actor identity
//!   fields as data until `Parent<Ruling>` supplies them structurally.
//! - live successor validation still consults mutable scheduler/invocation
//!   state for transport identity, but History admission now derives the tree
//!   key and surface commitment from the current checkout and checks both
//!   against the sealed head.
//! - gen0 setup currently writes and commits parent identity, but setup itself
//!   does not open or append a genesis History block. The first live handoff
//!   creates the genesis block if no configured History head exists.
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
//!
//! Generation is not the same concept as block height. In the current linear
//! prototype they may often move together, but branching, revisits, merges, and
//! cross-runtime generation over distinct `(Artifact, Runtime)` coordinates can
//! break that equivalence. Storage and projections must preserve the block's
//! lineage/head metadata explicitly instead of reconstructing authority from a
//! generation number, branch name, or scheduler frontier.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    fs::{self, OpenOptions},
    io::{self, Write},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use sha2::{Digest as ShaDigest, Sha256};
use sparse_merkle_tree::{
    CompiledMerkleProof, H256, SparseMerkleTree, default_store::DefaultStore, traits::Hasher,
};
use thiserror::Error;
use uuid::Uuid;

use super::event::{RecordedAt, RuntimeId};
use super::identity::ParentIdentity;
#[cfg(test)]
use super::identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentityRecord};
use crate::OperationalRunMetrics;
use crate::loop_graph::{ArtifactId, PatchId};
use crate::metric;

const SCHEMA_VERSION: u32 = 1;

/// Deterministic content digest used by History entries and blocks.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct HistoryHash(String);

impl HistoryHash {
    pub(crate) fn of_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }

    fn from_digest_bytes(bytes: [u8; 32]) -> Self {
        Self(encode_hex_32(&bytes))
    }

    fn to_digest_bytes(&self) -> Result<[u8; 32], String> {
        decode_hex_32(&self.0)
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
///
/// Storage serializes this as lowercase hex, but the in-memory value is a fixed
/// 32-byte digest so a `BlockHash` cannot carry an arbitrary string payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct BlockHash([u8; 32]);

impl BlockHash {
    fn from_hex(value: &str) -> Result<Self, String> {
        decode_hex_32(value).map(Self)
    }

    pub(crate) fn to_hex(self) -> String {
        encode_hex_32(&self.0)
    }

    fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl From<HistoryHash> for BlockHash {
    fn from(value: HistoryHash) -> Self {
        Self::from_hex(value.as_str()).expect("HistoryHash must be a SHA-256 hex digest")
    }
}

impl Serialize for BlockHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for BlockHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_hex(&value).map_err(de::Error::custom)
    }
}

impl fmt::Display for BlockHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_hex())
    }
}

fn encode_hex_32(bytes: &[u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn decode_hex_32(value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64 {
        return Err(format!(
            "expected 64 hex characters for BlockHash, got {}",
            value.len()
        ));
    }

    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = decode_hex_nibble(chunk[0])?;
        let low = decode_hex_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Ok(bytes)
}

fn decode_hex_nibble(byte: u8) -> Result<u8, String> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        b'A'..=b'F' => Ok(byte - b'A' + 10),
        _ => Err(format!("invalid hex character {:?}", byte as char)),
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

impl fmt::Display for EntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
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

/// Append-only storage port for sealed History blocks.
///
/// This is intentionally separate from the read-only preview `EvidenceStore`
/// and the intervention `RecordStore`. It stores authority-bearing
/// `Block<Sealed>` values and may maintain rebuildable indexes.
///
/// `append` is the only semantic operation that may advance the lineage head.
/// Filesystem details such as `heads.json` are projections of the sealed block
/// stream, not independent authority. A database-backed implementation may use
/// rows or transactions instead, but it must preserve the same contract: the
/// head is derived from accepted sealed blocks, not written as a free-standing
/// status field.
///
/// Update recorded 2026-05-01 10:57 PDT: this trait is still a local prototype
/// port, not the final distributed store contract. `LineageState` now includes
/// a sparse-Merkle proof for the local projected lineage-head map, while
/// `StoreHead` remains the domain projection of that proof into absent/present
/// predecessor state. `append` must consume the expected lineage state so a
/// sealed block can advance a lineage only from the observed absent or present
/// predecessor state and the state root it was opened from.
///
pub(crate) trait BlockStore {
    type Error;

    fn append(
        &self,
        expected: &LineageState,
        block: &Block<block::Sealed>,
    ) -> Result<StoredBlock, Self::Error>;

    fn lineage_state(&self, lineage: &LineageId) -> Result<LineageState, Self::Error>;
}

/// Filesystem-backed sealed block store for Prototype 1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FsBlockStore {
    root: PathBuf,
}

impl FsBlockStore {
    const SEGMENT_NAME: &'static str = "segment-000000.jsonl";

    pub(crate) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub(crate) fn for_campaign_manifest(manifest_path: &Path) -> Self {
        let root = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("prototype1")
            .join("history");
        Self::new(root)
    }

    fn blocks_dir(&self) -> PathBuf {
        self.root.join("blocks")
    }

    fn index_dir(&self) -> PathBuf {
        self.root.join("index")
    }

    fn segment_path(&self) -> PathBuf {
        self.blocks_dir().join(Self::SEGMENT_NAME)
    }

    fn by_hash_path(&self) -> PathBuf {
        self.index_dir().join("by-hash.jsonl")
    }

    fn by_lineage_height_path(&self) -> PathBuf {
        self.index_dir().join("by-lineage-height.jsonl")
    }

    fn heads_path(&self) -> PathBuf {
        self.index_dir().join("heads.json")
    }

    fn ensure_dirs(&self) -> Result<(), BlockStoreError> {
        fs::create_dir_all(self.blocks_dir()).map_err(|source| BlockStoreError::CreateDir {
            path: self.blocks_dir(),
            source,
        })?;
        fs::create_dir_all(self.index_dir()).map_err(|source| BlockStoreError::CreateDir {
            path: self.index_dir(),
            source,
        })?;
        Ok(())
    }

    fn append_jsonl<T: Serialize>(&self, path: &Path, value: &T) -> Result<(), BlockStoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| BlockStoreError::CreateDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .map_err(|source| BlockStoreError::Open {
                path: path.to_path_buf(),
                source,
            })?;
        let mut line = serde_json::to_string(value).map_err(BlockStoreError::Serialize)?;
        line.push('\n');
        file.write_all(line.as_bytes())
            .map_err(|source| BlockStoreError::Write {
                path: path.to_path_buf(),
                source,
            })?;
        file.sync_data().map_err(|source| BlockStoreError::Sync {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    }

    fn read_heads(&self) -> Result<BTreeMap<LineageId, BlockHash>, BlockStoreError> {
        let path = self.heads_path();
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(BlockStoreError::Deserialize),
            Err(source) if source.kind() == io::ErrorKind::NotFound => {
                if self.has_stored_blocks()? {
                    Err(BlockStoreError::MissingHeadsProjection { path })
                } else {
                    Ok(BTreeMap::new())
                }
            }
            Err(source) => Err(BlockStoreError::Read { path, source }),
        }
    }

    fn has_stored_blocks(&self) -> Result<bool, BlockStoreError> {
        for path in [
            self.segment_path(),
            self.by_hash_path(),
            self.by_lineage_height_path(),
        ] {
            match fs::metadata(&path) {
                Ok(metadata) if metadata.len() > 0 => return Ok(true),
                Ok(_) => {}
                Err(source) if source.kind() == io::ErrorKind::NotFound => {}
                Err(source) => return Err(BlockStoreError::Read { path, source }),
            }
        }
        Ok(false)
    }

    fn has_lineage_index(&self, lineage: &LineageId) -> Result<bool, BlockStoreError> {
        let path = self.by_lineage_height_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(source) => return Err(BlockStoreError::Read { path, source }),
        };
        for line in text.lines() {
            let stored: LineageHeight =
                serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
            if &stored.lineage_id == lineage {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn write_heads(&self, heads: &BTreeMap<LineageId, BlockHash>) -> Result<(), BlockStoreError> {
        let path = self.heads_path();
        let bytes = serde_json::to_vec_pretty(heads).map_err(BlockStoreError::Serialize)?;
        fs::write(&path, bytes).map_err(|source| BlockStoreError::Write { path, source })
    }

    fn stored_record_by_hash(
        &self,
        lineage: &LineageId,
        block_hash: &BlockHash,
    ) -> Result<StoredBlock, BlockStoreError> {
        let path = self.by_hash_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => return Err(BlockStoreError::Read { path, source }),
        };
        for line in text.lines() {
            let stored: StoredBlock =
                serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
            if &stored.lineage_id == lineage && &stored.block_hash == block_hash {
                return Ok(stored);
            }
        }
        Err(BlockStoreError::MissingHeadIndex {
            lineage_id: lineage.clone(),
            block_hash: *block_hash,
        })
    }

    fn stored_by_hash(
        &self,
        lineage: &LineageId,
        block_hash: &BlockHash,
    ) -> Result<BlockHead, BlockStoreError> {
        self.stored_record_by_hash(lineage, block_hash)
            .map(|stored| BlockHead {
                block_hash: stored.block_hash,
                lineage_id: stored.lineage_id,
                block_height: stored.block_height,
            })
    }

    /// Load and verify the sealed block currently named by a checked head.
    ///
    /// This is deliberately a loader transition instead of `Deserialize` for
    /// `Block<block::Sealed>`. The current Prototype 1 handoff blocks have no
    /// admitted entries; until entry loading has its own transition, this method
    /// rejects non-empty stored blocks instead of silently reconstructing them.
    pub(crate) fn sealed_head_block(
        &self,
        head: &BlockHead,
    ) -> Result<Block<block::Sealed>, BlockStoreError> {
        let stored = self.stored_record_by_hash(&head.lineage_id, &head.block_hash)?;
        let path = self.blocks_dir().join(&stored.location.segment);
        let text = fs::read_to_string(&path).map_err(|source| BlockStoreError::Read {
            path: path.clone(),
            source,
        })?;
        let line = text
            .lines()
            .nth(stored.location.line_index as usize)
            .ok_or_else(|| BlockStoreError::MissingStoredBlockLine {
                path: path.clone(),
                line_index: stored.location.line_index,
            })?;
        let stored_block: StoredSealedBlock =
            serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
        let block = stored_block.into_verified_block(path, stored.location.line_index)?;
        block.verify_expected_hash(&head.block_hash)?;
        Ok(block)
    }

    /// Load and verify every sealed block line from the primary segment file, in
    /// append order. Missing segment file yields an empty list.
    pub(crate) fn load_segment_verified_blocks(
        &self,
    ) -> Result<Vec<(u64, Block<block::Sealed>)>, BlockStoreError> {
        let path = self.segment_path();
        let text = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(BlockStoreError::Read {
                    path: path.clone(),
                    source,
                });
            }
        };
        let mut out = Vec::new();
        for (line_index, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let stored_block: StoredSealedBlock =
                serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
            let block = stored_block.into_verified_block(path.clone(), line_index as u64)?;
            out.push((line_index as u64, block));
        }
        Ok(out)
    }
}

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

impl BlockStore for FsBlockStore {
    type Error = BlockStoreError;

    fn append(
        &self,
        expected: &LineageState,
        block: &Block<block::Sealed>,
    ) -> Result<StoredBlock, Self::Error> {
        block.verify_hash()?;
        self.ensure_dirs()?;
        let lineage_id = block.header().common.lineage_id.clone();
        let current = self.lineage_state(&lineage_id)?;
        if &current != expected {
            return Err(BlockStoreError::StaleStoreHead {
                expected: expected.clone(),
                actual: current,
            });
        }
        expected.verify_append(block)?;
        let mut heads = self.read_heads()?;

        let segment_path = self.segment_path();
        let location = BlockLocation {
            segment: Self::SEGMENT_NAME.to_string(),
            line_index: count_lines(&segment_path)?,
        };
        let stored = StoredBlock {
            block_hash: *block.block_hash(),
            lineage_id: block.header().common.lineage_id.clone(),
            block_height: block.header().common.block_height,
            location,
        };

        self.append_jsonl(&segment_path, block)?;
        self.append_jsonl(&self.by_hash_path(), &stored)?;
        self.append_jsonl(
            &self.by_lineage_height_path(),
            &LineageHeight {
                lineage_id: stored.lineage_id.clone(),
                block_height: stored.block_height,
                block_hash: stored.block_hash,
            },
        )?;

        heads.insert(stored.lineage_id.clone(), stored.block_hash);
        self.write_heads(&heads)?;

        Ok(stored)
    }

    fn lineage_state(&self, lineage: &LineageId) -> Result<LineageState, Self::Error> {
        let heads = self.read_heads()?;
        let map = state_map::Map::from_heads(&heads)?;
        let root = map.root();
        let Some(block_hash) = heads.get(lineage).cloned() else {
            if self.has_lineage_index(lineage)? {
                return Err(BlockStoreError::MissingLineageHeadProjection {
                    lineage_id: lineage.clone(),
                });
            }
            let head = StoreHead::Absent {
                lineage_id: lineage.clone(),
            };
            let proof = map.proof(lineage, &head)?;
            return Ok(LineageState::new(root, proof, head));
        };
        let head = StoreHead::Present(self.stored_by_hash(lineage, &block_hash)?);
        let proof = map.proof(lineage, &head)?;
        Ok(LineageState::new(root, proof, head))
    }
}

fn count_lines(path: &Path) -> Result<u64, BlockStoreError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(text.lines().count() as u64),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(source) => Err(BlockStoreError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Physical location of one sealed block in the append-only block store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BlockLocation {
    segment: String,
    line_index: u64,
}

/// Result of appending a sealed block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct StoredBlock {
    block_hash: BlockHash,
    lineage_id: LineageId,
    block_height: u64,
    location: BlockLocation,
}

impl StoredBlock {
    pub(crate) fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }

    pub(crate) fn block_height(&self) -> u64 {
        self.block_height
    }
}

#[derive(Debug, Deserialize)]
struct StoredSealedBlock {
    state: StoredSealedState,
    entries: Vec<stored::StoredEntryAdmitted>,
}

#[derive(Debug, Deserialize)]
struct StoredSealedState {
    header: SealedBlockHeader,
    #[serde(rename = "_private")]
    _private: serde::de::IgnoredAny,
}

impl StoredSealedBlock {
    fn into_verified_block(
        self,
        path: PathBuf,
        line_index: u64,
    ) -> Result<Block<block::Sealed>, BlockStoreError> {
        if self.entries.len() != self.state.header.entry_count {
            return Err(BlockStoreError::UnsupportedStoredEntries {
                path,
                line_index,
                entry_count: self.state.header.entry_count,
            });
        }

        let mut entries = Vec::with_capacity(self.entries.len());
        for stored in self.entries {
            entries.push(stored.into_entry());
        }

        let block = Block {
            state: block::Sealed {
                header: self.state.header,
                _private: Private,
            },
            entries,
        };
        block.verify_hash()?;
        Ok(block)
    }
}

/// Stored DTOs for verified disk loading.
///
/// Authoritative typestate carriers intentionally do not derive `Deserialize`.
/// Disk loading is routed through these DTOs plus `Block::verify_hash()`.
mod stored {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Clone, Deserialize)]
    pub(super) struct StoredEntryAdmitted {
        core: StoredEntryCore,
        state: StoredAdmitted,
    }

    impl StoredEntryAdmitted {
        pub(super) fn into_entry(self) -> Entry<Admitted> {
            Entry {
                core: self.core.into_core(),
                state: self.state.into_state(),
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    struct StoredEntryCore {
        entry_id: EntryId,
        entry_kind: EntryKind,
        subject: SubjectRef,
        executor: ActorRef,
        input_refs: Vec<EvidenceRef>,
        output_refs: Vec<EvidenceRef>,
        occurred_at: RecordedAt,
        payload: StoredEntryPayload,
    }

    impl StoredEntryCore {
        fn into_core(self) -> EntryCore {
            EntryCore {
                entry_id: self.entry_id,
                entry_kind: self.entry_kind,
                subject: self.subject,
                executor: self.executor,
                input_refs: self.input_refs,
                output_refs: self.output_refs,
                occurred_at: self.occurred_at,
                payload: self.payload.into_payload(),
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(rename_all = "snake_case", tag = "kind")]
    enum StoredEntryPayload {
        Direct,
        SelectionDecision(SelectionDecisionEntry),
        IngressImport(IngressImportPayload),
    }

    impl StoredEntryPayload {
        fn into_payload(self) -> EntryPayload {
            match self {
                StoredEntryPayload::Direct => EntryPayload::Direct,
                StoredEntryPayload::SelectionDecision(value) => {
                    EntryPayload::SelectionDecision(value)
                }
                StoredEntryPayload::IngressImport(value) => EntryPayload::IngressImport(value),
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    struct StoredAdmitted {
        observed: StoredObserved,
        proposer: ActorRef,
        procedure_or_policy: ProcedureRef,
        admitting_authority: ActorRef,
        ruling_authority: ActorRef,
        lineage_id: LineageId,
        block_id: BlockId,
        block_height: u64,
    }

    impl StoredAdmitted {
        fn into_state(self) -> Admitted {
            Admitted {
                observed: self.observed.into_state(),
                proposer: self.proposer,
                procedure_or_policy: self.procedure_or_policy,
                admitting_authority: self.admitting_authority,
                ruling_authority: self.ruling_authority,
                lineage_id: self.lineage_id,
                block_id: self.block_id,
                block_height: self.block_height,
            }
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    struct StoredObserved {
        observer: ActorRef,
        recorder: ActorRef,
        operational_environment: OperationalEnvironment,
        payload_ref: EvidenceRef,
        payload_hash: HistoryHash,
        observed_at: RecordedAt,
        recorded_at: RecordedAt,
    }

    impl StoredObserved {
        fn into_state(self) -> Observed {
            Observed {
                observer: self.observer,
                recorder: self.recorder,
                operational_environment: self.operational_environment,
                payload_ref: self.payload_ref,
                payload_hash: self.payload_hash,
                observed_at: self.observed_at,
                recorded_at: self.recorded_at,
            }
        }
    }
}

/// Store-derived current head for one lineage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct BlockHead {
    block_hash: BlockHash,
    lineage_id: LineageId,
    block_height: u64,
}

impl BlockHead {
    pub(crate) fn block_hash(&self) -> &BlockHash {
        &self.block_hash
    }

    pub(crate) fn block_height(&self) -> u64 {
        self.block_height
    }
}

/// Root commitment for the local History state map.
///
/// This is the sparse-Merkle root for the current local filesystem projection
/// of lineage heads. It is carried explicitly so block opening and append say:
/// "this block was opened from this observed History state". For a distributed
/// store, the same role must be backed by the consensus-selected state root;
/// local proof validity alone does not establish global canonical state.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct HistoryStateRoot(HistoryHash);

impl HistoryStateRoot {
    fn from_h256(root: H256) -> Self {
        Self(HistoryHash::from_digest_bytes(root.into()))
    }

    fn to_h256(&self) -> Result<H256, BlockStoreError> {
        self.0
            .to_digest_bytes()
            .map(H256::from)
            .map_err(BlockStoreError::StateRootDigest)
    }

    #[cfg(test)]
    fn test(label: &'static str) -> Self {
        Self(HistoryHash::of_bytes(label.as_bytes()))
    }
}

mod state_map {
    use super::*;

    type Tree = SparseMerkleTree<Sha256StateHasher, H256, DefaultStore<H256>>;

    pub(super) struct Map {
        tree: Tree,
    }

    impl Map {
        pub(super) fn from_heads(
            heads: &BTreeMap<LineageId, BlockHash>,
        ) -> Result<Self, BlockStoreError> {
            let mut tree = Tree::default();
            let leaves = heads
                .iter()
                .map(|(lineage_id, block_hash)| {
                    Ok((key(lineage_id)?, value_for_hash(*block_hash)?))
                })
                .collect::<Result<Vec<_>, BlockStoreError>>()?;
            tree.update_all(leaves).map_err(BlockStoreError::StateMap)?;
            Ok(Self { tree })
        }

        pub(super) fn root(&self) -> HistoryStateRoot {
            HistoryStateRoot::from_h256(*self.tree.root())
        }

        pub(super) fn proof(
            &self,
            lineage_id: &LineageId,
            head: &StoreHead,
        ) -> Result<Proof, BlockStoreError> {
            let key = key(lineage_id)?;
            let value = value_for_head(head)?;
            let proof = self
                .tree
                .merkle_proof(vec![key])
                .map_err(BlockStoreError::StateMap)?
                .compile(vec![key])
                .map_err(BlockStoreError::StateMap)?;
            let proof = Proof {
                key: key.into(),
                value: value.into(),
                program: proof.into(),
            };
            proof.verify(&self.root(), head)?;
            Ok(proof)
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub(super) struct Proof {
        key: [u8; 32],
        value: [u8; 32],
        program: Vec<u8>,
    }

    impl Proof {
        pub(super) fn verify(
            &self,
            root: &HistoryStateRoot,
            head: &StoreHead,
        ) -> Result<(), BlockStoreError> {
            let expected_key = key(head.lineage_id())?;
            let expected_value = value_for_head(head)?;
            let key = H256::from(self.key);
            let value = H256::from(self.value);
            if key != expected_key || value != expected_value {
                return Err(BlockStoreError::StateProofMismatch);
            }
            let root = root.to_h256()?;
            let proof = CompiledMerkleProof(self.program.clone());
            let verified = proof
                .verify::<Sha256StateHasher>(&root, vec![(key, value)])
                .map_err(BlockStoreError::StateMap)?;
            if !verified {
                return Err(BlockStoreError::StateProofMismatch);
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct Sha256StateHasher {
        bytes: Vec<u8>,
    }

    impl Hasher for Sha256StateHasher {
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

    fn key(lineage_id: &LineageId) -> Result<H256, BlockStoreError> {
        HistoryHash::of_domain_json("prototype1.history.state.key.v1", lineage_id)
            .map(|hash| H256::from(BlockHash::from(hash).to_bytes()))
            .map_err(BlockStoreError::Verify)
    }

    fn value_for_hash(block_hash: BlockHash) -> Result<H256, BlockStoreError> {
        let value = HistoryHash::of_domain_json("prototype1.history.state.value.v1", &block_hash)
            .map(|hash| H256::from(BlockHash::from(hash).to_bytes()))
            .map_err(BlockStoreError::Verify)?;
        if value.is_zero() {
            return Err(BlockStoreError::StateValueZero);
        }
        Ok(value)
    }

    fn value_for_head(head: &StoreHead) -> Result<H256, BlockStoreError> {
        match head.block_hash().copied() {
            Some(block_hash) => value_for_hash(block_hash),
            None => Ok(H256::zero()),
        }
    }
}

/// Local state-map observation for one lineage.
///
/// `StoreHead` remains the single-lineage predecessor/absence projection, while
/// `HistoryStateRoot` commits the surrounding map state from which that
/// projection was read. `state_map::Proof` is the sparse-Merkle proof for the
/// observed lineage key. It proves the local projected state under this root:
/// `Absent` verifies as the zero value for the lineage key, and `Present`
/// verifies as a domain-separated digest of the current block hash. This is
/// still a local single-ruler store, not distributed consensus or process
/// uniqueness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LineageState {
    root: HistoryStateRoot,
    proof: state_map::Proof,
    head: StoreHead,
}

impl LineageState {
    fn new(root: HistoryStateRoot, proof: state_map::Proof, head: StoreHead) -> Self {
        Self { root, proof, head }
    }

    pub(crate) fn root(&self) -> &HistoryStateRoot {
        &self.root
    }

    pub(crate) fn head(&self) -> &StoreHead {
        &self.head
    }

    #[cfg(test)]
    fn lineage_id(&self) -> &LineageId {
        self.head.lineage_id()
    }

    fn verify_append(&self, block: &Block<block::Sealed>) -> Result<(), BlockStoreError> {
        self.proof.verify(self.root(), self.head())?;
        if &block.header().common.opened_from_state != self.root() {
            return Err(BlockStoreError::WrongOpeningStateRoot {
                expected: self.root.clone(),
                actual: block.header().common.opened_from_state.clone(),
            });
        }
        self.head.verify_append(block)
    }
}

/// Store-derived predecessor state for one lineage.
///
/// This is the local filesystem predecessor proof used by the current
/// single-ruler implementation. It is deliberately weaker than the future
/// authenticated lineage-head map: `Absent` means "no head for this lineage in
/// this checked store after projection consistency checks", not "no such head
/// exists globally".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StoreHead {
    Absent { lineage_id: LineageId },
    Present(BlockHead),
}

impl StoreHead {
    pub(crate) fn lineage_id(&self) -> &LineageId {
        match self {
            Self::Absent { lineage_id } => lineage_id,
            Self::Present(head) => &head.lineage_id,
        }
    }

    pub(crate) fn block_hash(&self) -> Option<&BlockHash> {
        match self {
            Self::Absent { .. } => None,
            Self::Present(head) => Some(&head.block_hash),
        }
    }

    pub(crate) fn block_height(&self) -> Option<u64> {
        match self {
            Self::Absent { .. } => None,
            Self::Present(head) => Some(head.block_height),
        }
    }

    fn verify_append(&self, block: &Block<block::Sealed>) -> Result<(), BlockStoreError> {
        let block_lineage = &block.header().common.lineage_id;
        if self.lineage_id() != block_lineage {
            return Err(BlockStoreError::WrongStoreHeadLineage {
                expected: self.lineage_id().clone(),
                actual: block_lineage.clone(),
            });
        }

        match self {
            Self::Absent { lineage_id } => {
                if block.header().common.block_height != 0 {
                    return Err(BlockStoreError::NonGenesisWithoutHead {
                        lineage_id: lineage_id.clone(),
                        block_height: block.header().common.block_height,
                    });
                }
                if !block.header().common.parent_block_hashes.is_empty() {
                    return Err(BlockStoreError::GenesisWithStoreParents {
                        lineage_id: lineage_id.clone(),
                    });
                }
            }
            Self::Present(head) => {
                if block.header().common.block_height == 0 {
                    return Err(BlockStoreError::DuplicateGenesis {
                        lineage_id: head.lineage_id.clone(),
                    });
                }
                let expected_height = head.block_height + 1;
                if block.header().common.block_height != expected_height {
                    return Err(BlockStoreError::NonConsecutiveHeight {
                        lineage_id: head.lineage_id.clone(),
                        expected: expected_height,
                        actual: block.header().common.block_height,
                    });
                }
                if !block
                    .header()
                    .common
                    .parent_block_hashes
                    .contains(&head.block_hash)
                {
                    return Err(BlockStoreError::WrongStoreHeadParent {
                        lineage_id: head.lineage_id.clone(),
                        expected: head.block_hash,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Rebuildable projection from `(lineage, height)` to block hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LineageHeight {
    lineage_id: LineageId,
    block_height: u64,
    block_hash: BlockHash,
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
pub(crate) struct ArtifactRef {
    value: String,
}

impl ArtifactRef {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.value
    }
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn checked(
        producer_id: impl Into<String>,
        proposal_id: impl Into<String>,
        run_id: impl Into<String>,
        policy: impl Into<String>,
        target_relpath: PathBuf,
        base: SurfaceArtifactRef,
        after: SurfaceArtifactRef,
        patch_id: PatchId,
        source_content_hash: impl Into<String>,
        proposed_content_hash: impl Into<String>,
        proposal_producer: super::edit_surface::request_policy::ProposalProducer,
        generator_surface: super::edit_surface::tui::GeneratorSurfaceVersion,
        touches: Vec<SurfaceTouch>,
    ) -> Result<Self, HistoryError> {
        let touches_digest = HistoryHash::of_domain_json(
            "prototype1.history.surface_evidence.touches.v1",
            &touches,
        )?;
        let delta = SurfaceDeltaPreimage {
            base: &base,
            after: &after,
            patch_id: &patch_id,
            touches_digest: &touches_digest,
        };
        let delta_digest =
            HistoryHash::of_domain_json("prototype1.history.surface_evidence.delta.v1", &delta)?;
        let delta_id = format!("surface-delta:{}", delta_digest.as_str());
        Ok(Self {
            schema_version: 2,
            producer_id: producer_id.into(),
            proposal_id: proposal_id.into(),
            run_id: run_id.into(),
            policy: policy.into(),
            target_relpath,
            base,
            after,
            patch_id,
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
    }

    pub(crate) fn verify_integrity(&self) -> Result<(), HistoryError> {
        if !matches!(self.schema_version, 1 | 2) {
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
        Ok(Self {
            schema_version: if selected_occurrence_id.is_some() || selected_membership_id.is_some()
            {
                3
            } else {
                2
            },
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
            decision,
        })
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

    /// When this entry carries a selection decision payload, checks that
    /// [`Self::payload_hash`] matches [`SelectionDecisionEntry::decision_hash`].
    pub(crate) fn verify_selection_decision_observation(
        &self,
    ) -> Result<Option<bool>, HistoryError> {
        let Some(selection) = self.selection_decision() else {
            return Ok(None);
        };
        let expected = selection.decision_hash()?;
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
        campaign_id: "campaign:test".to_string(),
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
                ArtifactRef::new("artifact:successor"),
            ),
            test_parent_identity(),
            ArtifactRef::new("artifact:successor"),
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
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Block<S> {
    entries: Vec<Entry<Admitted>>,
    state: S,
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
        "stored History block at '{}':{} has {entry_count} entries; verified entry loading is not implemented yet",
        path.display(),
        line_index
    )]
    UnsupportedStoredEntries {
        path: PathBuf,
        line_index: u64,
        entry_count: usize,
    },

    #[error("sealed block failed verification before storage")]
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
            .artifact(ArtifactRef::new("artifact:base"))
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
            opened_from_artifact: ArtifactRef::new("artifact:base"),
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

    #[test]
    fn candidate_occurrence_id_is_deterministic_for_same_preimage() {
        let lineage = LineageId::new("lineage:a");
        let coordinate = occurrence_coordinate("node-a", "branch-a", 0);
        let artifact = ArtifactRef::new("artifact:a");
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
                ArtifactRef::new("artifact:successor"),
            ),
            selected_parent_identity: test_parent_identity(),
            active_artifact: ArtifactRef::new("artifact:successor"),
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
                    opened_from_artifact: ArtifactRef::new("artifact:successor"),
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
            opened_from_artifact: ArtifactRef::new("artifact:base"),
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
            opened_from_artifact: ArtifactRef::new("artifact:base"),
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
            opened_from_artifact: ArtifactRef::new("artifact:base"),
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
            opened_from_artifact: ArtifactRef::new("artifact:base"),
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
                opened_from_artifact: ArtifactRef::new("artifact:base"),
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
                opened_from_artifact: ArtifactRef::new("artifact:base"),
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
                opened_from_artifact: ArtifactRef::new("artifact:successor"),
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
