# Raw Response Schema

Source reviewed: [llm-full-responses.jsonl](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/llm-full-responses.jsonl:1). This note covers all 9 JSONL entries, not just the final response.

No reasoning-like payload appears anywhere in this log. The complete nested key inventory contains only the paths below, so there is no hidden `reasoning`, `analysis`, `thoughts`, `scratchpad`, `refusal`, `annotations`, `logprobs`, or similar side-channel branch in these records.

Normalized field paths observed across the full file (`[]` = any array element):

```text
assistant_message_id                              string  x9
response                                          object  x9
response.choices                                  array   x9
response.choices[]                                object  x9
response.choices[].finish_reason                  string  x9
response.choices[].index                          number  x9
response.choices[].message                        object  x9
response.choices[].message.content                string  x1
response.choices[].message.role                   string  x9
response.choices[].message.tool_calls             array   x8
response.choices[].message.tool_calls[]           object  x8
response.choices[].message.tool_calls[].function  object  x8
response.choices[].message.tool_calls[].function.arguments string x8
response.choices[].message.tool_calls[].function.name string x8
response.choices[].message.tool_calls[].id        string  x8
response.choices[].message.tool_calls[].type      string  x8
response.choices[].native_finish_reason           string  x9
response.created                                  number  x9
response.id                                       string  x9
response.model                                    string  x9
response.object                                   string  x9
response.usage                                    object  x9
response.usage.completion_tokens                  number  x9
response.usage.prompt_tokens                      number  x9
response.usage.total_tokens                       number  x9
response_index                                    number  x9
```

Two concrete message variants are present:

1. Tool-call responses on [llm-full-responses.jsonl:1](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/llm-full-responses.jsonl:1) through line 8.
   `response.choices[0]` is always:
   - `finish_reason: "tool_calls"`
   - `native_finish_reason: "completed"`
   - `index: 0`
   - `message.role: "assistant"`
   - `message.tool_calls`: array length 1

   The sole tool call always has:
   - `id: string`
   - `type: "function"`
   - `function.name`: one of `read_file` or `request_code_context`
   - `function.arguments`: a JSON-encoded string, not a nested object

   Exact argument schemas observed in this log:
   - `read_file`: `{"file":"<absolute path>"}` on lines 1 and 6
   - `request_code_context`: `{"search_term":"<string>","token_budget":<number>}` on lines 2, 3, 4, 5, 7, and 8

2. Final content response on [llm-full-responses.jsonl:9](/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/runs/run-1776814500568-structured-current-policy-245b2fa1/llm-full-responses.jsonl:9).
   `response.choices[0]` is:
   - `finish_reason: "stop"`
   - `native_finish_reason: "completed"`
   - `index: 0`
   - `message.role: "assistant"`
   - `message.content: string`

Other stable wrapper facts across all 9 lines:

- `assistant_message_id` is the same UUID on every line: `b35b8dcd-4b8a-4228-9ed1-2a0774f31226`.
- `response_index` runs from `0` through `8`.
- `response.choices` always has length 1.
- `response.model` is always `x-ai/grok-4-fast`.
- `response.object` is always `chat.completion`.
- `response.choices[0].native_finish_reason` is always `completed`.
- `response.usage` always contains exactly `prompt_tokens`, `completion_tokens`, and `total_tokens`.

Bottom line: this sidecar is a thin chat-completion wrapper plus either one function tool call or one final `content` string. There is no separate reasoning payload anywhere in the raw responses for this rerun.
