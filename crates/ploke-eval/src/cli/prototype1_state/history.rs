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
//! successor launch. The successor still does not verify that sealed head on
//! startup, so History is not yet the live admission gate.
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
//! committed by the current sealed History head. The live startup path does not
//! yet enforce that sealed-head admission; it still relies on checkout, parent
//! identity, scheduler, and invocation evidence. Even after the sealed-head gate
//! is wired, this does not prove that only one OS process is running from that
//! Artifact; process uniqueness requires a later lease, lock, or consensus
//! mechanism.
//!
//! The Crown is the one-at-a-time lineage authority. Parent is a role a Runtime
//! may hold; the Crown is the capability that prevents two Parents from
//! mutating the same lineage as if both were ruling. Future multi-parent or
//! consensus work must make the lineage coordinate explicit, so "one Crown"
//! means one Crown per lineage, not one global singleton for the whole tree.
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
//! Intended, not implemented as of 2026-04-29 10:35 UTC: the store should
//! eventually maintain an authenticated lineage-head map, likely using a
//! Merkle-Patricia trie or equivalent authenticated map rather than a
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
//! - successor verification of the predecessor `Block<Sealed>` before becoming
//!   authoritative;
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
//! //   .seal_block(open_block, selected_successor)
//! //   -> (Parent<Retired>, Block<Sealed>)
//! //
//! // Runtime<Checked>
//! //   .verify(sealed_head, tree_key, parent_identity)
//! //   -> Parent<Ruling>
//! ```
//!
//! Blocking reasons:
//!
//! - `Parent<Ruling>` is not yet gated by `Startup<Validated>`. The live
//!   handoff now seals and appends a lineage-bound History block before
//!   successor launch, but the incoming Runtime does not yet validate through
//!   History before entering the parent path.
//! - `Block<Open>` construction and entry admission are locally gated by
//!   `Crown<Ruling>`, but the current methods still accept actor identity
//!   fields as data until `Parent<Ruling>` supplies them structurally.
//! - live successor validation still consults mutable scheduler/invocation
//!   state instead of deriving `Tree<Key>` from the current checkout and
//!   checking that it matches the current sealed History head.
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
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::{self, Write},
    marker::PhantomData,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as ShaDigest, Sha256};
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
/// Update recorded 2026-04-30 04:08 PDT: this trait is still a local
/// prototype port, not the final authenticated store contract. `StoreHead`
/// validates only the filesystem store projections available today; it is not a
/// Merkle/authenticated inclusion or absence proof. `append` must still consume
/// the expected store head so a sealed block can advance a lineage only from a
/// verified absent or present predecessor state.
///
pub(crate) trait BlockStore {
    type Error;

    fn append(
        &self,
        expected: &StoreHead,
        block: &Block<block::Sealed>,
    ) -> Result<StoredBlock, Self::Error>;

    fn head(&self, lineage: &LineageId) -> Result<StoreHead, Self::Error>;
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

    fn stored_by_hash(
        &self,
        lineage: &LineageId,
        block_hash: &BlockHash,
    ) -> Result<BlockHead, BlockStoreError> {
        let path = self.by_hash_path();
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(source) => return Err(BlockStoreError::Read { path, source }),
        };
        for line in text.lines() {
            let stored: StoredBlock =
                serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
            if &stored.lineage_id == lineage && &stored.block_hash == block_hash {
                return Ok(BlockHead {
                    block_hash: stored.block_hash,
                    lineage_id: stored.lineage_id,
                    block_height: stored.block_height,
                });
            }
        }
        Err(BlockStoreError::MissingHeadIndex {
            lineage_id: lineage.clone(),
            block_hash: block_hash.clone(),
        })
    }
}

impl BlockStore for FsBlockStore {
    type Error = BlockStoreError;

    fn append(
        &self,
        expected: &StoreHead,
        block: &Block<block::Sealed>,
    ) -> Result<StoredBlock, Self::Error> {
        block.verify_hash()?;
        self.ensure_dirs()?;
        let lineage_id = block.header().common.lineage_id.clone();
        let current = self.head(&lineage_id)?;
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
            block_hash: block.block_hash().clone(),
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
                block_hash: stored.block_hash.clone(),
            },
        )?;

        heads.insert(stored.lineage_id.clone(), stored.block_hash.clone());
        self.write_heads(&heads)?;

        Ok(stored)
    }

    fn head(&self, lineage: &LineageId) -> Result<StoreHead, Self::Error> {
        let Some(block_hash) = self.read_heads()?.get(lineage).cloned() else {
            if self.has_lineage_index(lineage)? {
                return Err(BlockStoreError::MissingLineageHeadProjection {
                    lineage_id: lineage.clone(),
                });
            }
            return Ok(StoreHead::Absent {
                lineage_id: lineage.clone(),
            });
        };
        self.stored_by_hash(lineage, &block_hash)
            .map(StoreHead::Present)
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
                        expected: head.block_hash.clone(),
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

/// Marker for the bounded surface over which a policy applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Surface;

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
///
/// Implemented now: `block_height` is validated as lineage-local height:
/// genesis opens height 0 with no parents, and predecessor authority opens
/// nonzero heights with parent hashes. Not implemented yet: global append
/// position, authenticated lineage-head map proofs, artifact manifest digest
/// commitments, or a first-class `PolicyRef` distinct from `ProcedureRef`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenBlock {
    pub(crate) lineage_id: LineageId,
    pub(crate) block_height: u64,
    pub(crate) parent_block_hashes: Vec<BlockHash>,
    pub(crate) regime: Regime,
    pub(crate) opening_authority: OpeningAuthority,
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
    regime: Regime,
    opening_authority: OpeningAuthority,
    opened_by: ActorRef,
    opened_from_artifact: ArtifactRef,
    ruling_authority: ActorRef,
    policy_ref: ProcedureRef,
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
    pub(crate) active_artifact: ArtifactRef,
    pub(crate) claims: block::Claims,
    pub(crate) sealed_at: RecordedAt,
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
        active_artifact: ArtifactRef,
        sealed_at: RecordedAt,
    ) -> Self {
        Self {
            crown_lock_transition,
            selected_successor,
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
            ArtifactRef::new("artifact:successor"),
            RecordedAt(30),
        )
    }
}

