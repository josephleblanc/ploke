# Prototype 1 Quick Start

Prototype 1 mutates campaign state and the active parent checkout. Read
[Command Safety](../operations/command-safety.md) before running live steps.

## 1. Build inside the intended parent checkout

```bash
cargo build -p ploke-eval
```

For live parent execution, use the `ploke-eval` binary built inside the active
parent checkout. Do not run a binary from one checkout against another
checkout's `--repo-root`.

## 2. Admit a run profile

```bash
P1_PROFILE="${P1_PROFILE:?set P1_PROFILE to a profile name or TOML path}"
./target/debug/ploke-eval loop prototype1-setup --profile "$P1_PROFILE"
```

Expected state includes:

```text
~/.ploke-eval/campaigns/<campaign-id>/campaign.json
~/.ploke-eval/campaigns/<campaign-id>/prototype1/run-profile.toml
<repo-root>/.ploke/prototype1/parent_identity.json
```

## 3. Diagnose before advancing

```bash
./target/debug/ploke-eval loop prototype1-doctor --repo-root .
```

The doctor output should name the current phase and suggested next command.

## 4. Advance cautiously

One diagnosed phase:

```bash
./target/debug/ploke-eval loop prototype1-step --repo-root .
```

Run until blocked, complete, or handoff:

```bash
./target/debug/ploke-eval loop prototype1-continue --repo-root .
```

## 5. Use `walk` for typestate debugging

```bash
./target/debug/ploke-eval loop walk use .
./target/debug/ploke-eval loop walk summary -v
./target/debug/ploke-eval loop walk replay --index 0
```

Use live `walk step` only when you intend to drive real transitions. Long live
edges require `--watch`; handoff requires `--watch --allow git-changes`.

Next pages:

- [Operator Map](../prototype1/operator-map.md)
- [Phase Map](../prototype1/phase-map.md)
- [Debugging Playbook](../prototype1/debugging-playbook.md)
- [Recovery](../prototype1/recovery.md)
