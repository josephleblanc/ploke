# Tool Use — End-to-End Workflow

Version: 2025-08-21

Purpose
- Make it trivial to select a tool-capable endpoint and get reliable tool-calls working in the TUI.

Key Concepts
- Model: The logical model id (e.g., qwen/qwen-2.5-72b-instruct).
- Provider endpoint: An upstream provider (e.g., openai, anthropic, mistralai) behind OpenRouter that hosts an endpoint for a model. Some endpoints support tools; others do not.
- Provider slug: The stable, lowercase identifier for a provider endpoint (e.g., "openai").

Golden Path (CLI)
1) Search and pick a model (optional)
   :model search qwen
   - Use the browser overlay to pick a model or note the id.

2) List endpoints for the chosen model and identify tool-capable ones
   :model providers qwen/qwen-2.5-72b-instruct
   - Output includes provider_name and (slug). Tool-capable endpoints are marked [tools].

3) Pin a provider endpoint for that model
   :provider pin qwen/qwen-2.5-72b-instruct openai
   - Alias of :provider select …; this configures routing hints to prefer the given provider.

4) Enforce tool-capable routing (recommended)
   :provider tools-only on
   - Prevents silent fallback to non-tool calls when a selected endpoint lacks tools.

5) Run your tool-enabled prompt
   - Provide a tool schema in your message context (handled by the app), and dispatch as usual.

Failure Modes and What To Do
- Error “Selected endpoint appears to lack tool support.”:
  - Run :model providers <model_id> to see which endpoints support tools.
  - Pin a tool-capable endpoint with :provider pin <model_id> <provider_slug>.
  - Or temporarily allow non-tool completions with :provider tools-only off.

- Authentication errors:
  - Ensure OPENROUTER_API_KEY is set in your environment.

- Timeouts:
  - Network calls to chat/completions use a 45s timeout. Retry or choose another endpoint.

Notes on Routing
- Specifying supported_parameters allows us to use any model with those parameters, e.g. tools, routed by OpenRouter, so the model can be none in that case and a model will be selected by OpenRouter.
- The request payload includes a provider preference of the form:
  provider: { order: ["<slug>"] }
  when a valid provider slug is set for the active selection.
- This steers OpenRouter toward the chosen provider endpoint.

Operational Tips
- Use :model info to see the active provider id, base URL, and slug.
- Keep :provider tools-only on while iterating on agentic flows; it fails fast on misconfiguration.

Troubleshooting Checklist
- Is the model id correct? Format must be <author>/<slug>.
- Does the chosen endpoint advertise tool support? Check :model providers <model_id>.
- Is the provider slug valid? Use the slug printed by :model providers.
- Is OPENROUTER_API_KEY set? Use :check api or environment check.

Change Summary (This iteration)
- Added :model providers to enumerate endpoints and tool support.
- Added :provider pin as an alias to simplify selection.
- Request payloads now include provider preferences (order) when a slug is set.
- Removed fallback to non-tool calls on 404 tool-unsupported responses; we now fail fast with guidance.
- Added a 45s per-request timeout to improve resilience.
