# Child outcomes after R10 timeout

timestamp: 2026-06-30T14:33:00-07:00

## node-fe4a6decb46ca481

### node.json status
node_id: node-fe4a6decb46ca481
branch_id: branch-f41b072e4787d706
candidate_id: broad-harness-g1-01
generation: 1
status: succeeded
runner_result_path: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/runner-result.json
workspace_root: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-ec383aa38762a3d4
binary_path: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/bin/ploke-eval

### runner-result.json
{
    "schema_version": "prototype1-treatment-node.v1",
    "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
    "node_id": "node-fe4a6decb46ca481",
    "generation": 1,
    "branch_id": "branch-f41b072e4787d706",
    "status": "succeeded",
    "disposition": "succeeded",
    "treatment_campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627",
    "exit_code": 0,
    "recorded_at": "2026-06-30T21:23:50.184452925+00:00"
}

### channel child-to-parent
file: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/channels/5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7/child-to-parent.jsonl
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"e4733255-f31c-49df-a828-876c5517d80b","recorded_at":1782853476120,"body_hash":"40ec7f71ea684c8b976e79e8e425f87779e6de57f4821dcfc8066dbcad2defe0","body":"ready"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"688ccf9e-c406-4bad-8073-ba411bc5cdff","recorded_at":1782853476370,"body_hash":"845efe165271c4c3279dd04a12c8b1fa3bf3de131a53ed54ff15905a771c0498","body":"evaluating"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"e0807e06-23e2-4d53-93ce-a04f22dc6944","recorded_at":1782854631164,"body_hash":"ba72ce7ed91acdb5e9d2181ce720cc58ae625f21e7b191acb875793a67c98330","body":{"result":{"runner_result":{"schema_version":"prototype1-treatment-node.v1","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","generation":1,"branch_id":"branch-f41b072e4787d706","status":"succeeded","disposition":"succeeded","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627","exit_code":0,"recorded_at":"2026-06-30T21:23:50.184452925+00:00"},"treatment":{"baseline_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","branch_id":"branch-f41b072e4787d706","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627","treatment_campaign_manifest":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/campaign.json","treatment_closure_state_path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/closure-state.json","eval_policy":{"include_partial":false,"stop_on_error":false,"budget":{"max_turns":40,"max_tool_calls":200,"wall_clock_secs":1800},"batch_prefix":"ripgrep-burntsushi-ripgrep-2209"},"benchmark_family":"multi_swe_bench_rust","dataset_sources":[{"path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/slice.jsonl","label":"prototype1/ripgrep-burntsushi-ripgrep-2209","url":"https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl"}],"instances":[{"instance_id":"BurntSushi__ripgrep-2209","registration_path":"/home/brasides/.ploke-eval/registries/runs/run-1782853477752-structured-current-policy-1d368786.json","record_path":"/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/treatments/branch-f41b072e4787d706/instances/BurntSushi__ripgrep-2209/runs/run-1782853477752-structured-current-policy-1d368786/record.json.gz","metrics":{"tool_calls_total":47,"tool_calls_failed":3,"patch_attempted":true,"patch_apply_state":"applied","submission_artifact_state":"nonempty","patch_projection_check_state":"passed","partial_patch_failures":0,"same_file_patch_retry_count":0,"same_file_patch_max_streak":0,"aborted":false,"aborted_repair_loop":false,"nonempty_valid_patch":true,"convergence":true,"oracle_eligible":true},"status":"complete"}]}}}}

## node-2fe75acd9e9cf6c3

### node.json status
node_id: node-2fe75acd9e9cf6c3
branch_id: branch-88aaa7a4b0328baf
candidate_id: broad-harness-g1-02
generation: 1
status: succeeded
runner_result_path: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/runner-result.json
workspace_root: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-ec383aa38762a3d4-r2
binary_path: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/bin/ploke-eval

### runner-result.json
{
    "schema_version": "prototype1-treatment-node.v1",
    "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
    "node_id": "node-2fe75acd9e9cf6c3",
    "generation": 1,
    "branch_id": "branch-88aaa7a4b0328baf",
    "status": "succeeded",
    "disposition": "succeeded",
    "treatment_campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796",
    "exit_code": 0,
    "recorded_at": "2026-06-30T21:24:50.351751440+00:00"
}

### channel child-to-parent
file: /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/channels/5d1673be-8222-462e-82b4-0ff9aa9c9f8a/child-to-parent.jsonl
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"af5d5e6c-d664-4e61-a816-404fe64b223b","recorded_at":1782853476345,"body_hash":"40ec7f71ea684c8b976e79e8e425f87779e6de57f4821dcfc8066dbcad2defe0","body":"ready"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"cdc4947f-eee3-49cd-84bb-be5c4f11e1f6","recorded_at":1782853476617,"body_hash":"845efe165271c4c3279dd04a12c8b1fa3bf3de131a53ed54ff15905a771c0498","body":"evaluating"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"1538893b-e330-430f-b380-370adbbfb433","recorded_at":1782854691312,"body_hash":"bddfd14229da49222382b4058d4f178f008a34fea67b30ae652126f552f074a2","body":{"result":{"runner_result":{"schema_version":"prototype1-treatment-node.v1","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","generation":1,"branch_id":"branch-88aaa7a4b0328baf","status":"succeeded","disposition":"succeeded","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796","exit_code":0,"recorded_at":"2026-06-30T21:24:50.351751440+00:00"},"treatment":{"baseline_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","branch_id":"branch-88aaa7a4b0328baf","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796","treatment_campaign_manifest":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796/campaign.json","treatment_closure_state_path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796/closure-state.json","eval_policy":{"include_partial":false,"stop_on_error":false,"budget":{"max_turns":40,"max_tool_calls":200,"wall_clock_secs":1800},"batch_prefix":"ripgrep-burntsushi-ripgrep-2209"},"benchmark_family":"multi_swe_bench_rust","dataset_sources":[{"path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/slice.jsonl","label":"prototype1/ripgrep-burntsushi-ripgrep-2209","url":"https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl"}],"instances":[{"instance_id":"BurntSushi__ripgrep-2209","registration_path":"/home/brasides/.ploke-eval/registries/runs/run-1782853477663-structured-current-policy-fef994d3.json","record_path":"/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/treatments/branch-88aaa7a4b0328baf/instances/BurntSushi__ripgrep-2209/runs/run-1782853477663-structured-current-policy-fef994d3/record.json.gz","metrics":{"tool_calls_total":55,"tool_calls_failed":5,"patch_attempted":true,"patch_apply_state":"applied","submission_artifact_state":"nonempty","patch_projection_check_state":"passed","partial_patch_failures":0,"same_file_patch_retry_count":2,"same_file_patch_max_streak":3,"aborted":false,"aborted_repair_loop":false,"nonempty_valid_patch":true,"convergence":true,"oracle_eligible":true},"status":"complete"}]}}}}

## evaluations present
### /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/evaluations/branch-f41b072e4787d706.json
{
    "baseline_campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
    "branch_id": "branch-f41b072e4787d706",
    "treatment_campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627",
    "evaluation_procedure_id": "prototype1.branch_evaluation.operational_metrics.v1",
    "evaluator_identity": {
        "id": "prototype1.branch_evaluation.mechanized",
        "version": "v1"
    },
    "eval_set_identity": {
        "id": "prototype1.eval_set.closure_instance_slice.v1:99d3bfbf4b79d1c46a3d695fe350e1836de37ddd8be36d8e6cf680512a52ec08",
        "kind": "closure_instance_slice",
        "authority": "typed_closure_context:prototype1.eval_set.closure_instance_slice.v1:2c07ffd89e6bcd00831861a269eef7cbcade30583e5de4fe7d86964067ef0b97",
        "explicit": true,
        "benchmark_family": "multi_swe_bench_rust",
        "dataset_sources": [
            {
                "path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/slice.jsonl",
                "label": "prototype1/ripgrep-burntsushi-ripgrep-2209",
                "url": "https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl"
            }
        ],
        "eval_policy": {
            "include_partial": false,
            "stop_on_error": false,
            "budget": {
                "max_turns": 40,
                "max_tool_calls": 200,
                "wall_clock_secs": 1800
            },
            "batch_prefix": "ripgrep-burntsushi-ripgrep-2209"
        },
        "instance_ids": [
            "BurntSushi__ripgrep-2209"
        ],
        "note": "eval set identity is derived from typed closure/campaign context and the compared baseline closure slice"
    },
    "branch_registry_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/branches.json",
    "evaluation_artifact_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/evaluations/branch-f41b072e4787d706.json",
    "treatment_campaign_manifest": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/campaign.json",
    "treatment_closure_state_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/closure-state.json",
    "overall_disposition": "reject",
    "reasons": [
        "BurntSushi__ripgrep-2209: tool_calls_failed regressed: 0 -> 3"
    ],
    "compared_instances": [
        {
            "instance_id": "BurntSushi__ripgrep-2209",
            "baseline_registration_path": "/home/brasides/.ploke-eval/registries/runs/run-1782848953623-structured-current-policy-85169a85.json",
            "treatment_registration_path": "/home/brasides/.ploke-eval/registries/runs/run-1782853477752-structured-current-policy-1d368786.json",
            "baseline_record_path": "/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/BurntSushi__ripgrep-2209/runs/run-1782848953623-structured-current-policy-85169a85/record.json.gz",
            "treatment_record_path": "/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/treatments/branch-f41b072e4787d706/instances/BurntSushi__ripgrep-2209/runs/run-1782853477752-structured-current-policy-1d368786/record.json.gz",
            "baseline_metrics": {
                "tool_calls_total": 51,
                "tool_calls_failed": 0,
                "patch_attempted": true,
                "patch_apply_state": "applied",
                "submission_artifact_state": "nonempty",
                "patch_projection_check_state": "passed",
                "partial_patch_failures": 0,
                "same_file_patch_retry_count": 0,
                "same_file_patch_max_streak": 0,
                "aborted": false,
                "aborted_repair_loop": false,
                "nonempty_valid_patch": true,
                "convergence": true,
                "oracle_eligible": true
            },
            "treatment_metrics": {
                "tool_calls_total": 47,
                "tool_calls_failed": 3,
                "patch_attempted": true,
                "patch_apply_state": "applied",
                "submission_artifact_state": "nonempty",
                "patch_projection_check_state": "passed",
                "partial_patch_failures": 0,
                "same_file_patch_retry_count": 0,
                "same_file_patch_max_streak": 0,
                "aborted": false,
                "aborted_repair_loop": false,
                "nonempty_valid_patch": true,
                "convergence": true,
                "oracle_eligible": true
            },
            "evaluation": {
                "disposition": "reject",
                "reasons": [
                    "tool_calls_failed regressed: 0 -> 3"
                ]
            },
            "status": "compared"
        }
    ]
}
