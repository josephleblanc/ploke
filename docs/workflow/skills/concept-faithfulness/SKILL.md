---
name: concept-faithfulness
description: Use this skill when a design or implementation task risks drifting away from its conceptual framework, especially during eval/protocol work where ontology, roles, and composition need to remain faithful between prose, notation, and code.
---

# Concept Faithfulness

Use this skill when translating a conceptual framework into code, or when
reviewing code that is meant to embody a framework already defined elsewhere.

The goal is to prevent premature concretization from flattening or distorting
the conceptual structure.

## When To Use It

- A concept is being encoded as types, traits, modules, or step pipelines.
- Naming pressure starts carrying semantic meaning that should live in the type
  structure.
- A local implementation feels workable but no longer obviously matches the
  framework.
- A composed procedure risks collapsing into one opaque step or one ad hoc
  object.

## Required Workflow

1. Restate the conceptual object before editing code.
   - What kind of thing is this?
   - At what level does it live?
     - whole procedure
     - subprocedure
     - executor
     - evidence
     - value
     - shared role

2. Restate the relationship being encoded.
   - composition
   - role sharing
   - value domain
   - executor/method split
   - recursive procedure structure

3. Encode the shared role structurally.
   - Prefer traits and associated types for shared functional roles.
   - Do not rely on repeated generic names alone (`Context`, `State`,
     `Output`, `ProtocolDef`, etc.) to imply structural similarity.

4. Keep semantics in the right place.
   - Use the module tree and type structure to carry semantic identity.
   - Avoid long compound names whose only job is to smuggle ontology into the
     identifier.

5. After the edit, perform a concept check.
   - Which sentence or notation in the conceptual framework does this code now
     encode?
   - Did any conceptual distinction collapse?
   - Did a step output get mistaken for a full-procedure output?
   - Did a lexical naming trick replace a structural relationship?

## Design Heuristics

- Prefer temporary awkwardness over false clarity.
- If a concept is still unsettled, keep the implementation provisional rather
  than baking drift into stable names.
- Separate exploration mode from encoding mode:
  - first pin the concept in prose or notation
  - then encode it
  - then compare the encoding back to the concept

## Strong Preferences

- Shared role names belong on traits.
- Concrete module-local types should fill those roles.
- Prefer module-qualified use sites for generic type names.
- If composition is first-class, ask whether the ontology should be recursively
  uniform before stratifying it into unrelated kinds.

## Anti-Drift Questions

- Is this object a whole procedure or one `x_i` inside `x = (x_1, ..., x_n)`?
- Is this the metric, or the value in `Val(m)`?
- Is this the method specification, the executor, or the evidence bundle?
- Are we naming around a missing abstraction?
- If the code compiles but the ontology feels wrong, what relationship is still
  only implicit?

