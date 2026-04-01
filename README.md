
  <p align="center">
    <img src="./assets/ploki.png" alt="Ploki" width="300" />
  </p>

# Ploke - LLM Coding for Rust

Ploke is a Rust-focused terminal-based application for developer-LLM collaboration. It natively parses your rust codebase into a vector-graph database and provides tools for efficient LLM search/edit functionality such as semantic search/edits, and Rust-focused tooling like a native "cargo test" tool that parses and filters test output for LLMs. Ploke focuses on providing vim-like bindings (Insert vs. Normal modes), extensive configuration through interactive overlays (model picker covers all available OpenRouter models, tool verbosity, embedding picker), and a design that invites discovery of code analysis and LLM functionality (semantic + bm25 search overlay mirrors LLM-facing code search tool).

If you want your coding LLMs to have deep comprehension of your Rust code base, you are in the right place.

<p align="center">
  <a href="#quick-start">Getting Started</a> ·
  <a href="#features-roadmap">Features</a> ·
  <a href="#docs">Docs (mdBook)</a> ·
  <a href="#troubleshooting">Troubleshoot</a> ·
  <a href="https://github.com/josephleblanc/ploke/issues">Contact Us</a>
</p>

<p align="center">
🚧 WARNING: Early Alpha 🚧
</p>

