// sketch of a ledger-shaped surface used as the shape for future backends attached to things like a
// merkle log, and eventually a distributed ledger.
//
// trait RecordStore {
//     fn append(record: SignedRecord) -> Result<RecordId>;
//     fn load(id: RecordId) -> Result<Record>;
//     fn query(selector: Query) -> Result<RecordSet>;
// }
//
// RecordStore can contain:
//
//   artifact_digest
//   binary_digest
//   build_recipe_digest
//   runtime_identity
//   role_authority
//   eval_set_digest
//   evaluator_artifact_digest
//   metric_schema_digest
//   policy_artifact_digest
//   input_evidence_set_digest
//   oracle_attestation_digest
//   parent_decision_digest
