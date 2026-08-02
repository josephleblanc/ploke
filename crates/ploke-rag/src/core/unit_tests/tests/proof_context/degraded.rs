use super::super::*;
use ploke_db::ProofGraphStore;

#[tokio::test]
async fn proof_context_disabled_safely_when_facts_absent() -> Result<(), Error> {
    init_tracing_once();

    let raw = Db::new(MemStorage::default()).expect("in-memory cozo db");
    raw.initialize().expect("initialize cozo db");
    let db = Arc::new(Database::new(raw));
    assert!(
        !db.has_proof_graph_facts().map_err(Error::from)?,
        "schema-less DB must not expose populated proof facts for this regression"
    );

    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        rag.proof_context_degraded(),
        "RagService must record degraded proof context when proof facts are absent"
    );
    assert!(
        !rag.cfg.proof_context.enabled,
        "degraded proof context should disable downstream proof collection"
    );

    let seed = Uuid::from_u128(0xfeed);
    let context = rag.collect_proof_context(&[(seed, 1.0)])?;
    assert!(
        context.is_empty(),
        "degraded proof context must not attach proof rows from a DB without proof facts: {context:#?}"
    );

    Ok(())
}
