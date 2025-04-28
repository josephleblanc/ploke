# Edge Cases: Path Attribute (`[#path = "..."]`)

### 1. One Syntactic Module → Multiple Logical Modules
**How it happens**:
- Multiple `mod` declarations with different `#[path]` attributes pointing to the same file
- Each declaration creates a separate logical module

**Example**:
```rust
// File: src/foo.rs
#[path = "bar.rs"] mod mod1;  // Logical module A
#[path = "bar.rs"] mod mod2;  // Logical module B
```

**In graph**:
- One file node (bar.rs)
- Two syntactic module nodes (mod1, mod2)
- Two logical module nodes (A, B)
- Cross-layer links: mod1→A, mod2→B, both mod1/mod2→bar.rs

### 2. One Logical Module → Multiple Syntactic Modules
**How it happens**:
- Module re-exports (`pub use`)
- Multiple declarations of the same module (Rust allows this)
- Inline modules with the same name in different files

**Example**:
```rust
// File1: src/foo.rs
mod bar { pub const X: u32 = 1; }

// File2: src/baz.rs
mod bar { pub const Y: u32 = 2; }

// File3: src/lib.rs
pub mod bar;  // Re-exporting one of them
```

**In graph**:
- One logical module node (bar)
- Multiple syntactic nodes (from foo.rs, baz.rs, lib.rs)
- Cross-layer links showing all connections