/// Block typestate payloads.
pub(crate) mod block {
    use serde::Serialize;

    use super::BlockCommon;
    use super::{
        Admission, ArtifactPath, Digest, FlatClaim, Locator, Manifest, Policy, Private,
        RulerWitness, SealedBlockHeader, Surface, Verifiable, Witnessed, claim,
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
    #[derive(Debug, Clone, PartialEq, Eq, Serialize)]
    pub(crate) struct Claims {
        policy: Option<FlatClaim<ArtifactPath, Digest<Policy>>>,
        surface: Option<FlatClaim<ArtifactPath, Digest<Surface>>>,
        manifest: Option<FlatClaim<ArtifactPath, Digest<Manifest>>>,
    }

    impl Claims {
        pub(super) fn empty_unchecked() -> Self {
            Self {
                policy: None,
                surface: None,
                manifest: None,
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
            claim: claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Surface, L>>>,
        ) -> Self
        where
            L: Locator<Surface, Key = ArtifactPath, Digest = Digest<Surface>>,
        {
            self.surface = Some(FlatClaim::from_admitted(claim));
            self
        }

        pub(crate) fn surface<L>(
            &self,
        ) -> Option<claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<Surface, L>>>>
        where
            L: Locator<Surface, Key = ArtifactPath, Digest = Digest<Surface>>,
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct SealedBlockHeader {
    common: BlockCommon,
    crown_lock_transition: EvidenceRef,
    selected_successor: SuccessorRef,
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
                    regime: fields.regime,
                    opening_authority: fields.opening_authority,
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

    pub(crate) fn regime(&self) -> &Regime {
        &self.header().common.regime
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

    fn open_successor(
        &self,
        fields: OpenSuccessorBlock,
    ) -> Result<Block<block::Open>, HistoryError> {
        let mut parent_block_hashes = vec![self.block_hash().clone()];
        parent_block_hashes.extend(fields.additional_parent_block_hashes);
        let block_height = self.header().common.block_height + 1;
        Block::<block::Open>::open(OpenBlock {
            lineage_id: self.header().common.lineage_id.clone(),
            block_height,
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                self.block_hash().clone(),
            )),
            parent_block_hashes,
            regime: Regime::prototype1_baseline(block_height),
            opened_by: fields.opened_by,
            opened_from_artifact: fields.opened_from_artifact,
            ruling_authority: fields.ruling_authority,
            policy_ref: fields.policy_ref,
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

    #[error("History append used stale store head: expected {expected:?}, actual {actual:?}")]
    StaleStoreHead {
        expected: StoreHead,
        actual: StoreHead,
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

    #[error("sealed block failed verification before storage")]
    Verify(#[from] HistoryError),
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

    fn ruling_crown() -> super::super::inner::Crown<super::super::inner::crown::Ruling> {
        super::super::inner::Crown::test_ruling("lineage:a")
    }

    fn open_block(height: u64, parents: Vec<BlockHash>) -> Block<block::Open> {
        open_block_with_id(BlockId::new(), height, parents)
    }

    fn open_block_fields(lineage: &'static str, height: u64, parents: Vec<BlockHash>) -> OpenBlock {
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
            regime: Regime::prototype1_baseline(height),
            opening_authority,
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::new("artifact:base"),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
            opened_at: at(10),
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
            active_artifact: ArtifactRef::new("artifact:successor"),
            claims,
            sealed_at: at(30),
        };
        super::super::inner::Crown::test_locked_with_seal(block.lineage_id().as_str(), seal)
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
        let crown: super::super::inner::Crown<super::super::inner::crown::Ruling> =
            super::super::inner::Crown::test_ruling("lineage:a");
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

    #[test]
    fn locked_crown_must_match_block_lineage() {
        let block = open_block(0, Vec::new());
        let err = super::super::inner::Crown::test_locked("lineage:other")
            .seal(block)
            .expect_err("wrong lineage must not seal");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn ruling_crown_must_match_open_block_lineage() {
        let err = super::super::inner::Crown::test_ruling("lineage:other")
            .open_block(open_block_fields("lineage:a", 0, Vec::new()))
            .expect_err("wrong lineage must not open block");

        assert!(matches!(err, HistoryError::WrongCrownLineage));
    }

    #[test]
    fn ruling_crown_must_match_admitted_block_lineage() {
        let mut block = super::super::inner::Crown::test_ruling("lineage:other")
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
            super::super::inner::Crown::test_ruling("lineage:other")
                .open_block(open_block_fields("lineage:other", 0, Vec::new()))
                .expect("open other lineage block"),
        );

        let err = ruling_crown()
            .open_successor(
                &predecessor,
                OpenSuccessorBlock {
                    additional_parent_block_hashes: Vec::new(),
                    opened_by: actor("successor"),
                    opened_from_artifact: ArtifactRef::new("artifact:successor"),
                    ruling_authority: actor("successor"),
                    policy_ref: ProcedureRef::new("policy:next"),
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
        let mut block = open_block(0, Vec::new());
        block
            .admit(proposed_entry(), actor("admitter"))
            .expect("admit");
        let sealed = seal(block);
        let expected_hash = sealed.block_hash().clone();
        let expected_head = store
            .head(&LineageId::new("lineage:a"))
            .expect("read empty head");

        let stored = store
            .append(&expected_head, &sealed)
            .expect("append sealed block");

        assert_eq!(stored.block_hash, expected_hash);
        assert_eq!(
            store
                .head(&LineageId::new("lineage:a"))
                .expect("read head")
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
        assert!(by_hash.contains(expected_hash.as_str()));

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
    fn fs_block_store_rejects_duplicate_genesis() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let first = seal(open_block(0, Vec::new()));
        let first_head = store.head(&lineage).expect("read empty head");
        store
            .append(&first_head, &first)
            .expect("append first genesis");

        let second = seal(open_block(0, Vec::new()));
        let current = store.head(&lineage).expect("read current head");
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
        let block = seal(open_block(1, vec![missing_parent]));
        let head = store.head(&lineage).expect("read empty head");
        let err = store
            .append(&head, &block)
            .expect_err("non-genesis without head must fail");

        assert!(matches!(err, BlockStoreError::NonGenesisWithoutHead { .. }));
    }

    #[test]
    fn fs_block_store_rejects_child_that_does_not_extend_current_head() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let genesis = seal(open_block(0, Vec::new()));
        let genesis_head = store.head(&lineage).expect("read empty head");
        store
            .append(&genesis_head, &genesis)
            .expect("append genesis");

        let wrong_parent = BlockHash::from(HistoryHash::of_bytes(b"wrong-parent"));
        let child = seal(open_block(1, vec![wrong_parent]));
        let current = store.head(&lineage).expect("read current head");
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
        let stale = store.head(&lineage).expect("read empty head");
        let genesis = seal(open_block(0, Vec::new()));
        store.append(&stale, &genesis).expect("append genesis");

        let child = seal(open_block(1, vec![genesis.block_hash().clone()]));
        let err = store
            .append(&stale, &child)
            .expect_err("stale expected head must fail");

        assert!(matches!(err, BlockStoreError::StaleStoreHead { .. }));
    }

    #[test]
    fn fs_block_store_rejects_missing_heads_projection_when_blocks_exist() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = FsBlockStore::new(tmp.path().join("history"));
        let lineage = LineageId::new("lineage:a");
        let genesis = seal(open_block(0, Vec::new()));
        let head = store.head(&lineage).expect("read empty head");
        store.append(&head, &genesis).expect("append genesis");
        std::fs::remove_file(store.heads_path()).expect("remove heads projection");

        let err = store
            .head(&lineage)
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
        let genesis = seal(open_block(0, Vec::new()));
        let head = store.head(&lineage).expect("read empty head");
        store.append(&head, &genesis).expect("append genesis");
        std::fs::write(store.heads_path(), "{}").expect("clear heads projection");

        let err = store
            .head(&lineage)
            .expect_err("lineage index without head must not become genesis absence");

        assert!(matches!(
            err,
            BlockStoreError::MissingLineageHeadProjection { .. }
        ));
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
            regime: Regime::prototype1_baseline(1),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                BlockHash::from(HistoryHash::of_bytes(b"parent")),
            )),
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
    fn genesis_block_requires_bootstrap_authority() {
        let err = Block::open(OpenBlock {
            lineage_id: LineageId::new("lineage:a"),
            block_height: 0,
            parent_block_hashes: Vec::new(),
            regime: Regime::prototype1_baseline(0),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(
                BlockHash::from(HistoryHash::of_bytes(b"parent")),
            )),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::new("artifact:base"),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
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
            regime: Regime::prototype1_baseline(1),
            opening_authority: OpeningAuthority::Predecessor(PredecessorAuthority::new(other_hash)),
            opened_by: actor("parent"),
            opened_from_artifact: ArtifactRef::new("artifact:base"),
            ruling_authority: actor("ruler"),
            policy_ref: ProcedureRef::new("policy:test"),
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
