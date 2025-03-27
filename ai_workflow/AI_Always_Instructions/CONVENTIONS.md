# CONVENTIONS
## **Conventions must always be followed**

### **1. General Conventions**
- Documentation tests required for all public API examples
- Error handling: Use `Result<_, Box<dyn Error>>` at boundaries, custom errors internally
- Ownership: Strict adherence to zero-copy parsing where possible

### 2. Type System
- All core data structures must automatically derive `Send + Sync`
  - **NO TYPES SHOULD EXPLICITLY DERIVE `Send + Sync`**
- Definition: core data structures are data structures shared across crates in
workspace.

### 3. Concurrency Model
- **I/O Domain** (󰚩 Tokio/async):
  - File watching (Unimplemented)
  - Database operations (In Progress/async Unimplemented)
  - Network requests (Unimplemented)
  - UI interactions (Unimplemented)
  - Uses non-blocking I/O (Unimplemented)
- **Compute Domain** (󰆧 Rayon/parallel):
  - Code parsing (Currently Single-thread Implementation)
  - AST processing (Currently Single-thread Implementation)
  - Graph transformations (Currently Single-thread Implementation)
  - Vector embeddings (Stub Implementation)
  - CPU-bound workloads
- Use `flume` for Crossing all async/parallelism domain boundaries:
  - e.g. File Watcher (`async`) -> Parallel Parsing (`rayon`)
  - See example of using [`flume` across boundaries].


[`flume` across boundaries]:/home/brasides/code/second_aider_dir/ploke/docs/design/concurrency/boundary_flume_example.md


