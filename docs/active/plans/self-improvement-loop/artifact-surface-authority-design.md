# Artifact Surface And Authority Design Intent

Updated: 2026-05-10

## Purpose

This note records the design intent behind Prototype 1 artifact surfaces,
candidate patch provenance, and History authority succession.

The immediate motivation is the historical-successor surface-root mismatch
seen in `p1-overnight-edit-surface-20260510-3`, where a sealed History block
paired one artifact's tree claim with another artifact's surface roots. The
larger issue is conceptual: patch lineage, artifact identity, and authority
succession are related, but they are not the same structure.

## Three Distinct Axes

### Patch Provenance

Patch provenance answers:

```text
How was artifact B produced?
```

The intended shape is:

```text
runtime R observes/input artifact A
runtime R produces patch P
patch P is applied to A
artifact B is created
```

This is where edit-operation evidence belongs:

- parent/runtime attribution
- base artifact
- patch id
- target path
- touches
- source and proposed content hashes
- proposal/generator policy
- check/apply status

This axis supports attribution:

```text
Which parent produced this child?
Which patch improved the benchmark?
Which edit surface or proposal method produced the useful change?
```

### Artifact Surface Measurement

Artifact surface measurement answers:

```text
What is artifact B?
```

For a measurable artifact `B`, the backend should be able to produce a stable
surface profile:

```text
B_t = artifact tree key
B_i = immutable authority surface root
B_m = mutated surface root
B_a = ambient surface root
```

These values are not the patch provenance. They are a measured profile of the
artifact after the patch has been applied and committed.

Any candidate eligible for successor handoff should carry, or be able to
validate, this measured artifact-surface evidence. The evidence should name the
measurement algorithm/version so `B_i`, `B_m`, and `B_a` are not ambiguous.

### Authority Succession

Authority succession answers:

```text
Who ruled next?
```

The intended shape is:

```text
current ruler artifact F
current ruler considers candidates {B, C, D, ...}
current ruler selects B
History seals the authority transition F -> B
successor runtime starts from B
B becomes the next ruler
```

This is where `Crown`, `Block`, History head movement, successor invocation,
and startup validation belong.

This axis supports protocol validation:

```text
Was there one local ruler?
Did that ruler seal a valid successor?
Does the successor checkout match the sealed head?
Can the next parent safely continue the chain?
```

## Surface Commitment

`SurfaceCommitment` is the block-level commitment for an authority transition.
For a transition from current ruler artifact `F` to selected successor artifact
`B`, it should encode:

```text
immutable:        F_i, with the required check F_i == B_i
mutated.before:  F_m
mutated.after:   B_m
ambient.before:  F_a
ambient.after:   B_a
```

The artifact claim admitted in the same block must use `B_t`.

The essential invariant is:

```text
The block's artifact claim and surface after-roots must describe the same
selected successor artifact.
```

The failed run violated this by sealing:

```text
SurfaceCommitment(F, F)
artifact claim B_t
```

instead of:

```text
SurfaceCommitment(F, B)
artifact claim B_t
```

## Candidate Evidence

The current name `SurfaceEvidence` is overloaded. It currently represents
candidate-local edit evidence: checked touches, source/proposed content hashes,
patch id, generator surface provenance, and related data.

That evidence is useful, but it is not the same as the artifact surface profile
needed by History startup validation. In particular, current candidate-local
edit evidence does not directly contain:

```text
B_t
B_i
B_m
B_a
measurement algorithm/version
```

The intended durable candidate shape is:

```text
CandidateArtifact {
  patch_provenance: optional or required by producer kind,
  artifact_surface: required for successor-eligible candidates,
}
```

Deterministic edit-surface candidates should carry both:

- patch provenance explaining how the child was produced
- artifact-surface measurement identifying what artifact was produced

Legacy candidates may have weaker patch provenance, but any candidate that can
be selected later from History must have artifact-surface evidence sufficient to
construct or validate `SurfaceCommitment(F, B)`.

## Validation Model

The git backend is the current truth anchor for artifact recovery and
measurement.

Given an artifact/tree/branch for `B`, validation should be able to recompute:

```text
B_t
B_i
B_m
B_a
```

and check those values against the admitted artifact-surface evidence.

This validation is separate from patch provenance validation. Patch provenance
can show that:

```text
P(A) -> B
```

Artifact surface validation shows that:

```text
the recoverable artifact B has the recorded surface profile
```

Authority succession then constructs:

```text
SurfaceCommitment(F, B)
```

from the active ruler surface `F_*` and the selected successor surface `B_*`.

## Required Invariants

1. Stable measurement:

   The same artifact measured by the same backend algorithm must produce the
   same tree key and surface roots.

2. Candidate recoverability:

   A candidate eligible for successor handoff must be recoverable enough to
   validate its recorded artifact surface.

3. Patch provenance:

   Non-genesis artifacts should be attributable to a runtime, input artifact,
   patch, and apply/check result. This explains how the artifact was produced;
   it does not replace artifact-surface measurement.

4. Handoff transition:

   A sealed authority transition from `F` to `B` must commit to
   `SurfaceCommitment(F, B)`.

5. Artifact/surface coherence:

   The artifact claim tree key and the surface after-roots in a sealed block
   must refer to the same selected successor artifact.

6. Startup validation:

   The successor runtime must recompute the current checkout tree key and
   surface profile, then verify both against the sealed History head before it
   becomes the next parent.

## Naming Guidance

Do not use one generic "surface" name for all of these concepts.

Prefer names that preserve the relation being modeled:

- patch/edit evidence for the patch-production axis
- artifact surface measurement for `B_t`, `B_i`, `B_m`, `B_a`
- transition surface commitment for `F -> B`
- authority succession for History block movement

Do not let a type named `SurfaceEvidence` imply that edit-proposal evidence and
block-level artifact-surface roots are interchangeable. If that type evolves,
its fields should make the two roles explicit rather than hiding them under one
flat name.

## Design Consequence

Future handoff code should not construct block surfaces by sampling arbitrary
workspace paths at unrelated times. It should construct the authority
transition from two artifact surfaces:

```text
current ruler surface F_*
selected successor surface B_*
```

The old child workspace is not authority. It is one possible materialization
source for validating `B_*`. If `B_*` was admitted durably when the child became
a candidate, historical selection should be able to use that admitted evidence
without requiring the old workspace to still exist.