Under development, will likely not crash/panic if it tries to parse a crate with an
error, or in certain [known limitations](.docs/design/known_limitations/) or
unknown bugs. Please [open an issue](https://github.com/josephleblanc/ploke/issues) if you discovery one, it will jump to the
top of our todo list.

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

#### Choose a model via the model-picker 
https://github.com/user-attachments/assets/391b3f5a-dcbb-46a5-afa9-f4bdeb0b5d01
Default: [kimi-k2](https://openrouter.ai/moonshotai/kimi-k2)\
This opens an overlay that lets you navigate and select a model/provider. Search matches exact string, so to see all gpt-family models, search "gpt" and you'll see "gpt-5.1", "gpt-5.2", etc.
```txt
/model search <model>

For example:
/model search minimax
/model search kimi
/model search anthropic
/model search gpt
```

#### Select an embedding provider 
Default: local, can be slow (update coming soon, select a model if possible)\
This opens an overlay for embedding models available through openrouter, e.g. codestral by searching "code" or qwen-3 by searching "q" or "qwen".
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

### Commands

<details>
<summary>Expand/Collapse</summary>

| Kind | Commands | Purpose |
|---|---|---|
| Index | `/index start [path]` | Index the most specific crate or workspace target. |
|  | `/index pause` | Pause the active indexing job. |
|  | `/index resume` | Resume the active indexing job. |
|  | `/index cancel` | Cancel the active indexing job. |
| Models | `/model list` | List available models. |
|  | `/model info` | Show the active model and provider settings. |
|  | `/model use <name>` | Switch the active model by alias or id. |
|  | `/model <name>` | Legacy shorthand for `/model use <name>`. |
|  | `/model refresh [--local]` | Refresh the model registry, or just reload API keys with `--local`. |
|  | `/model load [path]` | Load `config.toml` from disk. |
|  | `/model save [path] [--with-keys]` | Save `config.toml`, redacting secrets unless `--with-keys` is set. |
|  | `/model search <keyword>` | Search OpenRouter models and open the model browser. |
|  | `/embedding search <keyword>` | Search OpenRouter embedding models and open the embedding browser. |
| Provider | `/model providers <model_id>` | List provider endpoints for a model and show tool support. |
|  | <code>/provider strictness &lt;openrouter-only&#124;allow-custom&#124;allow-any&gt;</code> | Restrict which provider types can be selected. |
|  | <code>/provider tools-only &lt;on&#124;off&gt;</code> | Require tool-capable providers for routing. |
|  | `/provider select <model_id> <provider_slug>` | Pin a model to a specific provider endpoint. |
|  | `/provider pin <model_id> <provider_slug>` | Alias for `/provider select`. |
| Context Search | `/search <query>` | Search indexed code context and open the context browser. |
| BM25 | `/bm25 search <query> [top_k]` | Search the sparse BM25 index. |
|  | `/hybrid <query> [top_k]` | Search with BM25 plus dense retrieval. |
|  | `/bm25 rebuild` | Rebuild the sparse BM25 index. |
|  | `/bm25 status` | Show BM25 index status. |
|  | `/bm25 save <path>` | Save the BM25 sidecar index. |
|  | `/bm25 load <path>` | Load the BM25 sidecar index. |
| Workspace | `/load workspace <name-or-id>` | Load a saved workspace snapshot. |
|  | `/load crate <name-or-id>` | Legacy alias for loading a saved workspace snapshot. |
|  | `/load crates <workspace> <crate>` | Load one crate subset into the current workspace snapshot. |
|  | `/save db` | Save the active workspace snapshot and registry entry. |
|  | `/sd` | Alias for `/save db`. |
|  | `/workspace status` | Show the current workspace status. |
|  | `/workspace update` | Refresh workspace state after filesystem changes. |
|  | `/update` | Scan the workspace and report files that need database updates. |
|  | `/workspace rm <crate>` | Remove one loaded crate namespace from the current workspace. |
| Edits | <code>/edit preview mode &lt;code&#124;diff&gt;</code> | Choose how staged edits are previewed. |
|  | `/edit preview lines <N>` | Set the maximum preview lines per section. |
|  | <code>/edit auto &lt;on&#124;off&gt;</code> | Toggle auto-approval of staged edits. |
|  | `/edit approve <request_id>` | Apply staged code edits. |
|  | `/edit deny <request_id>` | Discard staged code edits. |
|  | `/create approve <request_id>` | Apply staged file-creation proposals. |
|  | `/create deny <request_id>` | Discard staged file-creation proposals. |
| View | <code>/preview [on&#124;off&#124;toggle]</code> | Toggle the context preview panel. |
| Settings | <code>/tool verbosity &lt;minimal&#124;normal&#124;verbose&#124;toggle&gt;</code> | Inspect or change tool-output verbosity. |
|  | <code>/verbosity profile &lt;minimal&#124;normal&#124;verbose&#124;custom&gt;</code> | Inspect or change the conversation verbosity profile. |
| Utilities | `/check api` | Show API key setup hints. |
|  | `/copy` | Copy the selected conversation message to the clipboard. |
|  | `/save history` | Save conversation history and state. |
|  | `/query load [name file]` | Load a named query from disk. |
|  | `/ql` | Alias for `/query load`. |
|  | `/batch [prompt_file] [out_file] [max_hits] [threshold]` | Run batch prompt search and write results to a file. |
|  | `/context plan` | Open the context plan overlay. |
|  | `/contextplan` | Alias for `/context plan`. |
|  | `/help [topic]` | Show general or topic-specific help. |
|  | `/quit` | Exit the application. |

</details>


### Keybindings

#### Main chat

<details>
<summary>Expand/Collapse</summary>

| Mode | Key | Action |
|---|---|---|
| Any | `Ctrl+c` | Quit |
| Any | `Ctrl+f` | Cycle context management policy |
| Insert | `Esc` | Return to Normal mode |
| Insert | `Enter` / `Shift+Enter` | Submit message |
| Insert | `Tab` | Accept completion |
| Insert | `Up` / `Down` | Cycle suggestions |
| Insert | `Ctrl+p` / `Ctrl+n` | Cycle suggestions |
| Insert | `Ctrl+Up` / `Ctrl+Down` | Scroll the input box |
| Command | `Esc` | Return to Normal mode |
| Command | `Enter` / `Shift+Enter` | Execute command |
| Command | `Tab` | Accept completion |
| Command | `Up` / `Down` | Cycle suggestions |
| Command | `Ctrl+p` / `Ctrl+n` | Cycle suggestions |
| Command | `Backspace` | Edit the command buffer |
| Normal | `q` | Quit |
| Normal | `i` | Enter Insert mode |
| Normal | `:` | Enter command mode (`NeoVim` style) |
| Normal | `/` | Open the hybrid search prompt |
| Normal | `m` | Open quick model selection |
| Normal | `?` | Show help |
| Normal | `P` | Toggle context preview |
| Normal | `v` | Cycle tool verbosity |
| Normal | `e` | Open approvals |
| Normal | `s` | Open context search |
| Normal | `p` | Open context plan |
| Normal | `o` | Open config / tool settings |
| Normal | `Enter` | Trigger the selected item |
| Normal | `y` | Copy selected message |
| Normal | `Y` | Approve all pending edits |
| Normal | `N` | Deny all pending edits |
| Normal | `j` / `Down` | Move down |
| Normal | `k` / `Up` | Move up |
| Normal | `h` / `Left` | Previous branch |
| Normal | `l` / `Right` | Next branch |
| Normal | `J` | Page down |
| Normal | `K` | Page up |
| Normal | `gg` | Go to top of the conversation |
| Normal | `G` | Go to bottom of the conversation |
| Normal | `Del` | Delete selected conversation item |
| Normal | `Ctrl+n` / `Ctrl+p` | Scroll one line down/up |

</details>

#### Overlays

<details>
<summary>Expand/Collapse</summary>

| Overlay | Key | Action |
|---|---|---|
| Approvals | `Esc` / `q` | Close |
| Approvals | `Enter` / `y` | Approve selected proposal |
| Approvals | `n` / `d` | Deny selected proposal |
| Approvals | `o` | Open selected proposal in editor |
| Approvals | `u` | Toggle unlimited view |
| Approvals | `f` | Cycle filter |
| Approvals | `v` | Toggle diff view |
| Approvals | `?` | Toggle overlay help |
| Approvals | `+` / `=` | Show more lines |
| Approvals | `-` / `_` | Show fewer lines |
| Approvals | `Up` / `Down` / `PageUp` / `PageDown` / `Home` / `End` | Navigate and scroll |
| Model Browser | `Esc` / `q` | Close |
| Model Browser | `Up` / `Down` / `k` / `j` | Move selection |
| Model Browser | `Enter` / `Space` | Expand/collapse details |
| Model Browser | `l` | Expand or enter provider selection |
| Model Browser | `h` | Collapse or leave provider selection |
| Model Browser | `s` | Select the current model |
| Model Browser | `?` | Toggle overlay help |
| Embedding Browser | `Esc` / `q` | Close |
| Embedding Browser | `Up` / `Down` / `k` / `j` | Move selection |
| Embedding Browser | `Enter` / `Space` | Toggle detail level |
| Embedding Browser | `h` / `l` | Move detail level left/right |
| Embedding Browser | `s` | Select the current embedding model |
| Embedding Browser | `?` | Toggle overlay help |
| Context Search | `Esc` / `q` | Close |
| Context Search | `i` / `/` | Enter search input |
| Context Search | `Up` / `Down` / `k` / `j` | Move selection |
| Context Search | `h` / `Left` / `Backspace` | Collapse or decrease detail |
| Context Search | `l` / `Right` / `Space` | Expand or increase detail |
| Context Search | `Shift+h` / `Shift+l` | Jump to least/most detail |
| Context Search | `Enter` | Confirm the search input |
| Context Search (input) | `Backspace` / `Delete` | Edit the search text |
| Context Search (input) | `Left` / `Right` | Move the cursor |
| Context Search (input) | `Home` / `End` | Jump to start/end of the input |
| Context Search | `?` | Toggle overlay help |
| Context Plan | `Esc` / `q` | Close |
| Context Plan | `Up` / `Down` / `k` / `j` | Move selection |
| Context Plan | `Enter` / `Space` | Expand/collapse the selected item |
| Context Plan | `h` / `Left` | Collapse or step back in detail |
| Context Plan | `l` / `Right` | Expand or step forward in detail |
| Context Plan | `Shift+h` / `Shift+Left` | Step to the previous history entry |
| Context Plan | `Shift+l` / `Shift+Right` | Step to the next history entry |
| Context Plan | `f` | Cycle the context-plan filter |
| Context Plan | `s` | Toggle snippet visibility |
| Context Plan | `Tab` / `Shift+Tab` | Switch sections |
| Context Plan | `?` | Toggle overlay help |
| Config Overlay | `Esc` / `q` | Close |
| Config Overlay | `Tab` / `Shift+Tab` | Switch panes |
| Config Overlay | `h` / `l` | Move between panes |
| Config Overlay | `k` / `j` | Move within the current pane |
| Config Overlay | `Enter` / `Space` | Activate the selected setting |
| Config Overlay | `+` / `=` / `-` / `_` | Adjust numeric values |
| Config Overlay | `c` / `s` / `v` | Jump to categories / items / values |
| Config Overlay | `?` | Toggle overlay help |

</details>

## Security defaults

#### Model Permissions
We make a best-effort to keep the models safe, but ultimately it is up to you to keep them from malicious behavior we cannot foresee.

- By default, the model cannot directly edit your code. 

- You can approve/deny either in the approvals overlay or by selecting the earlier message in Normal mode and using `y` or `n`, `Y` approves all pending and `N` denies all pending.

- The model is able to execute `cargo test`, but this command is internally managed by ploke, and only accepts arguments that are parsed into a specific set of arguments (so they never make it to the shell).

- Note that we do not currently allow the LLM any direct shell access, but this may change in the future.

We will do our very best to ensure that ploke will ask for your explicit permission before elevating permissions.

#### Data Privacy
We do not collect any user data. In the future we may add an opt-in to help improve ploke, but we will never require your personal information for any reason.

That said, we do not take responsibility for information shared via API access with the models provided through this application.

**Coming soon**: Additional config to optionally set OpenRouter's Zero Data Retention as required

<a id="features-roadmap"></a>
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

#### Coming Soon(TM)
You can expect some of these in the next few weeks:
- UI + onboarding polish
- Forking/swapping chat branches with keybinds
- Static code analysis with type resolution
- More config options: agent profiles, skills, slash-commands
- Sub-agent support
- Natively parsed call graph
- Multi-sampling

#### Someday
We hope to build these in the next few months, but they are very ambitious goals and some of them are open areas of research. \
User feedback may influence the direction our development takes.
- "Auto-optimize" mode, [OpenEvolve](https://github.com/algorithmicsuperintelligence/openevolve)-inspired
- Built-in CVE analysis
- LLM-assisted formal verification methods
- Teams/Collab support
- Swarm support with file lock semantics inspired by rust's concurrency model
- [JODIE](https://arxiv.org/abs/1908.01207)-inspired embeddings (private by default) so ploke learns your coding style

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
Under construction, mdbook soon!

## Troubleshooting

We are in early alpha, expect UI to be buggy sometimes (list scrolling, etc).
Definitely open an issue and it will jump to the top of our todo list.

#### Indexing takes too long

Vector embedding is computationally expensive, sadly. The best fix for this right now is to use a remote embedding service automatically available for a low cost through your OPENROUTER_API_KEY.

Try using something like:
```txt
/embedding search code

Select codestral

/index start
```

This is as fast as we can make it, and batches requests to the embedding provider.

## License

Ploke is licensed under a **dual-license model**:

- **Open Source License**: You may use, modify, and redistribute Ploke under the terms of the [GNU General Public License v3.0 (GPL-3.0)](https://www.gnu.org/licenses/gpl-3.0.en.html).
- **Commercial License**: If you would like to use Ploke as part of a proprietary product, SaaS offering, or any commercial service without complying with GPL-3.0, you must obtain a commercial license.

To inquire about commercial licensing, please contact:
📩 **team@ploke.dev**
