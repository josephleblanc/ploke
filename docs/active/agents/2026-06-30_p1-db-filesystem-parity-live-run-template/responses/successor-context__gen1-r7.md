# Successor context gen1 R7

timestamp: 2026-06-30T14:42:16-07:00

## binary
-rwxr-xr-x 2 brasides brasides 961124704 Jun 30 14:39 /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval
51cd1a0696bd16024452360bff0f190a08333616711b8ae50365c73a83d09b13  /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval

## git
e484103aa28af3b9e75254aa10a5bd17d588ad8e

## parent identity
{
    "schema_version": "prototype1-parent-identity.v1",
    "campaign_id": "p1-gated-parent-3g1x3-p3-20260630-174316",
    "parent_id": "node-2fe75acd9e9cf6c3",
    "node_id": "node-2fe75acd9e9cf6c3",
    "generation": 1,
    "instance_id": "c69194ab-5cc2-454f-ae46-9d142dd63d3a",
    "previous_parent_id": "node-ec383aa38762a3d4",
    "parent_node_id": "node-ec383aa38762a3d4",
    "branch_id": "branch-88aaa7a4b0328baf",
    "artifact_branch": "prototype1-broad-broad-harness-request-node-ec383aa38762a3d4-r2",
    "created_at": "2026-06-30T21:38:52.729772246+00:00"
}

