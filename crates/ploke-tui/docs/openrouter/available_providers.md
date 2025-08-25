# List Available Providers
Returns a list of providers available through the API with their policies and status information.

```bash
curl https://openrouter.ai/api/v1/providers
```

Retrieved:
```json
{
  "data": [
    {
      "name": "OpenAI",
      "slug": "openai",
      "privacy_policy_url": "https://openai.com/policies/privacy-policy/",
      "terms_of_service_url": "https://openai.com/policies/row-terms-of-use/",
      "status_page_url": "https://status.openai.com/"
    },
    {
      "name": "Anthropic",
      "slug": "anthropic",
      "privacy_policy_url": "https://www.anthropic.com/privacy",
      "terms_of_service_url": "https://www.anthropic.com/terms",
      "status_page_url": "https://status.anthropic.com/"
    }
  ]
}
```
