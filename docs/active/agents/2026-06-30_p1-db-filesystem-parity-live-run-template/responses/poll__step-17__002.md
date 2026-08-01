# Poll step-17 002

timestamp: 2026-06-30T14:50:57-07:00

## process family
    PID    PPID ELAPSED STAT CMD

## client log tail
BEGIN 2026-06-30T14:42:39-07:00
{
  "type": "error",
  "code": "request_failed",
  "detail": "database setup failed during 'eval_child_plan_put': eval-store validation failed for eval_child_plan.message_sha256: child plan '9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04' already exists with message hash be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499, attempted 361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f",
  "phase": "r7",
  "epoch": {
    "protocol_version": 3,
    "transition_graph_version": "walk-r0-r14a-v1",
    "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
    "exe_path": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval",
    "exe_modified_unix_ms": 1782855544799,
    "git_head": "e484103aa28af3b9e75254aa10a5bd17d588ad8e",
    "source_status_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  }
}
batch selection is invalid: walk request failed at Some(R7)
END 2026-06-30T14:48:51-07:00 exit=1

## walk show short
{
  "type": "error",
  "code": "request_failed",
  "detail": "database setup failed during 'eval_child_plan_put': eval-store validation failed for eval_child_plan.message_sha256: child plan '9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04' already exists with message hash be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499, attempted 361a41e203097d709cafb1cc7bb0088e2f6448a1278b3811f7ebd8e3dec0941f",
  "phase": "empty",
  "epoch": {
    "protocol_version": 3,
    "transition_graph_version": "walk-r0-r14a-v1",
    "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
    "exe_path": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval",
    "exe_modified_unix_ms": 1782855544799,
    "git_head": "e484103aa28af3b9e75254aa10a5bd17d588ad8e",
    "source_status_hash": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
  }
}
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
      "count": 9,
      "relation": "eval_agent_turn"
    },
    {
      "count": 3,
      "relation": "eval_artifact"
    },
    {
      "count": 6,
      "relation": "eval_binary_ref"
    },
    {
      "count": 6,
      "relation": "eval_build_event"
    },
    {
      "count": 1,
      "relation": "eval_campaign"
    },
    {
      "count": 9,
      "relation": "eval_channel_message"
    },
    {
      "count": 2,
      "relation": "eval_child_plan"
    },
    {
      "count": 3,
      "relation": "eval_child_plan_child"
    },
    {
      "count": 1,
      "relation": "eval_continuation_decision"
    },
    {
      "count": 2,
      "relation": "eval_evaluation"
    },
    {
      "count": 9,
      "relation": "eval_harness_diagnostic"
    },
    {
      "count": 6,
      "relation": "eval_harness_request"
    },
    {
      "count": 0,
      "relation": "eval_harness_submission"
    },
    {
      "count": 4,
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
      "count": 36,
      "relation": "eval_record_ref"
    },
    {
      "count": 1,
      "relation": "eval_run_profile_policy"
    },
    {
      "count": 7,
      "relation": "eval_runner_request"
    },
    {
      "count": 4,
      "relation": "eval_runner_result"
    },
    {
      "count": 7,
      "relation": "eval_scheduler_node"
    },
    {
      "count": 20,
      "relation": "eval_scheduler_node_status_event"
    },
    {
      "count": 1,
      "relation": "eval_selection_decision"
    },
    {
      "count": 261,
      "relation": "eval_tool_event"
    },
    {
      "count": 1,
      "relation": "eval_transition_event"
    },
    {
      "count": 13,
      "relation": "eval_walk_event"
    }
  ],
  "script": "campaign[count(campaign_id)] := *eval_campaign{campaign_id}\nprofile[count(profile_ref_id)] := *eval_profile_commitment{profile_ref_id}\npolicy[count(campaign_id)] := *eval_run_profile_policy{campaign_id}\nscheduler[count(node_id)] := *eval_scheduler_node{node_id}\nscheduler_events[count(status_event_id)] := *eval_scheduler_node_status_event{status_event_id}\nrunner_requests[count(node_id)] := *eval_runner_request{node_id}\nparent_identity[count(parent_id)] := *eval_parent_identity{parent_id}\nparent_start[count(start_event_id)] := *eval_parent_start{start_event_id}\ntransition_events[count(event_id)] := *eval_transition_event{event_id}\nrecord_refs[count(record_ref_id)] := *eval_record_ref{record_ref_id}\nharness_requests[count(request_id)] := *eval_harness_request{request_id}\nharness_diagnostics[count(request_id)] := *eval_harness_diagnostic{request_id}\nharness_submissions[count(request_id)] := *eval_harness_submission{request_id}\nagent_turns[count(turn_id)] := *eval_agent_turn{turn_id}\nmodel_exchanges[count(exchange_id)] := *eval_model_exchange{exchange_id}\ntool_events[count(tool_event_id)] := *eval_tool_event{tool_event_id}\nchild_plans[count(plan_id)] := *eval_child_plan{plan_id}\nchild_plan_children[count(child_node_id)] := *eval_child_plan_child{child_node_id}\nartifacts[count(artifact_id)] := *eval_artifact{artifact_id}\nbinaries[count(binary_ref_id)] := *eval_binary_ref{binary_ref_id}\nbuild_events[count(build_id)] := *eval_build_event{build_id}\ninvocations[count(invocation_id)] := *eval_invocation{invocation_id}\nchannel_messages[count(channel_message_id)] := *eval_channel_message{channel_message_id}\nrunner_results[count(result_path)] := *eval_runner_result{result_path}\nevaluations[count(evaluation_id)] := *eval_evaluation{evaluation_id}\nselection_decisions[count(decision_id)] := *eval_selection_decision{decision_id}\ncontinuations[count(decision_id)] := *eval_continuation_decision{decision_id}\nwalk_events[count(event_id)] := *eval_walk_event{event_id}\n\n?[relation, count] := campaign[count], relation = \"eval_campaign\"\n?[relation, count] := profile[count], relation = \"eval_profile_commitment\"\n?[relation, count] := policy[count], relation = \"eval_run_profile_policy\"\n?[relation, count] := scheduler[count], relation = \"eval_scheduler_node\"\n?[relation, count] := scheduler_events[count], relation = \"eval_scheduler_node_status_event\"\n?[relation, count] := runner_requests[count], relation = \"eval_runner_request\"\n?[relation, count] := parent_identity[count], relation = \"eval_parent_identity\"\n?[relation, count] := parent_start[count], relation = \"eval_parent_start\"\n?[relation, count] := transition_events[count], relation = \"eval_transition_event\"\n?[relation, count] := record_refs[count], relation = \"eval_record_ref\"\n?[relation, count] := harness_requests[count], relation = \"eval_harness_request\"\n?[relation, count] := harness_diagnostics[count], relation = \"eval_harness_diagnostic\"\n?[relation, count] := harness_submissions[count], relation = \"eval_harness_submission\"\n?[relation, count] := agent_turns[count], relation = \"eval_agent_turn\"\n?[relation, count] := model_exchanges[count], relation = \"eval_model_exchange\"\n?[relation, count] := tool_events[count], relation = \"eval_tool_event\"\n?[relation, count] := child_plans[count], relation = \"eval_child_plan\"\n?[relation, count] := child_plan_children[count], relation = \"eval_child_plan_child\"\n?[relation, count] := artifacts[count], relation = \"eval_artifact\"\n?[relation, count] := binaries[count], relation = \"eval_binary_ref\"\n?[relation, count] := build_events[count], relation = \"eval_build_event\"\n?[relation, count] := invocations[count], relation = \"eval_invocation\"\n?[relation, count] := channel_messages[count], relation = \"eval_channel_message\"\n?[relation, count] := runner_results[count], relation = \"eval_runner_result\"\n?[relation, count] := evaluations[count], relation = \"eval_evaluation\"\n?[relation, count] := selection_decisions[count], relation = \"eval_selection_decision\"\n?[relation, count] := continuations[count], relation = \"eval_continuation_decision\"\n?[relation, count] := walk_events[count], relation = \"eval_walk_event\"",
  "type": "walk_db_query"
}

