//! Pre-expansion `syn` parse failures on proc-macro-oriented source (compile-valid after expansion).
//!
//! Known limitation (L5 / KL-002):
//! [syn_parser_known_limitations.md](../../../../../../docs/design/syn_parser_known_limitations.md),
//! [KL-002-proc-macro-pre-expansion-syntax.md](../../../../../../docs/design/known_limitations/KL-002-proc-macro-pre-expansion-syntax.md).
//!
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

use syn_parser::error::SynParserError;
use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

fn parse_temp_crate(lib_rs_contents: &str) -> Result<(), SynParserError> {
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

    try_run_phases_and_resolve(td.path()).map(|_| ())
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

    let err = parse_temp_crate(lib_rs).expect_err("duplicate_item fixture should fail to parse");

    match err {
        SynParserError::MultipleErrors(errors) => {
            assert!(
                errors.len() >= 1,
                "expected at least one child parse error, got {errors:?}"
            );
            assert!(
                errors
                    .iter()
                    .all(|e| matches!(e, SynParserError::Syn { .. })),
                "expected only syn parse errors, got {errors:?}"
            );
        }
        other => panic!("unexpected error kind for duplicate_item fixture: {other:?}"),
    }
}
