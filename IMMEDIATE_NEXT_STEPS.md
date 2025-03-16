
-----

**NOTE: This is a foundational design document currently under review**
This file is speculative and actively being edited as the proposed
structure for the project. It will continue to be edited as we work on the
proposed project structure and does not accurately reflect the current state of
the project.

This is a planning document **only*** and will be archived once a design
decision is chosen. The only part of this project that is at MVP status so far
is the `syn_parser`, which is the parser for the project.

-----

# IMMEDIATE_NEXT_STEPS

**1. Submodules vs Workspace Packages: Strategic Choice**

**Your Current Situation**:
```
syn_parser          # Existing git submodule
Cargo.toml          # Empty workspace root
```

**Recommendation**:  
*Convert submodule to workspace member* unless this is a shared dependency across multiple repos. Here's why:

**Why Avoid Submodules**:
1. **Cognitive Load**: As a solo dev, managing submodule versions adds overhead
2. **Code Cohesion**: Sync changes between parser and RAG logic becomes harder
3. **Cargo Workspace Advantages**:
   ```toml
   # Cargo.toml (workspace root)
   [workspace]
   members = ["syn_parser", "crates/*"]
   
   # ALL crates share:
   resolvers = ["2"]       # Consistent dependency versions
   default-members = ["."] # Context-aware builds
   ```

**Alternative Path** if syn_parser **must** stay a submodule:
```bash
git submodule update --init --recursive
ln -s ../../syn_parser crates/syn_parser  # Workspace symlink
```

---

**2. Documentation Strategy (Minimal First Steps)**

**Phase 1 – Foundational Docs**
```diff
.
├── proposed_file_arch1.md → docs/ARCHITECTURE.md
├── proposed_docs.md       → docs/CONVENTIONS.md
├── DEV_WORKFLOW.md        # New
└── CRATE_TOUR.md          # New
```

**Component Purpose**:

| File | Contents | Maintenance |
|------|----------|-------------|
| `ARCHITECTURE.md` | Decision records + diagrams | Update per major change |
| `CONVENTIONS.md` | Linting/API rules | Check via CI |
| `DEV_WORKFLOW.md` | Local setup, test shortcuts | Link from README |
| `CRATE_TOUR.md` | 2-sentence crate purposes | Auto-gen from code |

---

**3. AI-Powered Documentation Workflow**

**Command Automation**:
```bash
# Generate crate cross-links
cargo doc --document-private-items --no-deps --open

# Check conventions compliance
cargo clippy -- -D warnings -D clippy::missing_docs

# Auto-update crate docs
echo "# Crate Overview\n" > CRATE_TOUR.md
for crate in $(ls crates); do
    echo "### $crate\n$(cat crates/$crate/src/lib.rs | grep '//! ')" >> CRATE_TOUR.md
done
```

**VIable CI Pipeline** (`.github/workflows/docs.yml`):
```yaml
name: Documentation Guardian

on: push

jobs:
  verify-docs:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo doc --no-deps
      - run: ! grep -R 'unwrap(' crates/ # Enforce ? convention
      - uses: actions-rs/clippy-check@v1
        with: args: -- -D warnings
```

---

**4. Living Documentation Tactics**

**In-Code Practices**:
```rust
//! # Code Graph Builder
//! 
//! Core AST → CozoDB mapping logic.
//!
//! ## Architecture Role
//! See [ARCHITECTURE.md#code-graph](...)

#[derive(Debug)]
pub struct GraphBuilder {
    /// Tracks cross-function dependencies.  
    /// Serialized via serde for CozoDB storage.
    relations: Vec<CodeRelation>
}
```

**Why this works**:  
1. `cargo doc` links become primary docs
2. Architectural context stays with code
3. Zero extra files to maintain

---

**5. Submodule Transition Plan**

**If you commit to workspace**:
```bash
# 1. Remove submodule
git submodule deinit syn_parser
git rm syn_parser
rm -rf .git/modules/syn_parser

# 2. Convert to workspace member
mv path/to/local/syn_parser . # Copy existing code
echo "[workspace]" > Cargo.toml
```

**If keeping submodule**:
```toml
# Cargo.toml (root)
[workspace]
members = ["syn_parser", "crates/*"]
```

---

**Next Action Recommendations**:
1. Choose submodule strategy (1h decision)
2. Initialize `ARCHITECTURE.md` from your existing plan
3. Set up documentation CI guardrails
4. Establish `DEV_WORKFLOW.md` with your personal coding patterns

Would you like to prototype any of these components first?
