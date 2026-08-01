# Gen1 R7-R8 failure inspection

timestamp: 2026-06-30T14:51:33-07:00

## status
walk
----------------------------------------
status: ok
phase: empty - no active walk
message:
  server_pid=258190
  phase: empty - no active walk
  steps: 0
  root: /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/
  tracking_dir: /home/brasides/.ploke-eval/
  tracked files:
    parent_identity: {root}/.ploke/prototype1/parent_identity.json
    active_monitor_target: {tracking_dir}/prototype1-monitor-target.json
  typestate:
    (no Runtime value is held yet)
  next:
    start -> r0 - create the initial command carrier
  previous:
    (empty)

## show
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

## child plan db rows
{
  "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
  "db_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite",
  "headers": [
    "plan_id",
    "parent_node_id",
    "child_generation",
    "child_count",
    "rejected_count",
    "message_path",
    "message_sha256"
  ],
  "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
  "row_count": 2,
  "rows": [
    {
      "child_count": 2,
      "child_generation": 1,
      "message_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-ec383aa38762a3d4.json",
      "message_sha256": "03a343ecea11e8bb5c5cad65973c0e69dfc323bdbb08f358bd98f7b79b1ea425",
      "parent_node_id": "node-ec383aa38762a3d4",
      "plan_id": "2a630c6065afe51ff110cc615f93188473c43c00697b58d427aa89a1b4d81ec3",
      "rejected_count": 1
    },
    {
      "child_count": 1,
      "child_generation": 2,
      "message_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/child-plan/node-2fe75acd9e9cf6c3.json",
      "message_sha256": "be1e284b2b8b8fe878a3fc1836a74a5f1baa33c97d78014d420b4086d20d1499",
      "parent_node_id": "node-2fe75acd9e9cf6c3",
      "plan_id": "9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04",
      "rejected_count": 2
    }
  ],
  "script": "?[plan_id, parent_node_id, child_generation, child_count, rejected_count, message_path, message_sha256] := *eval_child_plan{plan_id, parent_node_id, child_generation, child_count, rejected_count, message_path, message_sha256}",
  "type": "walk_db_query"
}

## child plan child db rows
{
  "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
  "db_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite",
  "headers": [
    "plan_id",
    "child_node_id",
    "child_index",
    "branch_id",
    "candidate_id",
    "status",
    "node_path",
    "runner_request_path"
  ],
  "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
  "row_count": 3,
  "rows": [
    {
      "branch_id": "branch-88aaa7a4b0328baf",
      "candidate_id": "broad-harness-g1-02",
      "child_index": 1,
      "child_node_id": "node-2fe75acd9e9cf6c3",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/node.json",
      "plan_id": "2a630c6065afe51ff110cc615f93188473c43c00697b58d427aa89a1b4d81ec3",
      "runner_request_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/runner-request.json",
      "status": "planned"
    },
    {
      "branch_id": "branch-f41b072e4787d706",
      "candidate_id": "broad-harness-g1-01",
      "child_index": 0,
      "child_node_id": "node-fe4a6decb46ca481",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/node.json",
      "plan_id": "2a630c6065afe51ff110cc615f93188473c43c00697b58d427aa89a1b4d81ec3",
      "runner_request_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/runner-request.json",
      "status": "planned"
    },
    {
      "branch_id": "branch-864d1a76684b74c5",
      "candidate_id": "broad-harness-g2-01",
      "child_index": 0,
      "child_node_id": "node-e9be81e08bda07a1",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/node.json",
      "plan_id": "9903062bde18d24e7a958d4c426efba5c6f927336ce54a99a44e2bcd3ee3ba04",
      "runner_request_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/runner-request.json",
      "status": "planned"
    }
  ],
  "script": "?[plan_id, child_node_id, child_index, branch_id, candidate_id, status, node_path, runner_request_path] := *eval_child_plan_child{plan_id, child_node_id, child_index, branch_id, candidate_id, status, node_path, runner_request_path}",
  "type": "walk_db_query"
}

## scheduler node rows
{
  "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
  "db_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite",
  "headers": [
    "node_id",
    "parent_node_id",
    "generation",
    "branch_id",
    "candidate_id",
    "status",
    "node_path",
    "runner_result_path"
  ],
  "repo_root": "/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316",
  "row_count": 7,
  "rows": [
    {
      "branch_id": "branch-88aaa7a4b0328baf",
      "candidate_id": "node-2fe75acd9e9cf6c3",
      "generation": 1,
      "node_id": "node-2fe75acd9e9cf6c3",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/node.json",
      "parent_node_id": "node-ec383aa38762a3d4",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/runner-result.json",
      "status": "running"
    },
    {
      "branch_id": "branch-a82b2ec84cc3f3bc",
      "candidate_id": "broad-harness-g2-02",
      "generation": 2,
      "node_id": "node-32891a1656546386",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-32891a1656546386/node.json",
      "parent_node_id": "node-2fe75acd9e9cf6c3",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-32891a1656546386/runner-result.json",
      "status": "planned"
    },
    {
      "branch_id": "branch-32bcfc7499d17354",
      "candidate_id": "broad-harness-g2-01",
      "generation": 2,
      "node_id": "node-5a626eeb21d0c85a",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-5a626eeb21d0c85a/node.json",
      "parent_node_id": "node-2fe75acd9e9cf6c3",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-5a626eeb21d0c85a/runner-result.json",
      "status": "planned"
    },
    {
      "branch_id": "branch-ffdec3f201615de6",
      "candidate_id": "broad-harness-g2-03",
      "generation": 2,
      "node_id": "node-64f31f889cd20ded",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-64f31f889cd20ded/node.json",
      "parent_node_id": "node-2fe75acd9e9cf6c3",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-64f31f889cd20ded/runner-result.json",
      "status": "planned"
    },
    {
      "branch_id": "branch-864d1a76684b74c5",
      "candidate_id": "broad-harness-g2-01",
      "generation": 2,
      "node_id": "node-e9be81e08bda07a1",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/node.json",
      "parent_node_id": "node-2fe75acd9e9cf6c3",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-e9be81e08bda07a1/runner-result.json",
      "status": "running"
    },
    {
      "branch_id": "prototype1-parent-p1-gated-parent-3g1x3-p3-20260630-174316-gen0",
      "candidate_id": "node-ec383aa38762a3d4",
      "generation": 0,
      "node_id": "node-ec383aa38762a3d4",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-ec383aa38762a3d4/node.json",
      "parent_node_id": null,
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-ec383aa38762a3d4/runner-result.json",
      "status": "running"
    },
    {
      "branch_id": "branch-f41b072e4787d706",
      "candidate_id": "broad-harness-g1-01",
      "generation": 1,
      "node_id": "node-fe4a6decb46ca481",
      "node_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/node.json",
      "parent_node_id": "node-ec383aa38762a3d4",
      "runner_result_path": "/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-fe4a6decb46ca481/runner-result.json",
      "status": "succeeded"
    }
  ],
  "script": "?[node_id, parent_node_id, generation, branch_id, candidate_id, status, node_path, runner_result_path] := *eval_scheduler_node{node_id, parent_node_id, generation, branch_id, candidate_id, status, node_path, runner_result_path}",
  "type": "walk_db_query"
}
