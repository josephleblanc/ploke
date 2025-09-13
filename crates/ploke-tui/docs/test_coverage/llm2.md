# Test Coverage in `ploke_tui::llm2`

## Responses

Tests to cover request items sent to the API.

### Fields of `models::Response`

 - `llm2::types::model_types::tests`
     - `ModelId`
     - `Architecture`
 - `llm2::router_only::openrouter::tests`
     - `TopProvider`
 - `llm2::request::tests`
     - `ModelPricing`
 - `llm2::types::enums::tests`
     - `SupportedParameters` (todo)

note: current tests handle deserializing and noting items from `/models`, but we could probably use some more tests for these.

### Fields of `models::Endpoint`

- serialization and deserialization unit tests for all fields

### Fields of `types::params::LLMParameters`

- serialization + deserialization tests for all fields
- checks if none/some serialized

### TODO

- [ ] add prop tests
