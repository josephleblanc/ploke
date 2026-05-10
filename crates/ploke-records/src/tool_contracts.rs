//! Re-exports of the serializable tool transport contracts.
//!
//! These types are defined beside the `ploke-tui` tools because the tool
//! implementation is the place where the LLM-facing argument and result shapes
//! are maintained. `ploke-records` re-exports the owned transport DTOs instead
//! of mirroring them so persisted tool-call readers use the same Rust shapes the
//! tools use when serializing/deserializing LLM tool traffic.
//!
//! This module is intentionally only a passive records surface. It must not
//! expose `ploke-tui` execution state, app context, event buses, async tool
//! execution, database handles, or edit authority. The goal is type identity for
//! persisted/replay/UI readers, not access to the TUI runtime.

pub use ploke_tui::tools::{
    cargo::{
        CargoCommand, CargoDiagnostic, CargoScope, CargoSpan, CargoStatusReason, CargoSummary,
        CargoToolParamsOwned, CargoToolResult,
    },
    code_edit::{CanonicalEditOwned, CodeEditParamsOwned},
    code_item_lookup::LookupParamsOwned,
    create_file::CreateFileParamsOwned,
    get_code_edges::EdgesParamsOwned,
    insert_rust_item::InsertRustItemParamsOwned,
    list_dir::{ListDirEntry, ListDirParamsOwned, ListDirResult},
    ns_patch::{ApplyNsPatchResult, NsPatchOwned, NsPatchParamsOwned},
    ns_read::{NsReadParamsOwned, NsReadResult},
    request_code_context::RequestCodeContextParamsOwned,
};
