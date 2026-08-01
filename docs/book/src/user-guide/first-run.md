# First Run

## Start the TUI

```bash
cargo run -p ploke-tui
```

or, after installation:

```bash
ploke
```

Run Ploke from the Rust crate or workspace you want to index. The current working directory is the default indexing target.

## Basic interaction model

The TUI uses a modal interaction style:

- Insert mode for typing chat text.
- Normal mode for navigation.
- Command mode for slash-style commands.

Use `/help` inside the app for the current command reference.

## Minimum useful session

```text
/model search kimi
/embedding search code
/index start
```

Then send a normal message. Ploke will use the indexed code graph and retrieval tools to assemble context for the LLM.
