# 2026-05-05 Type Resolution V1/V2 Perf

- date: 2026-05-05
- task title: Type resolution v1/v2 performance comparison
- task description: Compare the legacy post-tree type-use resolver with the typed v2 type-relation resolver, and separate resolver-only results from pipeline costs that may have moved into parsing/type construction.
- related planning files:
  - [`README.md`](README.md)
  - [`../readme.md`](../readme.md)

## Design Shape

The comparison has two rails:

- legacy: `resolve::type_resolution::resolve_type_uses_after_tree`
- typed v2: `resolve::type_resolution_v2::resolve_type_relations_after_tree`

The resolver-only benchmark intentionally starts from an already constructed
`ParsedCodeGraph` and `ModuleTree`. It measures post-tree resolution only, so it
does not prove that the whole ingestion pipeline is faster. Any cost shifted
into typed `TypeNode` construction, typed type-use slots, or type-family
refinement must be measured separately with pipeline benchmarks.

## Bench Commands

Resolver-only:

```bash
cargo bench -p syn_parser --bench type_resolution -- --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench -p syn_parser --bench type_resolution --features typed_type_graph -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Pipeline slices:

```bash
cargo bench -p syn_parser --bench parse_pipeline -- --sample-size 10 --warm-up-time 1 --measurement-time 2
cargo bench -p syn_parser --bench parse_pipeline --features typed_type_graph -- --sample-size 10 --warm-up-time 1 --measurement-time 2
```

Corpus resolver-only:

```bash
cargo bench -p syn_parser --bench type_resolution_corpus -- --sample-size 10 --warm-up-time 1 --measurement-time 1
cargo bench -p syn_parser --bench type_resolution_corpus --features typed_type_graph -- --sample-size 10 --warm-up-time 1 --measurement-time 1
```

For a durable reference before deleting the legacy resolver, keep a checkout or
commit at the point where both rails still compile and the two bench targets are
available.

## Initial Resolver-Only Results

Short Criterion run, ten samples, two-second measurement windows:

| Fixture | Legacy | Typed v2 |
| --- | ---: | ---: |
| `fixture_nodes` | ~429 us | ~55 us |
| `fixture_types` | ~93.6 us | ~18.9 us |
| `fixture_conflation` | ~168 us | ~40.2 us |
| `fixture_type_resolution_v2` | ~12.0 us | ~6.3 us |

These numbers are directional. The likely explanation is that v2 streams a
smaller typed relation object and avoids much of the legacy path/provenance
allocation and summary/reporting work. The missing question is whether typed
parsing and type graph construction absorbed meaningful cost.

## Initial Pipeline Slice Results

Short Criterion run on `fixture_nodes`, ten samples, two-second measurement
windows:

| Slice | Legacy/default | Typed v2 feature rail |
| --- | ---: | ---: |
| `try_run_phases_and_merge` | ~1.31 ms | ~1.28 ms |
| `analyze_files_parallel` | ~958 us | ~960 us |
| `merge_new` | ~64.1 us | ~62.6 us |
| `build_tree_and_prune` | ~300 us | ~300 us |

Criterion reported no meaningful change for parse/analyze, full merge/tree
setup, or tree build on this fixture. `merge_new` showed a small improvement in
the short run, but it is not the main design signal.

Taken together with the resolver-only benchmark, the first pass does not show
the typed type graph simply moving a large cost into parsing for
`fixture_nodes`. This should still be repeated on larger crates before treating
the ratio as stable.

## Initial Corpus Resolver Results

Short Criterion run over `tests/fixture_github_clones/corpus`, ten samples,
one-second requested measurement windows. The corpus path is a symlink to
`/home/brasides/code/github_clones/ploke-corpus`.

Coverage:

| Rail | Selected dirs | Benchable | Setup failures | Resolver failures |
| --- | ---: | ---: | ---: | ---: |
| legacy | 117 | 67 | 50 | 0 |
| typed v2 | 117 | 65 | 50 | 2 |

The 50 setup failures are mostly workspace-root manifests without
`package.name`, plus a small number of missing `Cargo.toml` paths and one
module-tree setup failure on `bytecodealliance__rustix`.

Typed v2 resolver failures:

| Target | Failure |
| --- | --- |
| `dtolnay__proc-macro2` | exceeded import chain depth limit of 100 |
| `dtolnay__syn` | exceeded import chain depth limit of 100 |

Largest common-target speedups by Criterion mean estimate:

| Target | Legacy | Typed v2 | Ratio |
| --- | ---: | ---: | ---: |
| `TheAlgorithms__Rust` | 71.9 ms | 4.62 ms | 15.6x |
| `BurntSushi__memchr` | 8.37 ms | 0.70 ms | 12.0x |
| `hyperium__hyper` | 8.46 ms | 0.83 ms | 10.2x |
| `BurntSushi__aho-corasick` | 34.5 ms | 3.54 ms | 9.75x |
| `matklad__once_cell` | 1.97 ms | 0.21 ms | 9.62x |
| `fish-shell__fish-shell` | 1.94 s | 222 ms | 8.71x |
| `jdx__mise` | 1.70 s | 227 ms | 7.50x |

Slowest successful typed v2 targets:

| Target | Typed v2 |
| --- | ---: |
| `jdx__mise` | 227 ms |
| `fish-shell__fish-shell` | 222 ms |
| `dani-garcia__vaultwarden` | 42.9 ms |
| `GyulyVGC__sniffnet` | 32.4 ms |
| `gitui-org__gitui` | 28.0 ms |

## Interpretation Guardrails

- Resolver-only speedups do not imply full-pipeline speedups.
- The v2 resolver currently emits typed successful relations, not the same rich
  diagnostic/report shape as the legacy resolver.
- Small fixtures amplify constant-factor wins.
- Pipeline results should be read as the end-to-end cost of typed type graph
  construction plus module tree construction, not as type resolution alone.
- Corpus target counts differ because typed v2 currently fails two import-chain
  depth cases that legacy resolves successfully.
