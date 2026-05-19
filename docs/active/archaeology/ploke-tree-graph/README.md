# ploke-tree Graph Archaeology

Reports in this directory cover debugger claims whose UI-facing witness is
expected to come from `ploke_tree::Graph` rather than from ad hoc UI strings or
projection-only labels.

- [`artifact-identity.md`](artifact-identity.md)
  Identity surface for Artifact inspector claims, including `ArtifactId`,
  `ArtifactRef`, `TreeKeyHash`, and nearby provenance carriers.
- [`artifact-relations.md`](artifact-relations.md)
  Default artifact-canvas relation families: visible `P_H ∪ P_C`, with `P_O`
  and `P_B` kept as context/provenance inventory, and the typed graph carriers
  they render from.
- [`artifact-child-consideration.md`](artifact-child-consideration.md)
  Mark/filter semantics for produced child Artifacts that never entered the
  current-generation selection process.
- [`artifact-lineage-highlight.md`](artifact-lineage-highlight.md)
  Primary History lineage and current-ruler highlight marks for the default
  artifact canvas.
- [`artifact-promotion-continuity.md`](artifact-promotion-continuity.md)
  Default artifact-tree identity quotient for selected child -> next parent
  continuity, keeping one displayed Artifact node when graph-owned child-plan
  facts prove the continuation.
- [`runtime-role.md`](runtime-role.md)
  Inspector role-badge semantics for Artifacts invoked as child runtimes or
  selected successor/parent runtimes, preserving `Badge<'_>` as the borrowed UI
  witness.
- [`run-record-branch-output.md`](run-record-branch-output.md)
  Inspector claim for selected run-forest nodes and uniquely branch-backed
  Artifacts showing typed baseline and treatment `record.json.gz` output
  records, LLM turns, and ordered tool steps through branch-scoped graph refs.
- [`selection-protocol-evidence.md`](selection-protocol-evidence.md)
  Inspector claim for selection-time procedure/traversal headers and protocol
  aggregate metrics sealed in History for a selected candidate-backed Artifact.
