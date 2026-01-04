# XML format issues

LLMs sometimes seem to send tool calls as xml messages, e.g. 

Found in the logs here:
`crates/ploke-tui/logs/api_responses_20260103_121206_11642.log`
```txt
// URL: https://openrouter.ai/api/v1/chat/completions
// Status: 200
{
  "id": "gen-1767471728-kcd3VwFCDlZGnamssnRf",
  "choices": [
    {
      "finish_reason": "stop",
      "native_finish_reason": "stop",
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Let me search for the actual function that creates ContextPlanMessages from messages:<tool_call>\n<function=search>\n<parameter=pattern>plan_messages\\.push</parameter>\n<parameter=path>/home/brasides/code/openai-codex/ploke/crates/ploke-tui/src</parameter>\n</function>\n</tool_call>"
      }
    }
  ],
  "created": 1767471728,
  "model": "xiaomi/mimo-v2-flash:free",
  "object": "chat.completion",
  "usage": {
    "prompt_tokens": 22461,
    "completion_tokens": 66,
    "total_tokens": 22527
  }
}
```
