### 1. Basic Test Setup with `tracing-subscriber`
```rust
// Cargo.toml
[dev-dependencies]
tracing = "0.1"
tracing-subscriber = "0.3"
```

```rust
// tests/tracing_test.rs
use tracing::{info, instrument, Level};
use tracing_subscriber::fmt;

#[test]
fn basic_test() {
    // Setup tracing for just this test
    let _guard = fmt()
        .with_max_level(Level::DEBUG)
        .with_test_writer()
        .try_init();
    
    info!("This will be captured during test");
    assert!(true);
}
```

### 2. Isolated Setup with Per-Test Filtering
```rust
#[instrument]
fn function_under_test() {
    tracing::debug!("Internal operation");
    tracing::warn!("Potential issue");
}

#[test]
fn isolated_test() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(Level::WARN) // Only capture warnings and errors
        .with_target(false) // Disable target names
        .without_time() // Remove timestamps
        .with_test_writer()
        .finish();
    
    let _guard = tracing::subscriber::set_default(subscriber);
    
    function_under_test();
    
    // Captured output will only contain the warning
}
```

### 3. Global Setup for All Tests
```rust
// tests/setup.rs
use tracing_subscriber::{fmt, EnvFilter};

pub fn init_tracing() {
    let _ = fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
}
```

```rust
// tests/global_test.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::setup::init_tracing;

    // Runs once before all tests
    #[ctor::ctor]
    fn init() {
        init_tracing();
    }

    #[test]
    fn test_one() {
        tracing::info!("Visible in all tests");
    }
}
```

### 4. Advanced: Capture Logs for Assertions
```rust
// tests/assertion_test.rs
use tracing::{error, Level};
use tracing_subscriber::{fmt, prelude::*};

#[test]
fn test_error_capture() {
    let logs = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let logs_clone = logs.clone();
    
    let subscriber = fmt()
        .with_max_level(Level::ERROR)
        .with_test_writer()
        .finish()
        .with(tracing_subscriber::registry().with(
            tracing_subscriber::filter::filter_fn(move |metadata| {
                if metadata.level() == &Level::ERROR {
                    logs_clone.lock().unwrap().push(metadata.target().to_string());
                }
                true
            })
        ));
    
    let _guard = tracing::subscriber::set_default(subscriber);
    
    error!("Critical failure in module_x");
    
    let logs = logs.lock().unwrap();
    assert!(logs.contains(&"module_x".to_string()));
}
```

### Key Configuration Options

| **Method** | **Purpose** | **Use Case** |
|------------|-------------|--------------|
| `.with_test_writer()` | Capture output to test console | Default for test logging |
| `.with_max_level()` | Set maximum log level | Performance-sensitive tests |
| `.with_env_filter()` | Configure via env vars | `RUST_LOG=trace cargo test` |
| `.without_time()` | Remove timestamps | Cleaner assertion outputs |
| `.with_target(false)` | Hide target paths | Simplified output |

### Best Practices
1. **Isolation**: Prefer per-test setup over global setup
   ```rust
   let _guard = tracing_subscriber::fmt().try_init();
   ```
   
2. **Environment Control**: Configure via `RUST_LOG`
   ```sh
   RUST_LOG=my_crate=debug cargo test
   ```

3. **Performance**: Limit tracing in release tests
   ```rust
   #[cfg_attr(not(debug_assertions), with_max_level(Level::WARN)]
   ```

4. **Async Support**: For async tests
   ```rust
   #[tokio::test]
   async fn async_test() {
       let _guard = tracing_subscriber::fmt().try_init();
       // Async code here
   }
   ```

### Common Pitfalls & Solutions
1. **"subscriber already set" errors**:
   ```rust
   // Use scoped subscribers instead
   let _guard = tracing::subscriber::set_default(subscriber);
   ```

2. **Missing logs in test output**:
   ```rust
   // Ensure .with_test_writer() is used
   .with_test_writer()
   ```

3. **Performance issues**:
   ```rust
   // Limit log levels in CI
   .with_env_filter(EnvFilter::try_from_env("RUST_LOG")
        .unwrap_or_else(|_| EnvFilter::new("warn")))
   ```

4. **Flaky async tests**:
   ```rust
   // Use tracing::Instrument for futures
   async_function()
       .instrument(tracing::info_span!("test_span"))
       .await
   ```

This setup ensures you get:
- Isolated tracing contexts per test
- Configurable log levels via environment
- Captured output in test runners
- Performance-optimized instrumentation
- Assertion-capable log capture
- Async-compatible tracing spans
