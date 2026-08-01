Findings:
- Medium: node-scoped agent-turn evidence is currently attached to runtime and
  operation nodes as though the join were specific. The artifacts do not carry
  runtime IDs or operation coordinates, so downstream consumers cannot
  distinguish "belongs to this operation" from "under the same scheduler node".
- Low: attachment helpers clone full runtime/operation coordinate vectors per
  artifact. This is small today, but worth tightening in the normal graph
  ingestion path.

History boundary:
- No History authority write was found. Agent-turn evidence flows through
  passive evidence and located evidence attachment only.

Changed files: none.
