**1. Core Public API Functions & Types (from `lib.rs`, `runtime/db.rs`, `storage/mem.rs`):**

*   **Database Instance Creation:**
    *   `pub fn new_cozo_mem() -> Result<crate::Db<MemStorage>>` (from `storage/mem.rs`, re-exported in `lib.rs`): This is the primary constructor for the in-memory database instance (`Db<MemStorage>`).
*   **Main Interaction Struct:**
    *   `pub struct Db<S>` (from `runtime/db.rs`, re-exported): This is the central database object you interact with. `S` will be `MemStorage` in your case.
*   **Script Execution:**
    *   `pub fn run_script(&'s self, payload: &str, params: BTreeMap<String, DataValue>, mutability: ScriptMutability) -> Result<NamedRows>` (on `Db<S>`): Confirmed as the main way to execute CozoScript strings. Takes the script, parameters, and mutability flag. Returns `NamedRows`.
    *   `pub fn run_script_ast(&'s self, payload: CozoScript, cur_vld: ValidityTs, mutability: ScriptMutability) -> Result<NamedRows>` (on `Db<S>`): Allows running pre-parsed scripts.
    *   `pub enum ScriptMutability { Mutable, Immutable }` (from `runtime/db.rs`, re-exported): Used to control write access.
*   **Batch Data Operations:**
    *   `pub fn import_relations(&'s self, data: BTreeMap<String, NamedRows>) -> Result<()>` (on `Db<S>`): Confirmed as the public API for batch importing data. It takes a map where keys are relation names and values are `NamedRows` structs containing the data to import. **Crucially, this matches the documentation's description and confirms it's the intended high-performance loading mechanism.**
    *   `pub fn export_relations<I, T>(&'s self, relations: I) -> Result<BTreeMap<String, NamedRows>>` (on `Db<S>`): Confirmed public API for exporting data.
*   **Transaction Management:**
    *   `pub fn run_multi_transaction(&'s self, is_write: bool, payloads: Receiver<TransactionPayload>, results: Sender<Result<NamedRows>>)` (on `Db<S>`): This is the public, lower-level API for managing multi-statement transactions using channels. It requires setting up `crossbeam` channels.
    *   `pub enum TransactionPayload { Commit, Abort, Query(Payload) }` (from `runtime/db.rs`, re-exported): Defines the commands sent via the channel.
    *   `pub type Payload = (String, BTreeMap<String, DataValue>)` (from `runtime/db.rs`, re-exported): The structure for query payloads within a transaction.
    *   **Note:** The `SessionTx` struct itself (from `runtime/transact.rs`) is *not* public. Transaction state is managed internally by `Db::run_multi_transaction`. You don't directly get or manipulate a `SessionTx` object from the public API.
*   **Other Public Utilities:**
    *   `pub fn register_fixed_rule`, `pub fn unregister_fixed_rule` (on `Db<S>`): For custom logic.
    *   `pub fn register_callback`, `pub fn unregister_callback` (on `Db<S>`, `cfg(not(target_arch = "wasm32"))`): For event callbacks.
    *   `pub fn backup_db`, `pub fn restore_backup`, `pub fn import_from_backup` (on `Db<S>`): Public methods, though less relevant for pure in-memory use unless backing up *to* a file format.
    *   `pub fn evaluate_expressions`, `pub fn get_variables` (from `runtime/db.rs`, re-exported): Utilities for evaluating standalone expressions.

**2. Key Public Data Structures (from `data/value.rs`, `data/tuple.rs`, `runtime/db.rs`):**

*   **`pub enum DataValue`:** Confirmed as the core public enum representing all data types. Its variants (`Null`, `Bool`, `Num`, `Str`, `Bytes`, `Uuid`, `List`, `Vec`, `Json`, `Validity`, `Bot`) are accessible.
*   **`pub enum Num { Int(i64), Float(f64) }`:** Public enum for numbers.
*   **`pub struct UuidWrapper(pub Uuid)`:** Public wrapper for UUIDs.
*   **`pub enum Vector { F32(Array1<f32>), F64(Array1<f64>) }`:** Public enum for vectors (requires `ndarray`).
*   **`pub struct JsonData(pub JsonValue)`:** Public wrapper for JSON.
*   **`pub struct Validity { ... }`, `pub struct ValidityTs(...)`:** Public structs for time travel data.
*   **`pub type Tuple = Vec<DataValue>`:** Confirmed public type alias for rows.
*   **`pub struct NamedRows { pub headers: Vec<String>, pub rows: Vec<Tuple>, pub next: Option<Box<NamedRows>> }`:** Confirmed public struct for returning query results with headers. Its fields are public.

**3. Error Handling:**

*   Most public API functions return `miette::Result<T>` (which is `Result<T, miette::Report>`). Errors from Cozo operations are wrapped in `miette::Report`, providing rich error reporting capabilities. You'll need to handle this `Result` type in `ploke-db`.

**4. Comparison with Documentation & Advice for `ploke-db`:**

*   **Consistency:** The public Rust API closely matches the features and concepts described in the general documentation. The names and structures (`run_script`, `import_relations`, `NamedRows`, `DataValue`) are consistent.
*   **Best Advice Confirmation:** The documentation's advice generally holds true for the Rust API:
    *   Using `run_script` for executing queries is correct.
    *   Using `import_relations` is indeed the intended and likely most performant way for batch loading data from Rust structures into the in-memory database. The structure required (`BTreeMap<String, NamedRows>`) is clearly defined by the public types.
    *   Defining schemas and indices via `:create` / `::index` scripts run via `run_script` is the standard approach.
*   **Rust-Specific Nuances:**
    *   **Transaction API:** While the *concept* of multi-statement transactions exists, the Rust API exposes it via the channel-based `run_multi_transaction` rather than explicit `begin/commit/rollback` methods directly on the `Db` object. Your `ploke-db` might want to provide a simpler wrapper around these channels if needed.
    *   **Error Handling:** You need to specifically handle `miette::Result` and potentially format the `miette::Report` errors for users of `ploke-db`.
    *   **Type Mapping:** Direct use of `DataValue` enum variants is necessary when constructing parameters or interpreting results. `From` implementations on `DataValue` (e.g., `From<i64>`, `From<&str>`, `From<Vec<u8>>`, `From<Vec<T>>`) are helpful conveniences.
    *   **No Direct `SessionTx` Access:** You cannot directly manipulate the internal transaction state; it's managed through the provided `Db` methods.

**Conclusion for `ploke-db`:**

The general Cozo documentation provides a reliable conceptual guide. For implementation within `ploke-db`:

1.  Use `new_cozo_mem()` to create the `Db<MemStorage>`.
2.  Use `db.run_script()` for schema creation, index creation, and executing analysis queries.
3.  **Strongly prefer `db.import_relations()` for loading the bulk graph data** from `syn_parser`. You'll need to transform your `CodeGraph` data into the required `BTreeMap<String, NamedRows>` format in Rust.
4.  Handle `miette::Result` for all calls into the `cozo` API.
5.  Use the channel-based `db.run_multi_transaction()` if you need atomicity across multiple distinct operations (e.g., loading several relations atomically), potentially wrapping it in a simpler interface within `ploke-db`.
6.  Map your Ploke types to/from the public `DataValue` enum variants.

The public API surface seems well-defined and sufficient for your embedded use case, with `import_relations` being the key function for efficient data loading from Rust.
