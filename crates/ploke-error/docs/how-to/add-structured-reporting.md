# How to add structured reporting with `PrettyDebug`

Use this checklist when wiring structured, log-friendly context onto new error types.

## 1) Enable features
- Add `ploke-error = { …, features = ["serde"] }` (and `"tracing"` if you want emit helpers) in the dependent crate.
- Keep the `Fields` payload minimal (only what is needed to debug).

## 2) Implement `PrettyDebug`
```rust
use ploke_error::PrettyDebug;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct MyFields<'a> {
    code: u32,
    ctx: &'a str,
}

#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("failed: {msg}")]
    Failed { code: u32, msg: String },
}

impl PrettyDebug for MyError {
    type Fields<'a> = MyFields<'a>;

    fn fields(&self) -> Option<Self::Fields<'_>> {
        match self {
            MyError::Failed { code, msg } => Some(MyFields { code: *code, ctx: msg }),
        }
    }
}
```
- Use references in fields to avoid cloning large data.
- If there is no meaningful structure, return `None`.

## 3) Logging patterns
- Structured field logging (preferred):
  ```rust
  if let Err(err) = do_work() {
      tracing::error!(error = %err, fields = ?err.fields(), "operation failed");
  }
  ```
- Pretty string (fallible) for UI/logs:
  ```rust
  if let Some(json) = err.pretty_json() {
      tracing::error!(error = %err, structured = %json, "operation failed");
  }
  ```
- Panic-on-serialize helper for “must log” paths:
  ```rust
  if let Some(json) = err.pretty_json_or_panic() {
      tracing::error!(error = %err, structured = %json, "operation failed");
  }
  ```
- Convenience emitter (requires `ploke-error/tracing`):
  ```rust
  err.emit_tracing(tracing::Level::ERROR, "operation failed");
  ```

## 4) Where it helps
- Errors with meaningful context (DB queries, tool calls, API requests, file ops).
- Actor/event systems where call sites are distant from emit sites.

## 5) Cautions
- Avoid large/PII payloads; keep fields concise.
- For hot paths, prefer `fields()` over stringifying to reduce allocations.
- `pretty_json_or_panic` will panic on serialization failure; use only where that is acceptable. 
