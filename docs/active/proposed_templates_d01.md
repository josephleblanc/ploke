reasoning effort high
thinking-tokens default

Here are three task breakdown templates focused on incremental execution and CI/CD safety:

---

## **Template 1 - Basic Checklist with Conditional Flags**  
```
### TASK: {TASK_NAME}
### FEATURE_FLAG: task_{verb}_{noun} // E.x. task_align_types
#### BREAKDOWN:
1. [ ] Identify all affected components (list files/modules)
2. [ ] Determine if feature flag is needed ([Yes/No] - Explain rationale)
3a. If flag needed:
   a. Add `#[cfg(feature = "{FEATURE_NAME}")]` to new code paths
   b. Add `--features {FEATURE_NAME}` to CI test commands
3b. If no flag:
   a. Write low-risk atomic changes
4. [ ] Suggest validation steps (unit tests/benchmarks)
```

**Rationale**: Separates flag decision-making from implementation. Uses markdown's nested lists for optional paths. Feature name convention uses task verb/noun for uniqueness.

---

## **Template 2 - Tabular Workflow**  
```markdown
| Step | Action | Requires Flag? | Documentation Link |
|------|--------|----------------|--------------------|
| 1    | {First subtask} | No | [CONVENTIONS#L3](...) |
| 2    | {Core refactor} | Yes | [IDIOMATIC_RUST#C-SEND-SYNC](...) |
| 3    | {Validation} | - | TESTING_GUIDE.md |

FLAG_NAME: {task_category}_staging  
FLAG_USAGE: Enabled in 3 CI jobs
```

**Rationale**: Tabular format forces explicit linking to conventions. Last column encourages referencing project standards. Shows flag usage at CI/CD level rather than per-file.

---

## **Template 3 - Multi-Layer Template**  
````markdown
```taskplan
TARGET: {USER_REQUEST}  
SAFETY_LEVEL: {HIGH/MEDIUM/LOW}  
-- ANALYSIS --  
1. Boundary identification (What can break?)  
2. Clipboard requirements (Need persistent storage?)  
-- EXECUTION --  
!if HIGH_SAFETY  
  FEATURE: task_quarantine_{timestamp}  
  APPLY_TO:  
    - {file_a.rs} via cfg_if! blocks  
    - Cargo.toml features  
!else  
  DIRECT_MODIFY: true  
-- TRACKING --  
METRICS:  
  • Lines changed per component  
  •#[cfg] annotations added  
```
````  

**Rationale**: Uses pseudo-code syntax for machine-readability. Introduces safety level assessment. Timestamp in feature name ensures uniqueness while retaining human meaning.

---

**My Uncertainties**:  
1. Feature naming convention assumes consistent verb/noun extraction from tasks
2. Flag necessity determination relies on LLM's understanding of code coupling
3. Table template assumes pre-existing documentation links that might need adaptation

**Suggested Adaptations**:  
- For type alignment tasks: `task_align_cozodb_types` feature flag
- For `Send+Sync` tasks: No flag needed (low risk) but extra doc tests
- For CodeGraph removal: `task_isolate_codegraph_removal` with staged cfg removal plan

Would you like me to:  
a) Refine any particular template's structure?  
b) Develop real examples using your sample requests?  
c) Create a CI/CD integration guide for these templates?
