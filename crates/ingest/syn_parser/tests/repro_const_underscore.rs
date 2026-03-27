//! TEST_NOTE:2026-03-27
//!
//! We hit a crash while parsing the Bevy fixture (`bevy_ecs/src/label.rs`) where multiple
//! anonymous const items (`const _`) are used as compile-time dyn-compatibility checks:
//! `const _: Option<Box<dyn Trait>> = None;`.
//!
//! Root cause: `syn_parser` generated synthetic node IDs using the const ident string; for
//! `const _` that string is `"_"`, so multiple `const _` items in the same file/module collided,
//! producing duplicate `AnyNodeId::Const(...)` and tripping `validate_unique_rels`.
//!
//! Fix: disambiguate anonymous `const _` by salting the ID-generation key with a monotonically
//! increasing per-file ordinal (the visitor is scoped to one file). This keeps the check stable
//! without depending on spans/byte offsets.
use std::fs;

use syn_parser::error::SynParserError;
use syn_parser::{ParseWorkspaceConfig, parse_workspace_with_config};
use tempfile::tempdir;

fn parse_temp_workspace_with_single_member(lib_rs_contents: &str) -> Result<(), SynParserError> {
    let td = tempdir().expect("create tempdir");

    fs::write(
        td.path().join("Cargo.toml"),
        r#"[workspace]
members = ["repro_crate"]
resolver = "2"
"#,
    )
    .expect("write workspace Cargo.toml");

    let crate_root = td.path().join("repro_crate");
    fs::create_dir_all(crate_root.join("src")).expect("create crate dirs");
    fs::write(
        crate_root.join("Cargo.toml"),
        r#"[package]
name = "repro_crate"
version = "0.1.0"
edition = "2024"
"#,
    )
    .expect("write crate Cargo.toml");

    fs::write(crate_root.join("src/lib.rs"), lib_rs_contents).expect("write lib.rs");

    let config = ParseWorkspaceConfig::default();
    parse_workspace_with_config(td.path(), &config).map(|_| ())
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
fn repro_duplicate_const_node_id_for_underscore_const_items() {
    let lib_rs = r#"// anonymous const items should not collide
const _: () = ();
const _: () = ();
const _: u32 = 0;
"#;

    let result = std::panic::catch_unwind(|| {
        parse_temp_workspace_with_single_member(lib_rs)
            .expect("parse should succeed even with multiple `const _` items");
    });

    if let Err(payload) = result {
        panic!(
            "unexpected panic while parsing underscore-const crate: {}",
            panic_payload_to_string(&payload)
        );
    }
}

#[test]
fn repro_duplicate_const_node_id_for_underscore_const_items_with_trait_objects() {
    let lib_rs = r#"extern crate alloc;

use alloc::boxed::Box;

pub trait DynEq {}
pub trait DynHash: DynEq {}

// Tests that these traits are dyn-compatible
const _: Option<Box<dyn DynEq>> = None;
const _: Option<Box<dyn DynHash>> = None;
"#;

    let result = std::panic::catch_unwind(|| {
        parse_temp_workspace_with_single_member(lib_rs)
            .expect("parse should succeed even with multiple trait-object `const _` items");
    });

    if let Err(payload) = result {
        panic!(
            "unexpected panic while parsing underscore-const crate with trait objects: {}",
            panic_payload_to_string(&payload)
        );
    }
}

#[test]
fn named_consts_with_trait_objects_do_not_trigger_duplicate_const_node_id() {
    let lib_rs = r#"extern crate alloc;

use alloc::boxed::Box;

pub trait DynEq {}
pub trait DynHash: DynEq {}

// Same types as the repro, but named consts instead of `const _`.
const DYNEQ_DYN_COMPAT_CHECK: Option<Box<dyn DynEq>> = None;
const DYNHASH_DYN_COMPAT_CHECK: Option<Box<dyn DynHash>> = None;
"#;

    let result = std::panic::catch_unwind(|| {
        parse_temp_workspace_with_single_member(lib_rs)
            .expect("parse_workspace_with_config should succeed for named consts");
    });

    if let Err(payload) = result {
        let panic_msg = panic_payload_to_string(&payload);
        panic!("unexpected panic while parsing named-const crate: {panic_msg}");
    }
}
