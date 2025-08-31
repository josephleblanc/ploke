Original User Prompt for Aug 31 2025 `tool-e2e.md` plan

Review this file and then analyze the test artifact it generates. You can see
our overall goals in AGENTS.md, and our long-term goals in AGENT_SYSTEM_PLAN.md,
and your last `impl-log` was `docs/plans/agentic-system-plan/impl-log/
impl_20250831-211500Z.md`. Now, We have been working on the tools-calling system,
but have made a digression after identifying the failure of a helper function that
the tool system has been using, and originates in `ploke-db`.

While investigating the source of the error, we determined it would be best to
clearly state the inter-crate contract between the tool system in `ploke-tui`
and the db helpers in `ploke-db`. Therefore we created the document, crates/
ploke-tui/docs/crate-contracts/tool-to-ploke-db.md, and are now working on
making sure the helper function works correctly, and documenting the reasons
for its failure in `/home/brasides/code/openai-codex/ploke/docs/design/queries/
canonical_resolution_notes.md`.

Our immediate goal is to understand the precise reasons for the failure of the db
helper function we are using in the tools system so we can document the reasons
for the failure and avoid such pitfalls in the future. Thankfully we have a working
test for a very similar function to the helper function in crates/ploke-db/src/
get_by_id/mod.rs and have been checking our failing helper function against the
function in get_by_id in a new test in crates/ploke-db/src/helpers.rs, which
persists a test artifact that we can analyze, and we can expand the test to add
more information to the test artifact so we can fully understand the reasons for
our previous failure. We need to understand why exactly it failed and thoroughly
document the failure, including any errors in logic and/or syntax in the query,
and/or any priors that led us astray. This will help in the near future, when we
analyze other tools in `ploke-tui` to ensure we have everything working correctly
with the database, and should provide a guide for adding future db helpers as we
expand our tool system.

Once we have achieved our immediate goal we can update the tool-to-ploke-db
inter-crate contract with any further notes, and return to our primary task of
testing the tool system in preparation for live API endpoint testing against
the OpenRouter API system using our `ploke-tui` test harness and making requests
against the live API endpoints. Our goal here is to thoroughly test the full
pipeline in a realistic environment, and we will reference a recent document
crates/ploke-tui/docs/feature/agent-system/workflows/tool_use_end_to_end.md to
help in testing the entire system from end to end, verifying that each step is
correct. We will use the test harness in crates/ploke-tui/src/test_harness.rs and
if necessary expand the test harness to handle potential issues and/or desired
capabilities of the test harness. We will test that events are sent and received
where expected, that the shapes of the queries against the API endpoint are
of the correct shape and have expected values, that the responses are handled
correctly and are of the expected shape with expected fields, that the responses
are deserialized correctly by our system and we are not missing any fields during
deserialization or deserializing any fields into incorrect types, that our GAT
deserialization system is working correctly, that the tool requests are sent
with expected values in the correct shape, that the responses from the live api
endpoint are correctly deserialized, that the tool processes run correctly to
return expected outputs, that the tool outputs are sent back to the live API
endpoint using a correct shape, that the overall tool-call and messaging loop is
working correctly, and test for errors at each stage to ensure both happy-paths and
fail-states return expected values.

After we have our full end-to-end system tests ensuring valid, expected behavior
and all green, we can document the tests, add documentation to the tested items
with links to the tests describing all the validated behavior. Then we can create a
test review that provides a critique of the test, identifying coverage, weaknesses,
gaps, and room for improvement, along with all explicitly validated behavior.
Any behavior and/or states and/or execution paths not explicitly tested should be
identified and triaged.

Then we can begin adding benchmarks for the whole system, with a similar or greater
rigor, and use `criterion` and other testing tools (some present already under the
dev build in `ploke-tui`), and create a comprehensive performance profile for the
application. This comprehensive analysis will give us an idea of where to tighten
up the project, and we will identify the hot paths most in need of addressing.

So first, I would like you to create a new document in a new directory `plans`
crates/ploke-tui/docs/agent-system/plans/ with the plan for everything I have
described above, so we can track it over time.
