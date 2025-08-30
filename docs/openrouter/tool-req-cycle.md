# Lifecycle of Tool Call

We begin with a model we want to call, need to get the tool output + loop.

1. Request info on providers for the model
- endpoint: 
  - https://openrouter.ai/api/v1/models/author/slug/endpoints
  - super confusing names conflict with other places in API, namely the model list returns "canonical_slug": "author/slug", and "available_providers" returns a "slug": "qwen".
  - example:
    - where
      - author  = qwen
      - slug    = qwen3-30b-a3b-thinking-2507
    - https://openrouter.ai/api/v1/models/qwen/qwen3-30b-a3b-thinking-2507/endpoints
- response type:
- all fields deseralized?

2. 

