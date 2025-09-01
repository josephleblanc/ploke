## Squashed Bugs

 ### OpenRouter 500 caused by incorrect message field name

- Date found: Jul 22, 2025
- Date fixed: Jul 22, 2025

**Description**:
The OpenRouter API expects each chat message to contain a field named `"role"`
whose value is one of `"system"`, `"user"`, or `"assistant"`.
Our code was serializing messages with a field named `"kind"` instead of
`"role"`.
OpenRouter therefore rejected every request with an opaque 500 Internal Server
Error.

**The Fix**:
Change the serialization attribute in `RequestMessage` from `kind` to `role`, a
update the field name accordingly.

**Files changed**:
`crates/ploke-tui/src/llm/mod.rs`

**Added Tests**:
None yet (manual verification with OpenRouter endpoint required).

### most recent user message not included in request to LLM 

- Date found: Jul 20, 2025
- Date fixed: Jul 20, 2025

**Description**: The most recent user message was being sent through `AddUserMessage` at the same time as `EmbedMessage`. Because `AddUserMessage` writes a message to state.chat, and the `EmbedMessage` needs to read from state.chat (which is an RwLock), a race condition occurs between the two. 

This results in a non-deterministic bug where sometimes the `EmbedMessage` would read first, and sometimes the `AddUserMessage` would write first.

**The Fix**: Added a `tokio::sync::oneshot` Sender to the `AddUserMessage` and a Receiver to `EmbedMessage`, the make `EmbedMessage` await on the Receiver. This results in the `EmbedMessage` being guaranteed not to read from the state.chat until *after* the user's message has been written to the chat.

**Sanity Check**: A `tokio::sleep` was placed in the `AddUserMessage` event handling within the `match` for the even within `state_manager`. This meant that for this sleep duration (2 seconds), the user message was not added to the chat history. After these two seconds, the message appeared, and shortly afterwards was answered correctly by the LLM ("tell me a haiku").

**Question**: Why is this a successful fix? It seems like because both events are being sent from the same thread where `App::run` is being processed to a second thread where `state_manager` is being processed, there would still be a race condition. But that isn't the case, as proved the **sanity check**. Why?

**Added Tests**:
test app_state::tests::test_race_condition_without_oneshot
test app_state::tests::test_fix_with_oneshot
test app_state::tests::test_concurrency_with_fuzzing


### Duplicate Relation (Sep 1, 2025)

- Date Found: Sep 1, 2025
- Date Fixed: Sep 1, 2025

Notes: Added note to add new test in `docs/active/TECH_DEBT.md` with reference to this document.

#### Initial detection
The test `test::test::transform_tui` in `ploke-transform` failed while running `cargo test`.

This test failed with a message that there was a duplicate relation detected, providing the following message.
```text
Expected unique relations, found invalid duplicate with error: Duplicate node found for ID AnyNodeId::Import(S:50299b64..4690258f) when only one was expected.
```

#### Investigating root cause

Due to the duplicate relation detected, the bug is known to be in `syn_parser`, where the panic occurs.

There was a similar test in `syn_parser`, which could be run with logging (`syn_parser` is still using `log` over `tracing`, needs update), we could run:
```bash
RUST_LOG=dup=trace cargo test -p syn_parser --test mod full::parse_self::new_parse_tui -- --test-threads=1 2>&1 | rg -C 20 "50299b64"
```
This output some helpful information about the nodes from the logging in the parsing process so we could identify the code items in the target files.

##### Initial assessment

A duplicate relation of an import (both `ModuleImports` and `Contains`) was detected for:
```txt
ImportNode {
  id: ImportNodeId(Synthetic(50299b64-dc28-515b-a0e5-890f4690258f)),
  span: (660, 669),
  source_path: ["crate", "tools", "Tool"],
  kind: UseStatement(Inherited),
  visible_name: "_",
  original_name: Some("Tool"),
  is_glob: false,
  is_self_import: false,
  cfgs: []
}
```

I suspect this is an issue that may be with the `_` anonymous name, and might be with cfgs, and might be with both.

##### Follow-up assessment

Upon further investigation, it appears that this is an issue of Id Conflation. There was a second node detected with the same Id:
```
ImportNode { 
  id: ImportNodeId(Synthetic(50299b64-dc28-515b-a0e5-890f4690258f)), 
  span: (95, 112),
  source_path: ["ploke_rag", "TokenCounter"],
  kind: UseStatement(Inherited),
  visible_name: "_",
  original_name: Some("TokenCounter"),
  is_glob: false,
  is_self_import: false,
  cfgs: [] 
}
```

Both of these nodes were in `ploke-tui/llm/mod.rs`, and not nested in any further modules.

#### Root cause identified

Looking at the definition of the function that generates the synthetic Ids:
```rust
        pub fn generate_synthetic(
            crate_namespace: uuid::Uuid,
            file_path: &std::path::Path,
            relative_path: &[String],
            item_name: &str,
            item_kind: crate::ItemKind, // Use ItemKind from this crate
            parent_scope_id: Option<NodeId>,
            cfg_bytes: Option<&[u8]>,
        ) -> Self {
```
Each of these is going to be the same, notably the `item_name` will be the same, `"_"`.
- Therefore the v5 hash will be identical from the identical inputs

#### Initial Fix Proposed

To resolve this, we will need to add an exception for the handling of the `_` name in the parsing of imports:
- use the `original_name` if the name is `_`
- panic if `original_name` is `None` when `visible_name` is `_`

##### Fix Applied

file: `crates/ingest/syn_parser/src/parser/visitor/code_visitor.rs`
```rust
                let registration_result = if visible_name.as_str() == "_" {
                    self.register_new_node_id(
                        &original_name,
                        ItemKind::Import,
                        cfg_bytes, // Pass down received cfg_bytes
                    )
                } else {
                    self.register_new_node_id(
                        &visible_name,
                        ItemKind::Import,
                        cfg_bytes, // Pass down received cfg_bytes
                    )
                };
```

- No performance impact
- No additional allocation

##### Fix assessment

Result of tests:
```bash
RUST_LOG=trace cargo test -p syn_parser --test mod full::parse_self::new_parse_tui -- --test-threads=1
```
 - passes 

```bash
cargo test -p ploke-transform --lib tests::test_transform_tui
```
- passes

```bash
cargo test
```
- All tests that passed previously still pass, along with the previously failing `tests::test_transform_tui`
- Previously failing tests still fail, unrelated issues local to `ploke-tui`

#### Conclusions

Initial project-wide test suite caught an issue because we have been parsing our own code base, this is a good habit and way to provide ongoing testing of the application. We had not created a test for this specific issue, and now are aware of a previously uncaught quirk in parsing Rust.

Bug completely resolved.

#### Next Steps

Add more tests to `syn_parser` for this specific case. While our current tests will catch the issue, it remains somewhat opaque and would require additional effort to diagnose the issue should it recur.

Additionally, this was caught by finding a change by parsing our own files, which may change, so there is no guarentee that the problematic pattern will be tested by analyzing our own crates in the future. Adding this to a dedicated fixture, or integrating with a previously existing fixture, will provide ongoing testing to catch potential future regressions.

##### Actions Taken

- Added to `docs/active/TECH_DEBT.md` with reference to this document.
