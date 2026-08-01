// ANCHOR: prototype1_block_store_contract
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
// ANCHOR_END: prototype1_block_store_contract

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
    /// `Block<block::Sealed>`. The loader reconstructs admitted entries through
    /// stored DTOs, then verifies the block against the raw entry JSON hashes
    /// that were committed at seal time.
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
        let stored_entry_hashes =
            stored_entry_hashes_from_line(line, &path, stored.location.line_index)?;
        let block = stored_block.into_verified_block(
            path,
            stored.location.line_index,
            stored_entry_hashes,
        )?;
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
            let stored_entry_hashes =
                stored_entry_hashes_from_line(line, &path, line_index as u64)?;
            let block = stored_block.into_verified_block(
                path.clone(),
                line_index as u64,
                stored_entry_hashes,
            )?;
            out.push((line_index as u64, block));
        }
        Ok(out)
    }
}
impl BlockStore for FsBlockStore {
    type Error = BlockStoreError;

    // ANCHOR: prototype1_fs_block_store_append
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
    // ANCHOR_END: prototype1_fs_block_store_append

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
        stored_hashes: StoredHashes,
    ) -> Result<Block<block::Sealed>, BlockStoreError> {
        if self.entries.len() != self.state.header.entry_count
            || self.entries.len() != stored_hashes.entry_hashes.len()
            || self.entries.len() != stored_hashes.selection_hashes.len()
        {
            return Err(BlockStoreError::UnsupportedStoredEntries {
                path,
                line_index,
                entry_count: self.state.header.entry_count,
            });
        }

        let mut entries = Vec::with_capacity(self.entries.len());
        for (stored, selection_hash) in self
            .entries
            .into_iter()
            .zip(stored_hashes.selection_hashes)
        {
            entries.push(stored.into_entry(selection_hash));
        }

        let block = Block {
            state: block::Sealed {
                header: self.state.header,
                _private: Private,
            },
            entries,
            stored_entry_hashes: Some(stored_hashes.entry_hashes),
        };
        block.verify_hash()?;
        Ok(block)
    }
}

#[derive(Debug)]
struct StoredHashes {
    entry_hashes: Vec<HistoryHash>,
    selection_hashes: Vec<Option<HistoryHash>>,
}

#[derive(Debug, Deserialize)]
struct StoredRawEntries<'a> {
    #[serde(borrow)]
    entries: Vec<&'a serde_json::value::RawValue>,
}

#[derive(Debug, Deserialize)]
struct StoredRawEntry<'a> {
    #[serde(borrow)]
    core: StoredRawEntryCore<'a>,
}

#[derive(Debug, Deserialize)]
struct StoredRawEntryCore<'a> {
    #[serde(borrow)]
    payload: &'a serde_json::value::RawValue,
}

#[derive(Debug, Deserialize)]
struct StoredRawPayloadKind {
    kind: String,
}

fn stored_entry_hashes_from_line(
    line: &str,
    _path: &Path,
    _line_index: u64,
) -> Result<StoredHashes, BlockStoreError> {
    let raw: StoredRawEntries<'_> =
        serde_json::from_str(line).map_err(BlockStoreError::Deserialize)?;
    let mut entry_hashes = Vec::with_capacity(raw.entries.len());
    let mut selection_hashes = Vec::with_capacity(raw.entries.len());
    for entry in raw.entries {
        let raw_entry = entry.get();
        entry_hashes.push(entry_hash_from_raw_json(raw_entry));
        selection_hashes.push(selection_hash_from_raw_entry(raw_entry)?);
    }
    Ok(StoredHashes {
        entry_hashes,
        selection_hashes,
    })
}

fn entry_hash_from_raw_json(raw_entry: &str) -> HistoryHash {
    let mut preimage = Vec::with_capacity(raw_entry.len() + 54);
    preimage.extend_from_slice(b"{\"domain\":\"prototype1.history.entry.v1\",\"value\":");
    preimage.extend_from_slice(raw_entry.as_bytes());
    preimage.extend_from_slice(b"}");
    HistoryHash::of_bytes(&preimage)
}

fn selection_hash_from_raw_entry(raw_entry: &str) -> Result<Option<HistoryHash>, BlockStoreError> {
    let raw: StoredRawEntry<'_> =
        serde_json::from_str(raw_entry).map_err(BlockStoreError::Deserialize)?;
    let kind: StoredRawPayloadKind =
        serde_json::from_str(raw.core.payload.get()).map_err(BlockStoreError::Deserialize)?;
    if kind.kind != "selection_decision" {
        return Ok(None);
    }

    let selection = selection_json_without_kind(raw.core.payload.get())?;
    Ok(Some(selection_hash_from_raw_json(&selection)))
}

fn selection_hash_from_raw_json(selection: &str) -> HistoryHash {
    let mut preimage = Vec::with_capacity(selection.len() + 68);
    preimage.extend_from_slice(
        b"{\"domain\":\"prototype1.history.selection_decision_entry.v1\",\"value\":",
    );
    preimage.extend_from_slice(selection.as_bytes());
    preimage.extend_from_slice(b"}");
    HistoryHash::of_bytes(&preimage)
}

fn selection_json_without_kind(raw_payload: &str) -> Result<String, BlockStoreError> {
    let raw_payload = raw_payload.trim();
    let Some(rest) = raw_payload.strip_prefix("{\"kind\":\"selection_decision\"") else {
        return Err(invalid_selection_shape(
            "stored selection decision payload is not the compact internally-tagged shape emitted at seal time",
        ));
    };
    let Some(rest) = rest.strip_suffix('}') else {
        return Err(invalid_selection_shape(
            "stored selection decision payload is missing its closing object delimiter",
        ));
    };
    if rest.is_empty() {
        return Ok("{}".to_string());
    }
    let Some(rest) = rest.strip_prefix(',') else {
        return Err(invalid_selection_shape(
            "stored selection decision payload has unexpected bytes after its kind tag",
        ));
    };
    Ok(format!("{{{rest}}}"))
}

fn invalid_selection_shape(detail: impl Into<String>) -> BlockStoreError {
    HistoryError::InvalidSelectionDecision {
        detail: detail.into(),
    }
    .into()
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
        pub(super) fn into_entry(self, selection_hash: Option<HistoryHash>) -> Entry<Admitted> {
            Entry {
                core: self.core.into_core(),
                state: self.state.into_state(selection_hash),
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
        fn into_state(self, selection_hash: Option<HistoryHash>) -> Admitted {
            Admitted {
                observed: self.observed.into_state(),
                proposer: self.proposer,
                procedure_or_policy: self.procedure_or_policy,
                admitting_authority: self.admitting_authority,
                ruling_authority: self.ruling_authority,
                lineage_id: self.lineage_id,
                block_id: self.block_id,
                block_height: self.block_height,
                selection_hash,
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
