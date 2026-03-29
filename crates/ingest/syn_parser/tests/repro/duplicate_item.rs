//! TEST_NOTE:2026-03-29
//!
//! Provenance:
//! - Corpus run: `run-1774765997311`
//! - Target repo: `FuelLabs/sway`
//! - Target crate: `forc-plugins/forc-migrate`
//! - First identified failing files:
//!   - `src/matching/lexed_tree.rs`
//!   - `src/migrations/mod.rs`
//!   - `src/visiting/mod.rs`
//!
//! Those files all contained large `#[duplicate_item(...)]` attribute blocks and
//! failed under `xtask parse debug pipeline` with the aggregate error
//! `Partial parsing success: 28 succeeded, 3 failed`. Narrower inspection showed
//! child parse failures of the form `expected ','`.
//!
//! Important nuance:
//! this appears to be valid project source that the Rust toolchain accepts in
//! the presence of the `duplicate` proc-macro, but `syn::parse_file` does not
//! necessarily accept every macro-oriented token payload as ordinary pre-expansion
//! Rust grammar. This repro captures that gap in a minimal fixture.
//!
//! This may not be a quick parser fix. If we keep the repro failing for now, it
//! serves as a marker for a broader decision about how `syn_parser` should treat
//! proc-macro-oriented syntax that is compile-valid but not directly parseable by
//! `syn`.

use std::fs;

use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

fn parse_temp_crate(lib_rs_contents: &str) {
    let td = tempdir().expect("create tempdir");
    fs::create_dir_all(td.path().join("src")).expect("create src dir");

    fs::write(
        td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_duplicate_item"
version = "0.1.0"
edition = "2021"
"#,
    )
    .expect("write Cargo.toml");

    fs::write(td.path().join("src/lib.rs"), lib_rs_contents).expect("write lib.rs");

    try_run_phases_and_resolve(td.path())
        .expect("parser should accept duplicate_item placeholder syntax");
}

fn panic_payload_to_string(payload: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".to_string()
    }
}

#[test]
fn repro_duplicate_item_placeholder_trait_signatures() {
    let lib_rs = r#"
use duplicate::duplicate_item;

#[allow(clippy::needless_arbitrary_self_type)]
#[allow(clippy::needless_lifetimes)]
#[duplicate_item(
    __mod_name
    __Visitor __visit __ref_type(type) __ref(value);

    [visitors]
    [Visitor] [visit] [&'a type] [&value];

    [visitors_mut]
    [VisitorMut] [visit_mut] [&'a mut type] [&mut value];
)]
pub mod __mod_name {
    pub trait __Visitor<T> {
        fn __visit<'a>(
            self: __ref_type([Self]),
            value: __ref([T]),
        );
    }
}
"#;

    let result = std::panic::catch_unwind(|| parse_temp_crate(lib_rs));

    if let Err(payload) = result {
        panic!(
            "unexpected panic while parsing duplicate_item placeholder fixture: {}",
            panic_payload_to_string(&payload)
        );
    }
}
