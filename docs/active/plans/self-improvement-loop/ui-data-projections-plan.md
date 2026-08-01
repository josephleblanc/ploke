# UI Implementation Plan: Proving Authority and Evolution in Ploke-Egui

**Date:** 2026-05-19
**Context:** The `ploke-eval` prototype operates as an Artifact-centric Authority Handoff Protocol. To prove this system is legitimately self-improving, the UI must visualize the strict chain of custody, selection evidence, and performance trajectory.
**Goal:** Map the core observer questions to the exact data types in `ploke-tree::Graph` and the persisted files they originate from, and outline how `ploke-egui` will render them.

---

## 1. Provenance & Authority: "Is the lineage unbroken and legitimate?"
**The Claim:** The system does not teleport or cheat. Code evolves through a strict chain of custody where each Parent hands off authority (`Crown<Locked>`) to a Successor only after verifying policy commitments (e.g., the `ploke-eval` digest).

*   **The Graph Data:** `ploke_tree::Graph::history` (`HistoryIndex`)
*   **The Types:** `HistoryBlockNode`, `AuthorityLabel`
*   **The Persisted Source:** `~/.ploke-eval/campaigns/<id>/prototype1/history/` (the `FsBlockStore` sealed-block stream).
*   **Why this data answers the question:** The History block contains the sealed cryptographic proof of the `Crown` transition. It holds the surface commitment and the `AdmittedBy` policy claim. If a block is valid in the graph, the transition was verified.
*   **UI Implementation in `ploke-egui`:**
    *   **Target:** `ui/inspector.rs`
    *   **Action:** When inspecting an `ArtifactNode` that served as a Parent, add a **"Lineage Authority"** panel.
    *   **Render:** Query `graph.history` to find the block that admitted this artifact. Display the Block Hash, the Transition State (e.g., `Parent<Ruling>` -> `Crown<Locked>`), and the verified surface digest (to prove `ploke-eval` wasn't illicitly modified).

## 2. Selection Defense: "Why was this successor chosen over its siblings?"
**The Claim:** The Parent evaluates multiple child candidates and explicitly selects the Successor based on verifiable performance metrics following a defined policy.

*   **The Graph Data:** `ploke_tree::Graph::child_plans`, `ploke_tree::Graph::selections`, `ploke_tree::Graph::metrics`
*   **The Types:** `ChildPlanNode`, `SelectionEdge`, `SuccessorDecision`, `MetricNode`
*   **The Persisted Source:** 
    *   Universe: `prototype1/messages/child-plan/<parent-node-id>.json`
    *   Decision: Extracted from `transition-journal.jsonl` or parent invocation files.
    *   Metrics: `prototype1/evaluations/<branch-id>.json`
*   **Why this data answers the question:** The `ChildPlanFile` is a cryptographic-style box where the Parent *commits* to the candidate set before evaluation. The `SuccessorDecision` shows the final choice. The `MetricNode` shows the scores. Together, they prove the Parent didn't cheat by generating a branch and selecting it post-hoc without evaluating alternatives.
*   **UI Implementation in `ploke-egui`:**
    *   **Target:** `ui/inspector.rs` (specifically when clicking a `SelectionEdge`).
    *   **Action:** Build a **"Candidate Comparison"** panel.
    *   **Render:** Lookup the `ChildPlanNode` from `graph.child_plans`. For every child listed, lookup their `MetricNode` in `graph.metrics`. Display a table ranking the children by score, highlighting the chosen `SuccessorDecision`.

## 3. Patch Impact & Value: "What value did this specific patch add?"
**The Claim:** Abstract performance improvements are directly correlated with specific, observable interventions (code patches) made by the Parent on the Target Artifact.

*   **The Graph Data:** `ploke_tree::Graph::artifacts`, `ploke_tree::Graph::child_plans`, `ploke_tree::Graph::metrics`
*   **The Types:** `ArtifactNode`, `ChildPlanNode` (contains patch refs/diffs), `MetricNode`
*   **The Persisted Source:**
    *   Patches: `prototype1/branches.json` (synthesized candidate state, proposed content).
    *   Scores: `prototype1/evaluations/<branch-id>.json`
*   **Why this data answers the question:** It links the raw text change to the measured outcome. Without this, the system is a black box of rising scores.
*   **UI Implementation in `ploke-egui`:**
    *   **Target:** `ui/diff.rs` or the Patch view in `ui/inspector.rs`.
    *   **Action:** Build a **"Patch Impact"** overlay.
    *   **Render:** When viewing a patch applied to create `Artifact(Child)`, fetch the `MetricNode` for that child. Render the diff alongside the delta in score (`Child Score - Parent Score`). 

## 4. The Trajectory: "Is the overall campaign progressing?"
**The Claim:** Over multiple generations (Crown handoffs), the performance of the active lineage reliably increases.

*   **The Graph Data:** `ploke_tree::Graph::history`, `ploke_tree::Graph::artifacts`, `ploke_tree::Graph::metrics`
*   **The Types:** `HistoryBlockNode`, `ArtifactNode`, `MetricNode`
*   **The Persisted Source:** Intersection of `prototype1/history/` (the chain of handoffs) and `prototype1/evaluations/` (the scores for those active artifacts).
*   **Why this data answers the question:** This is the ultimate proof of value. A high-scoring child means nothing if it is rejected. Only the sequence of artifacts that actually held the `Crown` matter for the macro-level trajectory.
*   **UI Implementation in `ploke-egui`:**
    *   **Target:** `ui/dashboard` or a new overlay in `ui/center.rs`.
    *   **Action:** Build a **"Lineage Trajectory"** chart.
    *   **Render:** Traverse the `HistoryIndex` spine. For each `HistoryBlockNode` that represents a `Parent<Ruling>`, fetch its `ArtifactNode` and corresponding `MetricNode`. Plot these metrics on a line chart over time (or generation index). This visually proves "Improvement across the lineage".

---

## Next Steps for Implementation
1.  **Inspect the existing `ploke_tree::Graph` accessors** in `crates/ploke-egui/src/ui/inspector.rs` to ensure we can easily query `graph.history` and `graph.child_plans` by ID.
2.  **Add the Candidate Comparison Panel** first. It is the most critical to proving the "Selection Defense" and utilizing the recently finalized `ChildPlanFile` boxes.