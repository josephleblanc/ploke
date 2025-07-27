 # Investigation: syn_parser failing to parse itself

 ## Summary
 The `syn_parser` crate fails when attempting to parse its own source code due to duplicate node detection. This appears to be
 related to cfg-gated code being processed multiple times under different configurations.

 ## Initial Failure Details
 - **Test**: `full::parse_self::basic_test`
 - **Error**: Panic in `crates/ingest/syn_parser/src/parser/graph/mod.rs:101`
 - **Message**: "Expected unique relations, found duplicate with error: Duplicate node found for ID..."

 ## Observable Symptoms
 1. Multiple cfg flags active simultaneously:
    - `cfg_content: test`
    - `cfg_content: not (feature = "not_wip_marker")`
    - `cfg_content: feature = "cfg_eval"`
    - `cfg_content: feature = "type_bearing_ids"`
    - `cfg_content: feature = "reexport"`

 2. Duplicate node types observed:
    - Impl nodes (`AnyNodeId::Impl`)
    - Import nodes (`AnyNodeId::Import`)
    - Macro nodes (`AnyNodeId::Macro`)

 3. Specific duplicate IDs from test output:
    - `AnyNodeId::Impl(S:9a1ea827..3807d5fb)`
    - `AnyNodeId::Impl(S:b4037036..a6fb0b08)`
    - `AnyNodeId::Import(S:33ead07f..c53f929a)`
    - `AnyNodeId::Macro(S:91149303..a0ac1319)`
    - `AnyNodeId::Macro(S:5a09c150..2592d8f8)`
    - `AnyNodeId::Import(S:e358f3f8..bd9b3d29)`

 ## Current Hypothesis
 The parser is visiting the same source items under multiple cfg configurations, creating what appear to be duplicates but are
 actually different cfg expansions of the same logical item. The uniqueness validation is too strict for this scenario.

 ## Default Feature Analysis
 From `Cargo.toml`, default features include:
 - `type_bearing_ids`
 - `validate`
 - `reexport`
 - `not_wip_marker`
 - `cfg_eval`

 The presence of `test` cfg suggests test code is being parsed alongside library code.

 ## Investigation Plan
 1. Determine how cfg configurations are being selected during discovery
 2. Identify if test code is being included in the parsing
 3. Verify whether the duplication is legitimate (different cfg variants) or an error
 4. Ensure we parse only the default feature set as intended

 ## Next Steps
 - Run test with debug logging to see active configurations
 - Examine discovery phase to understand cfg handling
 - Investigate how cfg evaluation is performed during parsing

## 2025-07-26 17:21
 - Discovery phase prints each `.rs` file **once**, with **empty cfg set** (`[]`).
 - The `cfg_content:` lines appear **after** discovery, inside the parallel parser threads.
 - Therefore **cfg evaluation happens *per worker*, not per discovery phase**.
 - Duplicates are **not** caused by multiple discovery passes; they are caused by **inconsistent cfg evaluation** inside the parser
 - Next step: inspect how cfg flags are determined inside `CodeVisitor`/`cfg_evaluator.rs`.
