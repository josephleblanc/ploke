Let an artifact-bound code graph be:

```text
Γₐ = (Vₐ, Eₐ, μₐ)
```

where:

```text
Vₐ = semantic code nodes
Eₐ = semantic/code relations
μₐ : Vₐ ⇀ Spanₐ
```

`μₐ` maps editable graph nodes to material spans in artifact `a`.

A surface grant is:

```text
g = (R, W, F)
```

with:

```text
R, W, F ⊆ Vₐ
W ⊆ R
W ∩ F = ∅
W ⊆ dom(μₐ)
```

A proposal resolves to:

```text
ρₐ(q) = (Qr, Qw)
```

with:

```text
Qr, Qw ⊆ Vₐ
Qw ⊆ dom(μₐ)
```

Containment is exactly:

```text
g ⊢ q  iff  Qr ⊆ R ∧ Qw ⊆ W ∧ Qw ∩ F = ∅
```

Material validity is:

```text
validₐ(q) iff ∀v ∈ Qw. hashₐ(file(μₐ(v))) = expected_hash_q(v)
```

Application is only defined when both hold:

```text
applyₐ(q) is defined iff g ⊢ q ∧ validₐ(q)
```

Then:

```text
applyₐ(q) = (a', δ)
δ : a → a'
```

History admission is:

```text
H' = H ⋅ (a, Γₐ, g, q, ρₐ(q), δ, a')
```

That is the actual core.

Everything else is implementation detail:

- path globs are ways to construct subsets of `Vₐ`
- canons are predicates over `Vₐ`
- node kinds are predicates over `Vₐ`
- forbidden crates are predicates over `Vₐ`
- byte spans come from `μₐ`
- code graph/index staleness means you do not have a valid `Γₐ`

So if we want to specify `ploke-tui-tools`, it should be a subset constructor:

```text
S_tools(Γₐ) =
  { v ∈ Vₐ |
      relpath(v) starts_with "crates/ploke-tui/src/tools/"
      ∧ kind(v) ∈ {function, method}
  }
```

Then:

```text
R = context_closure(S_tools(Γₐ))
W = S_tools(Γₐ)
F = { v ∈ Vₐ | relpath(v) starts_with "crates/ploke-eval/" }
```

And the grant is:

```text
g_tools = (R, W, F)
```

That is concise enough to implement:

1. build `Γₐ`
2. derive `R`, `W`, `F`
3. resolve proposal to `(Qr, Qw)`
4. check subset relations
5. check hashes through `μₐ`
6. apply
7. record `(g, q, ρ, δ)`

The formal anchor is:

```text
SurfaceGrant = (R, W, F) over Γₐ
```

Not a path list, not a UI approval, not a report.
