LangGraph: DAG for agentic workflow
  - shared state (common file)
    - like a shared whiteboard: initial request, data requested, accumulated report, errors, "next action"
    - every agent updates the shared state
    - signal the next state
  - agent nodes are like functions
    - input, output, set next action
  - routing logic
    - takes current state as input, decides what runs next
    - can return "end" to stop the workflow
    - can branch based on runtime behavior: e.g. calling an "error handling" agent in failure states
  - agents run sequentially
  - react to each other's progress
  - modular design: each agent is a single node in the graph
  - dynamic routing: workflow can change paths based on runtime conditions
  - agent behavior can alter the direction of the workflow.
CrewAI: Open source python framework, strong emphasis on structured workflows
Microsoft's Autogen: emphasis on Inter-agent communication, agentic self-organizing
IBM

On agentic RAG more generally:
- Have a "fail state" that identifies when the query is not relevant to the RAG database, and can then respond that the query is not within the types of questions it would help answer.

---

Core ideas and must‑watch/reads

 • Talk: Richard Feldman — Making Impossible States Impossible (Elm, language-agnostic but
   extremely practical) https://www.youtube.com/watch?v=IcgmSRJHu_8
 • Essay: Alexis King — Parse, don’t validate (turn unchecked data into well-typed structures
   early) https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/
 • Book: Scott Wlaschin — Domain Modeling Made Functional (F# but the best introduction to
   modeling with ADTs/newtypes and invariants)
   https://pragprog.com/titles/swdddf/domain-modeling-made-functional/
 • Blog series: F# for Fun and Profit — Domain modeling (short, concrete examples of modeling
   constraints with types) https://fsharpforfunandprofit.com/
 • Paper: Out of the Tar Pit (why complexity is the enemy; types and immutability as tools)
   http://curtclifton.net/papers/MoseleyMarks06a.pdf

Rust‑specific resources (how to encode invariants)

 • Rust API Guidelines — invariants, sealed traits, newtypes
   https://rust-lang.github.io/api-guidelines/
 • The Rustonomicon — PhantomData, variance; useful for typestate and sealed/internal APIs
   https://doc.rust-lang.org/nomicon/phantom-data.html
 • Hoverbear — Encoding state machines in Rust (typestate)
   https://hoverbear.org/blog/rust-state-machine-pattern/
 • The Book / Rust by Example — Newtype pattern
   https://doc.rust-lang.org/rust-by-example/generics/new_types.html
 • NonZero types in std (encode nonzero invariants)
   https://doc.rust-lang.org/std/num/struct.NonZeroUsize.html
 • Book: Rust for Rustaceans (type-driven APIs, error modeling, boundaries)
   https://rust-for-rustaceans.com/

Deeper/foundational (if you want to go further)

 • Book: Type‑Driven Development with Idris (smart constructors, proofs as types; great
   intuition builder) https://www.manning.com/books/type-driven-development-with-idris
 • Book: Algebra‑Driven Design (Sandy Maguire; modeling APIs via algebraic structure)
   https://leanpub.com/algebra-driven-design
 • Notes: Category Theory for Programmers (builds intuition for ADTs and compositional design)
   https://github.com/hmemcpy/milewski-ctfp-pdf
 • Refinement types (Liquid Haskell) for the “mathier” version of invariants at compile time
   https://ucsd-progsys.github.io/liquidhaskell-tutorial/

Practical patterns to apply in Rust (short checklist)

 • Prefer enums over boolean/config flag combos; carry data in variants.
 • Newtype wrappers for IDs/units to prevent mixing (UserId, ProjectId, Bytes, Lines).
 • Smart constructors returning Result to enforce invariants (NonEmptyString::new).
 • Private fields + public constructors to prevent invalid construction.
 • Typestate pattern for state machines and builders (Config<UnsetA, SetB>…), PhantomData
   markers.
 • Encode ranges and indexes with types (e.g., NonZeroUsize, BoundedU32 via constructor).
 • Parse early: convert “stringly-typed” inputs to domain types at the boundary (“parse, don’t
   validate”).
 • Exhaustive match internally to force handling of all cases; use non_exhaustive externally
   where you need evolution.
 • Property-based tests (proptest/quickcheck) to check invariants complementing the types.

How this shows up in large Rust codebases

 • Hot paths: enums/newtypes + smart constructors catch classes of bugs and simplify code
   reading.
 • APIs: keep constructors private, expose functions that uphold invariants (and document
   them).
 • Boundaries: validate once at ingress (files, DB, network) and convert to safe domain types;
   never pass raw inputs deeper.

A 6–8 week learning/apply plan

 • Week 1
    • Watch Feldman’s talk; read “Parse, don’t validate”.
    • Identify 3–5 places where booleans/options flag state; replace with enums.
 • Week 2
    • Introduce 2–3 newtypes (IDs, units). Add smart constructors; make fields private.
 • Weeks 3–4
    • Read Domain Modeling Made Functional. Pick one subsystem and model its states with ADTs
      and smart constructors.
    • Add 10–20 proptest cases that assert invariants you just encoded.
 • Week 5
    • Apply typestate to one builder/state machine that currently relies on runtime checks.
 • Week 6+
    • Write an internal mini-guide: “Invariants by construction” with 5 patterns you’ll reuse
      in the repo.
    • Add CI checks that forbid constructing certain types outside modules (via clippy lints
      and code review norms).

Common gotchas to avoid


 • Over-modeling: if the invariant is trivial and checked at parse-time already, don’t add
   ceremony.
 • Leaky constructors: a public struct with public fields subverts invariants—make fields
   private, offer smart constructors.
 • Mixed ownership: don’t bounce between String and &str/Arc; decide at boundary and stick to
   it.
 • Enums without data: carrying data in variants reduces invalid intermediate states.

If you share a small slice of a subsystem you’re unsure about, I can suggest concrete
enum/newtype/typestate shapes that encode its invariants directly.
