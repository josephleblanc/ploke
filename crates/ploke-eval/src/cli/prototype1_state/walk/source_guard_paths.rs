/// Runtime source paths whose dirty state can change transition authority.
pub(crate) const SOURCE_GUARD_PATHS: &[&str] = &[
    "Cargo.toml",
    "crates/ploke-eval/Cargo.toml",
    "crates/ploke-eval/src/cli/args",
    "crates/ploke-eval/src/cli/handlers/prototype1_loop.rs",
    "crates/ploke-eval/src/cli/prototype1_state",
];

/// Explicit source boundary that defines the sibling-client contract.
///
/// A matching fingerprint means the client and server were compiled with the
/// same public walk/setup carriers, socket framing, endpoint discovery, and
/// mutation-client guards. It does not mean their server-side transition,
/// persistence, provider, compiler, or optimization engines are identical.
/// Transitive carrier changes still require the explicit protocol or graph
/// version to advance; hashing their entire owning crates would turn unrelated
/// runtime edits into a false sibling-client incompatibility.
#[allow(dead_code)]
pub(crate) const CONTRACT_FINGERPRINT_PATHS: &[&str] = &[
    "crates/ploke-eval/build.rs",
    "crates/ploke-eval/src/setup_client.rs",
    "crates/ploke-eval/src/walk_client.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/audit.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/config.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/endpoint.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/epoch.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/ipc.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/llm_trace.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/paths.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/phase.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/protocol.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/query.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/source_guard_paths.rs",
    "crates/ploke-eval/src/cli/prototype1_state/walk/trace.rs",
];

/// Build-environment inputs permitted to affect sibling compatibility.
///
/// Empty by design: target, toolchain, rustflags, debug/optimization profile,
/// and current Cargo features do not alter the included carrier sources. A
/// future contract-affecting feature must opt in here together with a protocol
/// review and a boundary test.
#[allow(dead_code)]
pub(crate) const CONTRACT_FINGERPRINT_ENV: &[&str] = &[];

#[cfg(test)]
mod tests {
    use super::{CONTRACT_FINGERPRINT_ENV, CONTRACT_FINGERPRINT_PATHS};

    fn covered(path: &str) -> bool {
        CONTRACT_FINGERPRINT_PATHS.iter().any(|&root| {
            path == root
                || path
                    .strip_prefix(root)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    }

    #[test]
    fn contract_scope_boundary() {
        for path in [
            "crates/ploke-eval/src/cli/prototype1_state/walk/protocol.rs",
            "crates/ploke-eval/src/cli/prototype1_state/walk/epoch.rs",
            "crates/ploke-eval/src/cli/prototype1_state/walk/ipc.rs",
            "crates/ploke-eval/src/walk_client.rs",
            "crates/ploke-eval/src/setup_client.rs",
        ] {
            assert!(
                covered(path),
                "walk/setup client contract is outside the fingerprint: {path}"
            );
        }
        for path in [
            "Cargo.lock",
            "Cargo.toml",
            "crates/ploke-eval/Cargo.toml",
            "crates/ploke-eval/src/cli/prototype1_state/driver/control.rs",
            "crates/ploke-eval/src/cli/prototype1_state/session.rs",
            "crates/ploke-eval/src/cli/prototype1_state/walk/controller.rs",
            "crates/ploke-eval/src/cli/prototype1_state/walk/server.rs",
            "crates/ploke-records/src/identity.rs",
            "crates/ploke-protocol/src/tool_calls/trace.rs",
            "crates/ploke-tui/src/lib.rs",
            "crates/ploke-llm/src/lib.rs",
        ] {
            assert!(
                !covered(path),
                "runtime/build input must not become a sibling-client equality requirement: {path}"
            );
        }
    }

    #[test]
    fn contract_scope_excludes_build_environment() {
        for key in [
            "CARGO_ENCODED_RUSTFLAGS",
            "CARGO_FEATURE_LIVE_API_TESTS",
            "CARGO_PROFILE_DEV_OPT_LEVEL",
            "DEBUG",
            "OPT_LEVEL",
            "PROFILE",
            "RUSTC",
            "RUSTC_BOOTSTRAP",
            "TARGET",
        ] {
            assert!(
                !CONTRACT_FINGERPRINT_ENV.contains(&key),
                "build-only setting must not change sibling compatibility: {key}"
            );
        }
    }
}
