# Command Map

Top-level command families are organized by operator intent.

| Command family | Use for | Typical safety |
| --- | --- | --- |
| `doctor` | Local setup diagnosis | `RO` |
| `run ...` | Repo fetch, manifest preparation, execution, replay | `RO`/`FS`/`NET` depending on subcommand |
| `just ...` | Shortcuts mirroring common `run ...` paths | same as underlying command |
| `model ...` | Model registry, provider preferences, parent patcher model | `RO`/`FS`/`NET` |
| `transcript` | Print assistant messages from a resolved run | `RO` |
| `conversations` | List run conversation turns | `RO` |
| `inspect ...` | Inspect run artifacts, failures, tool calls, snapshots | `RO` |
| `history ...` | Inspect History-shaped projections and metrics | `RO` |
| `campaign ...` | Campaign manifests, validation, campaign submission export | `RO`/`FS` |
| `closure ...` | Campaign progress across eval/protocol closure | `RO`/`FS`/`NET` |
| `registry ...` | Persisted target inventory | `RO`/`FS` |
| `select ...` | Active operator selection context | `RO`/`FS` |
| `mbe ...` | MBE oracle config/report support | `RO`/`FS` |
| `protocol ...` | Review/adjudicate protocol artifacts | `RO`/`FS` |
| `loop ...` | Prototype intervention and Prototype 1 loops | see [Prototype 1](../prototype1/index.md) |

Use `cargo run -p ploke-eval -- help run` for source-generated examples and option lists; replace `run` with another command family as needed.
