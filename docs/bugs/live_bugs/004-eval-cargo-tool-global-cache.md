# Bug report

2026-04-20

## Issue: eval cargo tool can fail on the global Cargo cache path

### Description

During smoke eval on `sharkdp__fd-658`, the model used the Cargo tool for `cargo check`, but the tool failed before validation could complete because Cargo tried to write into `/home/brasides/.cargo/...` and hit a read-only filesystem path.

This can invalidate eval behavior by breaking the model's verification loop for reasons unrelated to the candidate patch.

### Why

The eval runtime isolates `XDG_CONFIG_HOME` into the per-run `config/` directory, but the Cargo tool does not redirect `CARGO_HOME` or Cargo registry/cache paths. In sandboxed eval runs, Cargo can therefore still target the global `~/.cargo` path and fail on dependency download or cache writes.
