# Error Handling Best Practices

## Fundamental Rules

1. **Use Rust's Type System**:
   - Represent errors in return types (`Result`)
   - Never use sentinel values or out parameters
   - Leverage `#[non_exhaustive]` for future-proof enums

2. **Error Design**:
   ```rust
   #[non_exhaustive]
   #[derive(Debug, thiserror::Error)]
   pub enum MyError {
       #[error("Configuration error: {0}")]
       Config(String),
       
       #[error("I/O error at {path}: {source}")]
       Io {
           path: PathBuf,
           #[source]
           source: std::io::Error,
       },
   }
   ```

3. **Context Preservation**:
   - Always include relevant context (file paths, spans, etc.)
   - Implement `std::error::Error::source()` for chaining
   - Consider adding error codes for programmatic handling

## Practical Patterns

1. **Error Construction**:
   ```rust
   // Prefer
   Err(MyError::Io { 
       path: path.to_owned(), 
       source: e 
   })

   // Over
   Err(MyError::Io(format!("Failed on {}: {}", path.display(), e)))
   ```

2. **Error Handling**:
   ```rust
   // Use combinators where appropriate
   let value = fallible_op()
       .context("additional context")?;
       
   // Match specific cases
   match err {
       MyError::Config(_) => /* recoverable */,
       _ => /* fatal */,
   }
   ```

3. **Testing**:
   ```rust
   #[test]
   fn test_error_conditions() {
       assert_matches!(
           parse("invalid"),
           Err(MyError::Syntax { .. })
       );
   }
   ```

## Project-Specific Conventions

1. **Boundary Crossings**:
   - Use distinct error types for different layers
   - Convert to domain-specific errors at boundaries

2. **Logging**:
   - Log errors at handling sites, not creation sites
   - Include full error chain in logs

3. **User-Facing Messages**:
   - Keep technical details for logs
   - Provide actionable messages to users
