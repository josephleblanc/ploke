//! Rust 2015 crates may use bare trait objects (without `dyn`), but `syn::parse_file`
//! rejects them because syn 2.x expects `dyn` for trait objects.
//!
//! TEST_NOTE:2026-04-10
//!
//! Provenance:
//! - Historical eval replay: `BurntSushi/ripgrep` setup failure
//! - Failing crate: `ignore` (effective edition 2015)
//! - Concrete failing file: `ignore/src/walk.rs`
//! - Hotspot line shape: `Arc<Fn(&OsStr, &OsStr) -> Ordering + Send + Sync + 'static>`
//!
//! This repro isolates the edition-sensitive case into a single valid Rust 2015
//! crate so we can confirm whether the failure is in our pipeline or in `syn`
//! itself.

use std::fs;

use syn_parser::GraphAccess;
use syn_parser::error::SynParserError;
use syn_parser::try_run_phases_and_resolve;
use tempfile::tempdir;

const BARE_TRAIT_OBJECT_LIB_RS: &str = r#"
use std::sync::Arc;
use std::ffi::OsStr;
use std::cmp;

#[derive(Clone)]
pub enum Sorter {
    ByName(Arc<Fn(&OsStr, &OsStr) -> cmp::Ordering + Send + Sync + 'static>),
    ByPath(Arc<Fn(&str, &str) -> cmp::Ordering + Send + Sync + 'static>),
}

impl Sorter {
    pub fn sort_names(&self, a: &OsStr, b: &OsStr) -> cmp::Ordering {
        match self {
            Sorter::ByName(f) => f(a, b),
            Sorter::ByPath(f) => cmp::Ordering::Equal,
        }
    }
}
"#;

fn write_file(path: &std::path::Path, contents: &str) {
    fs::write(path, contents).expect("write fixture file");
}

fn create_edition_2015_bare_trait_object_crate() -> tempfile::TempDir {
    let td = tempdir().expect("create tempdir");
    let src_root = td.path().join("src");
    fs::create_dir_all(&src_root).expect("create src dir");

    write_file(
        &td.path().join("Cargo.toml"),
        r#"[package]
name = "repro_edition_2015_bare_trait_object"
version = "0.1.0"
edition = "2015"
"#,
    );
    write_file(&src_root.join("lib.rs"), BARE_TRAIT_OBJECT_LIB_RS);

    td
}

#[test]
fn repro_syn_parse_file_rejects_bare_trait_object_without_dyn() {
    let err = match syn::parse_file(BARE_TRAIT_OBJECT_LIB_RS) {
        Ok(_) => panic!("syn should reject bare trait object without `dyn`"),
        Err(err) => err,
    };

    let err_str = err.to_string();
    assert!(
        err_str.contains("expected"),
        "expected parse error for bare trait object, got: {}",
        err_str
    );
}

#[test]
fn repro_try_run_phases_and_resolve_parses_2015_bare_trait_object_crate() {
    let td = create_edition_2015_bare_trait_object_crate();
    let result = try_run_phases_and_resolve(td.path());

    // With dual-syn support, edition 2015 crates should now parse successfully
    // using syn1 (which accepts bare trait objects)
    let graphs = result.expect("parser should succeed on edition-2015 bare trait object crate");

    assert_eq!(graphs.len(), 1, "expected one parsed file");
    let graph = &graphs[0].graph;

    // Verify the enum was parsed
    let enum_count = graph
        .defined_types
        .iter()
        .filter(|t| matches!(t, syn_parser::parser::nodes::TypeDefNode::Enum(_)))
        .count();
    assert_eq!(enum_count, 1, "expected one enum (Sorter)");
}
