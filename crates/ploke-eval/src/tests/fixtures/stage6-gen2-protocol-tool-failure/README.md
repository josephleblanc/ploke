# Stage 6 generation-2 protocol tool-failure fixture

`record.json.gz` is the exact persisted production run record from Prototype 1
campaign `p1-stage6-observe-g35f-orembed-3g1x3-p3-20260714-224040`, generation
2, branch `branch-2b06d0c2cb572280`, node `node-a69e1beed2b96e80`.

Original path:

`~/.ploke-eval/instances/prototype1/p1-stage6-observe-g35f-orembed-3g1x3-p3-20260714-224040/treatments/branch-2b06d0c2cb572280/instances/BurntSushi__ripgrep-2209/runs/run-1784100162780-structured-current-policy-28645c00/record.json.gz`

SHA-256:

`4a50000439fe955ab6c6f902bccb0e837762617d12007d483ccd29cd48d3ba2b`

The historical incident is carried by these completed cargo invocations:

- call index 21, `function-call-b98a0f0b-e62b-4e9c-b2a9-f7767c4ef024`:
  `ok=false`, `tests_failed_or_runtime`, exit 101;
- call index 25, `function-call-7de6f9a0-1768-41e5-a7e1-3bc65ee743e8`:
  `ok=false`, `tests_failed_or_runtime`, exit 101;
- call index 27, `function-call-b776949e-db76-4d45-a405-cc4551402146`:
  `ok=false`, `compile_failed`, exit 101.

The tool invocations completed normally at the lifecycle layer, so the record
correctly stores `ToolResult::Completed`. The original protocol projection
treated that lifecycle variant as semantic success, emitted `failed=false`, and
reported zero failed calls in the index-25 neighborhood even though calls 25
and 27 both carried typed semantic failures.

The regression reads this compressed record through the production record
reader, projects call 25 through the production protocol-neighborhood adapter,
and executes the production mechanized contextualization step. It does not
replace the incident with a synthetic tool-result projection.
