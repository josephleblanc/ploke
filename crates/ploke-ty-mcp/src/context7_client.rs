use anyhow::Result;
use itertools::Itertools;
use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, child_process::TokioChildProcess},
};
use tokio::process::Command;

pub async fn client() -> Result<()> {
    let service = ()
        .serve(TokioChildProcess::new(Command::new("npx").configure(
            |cmd| {
                cmd.args(["-y", "@upstash/context7-mcp"]);
            },
        ))?)
        .await?;

    // Initialize
    let server_info = service.peer_info();
    println!("Connected to server: {server_info:#?}");

    // List tools
    let tools = service.list_tools(Default::default()).await?;
    println!("Available tools: {tools:#?}");

    // Call tool 'resolve-library-id' with arguments = {"libraryName": "bevy"}
    let text_output = resolve_library_id(&service, "bevy").await?;
    println!("Tool result: {text_output}");

    // Call tool 'resolve-library-id' with arguments = {"libraryName": "bevy"}
    let text_output = get_library_docs(&service, "/bevyengine/bevy", 3000, "Component").await?;
    println!("Tool result: {text_output}");

    service.cancel().await?;
    Ok(())
}
/// Searches the context7 for a specific codext7 code. 
/// While the tool parameters claim that it is possible to limit the number of tokens in the
/// response, from the tests so far this does not seem to be the case.
/// However, the "topic" option does see to work correclty.
///
/// Tool {
///     name: "get-library-docs",
///     description: Some(
///         "Fetches up-to-date documentation for a library. You must call 'resolve-library-id' first to obtain the exact Context7-compatible library ID required to use this tool, UNLESS the user explicitly provides a library ID in the format '/org/project' or '/org/project/version' in their query.",
///     ),
///     input_schema: {
///         "$schema": String("http://json-schema.org/draft-07/schema#"),
///         "additionalProperties": Bool(false),
///         "properties": Object {
///             "context7CompatibleLibraryID": Object {
///                 "description": String("Exact Context7-compatible library ID (e.g., '/mongodb/docs', '/vercel/next.js', '/supabase/supabase', '/vercel/next.js/v14.3.0-canary.87') retrieved from 'resolve-library-id' or directly from user query in the format '/org/project' or '/org/project/version'."),
///                 "type": String("string"),
///             },
///             "tokens": Object {
///                 "description": String("Maximum number of tokens of documentation to retrieve (default: 10000). Higher values provide more context but consume more tokens."),
///                 "type": String("number"),
///             },
///             "topic": Object {
///                 "description": String("Topic to focus documentation on (e.g., 'hooks', 'routing')."),
///                 "type": String("string"),
///             },
///         },
///         "required": Array [
///             String("context7CompatibleLibraryID"),
///         ],
///         "type": String("object"),
///     },
///     annotations: None,
/// },
///
/// ## Example Error
/// When not following the instructions and entering, e.g. "bevy" or "bevyengine/bevy" instead of
/// "/bevyengine/bevy", the following error is received:
/// ```text
/// Error: Mcp error: -32602: MCP error -32602: Invalid arguments for tool get-library-docs: [
///   {
///     "code": "invalid_type",
///     "expected": "string",
///     "received": "undefined",
///     "path": [
///       "context7CompatibleLibraryID"
///     ],
///     "message": "Required"
///   }
/// ]
/// ```
///
/// ## Example Return
/// LANGUAGE: APIDOC
/// CODE:
/// ```
/// Entities API Changes:
///
/// 1. Entities::flush
///    - Purpose: Flushes entity operations, now with metadata support.
///    - Parameter Change: Now accepts `&mut EntityIdLocation` instead of `&mut EntityLocation`.
///    - Metadata: Asks for metadata about the flush operation.
///    - Source Location: `MaybeLocation::caller()` can be used.
///    - Tick: Should be retrieved from the world.
///
/// 2. EntityIdLocation
///    - Type: Alias for `Option<EntityLocation>`.
///    - Purpose: Represents an entity's location, allowing for `None` if an entity ID is allocated/reserved but not yet fully located (e.g., in commands).
///    - Behavior: Replaces invalid locations with `None`.
///
/// 3. Entities::free
///    - Return Type Change: Now returns `Option<EntityIdLocation>` instead of `Option<EntityLocation>`.
///
/// 4. Entities::get
///    - Status: Remains unchanged.
///
/// 5. Entities::get_id_location
///    - New Method: Provides access to an `Entity`'s `EntityIdLocation`.
/// ```
///
/// ----------------------------------------
///
/// ...
///
/// ----------------------------------------
///
/// TITLE: Observing `CheckChangeTicks` for Custom Schedules in Bevy ECS
/// DESCRIPTION: This Rust example demonstrates how to observe `CheckChangeTicks` and pass it to a custom schedule stored as a resource. This is useful when manually managing system ticks, ensuring that systems within the schedule correctly update their change ticks when `World::check_change_ticks` is called.
/// SOURCE: https://github.com/bevyengine/bevy/blob/main/release-content/migration-guides/check_change_ticks.md#_snippet_0
///
/// LANGUAGE: Rust
/// CODE:
/// ```
/// use bevy_ecs::prelude::*;
/// use bevy_ecs::component::CheckChangeTicks;
///
/// #[derive(Resource)]
/// struct CustomSchedule(Schedule);
///
/// let mut world = World::new();
/// world.add_observer(|check: On<CheckChangeTicks>, mut schedule: ResMut<CustomSchedule>| {
///     schedule.0.check_change_ticks(*check);
/// });
/// ```
pub async fn get_library_docs(
    service: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    input: &str,
    tokens: usize,
    topic: &str,
) -> Result<String, anyhow::Error> {
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "get-library-docs".into(),
            arguments: serde_json::json!({
                "context7CompatibleLibraryID": input.to_string(),
                "tokens": tokens,
                "topic": topic
            })
            .as_object()
            .cloned(),
        })
        .await?;
    let text_output = tool_result
        .content
        .into_iter()
        .filter_map(|a| a.as_text().map(|t| t.to_owned().text))
        .join("\n");
    Ok(text_output)
}

