//! Tests for PRIMARY_TASK_SPEC §C using the clap [`xtask::cli::Cli`] surface.
//!
//! Note: The `xtask` binary ([`main.rs`](../src/main.rs)) dispatches a few legacy workspace
//! helpers first, then parses the rest with the same [`xtask::cli::Cli`] type used here.

use clap::Parser;
use clap::error::ErrorKind;

use xtask::cli::Cli;

#[test]
fn cli_root_help_shows_parse_and_db() {
    let err = Cli::try_parse_from(["xtask", "--help"]).expect_err("clap emits DisplayHelp");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let s = err.to_string();
    assert!(s.contains("parse"), "§C.3: {s}");
    assert!(s.contains("db"), "§C.3: {s}");
}

#[test]
fn cli_parse_help_lists_subcommands() {
    let err = Cli::try_parse_from(["xtask", "parse", "--help"]).expect_err("DisplayHelp");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let s = err.to_string();
    assert!(s.contains("discovery"), "{s}");
    assert!(s.contains("debug"), "{s}");
}

#[test]
fn cli_parse_debug_help_lists_nested_subcommands() {
    let err = Cli::try_parse_from(["xtask", "parse", "debug", "--help"]).expect_err("DisplayHelp");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let s = err.to_string();
    assert!(s.contains("manifest") || s.to_lowercase().contains("manifest"), "{s}");
    assert!(s.contains("workspace") || s.to_lowercase().contains("workspace"), "{s}");
    assert!(
        s.contains("logical-paths") || s.contains("logical"),
        "path diagnostic subcommands: {s}"
    );
    assert!(
        s.contains("modules-premerge") || s.contains("premerge"),
        "path diagnostic subcommands: {s}"
    );
    assert!(
        s.contains("path-collisions") || s.contains("collisions"),
        "path diagnostic subcommands: {s}"
    );
}

#[test]
fn cli_db_help_lists_subcommands() {
    let err = Cli::try_parse_from(["xtask", "db", "--help"]).expect_err("DisplayHelp");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let s = err.to_string();
    assert!(s.contains("count"), "{s}");
}

#[test]
fn cli_parse_discovery_help_documents_path_target() {
    let err = Cli::try_parse_from(["xtask", "parse", "discovery", "--help"]).expect_err("DisplayHelp");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let s = err.to_string();
    assert!(
        s.contains("PATH") || s.to_lowercase().contains("path"),
        "§C.2 path/target documentation: {s}"
    );
}

#[test]
fn cli_unknown_subcommand_produces_clap_error() {
    let err = Cli::try_parse_from(["xtask", "__not_a_real_command__"]).expect_err("unknown");
    let s = err.to_string();
    assert!(
        !s.trim().is_empty(),
        "§C.1: user-visible feedback on bad input: {s}"
    );
}
