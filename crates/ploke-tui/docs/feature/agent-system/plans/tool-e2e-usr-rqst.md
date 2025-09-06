Original User Prompt for Aug 31 2025 `tool-e2e.md` plan

Our primary task of testing the tool system in preparation for live API
endpoint testing against the OpenRouter API system using our `ploke-tui` test
harness and making requests against the live API endpoints. Our goal here is to
thoroughly test the full pipeline in a realistic environment, and we will
reference a recent document
crates/ploke-tui/docs/feature/agent-system/workflows/tool_use_end_to_end.md to
help in testing the entire system from end to end, verifying that each step is
correct. We will use the test harness in crates/ploke-tui/src/test_harness.rs
and if necessary expand the test harness to handle potential issues and/or
desired capabilities of the test harness. 

We will test that:

- Events are sent and received where expected 
- The shapes of the queries against the API
- Endpoint are of the correct shape and have expected values 
- The responses
are handled correctly and are of the expected shape with expected fields
- The responses are deserialized correctly by our system and we are
  - not missing any fields during deserialization 
  - deserializing any fields into incorrect types 
- Our GAT deserialization system is working correctly
- The tool requests are sent with expected values in the correct shape
- The responses from the live api endpoint are correctly deserialized
- The tool processes run correctly to return expected outputs
- The tool outputs are sent back to the live API endpoint using a correct shape
- the overall tool-call and messaging loop is working correctly, 
- test for errors at each stage to ensure both happy-paths and fail-states
return expected values.

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
