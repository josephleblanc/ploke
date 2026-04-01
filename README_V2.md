> **Draft / in progress.** Prerequisites for building from source: **Rust stable ≥ 1.85** (Rust 2024 edition; see `rust-version` in the workspace `Cargo.toml`), **`cargo`**, and **`git`** (first build fetches a `[patch.crates-io]` dependency). From the repo root, `./install.sh` builds the release binary and copies **`ploke`** into `~/.local/bin` by default (override with `INSTALL_DIR`). You need network access for the initial `cargo build`.

<!-- Continue your README rewrite below. -->

logo here

# Ploke - LLM Coding for Rust

Ploke is a Rust-focused terminal-based application for developer-LLM collaboration. It natively parses your rust codebase into a vector-graph database and provides tools for efficient LLM search/edit functionality such as semantic search/edits, and rust-focused tooling like a native "cargo test" tool that parses and filters test output. It focuses on providing vim-like bindings (Insert vs. Normal modes), extensive configuration through interactive overlays (model picker covers all available OpenRouter models, tool verbosity, embedding picker), and a design that invites discovery of code analysis and LLM functionality (semantic + bm25 search overlay mirrors LLM-facing code search tool).

If you want your coding LLMs to have deep comprehension of your code base, no matter how large, you are in the right place.

PLACEHOLDER: center-aligned links to getting started, features, docs (mdbook), troubleshoot, contact us

## Install

### Simple + Easy
```bash
git clone https://github.com/josephleblanc/ploke.git
cd ploke
./install.sh
```

### From Source (This ain't my first rodeo)
```bash
git clone https://github.com/josephleblanc/ploke.git
cd ploke
cargo build --release
```
This will put the ploke the `<git clone dir>/target/ploke`

## Quick start

After installation, make sure you have `OPENROUTER_API_KEY` set, e.g.
```bash
export OPENROUTER_API_KEY="sk-..." # OpenRouter key
```

Next you just go to the directory with your rust project and run ploke:
```bash
cd ~/path/to/rust/crate-or-workspace
ploke
```

From here you can use some commands to select a model, an embedding model, and index your codebase.

