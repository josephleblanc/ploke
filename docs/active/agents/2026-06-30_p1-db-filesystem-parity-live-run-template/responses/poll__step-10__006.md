# Poll step-10 006

timestamp: 2026-06-30T14:26:44-07:00

## runner processes

## client log tail
BEGIN 2026-06-30T14:01:46-07:00
{
  "type": "error",
  "code": "request_failed",
  "detail": "database setup failed during 'prototype1_state_complete': Transition(TimedOutWaitingForResult { node_id: \"node-2fe75acd9e9cf6c3\", runtime_id: RuntimeId(5d1673be-8222-462e-82b4-0ff9aa9c9f8a), waited_ms: 1200053, runner_result_path: \"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/results/5d1673be-8222-462e-82b4-0ff9aa9c9f8a.json\" })",
  "phase": "r10",
  "epoch": {
    "protocol_version": 3,
    "transition_graph_version": "walk-r0-r14a-v1",
    "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
    "exe_path": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval",
    "exe_modified_unix_ms": 1782841491704,
    "git_head": "f906d59a8b1c0ee455ca9cf22a82363c0f8d3c23",
    "source_status_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  }
}
batch selection is invalid: walk request failed at Some(R10)
END 2026-06-30T14:24:50-07:00 exit=1

## walk show short
walk show timed out/failed

## core counts
{
  "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
  "db_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite",
  "headers": [
    "relation",
    "count"
  ],
  "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
  "row_count": 28,
  "rows": [
    {
      "count": 3,
      "relation": "eval_agent_turn"
    },
    {
      "count": 2,
      "relation": "eval_artifact"
    },
    {
      "count": 4,
      "relation": "eval_binary_ref"
    },
    {
      "count": 4,
      "relation": "eval_build_event"
    },
    {
      "count": 1,
      "relation": "eval_campaign"
    },
    {
      "count": 6,
      "relation": "eval_channel_message"
    },
    {
      "count": 1,
      "relation": "eval_child_plan"
    },
    {
      "count": 2,
      "relation": "eval_child_plan_child"
    },
    {
      "count": 0,
      "relation": "eval_continuation_decision"
    },
    {
      "count": 1,
      "relation": "eval_evaluation"
    },
    {
      "count": 3,
      "relation": "eval_harness_diagnostic"
    },
    {
      "count": 3,
      "relation": "eval_harness_request"
    },
    {
      "count": 0,
      "relation": "eval_harness_submission"
    },
    {
      "count": 2,
      "relation": "eval_invocation"
    },
    {
      "count": 0,
      "relation": "eval_model_exchange"
    },
    {
      "count": 1,
      "relation": "eval_parent_identity"
    },
    {
      "count": 1,
      "relation": "eval_parent_start"
    },
    {
      "count": 1,
      "relation": "eval_profile_commitment"
    },
    {
      "count": 23,
      "relation": "eval_record_ref"
    },
    {
      "count": 1,
      "relation": "eval_run_profile_policy"
    },
    {
      "count": 3,
      "relation": "eval_runner_request"
    },
    {
      "count": 4,
      "relation": "eval_runner_result"
    },
    {
      "count": 3,
      "relation": "eval_scheduler_node"
    },
    {
      "count": 12,
      "relation": "eval_scheduler_node_status_event"
    },
    {
      "count": 0,
      "relation": "eval_selection_decision"
    },
    {
      "count": 133,
      "relation": "eval_tool_event"
    },
    {
      "count": 1,
      "relation": "eval_transition_event"
    },
    {
      "count": 10,
      "relation": "eval_walk_event"
    }
  ],
  "script": "campaign[count(campaign_id)] := *eval_campaign{campaign_id}\nprofile[count(profile_ref_id)] := *eval_profile_commitment{profile_ref_id}\npolicy[count(campaign_id)] := *eval_run_profile_policy{campaign_id}\nscheduler[count(node_id)] := *eval_scheduler_node{node_id}\nscheduler_events[count(status_event_id)] := *eval_scheduler_node_status_event{status_event_id}\nrunner_requests[count(node_id)] := *eval_runner_request{node_id}\nparent_identity[count(parent_id)] := *eval_parent_identity{parent_id}\nparent_start[count(start_event_id)] := *eval_parent_start{start_event_id}\ntransition_events[count(event_id)] := *eval_transition_event{event_id}\nrecord_refs[count(record_ref_id)] := *eval_record_ref{record_ref_id}\nharness_requests[count(request_id)] := *eval_harness_request{request_id}\nharness_diagnostics[count(request_id)] := *eval_harness_diagnostic{request_id}\nharness_submissions[count(request_id)] := *eval_harness_submission{request_id}\nagent_turns[count(turn_id)] := *eval_agent_turn{turn_id}\nmodel_exchanges[count(exchange_id)] := *eval_model_exchange{exchange_id}\ntool_events[count(tool_event_id)] := *eval_tool_event{tool_event_id}\nchild_plans[count(plan_id)] := *eval_child_plan{plan_id}\nchild_plan_children[count(child_node_id)] := *eval_child_plan_child{child_node_id}\nartifacts[count(artifact_id)] := *eval_artifact{artifact_id}\nbinaries[count(binary_ref_id)] := *eval_binary_ref{binary_ref_id}\nbuild_events[count(build_id)] := *eval_build_event{build_id}\ninvocations[count(invocation_id)] := *eval_invocation{invocation_id}\nchannel_messages[count(channel_message_id)] := *eval_channel_message{channel_message_id}\nrunner_results[count(result_path)] := *eval_runner_result{result_path}\nevaluations[count(evaluation_id)] := *eval_evaluation{evaluation_id}\nselection_decisions[count(decision_id)] := *eval_selection_decision{decision_id}\ncontinuations[count(decision_id)] := *eval_continuation_decision{decision_id}\nwalk_events[count(event_id)] := *eval_walk_event{event_id}\n\n?[relation, count] := campaign[count], relation = \"eval_campaign\"\n?[relation, count] := profile[count], relation = \"eval_profile_commitment\"\n?[relation, count] := policy[count], relation = \"eval_run_profile_policy\"\n?[relation, count] := scheduler[count], relation = \"eval_scheduler_node\"\n?[relation, count] := scheduler_events[count], relation = \"eval_scheduler_node_status_event\"\n?[relation, count] := runner_requests[count], relation = \"eval_runner_request\"\n?[relation, count] := parent_identity[count], relation = \"eval_parent_identity\"\n?[relation, count] := parent_start[count], relation = \"eval_parent_start\"\n?[relation, count] := transition_events[count], relation = \"eval_transition_event\"\n?[relation, count] := record_refs[count], relation = \"eval_record_ref\"\n?[relation, count] := harness_requests[count], relation = \"eval_harness_request\"\n?[relation, count] := harness_diagnostics[count], relation = \"eval_harness_diagnostic\"\n?[relation, count] := harness_submissions[count], relation = \"eval_harness_submission\"\n?[relation, count] := agent_turns[count], relation = \"eval_agent_turn\"\n?[relation, count] := model_exchanges[count], relation = \"eval_model_exchange\"\n?[relation, count] := tool_events[count], relation = \"eval_tool_event\"\n?[relation, count] := child_plans[count], relation = \"eval_child_plan\"\n?[relation, count] := child_plan_children[count], relation = \"eval_child_plan_child\"\n?[relation, count] := artifacts[count], relation = \"eval_artifact\"\n?[relation, count] := binaries[count], relation = \"eval_binary_ref\"\n?[relation, count] := build_events[count], relation = \"eval_build_event\"\n?[relation, count] := invocations[count], relation = \"eval_invocation\"\n?[relation, count] := channel_messages[count], relation = \"eval_channel_message\"\n?[relation, count] := runner_results[count], relation = \"eval_runner_result\"\n?[relation, count] := evaluations[count], relation = \"eval_evaluation\"\n?[relation, count] := selection_decisions[count], relation = \"eval_selection_decision\"\n?[relation, count] := continuations[count], relation = \"eval_continuation_decision\"\n?[relation, count] := walk_events[count], relation = \"eval_walk_event\"",
  "type": "walk_db_query"
}

## actual node result/eval files
2026-06-30 14:23:50.1835234290 462 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/results/5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7.json
2026-06-30 14:23:50.5675261310 462 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/runner-result.json
2026-06-30 14:24:50.3509466140 462 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/results/5d1673be-8222-462e-82b4-0ff9aa9c9f8a.json
2026-06-30 14:24:50.7229492280 462 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/runner-result.json
2026-06-30 14:23:51.7875347180 4548 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/evaluations/branch-f41b072e4787d706.json

## stream tail summaries
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/streams/5d1673be-8222-462e-82b4-0ff9aa9c9f8a/stdout.log
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:87d0fa30..ec4fb82d)
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:a40851d5..7b57d83d)
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:e70bbdd4..fed3c07b)
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:99ccebc3..793e6965)
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:69ee7b47..8b3dbb3a)
[2026-06-30T14:11:46.485-07:00 elapsed_ms=430150] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:a40edbe5..c1590780)
[2026-06-30T14:12:04.064-07:00 elapsed_ms=447729] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:398: Sending shutdown signal to CallbackManager.
[2026-06-30T14:12:04.064-07:00 elapsed_ms=447729] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:422: Sending shutdown signal to CallbackManager.
[2026-06-30T14:12:04.064-07:00 elapsed_ms=447729] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:455: CallbackManager not closed?
[2026-06-30T14:12:21.580-07:00 elapsed_ms=465245] WARN  chat-loop: crates/ploke-tui/src/llm/manager/mod.rs:534: LLM request ended with error [3merror_id[0m[2m=[0m734abf7f-7f8f-4eb6-afa4-5c086eafdf65 [3mcode[0m[2m=[0mTOOL_EXECUTION_FAILED [3mkind[0m[2m=[0mToolExecution
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/streams/5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7/stdout.log
[2026-06-30T14:11:39.239-07:00 elapsed_ms=423127] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:d7aed0fd..c782fe0f)
[2026-06-30T14:11:39.239-07:00 elapsed_ms=423127] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:13b3a528..7be8cc97)
[2026-06-30T14:11:39.239-07:00 elapsed_ms=423127] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:86ab2542..7447051c)
[2026-06-30T14:11:39.239-07:00 elapsed_ms=423127] WARN  ploke_tui::app_state::database: crates/ploke-tui/src/app_state/database.rs:1741: Non-unique node id: AnyNodeId::Import(S:fb1c38f0..6dcaca9a)
[2026-06-30T14:11:53.772-07:00 elapsed_ms=437660] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:398: Sending shutdown signal to CallbackManager.
[2026-06-30T14:11:53.772-07:00 elapsed_ms=437660] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:422: Sending shutdown signal to CallbackManager.
[2026-06-30T14:11:53.772-07:00 elapsed_ms=437660] WARN  ploke_embed::indexer: crates/ingest/ploke-embed/src/indexer/mod.rs:455: CallbackManager not closed?
[2026-06-30T14:11:58.334-07:00 elapsed_ms=442222] WARN  chat_http: crates/ploke-llm/src/manager/session.rs:819: [3mevent[0m[2m=[0m"chat_http_response_error_status" [3mrequest_id[0m[2m=[0m47 [3mattempt[0m[2m=[0m1 [3mmax_attempts[0m[2m=[0m6 [3murl[0m[2m=[0m"https://aiplatform.googleapis.com/v1/projects/cs-poc-gtxw7jmtfuwfsiauziui9yx/locations/us-central1/endpoints/openapi/chat/completions" [3mstatus[0m[2m=[0m429 [3melapsed_ms[0m[2m=[0m959
[2026-06-30T14:11:58.334-07:00 elapsed_ms=442222] WARN  chat_http: crates/ploke-llm/src/manager/session.rs:853: [3mevent[0m[2m=[0m"chat_http_retry_scheduled" [3mrequest_id[0m[2m=[0m47 [3mattempt[0m[2m=[0m1 [3mmax_attempts[0m[2m=[0m6 [3mphase[0m[2m=[0m"status" [3murl[0m[2m=[0m"https://aiplatform.googleapis.com/v1/projects/cs-poc-gtxw7jmtfuwfsiauziui9yx/locations/us-central1/endpoints/openapi/chat/completions" [3mstatus[0m[2m=[0m429 [3mbackoff_ms[0m[2m=[0m210 [3melapsed_ms[0m[2m=[0m959
[2026-06-30T14:12:10.623-07:00 elapsed_ms=454511] WARN  chat-loop: crates/ploke-tui/src/llm/manager/mod.rs:534: LLM request ended with error [3merror_id[0m[2m=[0m4d468cb6-1e22-43d6-9237-eac06c9d77e5 [3mcode[0m[2m=[0mTOOL_EXECUTION_FAILED [3mkind[0m[2m=[0mToolExecution
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/streams/5d1673be-8222-462e-82b4-0ff9aa9c9f8a/stderr.log
thrash."
}
protocol progress: retrying tool_call_review[35] attempt 2/3 after malformed adjudication JSON: second procedure failed: branch procedure failed: right branch failed: failed to parse json response: expected `,` or `}` at line 5 column 1; content was: {
  "verdict": "no_recovery_needed",
  "confidence": "high",
  "rationale": "The focal call is a successful code edit followed by cargo test which passed, and then a search for more code context. The workflow is proceeding smoothly with no failed actions or errors in the scope."
this scope."
neighborhood."
}
21:24:50 loop.prototype1_branch.evaluate.branch-88aaa7a4b0328baf.end +1213.555s
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/streams/5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7/stderr.log
  "rationale": "The focal call is a successful read_file operation of a source file located via previous search terms. The subsequent calls in the scope continue to read consecutive sections of the same file (lines 501-700 and 701-900) to understand the code context, showing a clear, systematic, and successful investigation with no recovery needed."
  needed."
}
protocol progress: retrying tool_call_segment_review[2] attempt 2/3 after malformed adjudication JSON: second procedure failed: branch procedure failed: left branch failed: right branch failed: failed to parse json response: expected `,` or `}` at line 5 column 1; content was: {
  "verdict": "distinct",
  "confidence": "high",
  "rationale": "The selected scope represents a distinct edit attempt containing a code lookup, a code insertion, a failed edit, and its successful correction. There are no redundant actions or search thrashing in this segment."
this segment; it directly progresses the editing process."
}
21:23:50 loop.prototype1_branch.evaluate.branch-f41b072e4787d706.end +1153.559s

## channel tails
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/channels/5d1673be-8222-462e-82b4-0ff9aa9c9f8a/child-to-parent.jsonl
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"af5d5e6c-d664-4e61-a816-404fe64b223b","recorded_at":1782853476345,"body_hash":"40ec7f71ea684c8b976e79e8e425f87779e6de57f4821dcfc8066dbcad2defe0","body":"ready"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"cdc4947f-eee3-49cd-84bb-be5c4f11e1f6","recorded_at":1782853476617,"body_hash":"845efe165271c4c3279dd04a12c8b1fa3bf3de131a53ed54ff15905a771c0498","body":"evaluating"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","runtime_id":"5d1673be-8222-462e-82b4-0ff9aa9c9f8a","message_id":"1538893b-e330-430f-b380-370adbbfb433","recorded_at":1782854691312,"body_hash":"bddfd14229da49222382b4058d4f178f008a34fea67b30ae652126f552f074a2","body":{"result":{"runner_result":{"schema_version":"prototype1-treatment-node.v1","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-2fe75acd9e9cf6c3","generation":1,"branch_id":"branch-88aaa7a4b0328baf","status":"succeeded","disposition":"succeeded","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796","exit_code":0,"recorded_at":"2026-06-30T21:24:50.351751440+00:00"},"treatment":{"baseline_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","branch_id":"branch-88aaa7a4b0328baf","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796","treatment_campaign_manifest":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796/campaign.json","treatment_closure_state_path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-88aaa7a4b0328baf-1782853476796/closure-state.json","eval_policy":{"include_partial":false,"stop_on_error":false,"budget":{"max_turns":40,"max_tool_calls":200,"wall_clock_secs":1800},"batch_prefix":"ripgrep-burntsushi-ripgrep-2209"},"benchmark_family":"multi_swe_bench_rust","dataset_sources":[{"path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/slice.jsonl","label":"prototype1/ripgrep-burntsushi-ripgrep-2209","url":"https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl"}],"instances":[{"instance_id":"BurntSushi__ripgrep-2209","registration_path":"/home/brasides/.ploke-eval/registries/runs/run-1782853477663-structured-current-policy-fef994d3.json","record_path":"/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/treatments/branch-88aaa7a4b0328baf/instances/BurntSushi__ripgrep-2209/runs/run-1782853477663-structured-current-policy-fef994d3/record.json.gz","metrics":{"tool_calls_total":55,"tool_calls_failed":5,"patch_attempted":true,"patch_apply_state":"applied","submission_artifact_state":"nonempty","patch_projection_check_state":"passed","partial_patch_failures":0,"same_file_patch_retry_count":2,"same_file_patch_max_streak":3,"aborted":false,"aborted_repair_loop":false,"nonempty_valid_patch":true,"convergence":true,"oracle_eligible":true},"status":"complete"}]}}}}
--- /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/channels/5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7/child-to-parent.jsonl
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"e4733255-f31c-49df-a828-876c5517d80b","recorded_at":1782853476120,"body_hash":"40ec7f71ea684c8b976e79e8e425f87779e6de57f4821dcfc8066dbcad2defe0","body":"ready"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"688ccf9e-c406-4bad-8073-ba411bc5cdff","recorded_at":1782853476370,"body_hash":"845efe165271c4c3279dd04a12c8b1fa3bf3de131a53ed54ff15905a771c0498","body":"evaluating"}
{"schema_version":"prototype1-runtime-channel.v1","direction":"child_to_parent","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","runtime_id":"5e0b11a9-e7b4-4a28-b218-a9c9cd869ca7","message_id":"e0807e06-23e2-4d53-93ce-a04f22dc6944","recorded_at":1782854631164,"body_hash":"ba72ce7ed91acdb5e9d2181ce720cc58ae625f21e7b191acb875793a67c98330","body":{"result":{"runner_result":{"schema_version":"prototype1-treatment-node.v1","campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","node_id":"node-fe4a6decb46ca481","generation":1,"branch_id":"branch-f41b072e4787d706","status":"succeeded","disposition":"succeeded","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627","exit_code":0,"recorded_at":"2026-06-30T21:23:50.184452925+00:00"},"treatment":{"baseline_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316","branch_id":"branch-f41b072e4787d706","treatment_campaign_id":"p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627","treatment_campaign_manifest":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/campaign.json","treatment_closure_state_path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316-treatment-branch-f41b072e4787d706-1782853476627/closure-state.json","eval_policy":{"include_partial":false,"stop_on_error":false,"budget":{"max_turns":40,"max_tool_calls":200,"wall_clock_secs":1800},"batch_prefix":"ripgrep-burntsushi-ripgrep-2209"},"benchmark_family":"multi_swe_bench_rust","dataset_sources":[{"path":"/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/slice.jsonl","label":"prototype1/ripgrep-burntsushi-ripgrep-2209","url":"https://huggingface.co/datasets/ByteDance-Seed/Multi-SWE-bench/resolve/main/rust/BurntSushi__ripgrep_dataset.jsonl"}],"instances":[{"instance_id":"BurntSushi__ripgrep-2209","registration_path":"/home/brasides/.ploke-eval/registries/runs/run-1782853477752-structured-current-policy-1d368786.json","record_path":"/home/brasides/.ploke-eval/instances/prototype1/p1-gated-parent-3g1x3-p3-20260630-174316/treatments/branch-f41b072e4787d706/instances/BurntSushi__ripgrep-2209/runs/run-1782853477752-structured-current-policy-1d368786/record.json.gz","metrics":{"tool_calls_total":47,"tool_calls_failed":3,"patch_attempted":true,"patch_apply_state":"applied","submission_artifact_state":"nonempty","patch_projection_check_state":"passed","partial_patch_failures":0,"same_file_patch_retry_count":0,"same_file_patch_max_streak":0,"aborted":false,"aborted_repair_loop":false,"nonempty_valid_patch":true,"convergence":true,"oracle_eligible":true},"status":"complete"}]}}}}
