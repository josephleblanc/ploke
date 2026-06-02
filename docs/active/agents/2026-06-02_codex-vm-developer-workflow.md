# Codex VM Developer Workflow Setup

Date: 2026-06-02
Branch: `feature/ploke-loop`
Repo: `josephleblanc/ploke`

## VM State

- Repo cloned at `/home/team_ploke_dev/code/ploke`.
- Git remote uses HTTPS: `https://github.com/josephleblanc/ploke.git`.
- Git write access verified with `git push --dry-run origin HEAD:feature/ploke-loop`.
- Git author configured globally as `Joseph LeBlanc <40514613+josephleblanc@users.noreply.github.com>`.
- GitHub CLI `gh` is installed rootlessly and authenticated as `josephleblanc`.
- `gh repo view` reports `viewerPermission=ADMIN`, which reflects the account's repo role, not the fine-grained token's full effective permission set.
- Safe API probes confirmed the token is more restricted than the account role: repository administration, webhooks, and Actions secrets endpoints return `403 Resource not accessible by personal access token`.

## User-Local Tooling

- Rust stable `1.96.0` installed with rustup under `$HOME`.
- Rust components available: `cargo`, `rustc`, `rustfmt`, `clippy`.
- Target `wasm32-unknown-unknown` installed for WASM/Trunk work.
- Zig `0.16.0` installed under `~/.local/opt` and exposed through `cc`, `c++`, `ar`, `ranlib`, and `zig` wrappers in `~/.local/bin`.
- OpenSSL development headers/libs are rootlessly extracted from Ubuntu `libssl-dev_3.0.13-0ubuntu3.9_amd64.deb` under `~/.local/opt/libssl-dev`; Cargo is configured globally to use them.
- Node is installed rootlessly. Active links point to Node `v22.22.3` / npm `10.9.8`; Node `v24.16.0` is also present under `~/.local/opt`.
- GitHub CLI `gh` `2.93.0` installed under `~/.local/opt`.
- `rg` is linked into `~/.local/bin` from Codex's bundled ripgrep.

## GitNexus

- `npx gitnexus ...` currently fails in this VM with npm's `Cannot destructure property 'package' of 'node.target' as it is null` during temp install.
- Use `pnpm dlx gitnexus ...` instead.
- Initial manual workaround applied once: ran `@ladybugdb/core/install.js` to copy the prebuilt `lbugjs.node` from `@ladybugdb/core-linux-x64`.
- `pnpm dlx gitnexus analyze` completed successfully without embeddings.
- Current GitNexus status is up to date at commit `3a4ac77`.

## Verification Commands

Use `scripts/check_dev_env.sh quick` to confirm the VM still has the expected
developer tools, auth helpers, and Rust target. Use `scripts/check_dev_env.sh
fixtures` to stage generated DB fixtures required by the default test suite.
Use `scripts/check_dev_env.sh verify` to run fixture staging plus the full local
parity gate.

Use these as the normal pre-handoff baseline when the branch is expected to be
lint-clean:

```bash
cargo check
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

For broader changes, prefer:

```bash
cargo check --workspace
scripts/check_dev_env.sh fixtures
cargo test --workspace
```

The root `AGENTS.md` requires test runs to be delegated through a sub-agent when available.

## Baseline Results On 2026-06-02

- `cargo check` passed for the default workspace member.
- `cargo fmt --all -- --check` passed.
- `cargo check --workspace` passed in about 4m52s, with existing warnings in `ploke-egui` and `ploke-eval`.
- `cargo clippy --all-targets -- -D warnings` now passes on branch `codex/dev-workflow-baseline`.
- `cargo test -p ploke-db --lib` passed after staging checkout-local DB fixtures.
- `cargo test -p ploke-db --features typed_type_graph --test mod` passed after regenerating local plain typed corpus graph snapshots.
- `cargo test --workspace` is part of the active verification baseline and should be run before PR handoff.

## Workflow Artifacts

- Branch and PR workflow: `docs/active/agents/2026-06-02_codex-branch-pr-workflow.md`.
- VM health/parity script: `scripts/check_dev_env.sh`.
- GitHub Actions parity workflow: `.github/workflows/rust-ci.yml`.

## Known Caveats

- `npm ci` succeeds, but `npm audit` reports 15 high-severity issues in the current dependency tree. Do not run `npm audit fix --force` without explicit approval.
- The VM cannot use `sudo`; system packages must be installed by the host/container owner or replaced with user-local tools.
- `node_modules/` is now ignored in `.gitignore` so local npm installs do not dirty the repository.
- Do not put API keys in repo-tracked files or command text. Store local provider secrets in an ignored user config file or exported shell environment before running provider-backed workflows.
- The default fixture staging path regenerates plain typed graph corpus snapshots when committed seeds are absent. It does not regenerate `corpus_*_openrouter_embeddings` snapshots.
