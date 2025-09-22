
---
title: Using MCP Servers with OpenRouter
subtitle: Use MCP Servers with OpenRouter
headline: Using MCP Servers with OpenRouter
canonical-url: 'https://openrouter.ai/docs/use-cases/mcp-servers'
'og:site_name': OpenRouter Documentation
'og:title': Using MCP Servers with OpenRouter
'og:description': Learn how to use MCP Servers with OpenRouter
'og:image':
  type: url
  value: >-
    https://openrouter.ai/dynamic-og?title=Using%20MCP%20Servers%20with%20OpenRouter&description=Learn%20how%20to%20use%20MCP%20Servers%20with%20OpenRouter
'og:image:width': 1200
'og:image:height': 630
'twitter:card': summary_large_image
'twitter:site': '@OpenRouterAI'
noindex: false
nofollow: false
---

MCP servers are a popular way of providing LLMs with tool calling abilities, and are an alternative to using OpenAI-compatible tool calling.

By converting MCP (Anthropic) tool definitions to OpenAI-compatible tool definitions, you can use MCP servers with OpenRouter.

In this example, we'll use [Anthropic's MCP client SDK](https://github.com/modelcontextprotocol/python-sdk?tab=readme-ov-file#writing-mcp-clients) to interact with the File System MCP, all with OpenRouter under the hood.

<Warning>
  Note that interacting with MCP servers is more complex than calling a REST
  endpoint. The MCP protocol is stateful and requires session management. The
  example below uses the MCP client SDK, but is still somewhat complex.
</Warning>

First, some setup. In order to run this you will need to pip install the packages, and create a `.env` file with OPENAI_API_KEY set. This example also assumes the directory `/Applications` exists.

```python
import asyncio
from typing import Optional
from contextlib import AsyncExitStack

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from openai import OpenAI
from dotenv import load_dotenv
import json

load_dotenv()  # load environment variables from .env

MODEL = "anthropic/claude-3-7-sonnet"

SERVER_CONFIG = {
    "command": "npx",
    "args": ["-y",
              "@modelcontextprotocol/server-filesystem",
              f"/Applications/"],
    "env": None
}
```

Next, our helper function to convert MCP tool definitions to OpenAI tool definitions:

```python

def convert_tool_format(tool):
    converted_tool = {
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": {
                "type": "object",
                "properties": tool.inputSchema["properties"],
                "required": tool.inputSchema["required"]
            }
        }
    }
    return converted_tool

```

And, the MCP client itself; a regrettable ~100 lines of code. Note that the SERVER_CONFIG is hard-coded into the client, but of course could be parameterized for other MCP servers.

```python
class MCPClient:
    def __init__(self):
        self.session: Optional[ClientSession] = None
        self.exit_stack = AsyncExitStack()
        self.openai = OpenAI(
            base_url="https://openrouter.ai/api/v1"
        )

    async def connect_to_server(self, server_config):
        server_params = StdioServerParameters(**server_config)
        stdio_transport = await self.exit_stack.enter_async_context(stdio_client(server_params))
        self.stdio, self.write = stdio_transport
        self.session = await self.exit_stack.enter_async_context(ClientSession(self.stdio, self.write))

        await self.session.initialize()

        # List available tools from the MCP server
        response = await self.session.list_tools()
        print("\nConnected to server with tools:", [tool.name for tool in response.tools])

        self.messages = []

    async def process_query(self, query: str) -> str:

        self.messages.append({
            "role": "user",
            "content": query
        })

        response = await self.session.list_tools()
        available_tools = [convert_tool_format(tool) for tool in response.tools]

        response = self.openai.chat.completions.create(
            model=MODEL,
            tools=available_tools,
            messages=self.messages
        )
        self.messages.append(response.choices[0].message.model_dump())

        final_text = []
        content = response.choices[0].message
        if content.tool_calls is not None:
            tool_name = content.tool_calls[0].function.name
            tool_args = content.tool_calls[0].function.arguments
            tool_args = json.loads(tool_args) if tool_args else {}

            # Execute tool call
            try:
                result = await self.session.call_tool(tool_name, tool_args)
                final_text.append(f"[Calling tool {tool_name} with args {tool_args}]")
            except Exception as e:
                print(f"Error calling tool {tool_name}: {e}")
                result = None

            self.messages.append({
                "role": "tool",
                "tool_call_id": content.tool_calls[0].id,
                "name": tool_name,
                "content": result.content
            })

            response = self.openai.chat.completions.create(
                model=MODEL,
                max_tokens=1000,
                messages=self.messages,
            )

            final_text.append(response.choices[0].message.content)
        else:
            final_text.append(content.content)

        return "\n".join(final_text)

    async def chat_loop(self):
        """Run an interactive chat loop"""
        print("\nMCP Client Started!")
        print("Type your queries or 'quit' to exit.")

        while True:
            try:
                query = input("\nQuery: ").strip()
                result = await self.process_query(query)
                print("Result:")
                print(result)

            except Exception as e:
                print(f"Error: {str(e)}")

    async def cleanup(self):
        await self.exit_stack.aclose()

async def main():
    client = MCPClient()
    try:
        await client.connect_to_server(SERVER_CONFIG)
        await client.chat_loop()
    finally:
        await client.cleanup()

if __name__ == "__main__":
    import sys
    asyncio.run(main())
```

Assembling all of the above code into mcp-client.py, you get a client that behaves as follows (some outputs truncated for brevity):

```bash
% python mcp-client.py

Secure MCP Filesystem Server running on stdio
Allowed directories: [ '/Applications' ]

Connected to server with tools: ['read_file', 'read_multiple_files', 'write_file'...]

MCP Client Started!
Type your queries or 'quit' to exit.

Query: Do I have microsoft office installed?

Result:
[Calling tool list_allowed_directories with args {}]
I can check if Microsoft Office is installed in the Applications folder:

Query: continue

Result:
[Calling tool search_files with args {'path': '/Applications', 'pattern': 'Microsoft'}]
Now let me check specifically for Microsoft Office applications:

Query: continue

Result:
I can see from the search results that Microsoft Office is indeed installed on your system.
The search found the following main Microsoft Office applications:

1. Microsoft Excel - /Applications/Microsoft Excel.app
2. Microsoft PowerPoint - /Applications/Microsoft PowerPoint.app
3. Microsoft Word - /Applications/Microsoft Word.app
4. OneDrive - /Applications/OneDrive.app (which includes Microsoft SharePoint integration)
```
