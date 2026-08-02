use uuid::Uuid;

pub(in crate::unit) fn fact_count(facts: &[serde_json::Value], kind: &str) -> usize {
    facts
        .iter()
        .filter(|fact| fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some(kind))
        .count()
}

pub(in crate::unit) fn fact_for_call_site<'a>(
    facts: &'a [serde_json::Value],
    kind: &str,
    site: Uuid,
) -> &'a serde_json::Value {
    let site = site.to_string();
    let matches = facts
        .iter()
        .filter(|fact| {
            fact.get("fact_kind").and_then(serde_json::Value::as_str) == Some(kind)
                && fact.get("call_site_id").and_then(serde_json::Value::as_str)
                    == Some(site.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "expected exactly one {kind} fact for call site {site}; facts: {facts:#?}"
    );
    matches[0]
}
