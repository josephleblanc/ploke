native interactive window allocation benchmarking was run for Run Records and LLM Calls phase-sequence scenarios.

Command:

```sh
cargo run -p ploke-egui --features "dev native-benchmark" -- --run-root /home/brasides/.ploke-eval/campaigns/p1-five-gen-1x3-20260516-1/prototype1 --benchmark-suite standard --benchmark-output crates/ploke-egui/docs/profiling/benchmarks/20260518-llm-calls-phase-heap-deltas --benchmark-scenario inspector_run_records_phase_sequence_30 --benchmark-scenario inspector_run_records_phase_sequence_alternate_30 --benchmark-scenario inspector_llm_calls_phase_sequence_30 --benchmark-scenario inspector_llm_calls_phase_sequence_alternate_30
```

Generated files:

- `report.json`: native benchmark report.
- `all-span-phase-percentages.tsv`: every scenario phase, every reported span/group, phase totals, allocation/object/wrapped/live percentages.
- `collapsed-to-expanded-delta-percentages.tsv`: `idle_expanded - idle_selected_collapsed` attribution by span/group.
- `root-accounting-check.tsv`: per-phase root and total/summed-group accounting.

Root accounting check:

- Every row in `root-accounting-check.tsv` has `alloc_gap = 0` and `object_byte_gap = 0`.
- Root is therefore not being counted as a parent subtotal in these generated percentages.
- In this allocator report, `root` is a mutually exclusive group: allocations recorded while no registered allocation group was active.

Collapsed-to-expanded delta highlights:

| scenario | target | total delta allocs | total delta object bytes | largest byte groups |
| --- | --- | ---: | ---: | --- |
| `inspector_run_records_phase_sequence_30` | A1 | 7,078 | 5,399,189 | `root` 5,217,359 bytes, 96.63%; `inspector_run_records_widget_row` 144,000 bytes, 2.67% |
| `inspector_run_records_phase_sequence_alternate_30` | A2 | 7,111 | 5,415,582 | `root` 5,235,762 bytes, 96.68%; `inspector_run_records_widget_row` 144,000 bytes, 2.66% |
| `inspector_llm_calls_phase_sequence_30` | A1 | 26,880 | 6,820,995 | `root` 4,935,705 bytes, 72.36%; `inspector_run_record_tool_step` 1,715,760 bytes, 25.15%; `inspector_run_record_arm` 149,430 bytes, 2.19% |
| `inspector_llm_calls_phase_sequence_alternate_30` | A2 | 39,960 | 8,589,045 | `root` 5,626,335 bytes, 65.51%; `inspector_run_record_tool_step` 2,787,660 bytes, 32.46%; `inspector_run_record_arm` 149,430 bytes, 1.74% |

The `LLM calls` benchmark section is now forced open by the benchmark open-state path. The parent span `inspector_parent_create_llm_calls` is registered, but these phase windows show the rendered body cost under nested run-record arm/tool-step groups plus root, not as direct allocation under the parent span.
