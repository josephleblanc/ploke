use anyhow::Result;
use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, child_process::TokioChildProcess},
};
use tokio::process::Command;

pub async fn client() -> Result<()> {
    let service = ()
        .serve(TokioChildProcess::new(Command::new("uvx").configure(
            |cmd| {
                cmd.arg("mcp-server-git");
            },
        ))?)
        .await?;

    // Initialize
    let server_info = service.peer_info();
    println!("Connected to server: {server_info:#?}");

    // List tools
    let tools = service.list_tools(Default::default()).await?;
    println!("Available tools: {tools:#?}");

    // Call tool 'git_status' with arguments = {"repo_path": "."}
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "git_status".into(),
            arguments: serde_json::json!({ "repo_path": "." }).as_object().cloned(),
        })
        .await?;
    println!("Tool result: {tool_result:#?}");

    service.cancel().await?;
    Ok(())
}