/// Enter a name, e.g. "bevy", and context7 will search for a list of items from its database,
/// providing instructions to the LLM on how to rank the examples and instructing it to return
/// exactly one best choice.
/// - JL 2025
///
/// ```rust,test
///
/// let service = ()
///     .serve(TokioChildProcess::new(Command::new("npx").configure(
///         |cmd| {
///             cmd.args(["-y", "@upstash/context7-mcp"]);
///         },
///     ))?)
///     .await?;
///
/// // Initialize
/// let server_info = service.peer_info();
/// eprintln!("Connected to server: {server_info:#?}");
///
/// // List tools
/// let tools = service.list_tools(Default::default()).await?;
/// eprintln!("Available tools: {tools:#?}");
///
/// // Call tool 'resolve-library-id' with arguments = {"libraryName": "bevy"}
/// let text_output = resolve_library_id(&service, "bevy").await?;
/// eprintln!("Tool result: {text_output}");
/// let expected_text = r#"- Title: Bevy
/// - Context7-compatible library ID: /bevyengine/bevy"#;
///
/// assert!(text_output.contains(expected_text));
///
/// service.cancel().await?;
/// Ok(())
/// ```
///
/// ## Tool call info (truncated on description, which is long)
/// ```rust
/// Tool {
///     name: "resolve-library-id",
///     description: Some(
///         "Resolves a package/product name to a Context7-compatible library ID and returns a
///         list..."
///     ),
///     input_schema: {
///         "$schema": String("http://json-schema.org/draft-07/schema#"),
///         "additionalProperties": Bool(false),
///         "properties": Object {
///             "libraryName": Object {
///                 "description": String("Library name to search for and retrieve a
///                 Context7-compatible library ID."),
///                 "type": String("string"),
///             },
///         },
///         "required": Array [
///             String("libraryName"),
///         ],
///         "type": String("object"),
///     },
///     annotations: None,
/// }
/// ```
/// ## In-Tool Description for LLM
/// Resolves a package/product name to a Context7-compatible library ID and returns a list of
/// matching libraries.\n\nYou MUST call this function before 'get-library-docs' to obtain a valid
/// Context7-compatible library ID UNLESS the user explicitly provides a library ID in the format
/// '/org/project' or '/org/project/version' in their query.\n\nSelection Process:\n1. Analyze the
/// query to understand what library/package the user is looking for\n2. Return the most relevant
/// match based on:\n- Name similarity to the query (exact matches prioritized)\n- Description
/// relevance to the query's intent\n- Documentation coverage (prioritize libraries with higher
/// Code Snippet counts)\n- Trust score (consider libraries with scores of 7-10 more
/// authoritative)\n\nResponse Format:\n- Return the selected library ID in a clearly marked
/// section\n- Provide a brief explanation for why this library was chosen\n- If multiple good
/// matches exist, acknowledge this but proceed with the most relevant one\n- If no good matches
/// exist, clearly state this and suggest query refinements\n\nFor ambiguous queries, request
/// clarification before proceeding with a best-guess match.
///
/// ## Returns
/// Tool result: Available Libraries (top matches):
///
/// Each result includes:
/// - Library ID: Context7-compatible identifier (format: /org/project)
/// - Name: Library or package name
/// - Description: Short summary
/// - Code Snippets: Number of available code examples
/// - Trust Score: Authority indicator
/// - Versions: List of versions if available. Use one of those versions if and only if the user explicitly provides a version in their query.
///
/// For best results, select libraries based on name match, trust score, snippet coverage, and relevance to your use case.
///
/// ----------
///
/// - Title: Bevy
/// - Context7-compatible library ID: /bevyengine/bevy
/// - Description: A refreshingly simple data-driven game engine built in Rust
/// - Code Snippets: 216
/// - Trust Score: 8.8
/// - Versions: v0.15.3, v0.14.0, v0.16.1
/// ----------
/// - Title: Bevy Egui
/// - Context7-compatible library ID: /vladbat00/bevy_egui
/// - Description: This crate provides an Egui integration for the Bevy game engine. 🇺🇦 Please support the Ukrainian army: https://savelife.in.ua/en/
/// - Code Snippets: 3
/// - Trust Score: 9.7
/// ----------
/// ...
/// ----------
/// - Title: bevy_extended_ui
/// - Context7-compatible library ID: /context7/rs-bevy_extended_ui-0.2.0-bevy_extended_ui
/// - Description: A Bevy plugin that extends UI capabilities by providing advanced widget management, state tracking, configuration, and image caching for UI elements.
/// - Code Snippets: 5686
/// - Trust Score: 9
///
/// ## Example
/// ```rust
/// let expected = r#" TITLE: Define a Bevy ECS Component in Rust
/// DESCRIPTION: Components are normal Rust structs that store data in a `World`. Specific instances of Components correlate to Entities, representing their attributes or state.
/// SOURCE: https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/README.md#_snippet_0
///
/// LANGUAGE: Rust
/// CODE:
/// \`\`\`
/// use bevy_ecs::prelude::*;
///
/// #[derive(Component)]
/// struct Position { x: f32, y: f32 }
/// \`\`\`#
/// ```
pub async fn resolve_library_id(
    service: &rmcp::service::RunningService<rmcp::RoleClient, ()>,
    input: &str,
) -> Result<String, anyhow::Error> {
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "resolve-library-id".into(),
            arguments: serde_json::json!({ "libraryName": input })
                .as_object()
                .cloned(),
        })
        .await?;
    let text_output = tool_result
        .content
        .into_iter()
        .filter_map(|a| a.as_text().map(|t| t.to_owned().text))
        .join("\n");
    Ok(text_output)
}

