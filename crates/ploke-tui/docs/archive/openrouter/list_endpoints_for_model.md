# List endpoints for a model
https://openrouter.ai/api/v1/models/:author/:slug/endpoints

Path parameters
- author: string, Required
- slug: string, Required
```bash
curl https://openrouter.ai/api/v1/models/author/slug/endpoints
```

Retrieved:
```json
{
  "data": {
    "id": "string",
    "name": "string",
    "created": 1.1,
    "description": "string",
    "architecture": {
      "input_modalities": [
        "text",
        "image"
      ],
      "output_modalities": [
        "text"
      ],
      "tokenizer": "string",
      "instruct_type": "string"
    },
    "endpoints": [
      {
        "name": "string",
        "context_length": 1.1,
        "pricing": {
          "request": "string",
          "image": "string",
          "prompt": "string",
          "completion": "string"
        },
        "provider_name": "string",
        "supported_parameters": [
          "string"
        ],
        "quantization": "string",
        "max_completion_tokens": 1.1,
        "max_prompt_tokens": 1.1,
        "status": "string",
        "uptime_last_30m": 1.1
      }
    ]
  }
}
```

## List models filtered by user provider preferences
https://openrouter.ai/api/v1/models/user
Returns a list of models available through the API, filtered based on the user’s provider preferences. This endpoint returns the same data structure as /models but only includes models from providers that are not in the user’s ignored providers list and are either in the user’s allowed providers list (if configured) or from any provider (if no allowed providers are specified). These provider preferences can be configured on the /settings page.

Headers
Authorization
string
Required
Bearer authentication of the form Bearer <token>, where token is your auth token.

Response
List of available models filtered by user preferences
data
list of objects

```bash
curl https://openrouter.ai/api/v1/models/user \
     -H "Authorization: Bearer <token>"
```

Retrieved:
```json
{
  "data": [
    {
      "id": "string",
      "name": "string",
      "created": 1741818122,
      "description": "string",
      "architecture": {
        "input_modalities": [
          "text",
          "image"
        ],
        "output_modalities": [
          "text"
        ],
        "tokenizer": "GPT",
        "instruct_type": "string"
      },
      "top_provider": {
        "is_moderated": true,
        "context_length": 128000,
        "max_completion_tokens": 16384
      },
      "pricing": {
        "prompt": "0.0000007",
        "completion": "0.0000007",
        "image": "0",
        "request": "0",
        "web_search": "0",
        "internal_reasoning": "0",
        "input_cache_read": "0",
        "input_cache_write": "0"
      },
      "canonical_slug": "string",
      "context_length": 128000,
      "hugging_face_id": "string",
      "per_request_limits": {},
      "supported_parameters": [
        "string"
      ]
    }
  ]
}
```

### Errors
400
Bad Request Error

401
Unauthorized Error
