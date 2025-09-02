# Code Style and Conventions

## Core Philosophy
- **Extensible, maintainable, and highly performant code**
- **Upfront systems design** - Invest time in architecture to avoid technical debt
- **Strong type-safety** - Make invalid states unrepresentable at compile time
- **Performance by design** - Choose efficient patterns from the start

## Non-Negotiable Standards

### Strong Typing
- **No stringly typed plumbing**
- All OpenRouter-touching code must use strongly typed structs/enums with `Serialize`/`Deserialize`
- Numeric fields as numeric types (e.g., `u32` for tokens, `f64` for costs)
- Make invalid states unrepresentable with enums and tagged unions
- Treat ad-hoc JSON maps and loosely typed values as errors at boundaries

### Performance Patterns
- **Static dispatch over dynamic dispatch** - Compile-time polymorphism
- **Macros for boilerplate reduction** - Test generation, trait implementations
- **PhantomData for type-state patterns** - Compile-time state validation
- **GATs (Generic Associated Types)** - Enable zero-copy deserialization
- **Efficient memory patterns**:
  - Prefer iterators over collecting (enforced by `no_gratuitous_collect.sh`)
  - Use `ArcStr` over `Arc<str>` or `String` for cross-thread strings
  - Use `Arc`/`Rc` judiciously, prefer borrowing
  - Stack allocation over heap where possible
  - Consider `SmallVec` for small, variable-sized collections

### Error Handling
- Use `Result<T, E>` for error handling
- Avoid `unwrap()` in production code
- Validate early at boundaries, transform to strongly-typed internal representations

### API Design
- Design APIs to make misuse difficult or impossible
- Keep public APIs minimal and well-documented
- Use feature flags for optional functionality

### Testing
- Maintain comprehensive test coverage, especially for parser components
- Follow tiered testing: unit → integration → end-to-end → structural
- Use test fixtures in `tests/fixture_crates/` for realistic scenarios