Select a model (default [kimi-k2](https://openrouter.ai/moonshotai/kimi-k2))
```txt
/model search <model>

For example:
/model search kimi
```

Choose an embedding model (defaults to local CPU processing with sentence-transformers currently, can be slow!)
```txt
/embedding search <model>
```
See all embedding models on OpenRouter [here](https://openrouter.ai/models?fmt=cards&output_modalities=embeddings).

Generate the code graph for your codebase. By default this generates a graph from your working directory, and will index a workspace if you are in a workspace, or a crate if you are in a crate.
```txt
/index start
```

Now you're good to go. The next time you send a message to the LLM, ploke will automatically use semantic search on your message to find related code for quick initial context, and ploke's built-in tools for LLMs allow the model to lookup code items or semantically search the code base.

By default, the model's context window is managed throughout your conversation, but you can toggle that off with `Ctrl+f`.

To save the code graph for your embedded and indexed codebase, you can use:
```txt
/save db
```

Then later you can load it faster with:
```txt
/load <crate-name>
/load <workspace-dir-name-only>
```

## Commands

Here are some common commands. For the full reference on available commands, use `/help` within ploke.

Choose a model via the model-picker (default [kimi-k2](https://openrouter.ai/moonshotai/kimi-k2)). This opens an overlay that lets you navigate and select a model/provider. Search matches exact string, so to see all gpt-family models, search "gpt" and you'll see "gpt-5.1", etc.
```txt
/model search <model>

For example:
/model search kimi
```
Examples:
```txt
/model search minimax
/model search kimi
/model search anthropic
/model search gpt
```

Select an emebedding provider (default [codestral](https://openrouter.ai/mistralai/codestral-embed-2505)). This opens an overlay for embedding models available through openrouter, e.g. codestral by searching "code" or qwen-3 by searching "q" or "qwen".
```txt
/embedding search <model>
```
```txt
Examples:
/embedding search code
/embedding search q
/embedding search qwen
```

Use the index command to generate the embeddings + perform hnsw indexing for fast lookup of code snippets:
```txt
/index start
```

To generate only the target crate, use:
```text
/index start path/to/crate
```

TOOD: Fill out key-code commands and restructure as a table
In any mode:
`Ctrl+f` - cycle context management policy (automatic pruning of tool calls)

In normal mode:
`o` - Open your config to set tool call verbosity and turn other knobs
`e` - Review and approve/deny edits
`p` - View the code snippets packaged into the LLM's context window
`k` or `UpArrow` - select previous message

TODO: Add table of `/command` kinds of commands
`/index start` - descr - example


## Security defaults

#### Model Permissions
By default, the model cannot directly edit your code. You can approve/deny either in the approvals overlay or by selecting the earlier message in Normal mode and using `y` or `n`, `Y` approves all pending and `N` denies all pending.

The model is able to execute `cargo test`, but this command is internally managed by ploke, and only accepts arguments that are parsed into a specific set of arguments (so they never make it to the shell).

Note that we do not currenly allow the LLM any direct shell access, but this may change in the future.

However, you can be sure that by default we will always ask for your explicit permission before elevating permissions.

#### Data Privacy
We do not collect any user data. In the future we may add an opt-in to help improve ploke, but we will never require your personal information for any reason.

That said, we do not take responsibility for information shared via API access with the models provided through this application.

Coming soon: Additional config to optionally set OpenRouter's Zero Data Retention as required

## Features + RoadMap

#### We already built this
- Interactive model search with all OpenRouter models (live updates, no config required)
- Interactive embedding search with all OpenRouter embedding models (live updates, no config required)
- Natively parsed code graph (no rust-analzyer dependency required)
- Hybrid semantic + graph code search via generated code graph
- Semantic edits (LLM uses node file and module path instead of search/replace)
- Non-semantic search: BM25 keyword search
- Non-semantic edits: on failing to parse or tool failure, falls back to non-semantic (search/replace) edits
- Code changes as git patches

#### Coming soon(TM)
You can expect some of these in the next few weeks:
- UI + onboarding polish
- Forking/swapping chat branches with keybinds
- Static code analysis with type resolution
- More config options: agent profiles, skills, slash-commands
- Sub-agent support
- Natively parsed call graph
- Multi-sampling

#### Someday
We hope to build these in the next few months. User feedback may influence the direction our development takes.
- "Auto-optimize" mode, [OpenEvolve](https://deepmind.google/discover/blog/alphaevolve-a-gemini-powered-coding-agent-for-designing-advanced-algorithms/)-inspired
- Built-in CVE analysis
- Teams/Collab support
- Swarm support with file lock semantics inspired by rust's concurrency model
- [JODIE](https://pmc.ncbi.nlm.nih.gov/articles/PMC6752886/)-inspired embeddings (private by default) so ploke learns your coding style

## Config

Configuration is loaded at startup from **`~/.config/ploke/config.toml`** (optional; missing file uses defaults) and merged with **environment variables** (nested keys use `_` separators, e.g. `token_limit` → `TOKEN_LIMIT`). LLM API keys are expected from the environment (e.g. `OPENROUTER_API_KEY`), not from the router section of the file.

Workspace metadata (recent roots, snapshot paths, etc.) is stored separately as **`workspaces.toml`** next to the main config (typically **`~/.config/ploke/workspaces.toml`**; uses the XDG config-local dir, which may differ slightly by platform).

| Section | Purpose | Notes |
|--------|---------|--------|
| **`registry`** | LLM registry: per-model profiles and parameters, allowed routers/endpoints, OpenRouter provider routing preferences, and registry strictness. | Used by the model router and overlays. |
| **`command_style`** | Command input style: NeoVim-style or slash (`/`) commands. | |
| **`tool_verbosity`** | How much detail the TUI shows for tool calls (`minimal` / `normal` / `verbose`). | |
| **`message_verbosity_profiles`** | Per-role message display (minimal/normal/verbose/custom); UI-only, does not change prompts. | |
| **`default_verbosity`** | Which profile is active | |
| **`embedding`** | Pick at most one of `local`, `hugging_face`, `openai`, or `cozo`. | See **Embedding backends** below. OpenRouter embedding models are **not** configured here; use `/embedding search` (and persisted DB state) instead. |
| **`embedding_local`** | Local embedder tuning: device, batch size, optional CUDA index, sequence length, etc. | Applies when using a local backend (including the default local model when no remote block is set). |
| **`editing`** | `auto_confirm_edits` and nested `agent` in the schema. | Only **`auto_confirm_edits`** is applied at runtime. **`agent`** is not read by the app today, and saving config resets it to defaults. |
| **`ploke_editor`** | Optional external editor command (overridden by `PLOKE_EDITOR` when set). | |
| **`context_management`** | `mode` (off/light/heavy), per-mode `top_k` / `per_part_max_tokens`, `max_leased_tokens`. | The **`strategy`** field (`Automatic` / `Ask` / `Unlimited` turns-to-live) is present in the file format but **not wired** to chat behavior yet. |
| **`tooling`** | Timeouts for `cargo check` / `cargo test`, and allowed extensions for create-file tooling. | |
| **`chat_policy`** | Tool-call timeouts, chain limits, retry/timeout strategy, and related chat-loop behavior. | |
| **`rag`** | Retrieval: top-k, per-part token limits, dense/sparse/hybrid strategy, BM25 timeouts, RRF/MMR fusion. | |
| **`token_limit`** | Default token budget for the **`request_code_context`** tool when the model does not pass a budget. | Not a global max-tokens cap for all LLM traffic. |
| **`tool_retries`** | Intended tool retry count. | **Currently unused** by the chat/tool loop (value is loaded and saved only). |
| **`llm_timeout_secs`** | HTTP timeout for chat requests to the LLM API. | |

### Embedding backends (`embedding.*` in TOML)

| Key | Status |
|-----|--------|
| **`local`** | **Supported** — runs the in-process local embedder (default model if you omit a block, or `embedding.local.model_id`). |
| **`hugging_face`** | **Supported** — calls the Hugging Face Inference API; API key can live in config if you accept that risk. |
| **`openai`** | **Planned** — Would call OpenAi's embedding models |

Use `/model save [path]` in the TUI to write the current configuration; omit `--with-keys` when sharing (intended for redacting secrets—embedding keys in TOML should still be treated as sensitive).

## Prerequisites
To install and run Ploke, you'll need the following tools installed on your system:

- **Rust toolchain (2024 edition):** Ploke is built using Rust 2024. [Install Rust using rustup](https://rustup.rs/).

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

  After installing, ensure your toolchain is up to date and set to the stable channel:
  ```bash
  rustup update
  rustup default stable
  ```

- **Cargo (comes with Rust):** Used for building and installing Rust projects.
- **A Unix-like shell (bash, zsh, etc):** The installation script (`install.sh`) is a Bash script.

**Optional:**  
If you wish to install the binary to a custom location (such as `$HOME/.cargo/bin`), set the `INSTALL_DIR` environment variable when running the install script:

```bash
INSTALL_DIR="$HOME/.cargo/bin" ./install.sh
```

The default install location is `$HOME/.local/bin` if `INSTALL_DIR` is not specified.

Make sure the chosen `INSTALL_DIR` is in your `PATH` so the `ploke` command can be found.

> If you do not have `cargo` on your `PATH`, the installer will print an error and exit early.  
> See the [Rust documentation](https://www.rust-lang.org/tools/install) for troubleshooting.

## Docs
Under construction

## Legal
