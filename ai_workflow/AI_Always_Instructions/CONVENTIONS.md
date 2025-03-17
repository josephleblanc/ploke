# CONVENTIONS
**Conventions must always be followed**
- Documentation tests required for all public API examples
- Error handling: Use `Result<_, Box<dyn Error>>` at boundaries, custom errors internally
- Ownership: Strict adherence to zero-copy parsing where possible

