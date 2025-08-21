# GPT Coding Partner Addendum (Rust)

Purpose
- Encode the house style and behavioral guardrails so responses are consistent, idiomatic Rust, and performance-aware without premature micro-optimization.
- This addendum augments GPT_PROMPT.md and applies to all Rust work in this repo.

Golden rules
- Do not invent new public APIs, feature flags, or dependencies without explicit approval.
- Prefer minimal, surgical diffs; avoid broad refactors unless requested.
- Ask before changing behavior, error semantics, or visibility of existing items.
- Follow existing conventions in this codebase; when in doubt, match the surrounding style.

Documentation
- Use /// for item docs. Only use //! for file-top module docs when explicitly requested and placed before the first item.
- Default code fences in docs to ```rust,ignore. Only use runnable doctests when explicitly requested, and gate examples with feature comments when needed.
- Prefer short, accurate examples that compile conceptually; avoid long, multi-concern snippets.

Rust idioms to prefer (style signals)
- Abstraction
  - Prefer traits to encode behavior boundaries. Start with a trait and a single impl rather than free functions when polymorphism is plausible.
  - Choose generics for performance-critical paths; use dyn Trait when object-safety and heterogeneity are needed at runtime boundaries.
  - Use newtype wrappers to encode invariants and to avoid type confusion.
- Closures and helpers
  - Factor repeated constructions into closures or small helper functions. Let closures capture Copy fields to avoid repetition and allocation.
  - Prefer function items or local closures over copy-paste of near-identical struct literals.
- Iterators and combinators
  - Prefer iterator adapters (map, filter, flat_map, scan, try_fold) over push/extend loops and avoid multiple transitive allocations. Collect once at the boundary.
  - Prefer returning impl Iterator where practical; otherwise accept IntoIterator in APIs.
  - Use Option/Result combinators (map, and_then/ok_or_else, transpose) to reduce branching depth.
- Control flow
  - Prefer match and let-else over deep if/if let chains. Cap nested branching at 2 levels; otherwise refactor to smaller functions or combinators.
  - Prefer while let for draining/consuming patterns; reserve loop { .. break } for rare early-exit state machines.
- Ownership and allocation
  - Prefer borrowing over cloning. Accept &str/AsRef<[u8]> where possible; use Cow for conditional ownership.
  - Avoid intermediate Vecs unless necessary; operate on iterators. Use collect::<SmallVec<_>>() or similar only with clear benefit and explicit approval.
  - Avoid gratuitous to_string; prefer .into() or .to_owned() when needed.
- Recursion
  - Recursion is acceptable when depth is statically small and clarity benefits (e.g., shallow trees). Do not overuse; confirm for unbounded depths.
  - Prefer tail-recursive-like structure or explicit stack if depth may grow.

Error handling and logging
- Use the repo’s error types and helpers; prefer the Result alias and propagate with ?.
- Use policy-driven emission (ResultExt::emit_* and ErrorPolicy). Avoid println!/eprintln! in library code.
- During migrations, prefer #[allow(deprecated)] on the narrowest scope, not crate-wide.

API surface and ergonomics
- Keep public API minimal by default; prefer pub(crate) and re-export thoughtfully.
- Prefer Into/From and AsRef bounds for ergonomic APIs; avoid needless type parameters.
- Avoid exposing Arc/Rc in public types unless sharing semantics are part of the contract; prefer borrowing with lifetimes.

Concrete code tendencies to avoid
- Repeating similar struct literals instead of factoring a closure/helper capturing the shared fields.
- Building multiple temporary Vecs in sequence where an iterator chain would suffice.
- Deep chains of if/if let/else if; favor match, let-else, and combinators.
- Fully-qualified paths inside expressions; import the needed items with use to reduce noise.
- Overuse of loop where while let or for is clearer.

Always-ask list (require confirmation before proceeding)
- Adding dependencies, enabling new features, changing default features, or adding public items.
- Introducing macros (macro_rules! or proc-macro) or unsafe code.
- Switching between dyn Trait and generic impls in public APIs.
- Large-scale reformatting or module reorganization.

Review checklist (apply before proposing patches)
- Docs: /// vs //! rule honored; rust,ignore used unless explicitly requested otherwise.
- Control flow: no >2 nested branches; consider match/let-else/combinators.
- Allocation: no gratuitous new allocations; collect once; iterator chain preferred.
- Traits/generics: meaningful abstraction? choose generics vs dyn intentionally.
- Imports: local use statements added to avoid noisy fully-qualified names.
- Errors: use repo error types; policy-driven emission; no prints in library code.
- Recursion: used only if bounded and clearer; otherwise iterative.

Micro-guidance examples (stylistic)
- Mapping domain errors succinctly (avoid noisy fully-qualified paths and to_string):
  // Before
  // ploke_error::Error::Domain(ploke_error::domain::DomainError::Ui { message: msg.to_string() })
  // After
  // use ploke_error::{Error, domain::DomainError};
  // Error::from(DomainError::Ui { message: msg.into() })

- Prefer while let
  // Before
  // loop {
  //     if let Some(x) = iter.next() { handle(x); } else { break; }
  // }
  // After
  // while let Some(x) = iter.next() { handle(x); }

- Reduce nesting with let-else
  // let Some(v) = opt else { return Ok(None) };

Open questions to confirm (pick defaults)
- Traits vs generics vs dyn: prefer generics for perf-critical internals and dyn at boundaries? Y/N
- Iterator APIs: OK to return impl Iterator from internal helpers by default? Y/N
- Lifetimes: prefer borrowing APIs even if slightly less ergonomic? Y/N
- Recursion: allow in shallow-tree transforms without pre-approval? Y/N
- Lints: enforce deny(warnings) or allow during migrations? Which clippy tiers?
- Features: minimize default features; keep “diagnostic” and “tracing” opt-in? Y/N
- Testing: keep docs as rust,ignore by default; selectively enable doctests? Y/N

Implementation notes for the assistant
- Adhere to this addendum and the base prompt; when uncertain, pause and ask.
- Provide the smallest viable diff with clear reasoning, and list 1–3 ready-to-run shell commands to verify.
- If requested to add documentation, never use //! except at the file top with explicit approval; otherwise, use ///.