#[cfg(test)]
mod test {
    use super::*;
    #[tokio::test]
    async fn resolve_library_id_bevy() -> Result<()> {
        let service = ()
            .serve(TokioChildProcess::new(Command::new("npx").configure(
                |cmd| {
                    cmd.args(["-y", "@upstash/context7-mcp"]);
                },
            ))?)
            .await?;

        // Initialize
        let server_info = service.peer_info();
        eprintln!("Connected to server: {server_info:#?}");

        // List tools
        let tools = service.list_tools(Default::default()).await?;
        eprintln!("Available tools: {tools:#?}");

        // Call tool 'resolve-library-id' with arguments = {"libraryName": "bevy"}
        let text_output = resolve_library_id(&service, "bevy").await?;
        eprintln!("Tool result: {text_output}");
        let expected_text = r#"- Title: Bevy
- Context7-compatible library ID: /bevyengine/bevy
- Description: A refreshingly simple data-driven game engine built in Rust
- Code Snippets: 216
- Trust Score: 8.8
- Versions: v0.15.3, v0.14.0, v0.16.1"#;

        assert!(text_output.contains(expected_text));

        service.cancel().await?;
        Ok(())
    }

    #[tokio::test]
    async fn get_library_docs_bevy() -> Result<()> {
        let service = ()
            .serve(TokioChildProcess::new(Command::new("npx").configure(
                |cmd| {
                    cmd.args(["-y", "@upstash/context7-mcp"]);
                },
            ))?)
            .await?;

        // Initialize
        let server_info = service.peer_info();
        eprintln!("Connected to server: {server_info:#?}");

        // List tools
        let tools = service.list_tools(Default::default()).await?;
        eprintln!("Available tools: {tools:#?}");

        // Call tool 'resolve-library-id' with arguments = {"libraryName": "bevy"}
        let text_output = get_library_docs(&service, "/bevyengine/bevy", 3000, "Component").await?;
        let expected_text = r#"TITLE: Define a Bevy ECS Component in Rust
DESCRIPTION: Components are normal Rust structs that store data in a `World`. Specific instances of Components correlate to Entities, representing their attributes or state.
SOURCE: https://github.com/bevyengine/bevy/blob/main/crates/bevy_ecs/README.md#_snippet_0

LANGUAGE: Rust
CODE:
```
use bevy_ecs::prelude::*;

#[derive(Component)]
struct Position { x: f32, y: f32 }
```"#;
        eprintln!("Tool result: {text_output}");

        assert!(text_output.contains(expected_text));

        service.cancel().await?;
        Ok(())
    }
}
