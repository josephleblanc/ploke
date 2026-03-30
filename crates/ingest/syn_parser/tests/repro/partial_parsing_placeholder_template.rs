//! TEST_NOTE:2026-03-30
//!
//! Provenance:
//! - Corpus run: `run-1774867607815`
//! - Target repo: `linera-io/linera-protocol`
//! - Target crate: `linera-indexer/plugins`
//! - Saved failing member: `plugins`
//! - Saved hotspot file: `linera-indexer/plugins/src/template.rs`
//!
//! The original corpus failure was a resolve-stage error:
//! `Partial parsing success: 2 succeeded, 1 failed`.
//!
//! The failing item was a placeholder-heavy method signature in `template.rs`:
//! `async fn helper1(&self, ...) -> Result<.., IndexerError>`.
//! That syntax is not valid pre-expansion Rust, so `syn::parse_file` rejects it.
//!
//! This repro keeps the shape as a tiny temp crate with one valid impl body and
//! one invalid placeholder method so the parse failure stays easy to isolate.

use std::fs;

use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

fn parse_temp_crate(lib_rs_contents: &str) {
    let td = tempdir().expect("create tempdir");
    fs::create_dir_all(td.path().join("src")).expect("create src dir");

    fs::write(
        td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_partial_parsing_placeholder_template"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write Cargo.toml");

    fs::write(td.path().join("src/lib.rs"), lib_rs_contents).expect("write lib.rs");

    try_run_phases_and_resolve(td.path())
        .expect("parser should accept placeholder-heavy template syntax");
}

#[test]
fn repro_partial_parsing_placeholder_template() {
    let lib_rs = r#"
pub struct Template<C> {
    _phantom: core::marker::PhantomData<C>,
}

impl<C> Template<C> {
    async fn register(&self, _value: &()) -> Result<(), ()> {
        Ok(())
    }

    async fn helper1(&self, ...) -> Result<.., ()> {
        Ok(())
    }

    pub async fn entrypoint1(&self) -> String {
        String::new()
    }
}
"#;

    parse_temp_crate(lib_rs);
}
