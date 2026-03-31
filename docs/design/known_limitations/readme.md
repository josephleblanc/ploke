# Known limitations (`KL-*`)

This directory holds **known limitation** write-ups for `syn_parser` and related
ingestion behavior. The crate-level index is
[`syn_parser_known_limitations.md`](../syn_parser_known_limitations.md).

Corpus discovery and repro analysis that support many of these items:

- [`docs/active/agents/2026-03-29_corpus-triage/`](../../active/agents/2026-03-29_corpus-triage/)
  (runs from **2026-03-29** onward)
- [`docs/active/agents/2026-03-30_syn_parser_repro_rca/`](../../active/agents/2026-03-30_syn_parser_repro_rca/)
  (RCA summaries and per-cluster reports, **2026-03-30**)

---

## Lifecycle markers

Each `KL-*` file should keep these **near the top** (after the title) so status
is visible without reading the full note.

- **discovered** — First time we recorded the issue (corpus hit, test failure,
  or explicit note).
- **reproduced** — Minimal repro and/or `repro::fail` / success harness exists
  and documented.
- **deferred** — We intentionally postpone a code fix; often
  `PARSING_SOLUTION_DEFERRED` with a date in the KL body. Use **N/A** when the
  issue is **resolved** or not deferred.
- **resolved** — Fix landed in tree; limitation retired or narrowed. Use **N/A**
  while still active.

---

## Index (by id)

**KL-001** — [`KL-001-impl-dup-rel.md`](KL-001-impl-dup-rel.md)

- discovered: **2025-05-15** (approximate; no precise triage date)
- reproduced: **N/A** (no dedicated `repro::fail`; see file for legacy tests)
- deferred: **N/A** (fix still open; add a date when tagged `PARSING_SOLUTION_DEFERRED`)
- resolved: **N/A**

**KL-002** — [`KL-002-proc-macro-pre-expansion-syntax.md`](KL-002-proc-macro-pre-expansion-syntax.md)

- discovered: **2026-03-29** (corpus; see triage under `2026-03-29_corpus-triage`)
- reproduced: **2026-03-30** ([`proc_macro_parsing_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_proc_macro_parsing_report.md))
- deferred: **N/A**
- resolved: **N/A**

**KL-003** — [`KL-003-cfg-disjoint-duplicate-inline-mod.md`](KL-003-cfg-disjoint-duplicate-inline-mod.md)

- discovered: **2026-03-29**
- reproduced: **2026-03-30** ([`cfg_gates_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_cfg_gates_report.md))
- deferred: **N/A**
- resolved: **N/A**

**KL-004** — [`KL-004-nested-main-rs-logical-path.md`](KL-004-nested-main-rs-logical-path.md)

- discovered: **2026-03-29**
- reproduced: **2026-03-30** ([`file_links_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_file_links_report.md))
- deferred: **N/A**
- resolved: **N/A**

**KL-005** — [`KL-005-manifest-stricter-than-cargo-defaults.md`](KL-005-manifest-stricter-than-cargo-defaults.md)

- discovered: **2026-03-29** (same corpus window as strict manifest discovery)
- reproduced: **2026-03-30** ([`manifest_errors_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_manifest_errors_report.md); historical fail repros)
- deferred: **N/A** (superseded by migration to `cargo_toml`)
- resolved: **2026-03-31** (`cargo_toml::Manifest::from_path` discovery path)

**KL-006** — [`KL-006-partial-parse-non-compilable-files.md`](KL-006-partial-parse-non-compilable-files.md)

- discovered: **2026-03-29**
- reproduced: **2026-03-30** ([`partial_parsing_report.md`](../../active/agents/2026-03-30_syn_parser_repro_rca/reports/2026-03-30_partial_parsing_report.md))
- deferred: **N/A**
- resolved: **N/A**

**KL-007+** — *placeholder* — assign the next free id when adding a new limitation;
update this readme in the same PR.

---

## New issue template (copy into `KL-NNN-short-slug.md`)

Use the filename pattern `KL-NNN-kebab-case-title.md`. Link the new file from
[`syn_parser_known_limitations.md`](../syn_parser_known_limitations.md) and add a
row to the **Index** above.

```markdown
# KL-NNN Short title

## Lifecycle

- **discovered:** YYYY-MM-DD
- **reproduced:** YYYY-MM-DD (or N/A until a repro exists)
- **deferred:** YYYY-MM-DD (or N/A if resolved / not deferred)
- **resolved:** YYYY-MM-DD (or N/A while active)

## Description

What fails, under what conditions.

## Symptom

User-visible error strings or behaviors.

## Cause

Narrowest root cause we can defend.

## Relationship to other work

Cross-links to ADRs, other KL ids, or `syn_parser_known_limitations` sections.

## Current policy

Fail closed vs silent behavior; what we do **not** do without an explicit decision.

## Repro tests / fixtures

Paths under `crates/ingest/syn_parser/tests/...` and fixture workspace ids.

## Possible future resolution paths

Ordered bullets; large vs small effort.

## Further reading / evidence

Agent notes, corpus run ids, optional links to `docs/active/agents/...`.
```

---

## Other files in this directory

- [`github_project_targets/`](github_project_targets/) — templates for GitHub-scale tracking.
- **P3-00-cfg-duplication** (Phase 3, same mechanism as KL-003) — canonical copy in git:
  `git show 8de1588216561ad23290fac0a35993e4b2288e16:docs/design/known_limitations/P3-00-cfg-duplication.md`.
  Optional local mirror (gitignored): `docs/archive/syn_parser/P3-00-cfg-duplication.md`.
- [`requires_bin.md`](requires_bin.md) — standalone note (not a numbered KL).
