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
use crate::successor_selection::metrics as selection_metrics;

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

include!("stored/mod.rs");
include!("projection/mod.rs");
include!("seal/mod.rs");
