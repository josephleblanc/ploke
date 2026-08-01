# Prototype 1 handoff loses predecessor executable epoch after rebuild

Status: source repaired; focused and historical gates pass; live revalidation
requires a fresh campaign.

## Broken Contract

A valid `R12 -> R13b` successor handoff must let the predecessor persist its
terminal receipt and clean release after the selected checkout and successor
binary are installed, while still proving that the receipt came from the same
running predecessor executable.

## Evidence

Campaign:
`p1-v23-strictkeephandoff-mbe-g35f-direct-3g1x3-p3-obs2400-20260717-191722`.

Command:

```text
ploke-eval loop walk step --until r13b --watch --allow git-changes
```

The generation-1 controller journal is:

```text
~/.ploke-eval/campaigns/<campaign>/prototype1/control/sessions/
363a4120f839b20fbaea40753b1bc2209a0875b6807aeca5145e37a22bd95665/
control-journal.jsonl
```

Fence 10 admitted the exact `R12` intent with `allow_git_changes=true` and
targets `R13a`, `R13b`, and `R13c`. Its terminal record is instead
`indeterminate`:

```text
controller epoch changed across an edge that does not admit checkout mutation
```

The epoch receipt changed from:

```text
HEAD 4a75835069cdb0ae9feed33ad204f856127a7879
exe  .../target/debug/ploke-eval
mtime 1784346271912
```

to:

```text
HEAD 1c6f466321cdc22c4e31457726d8540a40256f19
exe  .../target/debug/ploke-eval (deleted)
mtime null
```

The handoff authority itself remained strict and internally consistent:

- History block height 1 is
  `dde1c02dfd76da6b750eff3b3ba880d01cc3ed57e358ed8d30411ad925bd33cc`;
- its parent is
  `04fc12a5f6746959d31b15748982768c72c516ab9689ea3a4b9722675f96ff1e`;
- selected Artifact commit is
  `e34bafe18256cf8cc3824caa10ea5265b7d5ca0a`;
- the immutable surface root remained
  `ce6bda2406476bd7e9aaebb59d5488313869bf1e3ef9e40bf018b9ce89c5b51c`;
- successor session `dba81a0f-0965-4e23-a4f1-e616bc9d96fb`
  passed strict startup through `R4c` evidence
  `d718b015e06b066a26189f31aaa2ae7326e2071b8ec822f8432a90a10d8037d4`.

The successor then failed closed after 30 seconds because the predecessor had
not committed and released its accepted handoff fence. V23 is preserved and
must not be advanced or repaired in place.

## Source Trace

`build_prototype1_active_successor_binary` rebuilds
`<active-root>/target/debug/ploke-eval`. On Linux that replaces the pathname of
the still-running predecessor executable.

`ServerEpoch::capture` in
`crates/ploke-eval/src/cli/prototype1_state/walk/epoch.rs` called
`current_exe()` and then statted that returned pathname. Linux reports an
unlinked running executable with the exact ` (deleted)` suffix, and the suffixed
pathname cannot supply metadata.

`Lease<Pending>::finish` then passed that receipt to `settle_epoch`.
`admits_epoch_change` correctly required the executable identity to remain the
same, so the lossy capture forced the otherwise valid handoff result to
`indeterminate`. Because no committed `R13b` receipt and clean release existed,
`predecessor_release` correctly withheld successor mutation authority.

## Docs and Policy Expectation

The Prototype 1 History/Crown policy requires a selected successor to pass
sealed Artifact and surface verification before authority transfers. It also
requires the predecessor to commit the exact accepted handoff attempt and
release its fence before the successor can mutate. The fix must preserve both
checks; it must not reinterpret V23 or accept an arbitrary changed executable.

## Current Repro Coverage

`epoch_tracks_running_executable_after_path_replacement` runs a copied Rust test
binary, replaces its executable pathname while it is still alive, and then
calls production `ServerEpoch::capture`. Before the repair it reproduced the
V23 path exactly as `<path> (deleted)`.

Existing Stage 6, Stage 7, and V12 historical handoff fixtures replay successful
Ready/receipt/release ordering through production session, invocation, journal,
and server readers. They remain required guardrails for the authority transfer.

After the repair, the epoch module's four tests pass, including the nested
process replacement helper. The illegal checkout-edge epoch test and all three
historical handoff replays also pass.

## Fix Direction

On Linux, use `/proc/self/exe` metadata to retain the running inode's original
mtime, and normalize the kernel's exact deleted-dentry suffix only when procfs
proves that live inode. Keep `admits_epoch_change`, client compatibility,
History, surface, Ready, and predecessor-release validation unchanged.

Do not:

- strip arbitrary executable suffixes without live-inode evidence;
- ignore executable mtimes;
- admit or rewrite V23's indeterminate receipt;
- manually respawn its dead successor or mutate its sealed artifacts.

## Missing Validation

A fresh strict campaign must replace the active binary during a later
generation, commit and release `R12 -> R13b`, transfer the walk endpoint, and
continue under the successor.
