# Responses for live-loop Cozo query probes

Store captured command output and interpretation for queries in `../queries/`.

Naming convention:

```text
<section-id>-<number>__<short-slug>__step-<n>.md
```

Recommended response template:

```md
# <question id> — <short title>

## Question

<copy or link the live-loop functionality question>

## Query

- Query file: `../queries/<file>.cozo`
- Campaign: `<campaign-id>`
- Step: `<step number / phase>`

## Command

```bash
<P1_BIN> loop walk db_query --repo-root <P1_ROOT> --script "$(cat ../queries/<file>.cozo)"
```

## Raw output

```text
<paste stdout/stderr>
```

## Answer

Yes / Partial / No / Not reached.

## Helpfulness

Was this query useful? What did it answer or fail to answer?

## Schema/query design notes

What was easy? What required awkward joins, path interpretation, or fallback refs? What schema field/relation/index would make this question easier next time?
```
