use super::*;

pub(in crate::unit) fn proof_kind_count(rows: &[&ProofGraphContextRow], kind: &str) -> usize {
    rows.iter().filter(|row| row.kind == kind).count()
}

pub(in crate::unit) fn proof_fact_for_kind<'a>(
    rows: &'a [&ProofGraphContextRow],
    kind: &str,
) -> &'a ProofGraphContextRow {
    let matches = rows
        .iter()
        .copied()
        .filter(|row| row.kind == kind)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind} proof fact; rows: {rows:#?}"
    );
    matches[0]
}
