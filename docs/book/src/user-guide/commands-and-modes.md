# Commands and Modes

This page lists the stable command families documented in the root README. Use `/help` in the TUI for the most current command surface.

## Model commands

```text
/model search <query>
/model list
/model info
/model use <name>
/model refresh
```

## Embedding commands

```text
/embedding search <query>
/index start [path]
```

## Indexing commands

```text
/index start [path]
/index pause
/index resume
/index cancel
```

## Persistence commands

```text
/save db
/load <crate-or-workspace-name>
```

## Modes

Ploke's TUI is designed around keyboard-driven discovery: model picker overlays, embedding picker overlays, code search, chat, and tool output all live in the same terminal application.
