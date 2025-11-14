# Blocker 06 – Provider Credentials & Model Catalog Sources

_Last updated: 2025-11-14_

## Problem statement
- Remote embedding UX must “meet users where they are,” but we currently lack:
  - Canonical env-var/config names for OpenAI, Hugging Face, local, and future providers.
  - A secure handoff from config → runtime (`EmbeddingServiceFactory`) that respects IoManager’s hash-based safety rules.
  - A reproducible way to acquire provider model catalogs (dimensions, limits) for CLI overlays and validation.
- Without these decisions, `/embedding rebuild` cannot validate prerequisites (API keys, quotas) or populate the registry (Blocker 03).

## Goals
1. Enumerate credential sources (env vars, config files, OS keyrings) and define precedence rules per provider.
2. Specify how IoManager handles secret material (hashing, staging, redaction) when persisting defaults.
3. Define catalog acquisition strategy per provider and caching policy, including fallback to bundled data when offline.
4. Provide testing/verification strategy to ensure credentials and catalogs are valid before remote embedding runs.

## Credential matrix
| Provider | Env var(s) | Config keys | Notes |
| --- | --- | --- | --- |
| OpenAI | `OPENAI_API_KEY`, `OPENAI_ORG_ID` (optional) | `embedding.providers.openai.api_key`, `org_id` (stored encrypted by IoManager) | Accept either env or config; env takes precedence. Config values stored via IoManager secrets file `~/.config/ploke/secrets.toml` with hash verification. |
| Hugging Face | `HF_API_TOKEN` | `embedding.providers.hugging_face.api_token` | Must support both Inference API and dedicated endpoints; add `inference_url` override for private deployments. |
| Azure OpenAI (future) | `AZURE_OPENAI_KEY`, `AZURE_OPENAI_ENDPOINT` | `embedding.providers.azure.openai` | Requires additional `deployment` field. |
| Cohere (future) | `COHERE_API_KEY` | `embedding.providers.cohere.api_key` |  |
| Local | none | `embedding.providers.local.model_path`, `auth_token` (optional) | Not secret; file path hashed by IoManager.

Rules:
- Secrets stored in config files never leave disk without IoManager hashed writes.
- Runtime resolution order per provider: CLI flag → env var → secrets file → plain config. Missing credentials cause `/embedding rebuild` to fail early with actionable message.
- Provide `ploke secrets set <provider>` helper (existing CLI extension) to write secrets via IoManager.

## Secrets persistence
- Introduce `~/.config/ploke/secrets.toml` managed exclusively by IoManager. Format:
```
[embedding.openai]
api_key = "encrypted:..."
org_id = "encrypted:..."
```
- Use libsodium or age to encrypt with a machine-local key; store key under `~/.config/ploke/key` (protected by 0600 permissions). At minimum, base64-encoded XOR with machine id (if libsodium not available) – but prefer real crypto.
- IoManager ensures writes include expected file hash; commands fail if file changed outside manager.

## Catalog acquisition
### OpenAI
- Endpoint: `GET https://api.openai.com/v1/models`.
- Response filtered for models with `capabilities.embedding = true`; call follow-up endpoint `GET /v1/embeddings?model=<id>` (with `input=[]`) to fetch `dimensions` if not in metadata.
- Cache file: `~/.cache/ploke/embedding_models/openai.json`. Contains provider response + timestamp + hash of HTTP body.
- Expiration: 24h by default; command `/embedding provider refresh openai` forces refresh.

### Hugging Face
- Source: GraphQL `https://huggingface.co/api/models?filter=embeddings` or REST `GET /models/<id>` for each pinned model.
- Additional metadata (dimensions) pulled from model card tags (field `config.hidden_size` or `sentence_embedding_dimension`).
- Cache file: `.../hugging_face.json` with same format.

### Local/Custom providers
- Users supply `catalog` file path in config. IoManager validates hash before use.

### Pricing integration
- After fetching remote catalogs, merge with `crates/ploke-tui/data/models/all_pricing_parsed.json`. If provider/model not present, set `cost = null` and highlight in CLI.

## Validation workflow
1. `EmbeddingRegistryLoader::load()` collects caches and ensures `timestamp + ttl` valid. If stale, fetch new catalog.
2. On `/embedding rebuild`, manager checks `credential_source(provider)`; if missing, return error referencing `ploke secrets set` command.
3. `cargo xtask verify-embedding-setup` runs before live tests:
   - Confirms secrets exist for providers flagged in config.
   - Ensures cache files exist and are <24h old.
   - Emits artifact `target/test-output/embedding/credentials_<timestamp>.json` summarizing status.

## Tests
- Unit tests mocking env vars + secrets file to ensure precedence order correct.
- Integration test for registry loader that simulates stale cache → HTTP fetch (use `wiremock`).
- CLI tests verifying `/embedding provider refresh` updates cache and reports file paths.

## Open issues
1. Where to store encryption key for secrets? Need decision between OS keyring vs. file-based approach. Proposed: start with file-based + 0600 perms, later integrate platform keyrings.
2. Offline mode: if no network, we should still allow previously cached catalogs (with warning) and skip live tests. Provide `--offline` flag to gating script.

Resolving this blocker lets the registry (Blocker 03) and activation workflow (Blocker 02) rely on consistent credential/catal og sources and reduces friction for users configuring remote embeddings.
