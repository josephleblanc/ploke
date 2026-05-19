# Plan: Surfacing HyperAgents Evaluation Data in Ploke-Egui

**Date:** 2026-05-19
**Context:** This plan bridges the UI requirements in `ui-drilldown-contract.md` with the evaluation claims in `hyperagents-gap-review-2026-05-17.md`.
**Goal:** Define the next steps to surface the data necessary to empirically prove claims about the self-improving multi-generation agent loop in the `ploke-egui` interactive archive graph.

---

## 1. The Challenge: Proving the Claims
To prove that our HyperAgents-style loop is actually self-improving and generalizing (rather than just memorizing or overfitting to a selection cohort), we must surface three specific categories of evidence in the UI:
1.  **Generalization:** Proof that selected agents perform well on *held-out* tasks, not just the tasks used for selection.
2.  **Continuous Improvement (`imp@k`):** Proof that the system produces a maximum performance improvement over `k` generated task agents.
3.  **Lineage Value (Descendant-Growth):** Proof that certain "stepping-stone" agents reliably produce better descendants, even if their immediate score wasn't the absolute highest.

According to our UI contract, we **cannot** parse logs, CLI output, or raw JSON to answer these questions. The UI must be backed by typed projections and stable joins. Therefore, the plan is split into defining the backend projections and building the frontend UI panels.

---

## 2. Phase 1: Backend Projections (The Prerequisite Data)
Before we can build the UI, we must establish the typed objects in `ploke-records` and `ploke-tree` that the UI will project.

### Step 1.1: Split-Aware Evaluation Roles
Currently, `target.instances` is one effective selection cohort. We need to implement split roles to separate selection performance from generalization performance.
*   **Action:** Define `Probe`, `Train`, `ValidationSelect`, and `HeldOutFinal` roles in `ploke-records` eval-set identity.
*   **Action:** Update `ploke-eval` run profiles to name role-labeled task sets.
*   **Action:** Introduce a final-report command that runs selected agents against `HeldOutFinal` tasks and records the result *without* changing the selected parent.

### Step 2.2: The `imp@k` Projection
*   **Action:** Implement an archive projection that queries: "Given generator $G$ and budget $k$, what is the maximum score delta achieved by the best descendant compared to the starting artifact under evaluator $E$ on eval set $S$?"
*   **Output:** A typed projection object containing evaluator identity, budget, and the trajectory of best-scores over $k$ generations.

### Step 2.3: The Descendant-Growth Transfer Score Projection
*   **Action:** Implement an archive projection that calculates the discounted descendant-growth score $G_\gamma(i)$. This joins node ancestry, admitted child count, evaluation scores, and distance from ancestor to descendant.
*   **Output:** A typed projection ranking agents by their lineage value, serving as inspectable evidence.

---

## 3. Phase 2: Egui Frontend Drilldowns
Once the projections are available, we will implement the following visual primitives and inspector panels in `ploke-egui`.

### Step 3.1: The Generalization Gap Panel
*   **Goal:** Prove the agents are generalizing, not overfitting.
*   **UI Placement:** Accessible from a selected `Successor` node or the `ParentValueComparison` drilldown.
*   **Visuals:** A split bar chart or dual-axis graph comparing the agent's performance on the `ValidationSelect` cohort vs. the `HeldOutFinal` cohort.
*   **Contract Row:** Enhances `ui.parent.value.added`.

### Step 3.2: The Lineage & Value-Added Tree
*   **Goal:** Visualize descendant-growth and identify stepping-stone agents.
*   **UI Placement:** The primary archive graph canvas or a dedicated `LineagePath` overlay.
*   **Visuals:** Heatmap coloring on the graph nodes based on the `Descendant-Growth Transfer Score`. Stepping-stone agents that spawned highly successful descendants are visually emphasized (e.g., thicker borders, brighter colors), even if they weren't the "best" immediate child.
*   **Contract Row:** Satisfies `ui.child.lineage.genesis` and `ui.parent.value.added`.

### Step 3.3: Improvement Trajectory (`imp@k`) Timeline
*   **Goal:** Show the continuous improvement over generations and compute budget.
*   **UI Placement:** An expandable panel tied to the bottom timeline strip or an overarching run summary dashboard.
*   **Visuals:** A line chart plotting the highest achieved score against $k$ (number of generated task agents or compute budget). It visually proves "improvement at k".
*   **Contract Row:** Complements `ui.timeline.concurrency` and `ui.successor.selection`.

### Step 3.4: Locus Evidence Crosslist (The "Why")
*   **Goal:** Correlate score improvements with specific code changes to explain *why* an agent improved.
*   **UI Placement:** The Patch/Diff viewer panel.
*   **Visuals:** When inspecting a patch, a sub-panel lists code graph items (functions/types/modules) modified by this patch and shows their correlation with evaluation score deltas across repeated baselines or other children touching the same loci.
*   **Contract Row:** Satisfies `ui.patch.improvement.locus` and `ui.patch.code.graph.impact`.

---

## 4. Proposed Execution Order
To maintain the "UI Drilldown Contract" rule of typed projections, the work must flow from backend to frontend:
1.  **Backend:** Add Split-Aware roles to `ploke-records` and update the runner to persist these roles. (Addresses HyperAgents gap: *Split-Aware Evaluation Roles*).
2.  **Backend:** Create the `imp@k` and Descendant-Growth typed projections in `ploke-tree`. (Addresses HyperAgents gap: *HyperAgents Metrics Not First-Class Yet*).
3.  **Frontend:** Build the *Generalization Gap Panel* in `ploke-egui` using the new Split-Aware roles.
4.  **Frontend:** Update the primary archive graph visual primitives (`NodeBadge`, `GraphNode` styling) to reflect the Descendant-Growth scores.
5.  **Frontend:** Build the *Improvement Trajectory Timeline* for `imp@k`.
6.  **Frontend:** Implement the *Locus Evidence Crosslist* by joining patch impacts to code graph nodes.

---
**Status:** Draft ready for review. Let me know which area you'd like to refine or tackle first.