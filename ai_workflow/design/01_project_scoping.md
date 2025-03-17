# 01 Project Scoping
This document influenced the creation of the following files:

```
ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md
ai_workflow/AI_Always_Instructions/SCOPE_DEFINITION.md
ai_workflow/reference/arch_prompting.md
```

Let's break down the "1.1 Project Scoping" components with concrete examples specific to your Rust environment:

**1.1.1 Clear Boundaries**  
*Examples for a Rust workspace:*
- Module isolation: "`syn_parser` should only handle syntax tree operations, not I/O"
- Dependency firewall: "No direct database crates in core logic modules"
- API contracts: "All public functions in `lib.rs` must have panic-free documentation"

**1.1.2 AI Constitution**  
*Examples for safe code generation:*
- "Never suggest `unsafe` blocks without matching `// SAFETY:` justification comments"
- "Prefer `thiserror` crate patterns over string-based errors"
- "Lifetime parameters must be explicitly annotated in all parser structs"

### **1.1.3 Code Conventions (Rust-Specific)**
*Rust-specific examples:*
- "All `impl` blocks sorted by trait implementation first"
- `clippy::pedantic` as mandatory with specific allowed exceptions
- "Documentation tests required for all public API examples"

*AI-Enforceable Rules & Examples:*

Require `/// Description and /** */` for public functions.
Add `#![deny(missing_docs)]` in crate root (lib.rs) to enforce this during clippy.

1. **Module Organization**
```rust
// BAD: Free-floating functions
// GOOD:
mod parser {
    mod tokens;
    mod ast;
    mod visitor;
}
```
*Enforcement:* `cargo fmt --check` + custom script verifying `lib.rs` exports

2. **Error Handling**
```rust
// BAD: panic!("Invalid token")
// GOOD:
#[derive(Debug, thiserror::Error)]
enum ParserError {
    #[error("Invalid token at position {0}")]
    InvalidToken(usize),
}
```
*Enforcement:* Clippy rule `disallowed_macros = ["panic"]` in specific modules

3. **Documentation**
```rust
/// Parses token stream into AST
/// # Examples
/// let ast = parse("fn main() {}")?;
/// assert!(ast.is_function());
pub fn parse(tokens: &[Token]) -> Result<Ast> {...}
```
*Enforcement:* `cargo test --doc` + `#![warn(missing_docs)]`

4. **CI Pipeline Enforcement**
```toml
# .github/workflows/ci.yml
- name: Code Quality
  run: |
    cargo clippy -- -D warnings
    cargo fmt --check
    cargo deny check
```

**1.1.4 Language Idioms**  
*Rust best practices:*
- "Error handling: Use `Result<_, Box<dyn Error>>` at boundaries, custom errors internally"
- "Ownership: Strict adherence to zero-copy parsing where possible"
- "Concurrency: Prefer message passing over shared state using `tokio` channels"

### **1.1.5 Dependency Management Policy**
*Rust-Specific Guardrails:*

1. **Crate Selection Criteria**
```toml
# Allowed categories:
- Parsing (logos, pom, nom)
- Testing (proptest, fake-rs)
- No runtime reflection crates
```

2. **Version Pinning**
```toml
[dependencies]
syn = { version = "2.0.1", features = ["full"] } # Pinned major.minor
```

3. **Security Auditing**
```sh
# CI command
cargo audit --deny-warnings
cargo deny check bans
```

4. **Workspace Inheritance**
```toml
# Root Cargo.toml
[workspace.package]
rust-version = "1.70.0"
edition = "2021"

[workspace.dependencies]
thiserror = "1.0.56"
```

---

### **DeepSeek-R1 Considerations for Input Generation**
1. **Context Window Management**  
   DeepSeek-R1's 32k token context limit requires:  
   ```text
   - Concise requirement packaging per interaction  
   - Strategic summarization of prior decisions  
   - Use of "cheat sheet" headers in prompts:  
     [PROJECT] PDF Processor v1  
     [ACTIVE_MODULES] S3 Upload, Queue System  
     [CONSTRAINTS] AWS-only, Python 3.10  
   ```

2. **Structured Prompt Design**  
   Format that aligns with DeepSeek's training data strengths:
   ```text
   /scoping-request
   Objective: Create user-facing PDF text extraction  
   Key Capabilities:
   - Handle scanned documents via OCR  
   - Preserve original layout metadata  
   Non-Goals:  
   - No image extraction  
   - Desktop app support  
   Preferred Stack:  
   Existing: FastAPI, PostgreSQL  
   New: Open to suggestions  
   /end-request
   ```

3. **Precision Tuning**  
   Counteract potential overeager suggestions by:  
   - Explicit boundary statements:  
     ```text
     "Prioritize solutions compatible with our existing  
     AWS ECS deployment (4GB container limit)"
     ```
   - Negative reinforcement prompts:  
     ```text
     "Avoid suggesting:  
     1. Google Cloud services  
     2. GPU-dependent libraries  
     3. New database technologies"
     ```

---

### **aider-Centric Workflow Design**
1. **Conversation Scaffolding**  
   Start sessions with context priming:  
   ```bash
   aider --msg "CONTEXT: Building PDF processor v1.3  
           CONSTRAINTS: Budget $300/mo, AWS-only  
           PAST CHOICES: Chose Tesseract over Google Vision API"
   ```

2. **Iterative Refinement Pattern**  
   Manual workflow example:  
   ```text
   User: Suggest 3 architecture options for PDF OCR, 
         comparing cost vs accuracy.
   
   DeepSeek-R1: 
   1. Tesseract + Lambda (Lowest cost)  
      - 85% accuracy, $120/mo
   2. AWS Textract (Mid-range)  
      - 95% accuracy, $400/mo  
   3. Hybrid approach (Dynamic routing)  
      - Route simple docs to Tesseract...

   User: Refine option 3 - How to implement error fallback?
   ```

3. **Artifact Generation**  
   File-annotated prompting:  
   ```text
   /add arch-notes.md  
   [Current draft of architecture docs]  
   /msg Expand the 'Error Handling' section 
        specifically for OCR failures
   ```

---

