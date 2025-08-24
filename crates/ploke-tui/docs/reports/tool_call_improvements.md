# Tool Call Implementation Improvements

## Current Status vs Context-Only Goal

Based on the recent test output, we're partially achieving our context-only focus goal from `context_only_focus_0002.md`. The core functionality works - we can request code context and receive snippets - but there are several misalignments with OpenRouter's documented tool calling patterns.

## Key Issues Identified

### 1. Incorrect Message Structure for Tool Results

**Problem**: Our second leg payload doesn't follow OpenRouter's documented structure.

**According to `request_structure.md`**, the second leg should include:
- Assistant message with `tool_calls` array containing the original call
- Tool message with `role: "tool"` and `tool_call_id` matching the original call

**Current Implementation Issue**: Our test builds a simplified followup but doesn't properly reconstruct the message history as specified.

### 2. Incomplete Tool Call Handling

**Problem**: Some providers ignore `tool_choice` or return 404 for tool support.

**Evidence from test**:
- `z-ai/glm-4.5-air:free` returned 404: "No endpoints found that support tool use"
- `qwen/qwen3-235b-a22b-thinking-2507` returned content but no `tool_calls` in the response

### 3. Missing Required Fields in Tool Definitions

**According to `tool_calling.md`**, tool definitions should include:
- Proper `type: "function"` structure
- Complete parameter schemas with `type`, `properties`, and `required` fields

## Recommended Improvements

### 1. Fix Message Structure for Tool Results

Ensure the second leg follows the documented pattern:

```json
{
  "model": "model-id",
  "messages": [
    {"role": "user", "content": "request"},
    {
      "role": "assistant", 
      "content": null,
      "tool_calls": [{
        "id": "call_abc123",
        "type": "function",
        "function": {
          "name": "request_code_context",
          "arguments": "{\"token_budget\": 512, \"hint\": \"SimpleStruct\"}"
        }
      }]
    },
    {
      "role": "tool",
      "tool_call_id": "call_abc123",
      "content": "{\"ok\": true, \"query\": \"SimpleStruct\", ...}"
    }
  ]
}
```

### 2. Improve Tool Definition Structure

Update `request_code_context_tool_def()` to include complete parameter schemas:

```json
{
  "type": "function",
  "function": {
    "name": "request_code_context",
    "description": "Request code context from the codebase",
    "parameters": {
      "type": "object",
      "properties": {
        "token_budget": {
          "type": "integer",
          "description": "Maximum tokens to include in the context"
        },
        "hint": {
          "type": "string",
          "description": "Search hint for finding relevant code"
        }
      },
      "required": ["token_budget"]
    }
  }
}
```

### 3. Better Provider Selection Logic

Enhance the provider selection to:
- Filter out known non-tool endpoints early
- Implement retry logic with different providers
- Add more detailed logging for provider capabilities

### 4. Enhanced Error Handling

Improve handling of:
- 404 responses for tool support (skip gracefully)
- 429 rate limiting (implement proper backoff)
- Empty tool_calls responses (log and skip)

## Success Metrics

From the test output, we had:
- **5 total outcomes** 
- **2 successes** (deepseek/deepseek-chat-v3.1 and z-ai/glm-4.5)
- **2 no_tool_calls** (providers ignored tool_choice)
- **1 http_404_first_leg** (provider doesn't support tools)
- **1 http_429_any_leg** (rate limiting)

This shows our approach is viable but needs refinement for better reliability and compliance with OpenRouter's API patterns.

## Next Steps

1. Update tool call message construction to match documented patterns
2. Enhance tool definitions with proper schemas
3. Improve provider capability detection
4. Add more comprehensive error handling and logging
5. Expand test coverage for different provider behaviors
