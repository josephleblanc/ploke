# Fork Recovery Prompting Note

- date: 2026-04-15
- task title: fork recovery prompting note
- task description: compact note on how to resume the `ploke-protocol` thread efficiently across earlier-node forks with minimal chat-state dependence
- related planning files: `docs/active/agents/2026-04-15_ploke-protocol-control-note.md`, `docs/active/agents/2026-04-15_ploke-protocol-state-composition-checkpoint.md`, `docs/active/CURRENT_FOCUS.md`

## What Worked Well

- bring back durable repo artifacts rather than a long prose recap
- name the specific checkpoint and control docs to read first
- correct drift quickly if chat-memory reconstruction diverges from repo state
- force concrete inspection or CLI comparison when useful, instead of staying abstract

## What Would Make It Even Better Next Time

- start with one message that names the authoritative docs in priority order
- state which document is the current authority and which are supporting context
- state the exact mode for the turn:
  - recover and summarize
  - inspect current code only
  - implement
  - compare workflows
  - write checkpoint only
- explicitly say whether chat memory should be ignored unless it matches the docs

## Good Recovery Prompt Shape

```text
Cold recover from repo artifacts only.

Authority:
1. <authoritative checkpoint>
2. <control note>
3. <supporting checkpoint>
4. <concept doc>

Task for this fork:
- read those only
- summarize current state in 5-8 lines
- propose next implementation step
- do not edit yet
- do not rely on earlier chat state unless it matches those docs
```

## Good Recovery Prompt For Immediate Implementation

```text
Recover from these docs only, then continue implementation:
1. ...
2. ...
3. ...

Current authority: ...

Task:
- inspect current code in <surface>
- implement the next slice
- update the control note/checkpoint before stopping
```

## Core Rule

Treat repo docs as the continuity layer and source of truth.

Use chat only as a convenience layer; if chat and repo artifacts disagree,
prefer the repo artifacts.
