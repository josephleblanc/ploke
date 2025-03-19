reasoning effort default
thinking-tokens default

Here are three templates designed to systematically break down tasks while incorporating conditional cfg flag usage, following your conventions and addressing challenges from musings.md:

---

### **Template 1: Phase-Driven Breakdown (Intermediate Complexity)**
#### Format Rationale
Organizes work into clear phases for structured progression. Aligns with CI/CD testing needs by isolating cfg-guarded changes in dedicated phases.

```markdown
## Task: [INSERT_TASK_NAME]  
**Feature Name:** `task_[TASK_ID]_unstable` (e.g., `task_syn_refactor_001`)  

### Phases  
1. **Preparation**  
   - [ ] Audit existing code for impacted areas  
   - [ ] Define cfg feature gate (if needed)  
   - [ ] Write temporary tests to validate old behavior  

2. **Modification**  
   - [ ] Implement [SUB_TASK_1]  
     * (Optional CFG) Add `#[cfg(feature = "task_[TASK_ID]_unstable")]` guard  
   - [ ] Implement [SUB_TASK_2]  
     * (Optional CFG) [Reason for gating]  

3. **Integration**  
   - [ ] Enable cfg flag in CI/CD for testing  
   - [ ] Run `cargo test --features task_[TASK_ID]_unstable`  

4. **Finalization**  
   - [ ] Remove cfg guards  
   - [ ] Update documentation per CONVENTIONS.md
```

---

### **Template 2: Checklist Format (Simple)**  
#### Format Rationale  
Provides minimal structure for small tasks. Forces binary completion tracking and explicit cfg justification.

```text
TASK: [Name] | CFG: [Y/N] → "task_[VERB]_changes" (e.g., "task_remove_codegraph")  

Subtasks:  
- [ ] (1/N) [Brief description]  
  • Affected files:  
  • CFG Requirement?: [Yes/No] → [Explanation]  
- [ ] (2/N) [Update docs with cargo test --doc]  
  • Reference: CONVENTIONS.md Section 3  
- [ ] (3/N) [Test under feature gate] (if applicable)  
```

---

### **Template 3: Contextual Table (High Complexity)**  
#### Format Rationale  
Captures multiple dimensions of work for large refactors. Explicitly ties subtasks to project conventions.

```markdown
| Subtask ID | Description                     | Requires CFG? | Docs/Tests Impacted     |  
|------------|---------------------------------|---------------|-------------------------|  
| ST1        | [Change type signatures]        | Yes           | `syn_parser/src/ast.rs` |  
|            | • Newtype: `ContentHash(Bytes)` | • Feature: `task_type_align` | • Add #[doc = "..."] |  
|------------|---------------------------------|---------------|-------------------------|  
| ST2        | [Modify serialization logic]    | No            | `cozo_adapter/mod.rs`   |  
```

---

### Uncertainties & Notes  
1. **Feature Naming Schema:** The `task_[TASK_ID]_*` pattern ensures uniqueness but may conflict with cargo's feature semantics if used elsewhere. Alternative: Timestamp suffix (`task_remove_codegraph_20240518`).  
2. **CFG Applicability Threshold:** Templates assume the LLM can judge when gating is needed. For riskier changes, you might want a mandatory "Breakage Risk Assessment" subtask.  
3. **Granularity Control:** No explicit mechanism to prevent over/under-splitting of subtasks. A "Task Complexity Rating (1-5)" field could help, but requires subjective judgment.

Would you like me to refine any template's balance between flexibility and prescriptiveness?
