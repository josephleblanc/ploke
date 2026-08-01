# Lineage Authority

## 1. Target Entity / Visible Claim

The visible claim is: for an inspected Artifact that served as the active
Parent in sealed History, the Inspector can show the History block authority
that admitted that Parent epoch. The panel shows block hash, lineage id, block
height, opening authority, policy, selected successor, and surface root
commitments from the graph-owned `HistoryBlockNode`.

This is an authority/custody claim. It is not a patch diff claim, not a
candidate ranking claim, and not a filesystem path locator.

## 2. Competing IDs and Carriers

| carrier | where it appears | semantic class | notes |
| --- | --- | --- | --- |
| `SealedBlockRecord.state.header.common.lineage_id` | `ploke_records::history::BlockCommonRecord.lineage_id` | authority lineage identity | groups sealed block epochs |
| `SealedBlockRecord.state.header.block_hash` | `ploke_records::history::SealedBlockHeaderRecord.block_hash` | sealed block identity | primary visible block handle |
| `SealedBlockRecord.state.header.active_artifact` | `ploke_records::history::SealedBlockHeaderRecord.active_artifact` | authority/admission Artifact role | primary join from selected Artifact to Parent epoch |
| `SealedBlockRecord.state.header.common.opened_from_artifact` | `ploke_records::history::BlockCommonRecord.opened_from_artifact` | provenance | explains predecessor/opening source, not the active Parent claim |
| `SealedBlockRecord.state.header.selected_successor.artifact` | `ploke_records::history::SuccessorRefRecord.artifact` | selected successor identity | downstream handoff target, not the active Parent claim |
| `SealedBlockRecord.state.header.common.surface` | `ploke_records::history::SurfaceCommitmentRecord` | surface commitment | root digests rendered as custody evidence |
| `HistoryBlockNode.{block_hash,lineage_id,block_height,active_artifact,selected_successor,surface}` | `ploke_tree::graph::HistoryBlockNode` | graph read-model authority carrier | primary graph carrier for the panel |
| `ArtifactNode.{identity,ids}` | `ploke_tree::graph::ArtifactNode` | selected Artifact identity bundle | used only to match the inspected Artifact to `active_artifact` |

## 3. Minting Sites

- `ploke_records::history::SealedBlockRecord` mirrors the persisted sealed
  History block shape.
- `ploke_tree::graph::build::history::Builder::ingest_history` reads
  `SealedBlockRecord.state.header`, observes the active/opened/successor
  Artifact refs, attaches `HistoryBlockHeader` evidence, and inserts a
  `HistoryBlockNode` into `Graph.history.blocks`.
- `ploke-egui` receives only the loaded `ploke_tree::Graph`. It does not open
  `prototype1/history/` files and does not verify block hashes itself.

## 4. Chosen Primary Carrier

Chosen graph carrier:

```text
selected ArtifactNode identity bundle
  -> HistoryBlockNode.active_artifact
  -> HistoryBlockNode authority and surface fields
```

The panel deliberately matches on `active_artifact`, because the claim is that
the inspected Artifact served as the active Parent for that sealed authority
epoch.

## 5. Rejected Alternatives

### `opened_from_artifact` is not primary

It records how the block was opened and is useful provenance, but the panel
claim is about the active Parent that held authority during the block.

### `selected_successor.artifact` is not primary

It is the handoff target selected by the active Parent. It should be shown as
downstream context, not used to prove the inspected Artifact was the Parent.

### Raw block files are not primary in `ploke-egui`

The UI must render from `ploke_tree::Graph`. Filesystem block loading and typed
deserialization belong at the graph/import boundary.

## 6. Upstream Record Types and Fields

- `ploke_records::history::SealedBlockRecord`
- `ploke_records::history::SealedBlockHeaderRecord`
  - `block_hash`
  - `active_artifact`
  - `selected_successor`
  - `claims`
  - `sealed_at`
  - `entry_count`
- `ploke_records::history::BlockCommonRecord`
  - `lineage_id`
  - `block_height`
  - `parent_block_hashes`
  - `opening_authority`
  - `opened_from_artifact`
  - `ruling_authority`
  - `policy_ref`
  - `surface`
- `ploke_records::history::SurfaceCommitmentRecord`

## 7. Graph / Read-Model Carriers

- `ploke_tree::graph::HistoryIndex`
- `ploke_tree::graph::HistoryBlockNode`
- `ploke_tree::graph::OpeningAuthorityNode`
- `ploke_tree::graph::ArtifactNode`

## 8. Downstream UI Consumers

- `crates/ploke-egui/src/ui/inspector.rs`
  - `LineageAuthoritySlot`
  - `lineage_authority_slot_for_artifact`
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_lineage_authority_for_inspector`
- `crates/ploke-egui/src/ui/dashboard/tiles.rs`
  - pinned `InspectorPanelSection::LineageAuthority`

## 9. Open Gaps

- The panel reports graph-ingested sealed block fields. It does not perform
  cryptographic verification in `ploke-egui`.
- Claim/admission details from `ClaimsRecord` are not yet rendered beyond the
  surface roots already stored on `HistoryBlockNode`.
