# CONVENTIONS
## **Conventions must always be followed**

### **1. General Conventions**
- Documentation tests required for all public API examples
- Error handling: Use `Result<_, Box<dyn Error>>` at boundaries, custom errors internally
- Ownership: Strict adherence to zero-copy parsing where possible

### 2. Type System
- All core data structures must be `Send + Sync`
- Definition: core data structures are data structures shared across crates in
workspace.

### 3. Concurrency Model
- **async domain**: use `async` for:
  - File Watcher (Unimplemented)
  - Database Writer (Unimplemented)
- **parallelism domain**: Use `rayon` for:
  - Parallel Parsing
- Use `flume` for Crossing all async/parallelism domain boundaries:
  - e.g. File Watcher (`async`) -> Parallel Parsing (`rayon`)
  - See example of using [`flume` across boundaries].


[ `flume` across boundaries ]:/home/brasides/code/second_aider_dir/ploke/docs/design/concurrency/boundary_flume_example.md