## recent warnings/errors server+turn files

## recent files
2026-06-30 14:42:42.1687160840 306 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/fixtures/use_statement_edge_cases.rs
2026-06-30 14:42:42.1687160840 318 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/tests/malformed_fixtures/invalid_use.rs
2026-06-30 14:42:42.1687160840 383 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/fixtures/visibility.rs
2026-06-30 14:42:42.1687160840 383 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/tests/fixtures/visibility.rs
2026-06-30 14:42:42.1687160840 427 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/fixtures/traits.rs
2026-06-30 14:42:42.1687160840 485 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/fixtures/structs.rs
2026-06-30 14:42:42.1687160840 490 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/fixtures/use_statements.rs
2026-06-30 14:42:42.1687642750 318 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tests/malformed_fixtures/invalid_use.rs
2026-06-30 14:42:42.1687642750 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/tools/needed_files.sh
2026-06-30 14:42:42.1687642750 4114 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixture_test_crate/src/main.rs
2026-06-30 14:42:42.1687865370 9525 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixture_test_crate/src/second_sibling.rs
2026-06-30 14:42:42.1687985090 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/Cargo.toml
2026-06-30 14:42:42.1687985090 3286 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixture_test_crate/src/sibling_of_main.rs
2026-06-30 14:42:42.1687985090 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/tools/needed_files.sh
2026-06-30 14:42:42.1688165130 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/Cargo.toml
2026-06-30 14:42:42.1688165130 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/README.md
2026-06-30 14:42:42.1688325630 13092 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/cli.rs
2026-06-30 14:42:42.1688325630 1810 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/fixtures/corpus_repro_report_template.json
2026-06-30 14:42:42.1688325630 1810 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/fixtures/corpus_repro_report_template.json
2026-06-30 14:42:42.1688325630 29716 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixtures/openrouter/embeddings_models.json
2026-06-30 14:42:42.1688325630 339 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixtures/openrouter/embeddings_models.meta.json
2026-06-30 14:42:42.1688325630 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/README.md
2026-06-30 14:42:42.1688726490 13092 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/cli.rs
2026-06-30 14:42:42.1688987580 1136 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/install.sh
2026-06-30 14:42:42.1688987580 44344 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/fixtures/snippets/graph_access.rs
2026-06-30 14:42:42.1689237750 100291 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/package-lock.json
2026-06-30 14:42:42.1689237750 59 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/package.json
2026-06-30 14:42:42.1690111190 223 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/proc_macros/ploke-db-derive/Cargo.toml
2026-06-30 14:42:42.1692201320 5101 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/gen_project_context.sh
2026-06-30 14:42:42.1692642150 12285 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/prototype1_eval_trend.py
2026-06-30 14:42:42.1692642150 1441 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/openrouter_pricing_sync.py
2026-06-30 14:42:42.1692642150 1876 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/ploke_eval_doc_read_counts.sh
2026-06-30 14:42:42.1692642150 1990 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/no_gratuitous_collect.sh
2026-06-30 14:42:42.1692642150 2112 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/scripts/ploke_eval_doc_unread.sh
2026-06-30 14:42:42.1693281050 45056 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/backup_dbs/ws_fixture_01_canonical_2026-03-21.sqlite
2026-06-30 14:42:42.1693281050 789 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixture_chat/README.md
2026-06-30 14:42:42.1693839900 1847 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixture_chat/tokens_sample.log
2026-06-30 14:42:42.1693839900 287 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixture_crates/common_file.rs
2026-06-30 14:42:42.1693839900 39523 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixture_chat/tokens_20251221_210943_69895.log
2026-06-30 14:42:42.1695854590 11423 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/context.rs
2026-06-30 14:42:42.1695950970 6073 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/error.rs
2026-06-30 14:42:42.1696173590 11423 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/context.rs
2026-06-30 14:42:42.1696173590 1568 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/executor.rs
2026-06-30 14:42:42.1696335490 1568 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/executor.rs
2026-06-30 14:42:42.1696335490 3740 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/lib.rs
2026-06-30 14:42:42.1696335490 3740 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/lib.rs
2026-06-30 14:42:42.1696335490 6073 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/error.rs
2026-06-30 14:42:42.1696675130 126118 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/main.rs
2026-06-30 14:42:42.1697484050 126118 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/main.rs
2026-06-30 14:42:42.1697769090 3344 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/cli_invariant_tests.rs
2026-06-30 14:42:42.1697769090 43663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/src/profile_ingest.rs
2026-06-30 14:42:42.1698018660 2713 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/command_acceptance_parse.rs
2026-06-30 14:42:42.1698018660 3344 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/cli_invariant_tests.rs
2026-06-30 14:42:42.1698018660 43663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/src/profile_ingest.rs
2026-06-30 14:42:42.1698018660 6222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/command_acceptance_db.rs
2026-06-30 14:42:42.1698266720 6222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/command_acceptance_db.rs
2026-06-30 14:42:42.1698438750 2713 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/command_acceptance_parse.rs
2026-06-30 14:42:42.1698438750 8716 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/context_tests.rs
2026-06-30 14:42:42.1698627100 8716 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/context_tests.rs
2026-06-30 14:42:42.1698838700 63252 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/parse_debug_commands.rs
2026-06-30 14:42:42.1698838700 63252 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/parse_debug_commands.rs
2026-06-30 14:42:42.1698838700 663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/xtask/tests/test_matrix.md
2026-06-30 14:42:42.1699348860 663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r4/xtask/tests/test_matrix.md
2026-06-30 14:42:42.1719729980 8340 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/edge_shadowing.rs
2026-06-30 14:42:42.1720175010 111 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/simple_pub.rs
2026-06-30 14:42:42.1720175010 1412 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/functions.rs
2026-06-30 14:42:42.1720175010 26 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/empty.rs
2026-06-30 14:42:42.1720175010 279 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/invalid_uses.rs
2026-06-30 14:42:42.1720175010 281 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/modules.rs
2026-06-30 14:42:42.1720175010 306 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/use_statement_edge_cases.rs
2026-06-30 14:42:42.1720175010 318 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/malformed_fixtures/invalid_use.rs
2026-06-30 14:42:42.1720175010 383 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/visibility.rs
2026-06-30 14:42:42.1720175010 427 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/traits.rs
2026-06-30 14:42:42.1720175010 432 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/enums.rs
2026-06-30 14:42:42.1720175010 475 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/restricted_visibility.rs
2026-06-30 14:42:42.1720175010 485 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/structs.rs
2026-06-30 14:42:42.1720175010 490 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/use_statements.rs
2026-06-30 14:42:42.1720175010 4953 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/mixed_sample.rs
2026-06-30 14:42:42.1720175010 7074 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/sample.rs
2026-06-30 14:42:42.1720175010 858 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tests/fixtures/macros.rs
2026-06-30 14:42:42.1722177780 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/Cargo.toml
2026-06-30 14:42:42.1722177780 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/tools/needed_files.sh
2026-06-30 14:42:42.1722523830 13092 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/cli.rs
2026-06-30 14:42:42.1722523830 1810 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/fixtures/corpus_repro_report_template.json
2026-06-30 14:42:42.1722523830 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/README.md
2026-06-30 14:42:42.1729743000 11423 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/context.rs
2026-06-30 14:42:42.1729743000 126118 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/main.rs
2026-06-30 14:42:42.1729743000 1568 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/executor.rs
2026-06-30 14:42:42.1729743000 3344 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/cli_invariant_tests.rs
2026-06-30 14:42:42.1729743000 3740 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/lib.rs
2026-06-30 14:42:42.1729743000 43663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/profile_ingest.rs
2026-06-30 14:42:42.1729743000 6073 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/src/error.rs
2026-06-30 14:42:42.1732422340 2713 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/command_acceptance_parse.rs
2026-06-30 14:42:42.1732422340 6222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/command_acceptance_db.rs
2026-06-30 14:42:42.1732422340 63252 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/parse_debug_commands.rs
2026-06-30 14:42:42.1732422340 663 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/test_matrix.md
2026-06-30 14:42:42.1732422340 8716 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r5/xtask/tests/context_tests.rs
2026-06-30 14:42:42.8945757920 14504 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0004.json
2026-06-30 14:42:44.4725911260 37001 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0005.json
2026-06-30 14:42:50.2016467230 60600 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0008.json
2026-06-30 14:43:25.6379882130 63197 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0009.json
2026-06-30 14:43:30.2260321330 9206 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0001.json
2026-06-30 14:43:31.6620458660 45841 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0002.json
2026-06-30 14:43:32.3970528930 69469 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0010.json
2026-06-30 14:43:34.3240713070 100820 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0011.json
2026-06-30 14:43:36.4010911420 24397 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0003.json
2026-06-30 14:43:38.7451135110 84053 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0012.json
2026-06-30 14:43:47.1221933170 477 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/session.json
2026-06-30 14:43:47.1231933270 86759 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/steps/0013.json
2026-06-30 14:43:47.1241933360 84012 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/07e30aea-f8ee-44eb-a841-db98abe6fcdd/resume.json
2026-06-30 14:43:49.7132179590 28102 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0004.json
2026-06-30 14:43:51.7142369750 74768 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r2.headless-tui.json
2026-06-30 14:43:52.3502430160 137237 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r2.turn-live/agent-turn-trace.json
2026-06-30 14:43:52.3572430830 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r2.turn-live/llm-full-responses.jsonl
2026-06-30 14:43:52.3572430830 137237 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r2.turn-live/agent-turn-summary.json
2026-06-30 14:43:53.0642497980 31373 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0005.json
2026-06-30 14:43:58.0912974990 35576 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0006.json
2026-06-30 14:44:03.1133450780 39069 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0007.json
2026-06-30 14:44:05.3683664190 475 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/session.json
2026-06-30 14:44:05.3703664380 79067 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/steps/0008.json
2026-06-30 14:44:05.3713664470 50211 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/ea791f6e-0ff7-4cfb-9752-b1be944377b2/resume.json
2026-06-30 14:44:29.3825927800 40519 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0006.json
2026-06-30 14:44:34.5916416670 34418 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3.headless-tui.json
2026-06-30 14:44:35.9566544660 48995 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3.turn-live/agent-turn-trace.json
2026-06-30 14:44:35.9581182900 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3.turn-live/llm-full-responses.jsonl
2026-06-30 14:44:35.9581182900 48995 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3.turn-live/agent-turn-summary.json
2026-06-30 14:44:36.0826556470 5831 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3.json
2026-06-30 14:44:38.1196747360 67602 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0007.json
2026-06-30 14:44:57.2238532210 79594 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0008.json
2026-06-30 14:45:11.8099888500 8864 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0000.json
2026-06-30 14:45:11.9799904280 8481 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0000.json
2026-06-30 14:45:12.5499957160 8711 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0000.json
2026-06-30 14:45:13.0380002430 12209 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0001.json
2026-06-30 14:45:13.9610088050 11979 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0001.json
2026-06-30 14:45:14.0970100660 13737 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0002.json
2026-06-30 14:45:16.3160306390 14283 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0002.json
2026-06-30 14:45:16.9490365050 16025 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0003.json
2026-06-30 14:45:17.8520448720 17021 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0003.json
2026-06-30 14:45:19.5020601550 18236 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0004.json
2026-06-30 14:45:20.1050657390 18007 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0004.json
2026-06-30 14:45:21.2330761810 38467 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0005.json
2026-06-30 14:45:23.3710959650 34568 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0005.json
2026-06-30 14:45:28.9121471850 31770 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0006.json
2026-06-30 14:45:30.1931590160 42912 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0006.json
2026-06-30 14:45:30.2171592380 68449 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0007.json
2026-06-30 14:45:33.0101850190 63566 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0007.json
2026-06-30 14:45:34.9552029610 46555 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0008.json
2026-06-30 14:45:40.5032540880 50227 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0009.json
2026-06-30 14:45:45.7663025210 67973 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0008.json
2026-06-30 14:45:55.2933900260 80200 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0009.json
2026-06-30 14:45:57.5434106610 117448 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0010.json
2026-06-30 14:46:08.4545105620 481 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/session.json
2026-06-30 14:46:08.4565105800 52525 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/steps/0010.json
2026-06-30 14:46:08.4605106170 50752 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/c2fe010c-29b4-418e-8256-d45bbaea1240/resume.json
2026-06-30 14:46:11.3359702480 101342 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0011.json
2026-06-30 14:46:14.1865629350 104741 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0012.json
2026-06-30 14:46:36.6047670630 475 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/session.json
2026-06-30 14:46:36.6067670810 128516 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/steps/0009.json
2026-06-30 14:46:36.6087671000 110844 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/81ecf7ac-d564-4478-a364-9ed800877dd0/resume.json
2026-06-30 14:46:36.6557675260 43875 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r3.headless-tui.json
2026-06-30 14:46:37.0307709320 98432 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r3.turn-live/agent-turn-trace.json
2026-06-30 14:46:37.0317709410 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r3.turn-live/llm-full-responses.jsonl
2026-06-30 14:46:37.0317709410 98432 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r3.turn-live/agent-turn-summary.json
2026-06-30 14:46:37.1837723210 30087 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/streams/a96b5766-b5e0-43a8-99d8-b81936cb5449/stdout.log
2026-06-30 14:46:39.8217962660 111651 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0013.json
2026-06-30 14:46:40.1087988700 177 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/target/CACHEDIR.TAG
2026-06-30 14:46:40.6298035970 1351 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/runner-request.json
2026-06-30 14:46:42.4228198600 131351 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0014.json
2026-06-30 14:46:44.7268407490 122236 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0015.json
2026-06-30 14:46:49.3028822020 127238 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0016.json
2026-06-30 14:46:58.3359639010 129130 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0017.json
2026-06-30 14:47:00.6599848940 131619 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0018.json
2026-06-30 14:47:07.6630480830 135564 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0019.json
2026-06-30 14:47:13.7211026660 481 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/session.json
2026-06-30 14:47:13.7261027110 149074 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/steps/0020.json
2026-06-30 14:47:13.7301027470 147933 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/02a0b144-aa15-49d9-89f2-22f8cc0415da/resume.json
2026-06-30 14:47:15.4451181850 11621 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0001.json
2026-06-30 14:47:20.6431649430 25733 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0002.json
2026-06-30 14:47:22.2531794140 27298 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0003.json
2026-06-30 14:47:23.7391927670 30067 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0004.json
2026-06-30 14:47:24.4411990730 48806 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r4.headless-tui.json
2026-06-30 14:47:24.9842039500 78770 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r4.turn-live/agent-turn-trace.json
2026-06-30 14:47:24.9862039690 78770 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r4.turn-live/agent-turn-summary.json
2026-06-30 14:47:24.9872039770 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r4.turn-live/llm-full-responses.jsonl
2026-06-30 14:47:25.1812057200 5853 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r4.json
2026-06-30 14:47:25.4472081080 32487 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0005.json
2026-06-30 14:47:26.8802209760 33930 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0006.json
2026-06-30 14:47:28.5582360380 70810 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0007.json
2026-06-30 14:47:37.8313191760 76446 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r6.headless-tui.json
2026-06-30 14:47:37.9733204480 79907 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0008.json
2026-06-30 14:47:38.3763240570 142025 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r6.turn-live/agent-turn-trace.json
2026-06-30 14:47:38.3803240920 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r6.turn-live/llm-full-responses.jsonl
2026-06-30 14:47:38.3803240920 142025 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r6.turn-live/agent-turn-summary.json
2026-06-30 14:47:38.5733258210 5857 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r6.json
2026-06-30 14:47:38.7423273340 1042 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r6/target/.rustc_info.json
2026-06-30 14:47:43.1553668340 55487 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0009.json
2026-06-30 14:47:45.4523873790 57321 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0010.json
2026-06-30 14:47:46.9654009070 481 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/session.json
2026-06-30 14:47:46.9714009600 96017 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/steps/0011.json
2026-06-30 14:47:46.9724009690 67575 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/debug/tool-loop/2b0baf0b-5894-485a-bddb-7eaa1bd2251b/resume.json
2026-06-30 14:48:44.8869156440 50389 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r5.headless-tui.json
2026-06-30 14:48:45.2819191340 104029 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r5.turn-live/agent-turn-trace.json
2026-06-30 14:48:45.2849191600 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r5.turn-live/llm-full-responses.jsonl
2026-06-30 14:48:45.2849191600 104029 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r5.turn-live/agent-turn-summary.json
2026-06-30 14:48:45.4499206180 5852 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-result/node-2fe75acd9e9cf6c3-r5.json
2026-06-30 14:48:48.3087131800 1691 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-5a626eeb21d0c85a/node.json
2026-06-30 14:48:48.8389505520 1301 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-5a626eeb21d0c85a/runner-request.json
2026-06-30 14:48:49.3260847040 1690 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-32891a1656546386/node.json
2026-06-30 14:48:49.7999590370 1300 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-32891a1656546386/runner-request.json
2026-06-30 14:48:50.2689567040 1695 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-64f31f889cd20ded/node.json
2026-06-30 14:48:50.7449673790 1305 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-64f31f889cd20ded/runner-request.json
2026-06-30 14:48:51.2529718630 455810 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-2fe75acd9e9cf6c3.json
2026-06-30 14:49:00.2900515550 961124800 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/bin/ploke-eval
2026-06-30 14:49:18.7352138010 89486 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/invocations/22ad6f62-17af-4440-b5a7-b507dc262fe7.json
2026-06-30 14:49:19.1742176560 801 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/channels/22ad6f62-17af-4440-b5a7-b507dc262fe7/child-to-parent.jsonl
2026-06-30 14:49:19.6092214750 1741 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/node.json
2026-06-30 14:49:20.3715372550 128 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/streams/22ad6f62-17af-4440-b5a7-b507dc262fe7/stderr.log
2026-06-30 14:49:32.2613324380 73875 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/transition-journal.jsonl
2026-06-30 14:49:33.3393418810 3497984 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite
2026-06-30 14:50:39.7309201750 27641 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/streams/22ad6f62-17af-4440-b5a7-b507dc262fe7/stdout.log
