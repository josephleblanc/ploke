# Description and analysis of project READMEs

## Independent TUIs (Popular, 10k+ Stars)

### Aider

Stars: 39k

https://github.com/Aider-AI/aider

#### Structure

#### Notes

### Plandex

https://github.com/plandex-ai/plandex

#### Structure

#### Notes

### Goose

Stars: 24k

https://github.com/block/goose

#### Structure

#### Notes

## Independent TUIs (Chinese, 10k+ Stars)

### Crush

Stars: 16k

https://github.com/charmbracelet/crush

#### Structure

#### Notes


## Independent TUIs (Small, <10k Stars)

### Forge

Note: Written in Rust

https://github.com/antinomyhq/forge

#### Structure

#### Notes

### Tenere

Note: Written in Rust

https://github.com/pythops/tenere

#### Structure

#### Notes

### melty

Stars: 5.4k

https://github.com/meltylabs/melty

#### Structure

#### Notes

### OpenCode

#### Structure

Badges:
  - discord
  - ndpm
  - build (github actions)

Intro: very short under title - "The open source AI coding agent"

1. Installation
2. Desktop App (BETA)
3. Agents (build vs. plan + links to docs)
4. Documentation
5. Contributing
6. Build on OpenCode (asking for attribution)
7. FAQ

#### Notes

"installation" has different package managers and comments for which commands
are for which build targets.
  - Slightly annoying, since you can't just click the "copy" button

## Independent (Very small, <100 stars)

### Aider

Stars: 7

https://github.com/jyjeanne/crustly

#### Structure

#### Notes

## Model Provider TUIs (US-based)

OpenAI's Codex, Anthropic's Claude Code, etc

### Claude code (Anthropic)

Stars: 46k

https://github.com/anthropics/claude-code/blob/main/README.md

#### Structure

Badges: Node.js, npm

Intro: long single sentence

1. Intro (under title)
  - followed by gif showing claude running tests
2. Get started
3. Plugins
4. Reported Bugs
5. Connect on Discord
6. Data collection, usage, retention
7. Join our community (links to Discord/X.com)
  - small header, right at the end


#### Notes

Overall a very short readme, with the longest section being the "Get Started"
section that includes code blocks with bash commands that can be copied and
pasted to install the project. Each of these is a single line, and each is for
a different OS (or the npm install method).

Unrelated note: it looks like the OpenCode people were able to monetize under
[OpenCode Zen](https://opencode.ai/zen), which they only mention in passing in
the FAQ. Good for them!

### Codex (OpenAI)

Stars: 54k

https://github.com/openai/codex/blob/main/README.md

#### Structure

Badges: None

Intro: single short sentence

1. Intro (short sentence)
  - image of codex running on mac
  - links to install in IDE or cloud-based version
2. Quickstart
3. License

#### Notes

Longer than Claude Code, this has a shoter Intro but has more under
"Quickstart" with links to other parts of the documentat, e.g. config docs, and
then finishes with a longer file-tree-like series of links for different
documentation.

### Gemini CLI (Google)

Stars: 87k

https://github.com/google-gemini/gemini-cli

#### Structure

Badges: 
  - github Testing CI, 
  - github Testing: E2E (Chained), note it is "Failing"
  - npm
  - license
  - wiki badge for "View Code Wiki", a google-hosted page

Image: Gemini writing "hello world" in python

Intro: Long single sentence, followed by link to documentation

1. Why Gemini CLI (bullet point list)
2. Installation
  - prereqs
  - Quick install
3. Release Cadence and Tags
4. Key Features
  - sub-headers with bullet points
5. Authentication Options
6. Getting Started (example initial workflows)
7. Documentation
  - headers and bulletpoints with links, feels like a table of contents
8. Contributing
9. Resources
10. Legal

#### Notes

- The titles and bullet points have emojis. Feels very AI-generated

## Model Provider TUIs (China-based)

### Qwen Code 

Stars: 16.5k

https://github.com/QwenLM/qwen-code/blob/main/README.md

#### Structure

Badges:
  - npm
  - license
  - node
  - downloads (npmjs)

Intro: Single long sentence, describes being adapted from Gemini CLI and
optimized for Qwen3-Coder models

1. Free Options Available
2. Installation
3. VS Code Extension
4. Quick Start (actually quite a long section)
5. Usage Examples
6. Popular Tasks
7. Commands & Shortcuts
8. Benchmark Results (only Terminal-Bench)
9. Development & Contributing (link to contrib doc)
10. Troubleshooting (link to guide)
11. Acknowledgements (calls out Gemini CLI)
12. License
13. Star History

#### Notes

Installation section pretty short and nicely formed, link to Node.js version,
one-liners for install.

Notably the "Quick Start" section is quite long. Walks you through controlling
token usage, simple commands to use, then more specialized stuff like vision
model config.
