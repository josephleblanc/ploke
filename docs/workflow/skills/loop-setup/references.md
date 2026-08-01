
**What Closure Does**

`closure status` is not recomputing anything. It loads `~/.ploke-eval/campaigns/<campaign>/closure-state.json` and formats it, per [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:5814).

`closure advance all` does real work: eval advancement first, then protocol advancement, then it renders a report, per [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:6281). The table renderer at [cli.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli.rs:8003) only prints:

```text
campaign <id>
eval: selected <n> instance(s), missing now <n>
protocol: selected <n> run(s), missing now <n>
```

So for validation, table output is not enough. Use `--format json` or inspect the written `closure-state.json`.