## walk show
{
  "type": "ok",
  "phase": "r7",
  "message": "server_pid=187655\nphase: r7 - policy and child budget ready\nsteps: 0\nroot: /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/\ntracking_dir: /home/brasides/.ploke-eval/\ntracked files:\n  parent_identity: {root}/.ploke/prototype1/parent_identity.json\n  active_monitor_target: {tracking_dir}/prototype1-monitor-target.json\n  campaign_manifest: {tracking_dir}/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/campaign.json\n  transition_journal: {tracking_dir}/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/transition-journal.jsonl\nreconstruction:\n  - parent_identity: {root}/.ploke/prototype1/parent_identity.json\n    node=node-2fe75acd9e9cf6c3, generation=1, branch=branch-88aaa7a4b0328baf\n  - successor_handoff_invocation: {tracking_dir}/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/invocations/a96b5766-b5e0-43a8-99d8-b81936cb5449.json\n  - R1: campaign manifest and admitted run profile/defaults\n  - R3: checkout parent identity\n  - R4a: loading Parent<Unchecked>\n  - R4c: predecessor startup validation\n  - R5: matching parent-start journal evidence\n  - R6: durable parent baseline evidence\n  - R7: run policy and child budget inputs\ntypestate:\n  Runtime<\n      phase::R7,\n      parent_role::Parent<parent_role::Ready>,\n      Context<context::Collected<RunShape, CampaignConfig>>,\n      Plan<\n          plan::authority::None,\n          plan::schedule::None,\n      >,\n      Children<children::set::None, children::attempt::None>,\n      History<\n          history_axis::startup::Validated<history_axis::startup::Any>,\n          history_axis::head::FromStartup,\n          history_axis::epoch::None,\n      >,\n      Evidence<\n          evidence::parent_start::Recorded<ParentStartedEntry>,\n          evidence::baseline::Ready<CompleteBaseline>,\n          evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,\n          evidence::selection::None,\n          evidence::completion::None,\n      >,\n      Continuation<\n          continuation::selection::None,\n          continuation::decision::None,\n          continuation::handoff::None,\n      >,\n      Report<report::None>,\n  >;\nnext:\n  r7_to_r8 --watch -> r8 - resolve live child-plan authority; may wait on provider/harness work\nprevious:\n  reconstructed durable walk state at r7",
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
      "count": 7,
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
      "count": 1,
      "relation": "eval_continuation_decision"
    },
    {
      "count": 2,
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
      "count": 3,
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
      "count": 24,
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
      "count": 1,
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
      "count": 13,
      "relation": "eval_walk_event"
    }
  ],
  "script": "campaign[count(campaign_id)] := *eval_campaign{campaign_id}\nprofile[count(profile_ref_id)] := *eval_profile_commitment{profile_ref_id}\npolicy[count(campaign_id)] := *eval_run_profile_policy{campaign_id}\nscheduler[count(node_id)] := *eval_scheduler_node{node_id}\nscheduler_events[count(status_event_id)] := *eval_scheduler_node_status_event{status_event_id}\nrunner_requests[count(node_id)] := *eval_runner_request{node_id}\nparent_identity[count(parent_id)] := *eval_parent_identity{parent_id}\nparent_start[count(start_event_id)] := *eval_parent_start{start_event_id}\ntransition_events[count(event_id)] := *eval_transition_event{event_id}\nrecord_refs[count(record_ref_id)] := *eval_record_ref{record_ref_id}\nharness_requests[count(request_id)] := *eval_harness_request{request_id}\nharness_diagnostics[count(request_id)] := *eval_harness_diagnostic{request_id}\nharness_submissions[count(request_id)] := *eval_harness_submission{request_id}\nagent_turns[count(turn_id)] := *eval_agent_turn{turn_id}\nmodel_exchanges[count(exchange_id)] := *eval_model_exchange{exchange_id}\ntool_events[count(tool_event_id)] := *eval_tool_event{tool_event_id}\nchild_plans[count(plan_id)] := *eval_child_plan{plan_id}\nchild_plan_children[count(child_node_id)] := *eval_child_plan_child{child_node_id}\nartifacts[count(artifact_id)] := *eval_artifact{artifact_id}\nbinaries[count(binary_ref_id)] := *eval_binary_ref{binary_ref_id}\nbuild_events[count(build_id)] := *eval_build_event{build_id}\ninvocations[count(invocation_id)] := *eval_invocation{invocation_id}\nchannel_messages[count(channel_message_id)] := *eval_channel_message{channel_message_id}\nrunner_results[count(result_path)] := *eval_runner_result{result_path}\nevaluations[count(evaluation_id)] := *eval_evaluation{evaluation_id}\nselection_decisions[count(decision_id)] := *eval_selection_decision{decision_id}\ncontinuations[count(decision_id)] := *eval_continuation_decision{decision_id}\nwalk_events[count(event_id)] := *eval_walk_event{event_id}\n\n?[relation, count] := campaign[count], relation = \"eval_campaign\"\n?[relation, count] := profile[count], relation = \"eval_profile_commitment\"\n?[relation, count] := policy[count], relation = \"eval_run_profile_policy\"\n?[relation, count] := scheduler[count], relation = \"eval_scheduler_node\"\n?[relation, count] := scheduler_events[count], relation = \"eval_scheduler_node_status_event\"\n?[relation, count] := runner_requests[count], relation = \"eval_runner_request\"\n?[relation, count] := parent_identity[count], relation = \"eval_parent_identity\"\n?[relation, count] := parent_start[count], relation = \"eval_parent_start\"\n?[relation, count] := transition_events[count], relation = \"eval_transition_event\"\n?[relation, count] := record_refs[count], relation = \"eval_record_ref\"\n?[relation, count] := harness_requests[count], relation = \"eval_harness_request\"\n?[relation, count] := harness_diagnostics[count], relation = \"eval_harness_diagnostic\"\n?[relation, count] := harness_submissions[count], relation = \"eval_harness_submission\"\n?[relation, count] := agent_turns[count], relation = \"eval_agent_turn\"\n?[relation, count] := model_exchanges[count], relation = \"eval_model_exchange\"\n?[relation, count] := tool_events[count], relation = \"eval_tool_event\"\n?[relation, count] := child_plans[count], relation = \"eval_child_plan\"\n?[relation, count] := child_plan_children[count], relation = \"eval_child_plan_child\"\n?[relation, count] := artifacts[count], relation = \"eval_artifact\"\n?[relation, count] := binaries[count], relation = \"eval_binary_ref\"\n?[relation, count] := build_events[count], relation = \"eval_build_event\"\n?[relation, count] := invocations[count], relation = \"eval_invocation\"\n?[relation, count] := channel_messages[count], relation = \"eval_channel_message\"\n?[relation, count] := runner_results[count], relation = \"eval_runner_result\"\n?[relation, count] := evaluations[count], relation = \"eval_evaluation\"\n?[relation, count] := selection_decisions[count], relation = \"eval_selection_decision\"\n?[relation, count] := continuations[count], relation = \"eval_continuation_decision\"\n?[relation, count] := walk_events[count], relation = \"eval_walk_event\"",
  "type": "walk_db_query"
}
