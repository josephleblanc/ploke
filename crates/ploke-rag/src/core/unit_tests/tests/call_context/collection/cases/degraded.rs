use super::super::super::super::*;
use super::super::helpers::*;
#[tokio::test]
async fn call_context_disabled_safely_when_relations_absent() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    assert!(
        !db.has_call_graph_relations().map_err(Error::from)?,
        "schema-less DB must not expose call-graph relations for this regression"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        rag.call_context_degraded(),
        "RagService must record degraded call-context when call graph relations are absent"
    );
    assert!(
        !rag.cfg.call_context.enabled,
        "degraded call-context should disable downstream collection and expansion"
    );

    let seed = Uuid::from_u128(0xfeed);
    let expanded = rag.expand_hits_with_call_context(&[(seed, 1.0)])?;
    assert_eq!(
        expanded,
        vec![(seed, 1.0)],
        "degraded call-context must not query callers from a DB without call graph relations"
    );
    let context = rag.collect_call_context(&[(seed, 1.0)])?;
    assert!(
        context.is_empty(),
        "degraded call-context must not query or attach call rows from a DB without call graph relations: {context:#?}"
    );

    Ok(())
}
