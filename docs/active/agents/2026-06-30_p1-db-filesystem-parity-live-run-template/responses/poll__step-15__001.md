# Poll step-15 001

timestamp: 2026-06-30T14:40:05-07:00

## process family
    PID    PPID ELAPSED STAT CMD

## client log
BEGIN 2026-06-30T14:38:51-07:00
{
  "type": "ok",
  "phase": "r13b",
  "message": "from: r12 - report facts ready\nto: r13b - successor handoff committed\nedge: r12_to_r13 --watch --allow git-changes\ntypestate changes:\n  - phase: phase::R12 -> phase::R13b\n  - role: parent_role::Parent<parent_role::Selectable> -> parent_role::Parent<parent_role::Retired>\n  - children:\n      Children<\n          children::set::Report<PlannedChildOutcome>,\n          children::attempt::Complete,\n      >\n      -> Children<\n          children::set::Successor<PlannedChildOutcome>,\n          children::attempt::Complete,\n      >\n  - history:\n      History<\n          history_axis::startup::Validated<history_axis::startup::Any>,\n          history_axis::head::FromStartup,\n          history_axis::epoch::None,\n      >\n      -> History<\n          history_axis::startup::Validated<history_axis::startup::Any>,\n          history_axis::head::Advanced<Block<history_model::block::Sealed>>,\n          history_axis::epoch::Sealed<Block<history_model::block::Sealed>>,\n      >\n  - evidence:\n      Evidence<\n          evidence::parent_start::Recorded<ParentStartedEntry>,\n          evidence::baseline::Ready<CompleteBaseline>,\n          evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,\n          evidence::selection::Evidence<SelectionSealMaterial>,\n          evidence::completion::None,\n      >\n      -> Evidence<\n          evidence::parent_start::Recorded<ParentStartedEntry>,\n          evidence::baseline::Ready<CompleteBaseline>,\n          evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,\n          evidence::selection::Seal<SelectionSealMaterial>,\n          evidence::completion::None,\n      >\n  - continuation:\n      Continuation<\n          continuation::selection::Maybe<SuccessorDecision>,\n          continuation::decision::None,\n          continuation::handoff::None,\n      >\n      -> Continuation<\n          continuation::selection::Selected<SuccessorDecision>,\n          continuation::decision::Allowed<Prototype1ContinuationDecision>,\n          continuation::handoff::Recorded<successor::Record>,\n      >\n  - side effect: seals/appends History, installs selected successor checkout, retires parent, spawns successor, and waits for ready evidence\nnext:\n  r13_to_r14 -> r14b - emit final report after successor handoff",
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
END 2026-06-30T14:39:21-07:00 exit=0

## walk show short
{
  "type": "ok",
  "phase": "r13b",
  "message": "server_pid=182850\nphase: r13b - successor handoff committed\nsteps: 3\nroot: /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/\ntracking_dir: /home/brasides/.ploke-eval/\ntracked files:\n  parent_identity: {root}/.ploke/prototype1/parent_identity.json\n  active_monitor_target: {tracking_dir}/prototype1-monitor-target.json\n  campaign_manifest: {tracking_dir}/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/campaign.json\n  transition_journal: {tracking_dir}/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/transition-journal.jsonl\ntypestate:\n  Runtime<\n      phase::R13b,\n      parent_role::Parent<parent_role::Retired>,\n      Context<context::Collected<RunShape, CampaignConfig>>,\n      Plan<\n          plan::authority::Received<Received<parent_role::ChildPlan>>,\n          plan::schedule::Ready<\n              Prototype1ChildBudget,\n              Prototype1ChildScheduleMode,\n          >,\n      >,\n      Children<children::set::Successor<PlannedChildOutcome>, children::attempt::Complete>,\n      History<\n          history_axis::startup::Validated<history_axis::startup::Any>,\n          history_axis::head::Advanced<Block<history_model::block::Sealed>>,\n          history_axis::epoch::Sealed<Block<history_model::block::Sealed>>,\n      >,\n      Evidence<\n          evidence::parent_start::Recorded<ParentStartedEntry>,\n          evidence::baseline::Ready<CompleteBaseline>,\n          evidence::policy::Ready<Prototype1SearchPolicy, Prototype1ChildBudget>,\n          evidence::selection::Seal<SelectionSealMaterial>,\n          evidence::completion::None,\n      >,\n      Continuation<\n          continuation::selection::Selected<SuccessorDecision>,\n          continuation::decision::Allowed<Prototype1ContinuationDecision>,\n          continuation::handoff::Recorded<successor::Record>,\n      >,\n      Report<report::Facts>,\n  >;\nnext:\n  r13_to_r14 -> r14b - emit final report after successor handoff\nprevious:\n  reconstructed durable walk state at r10\n  step 1: r10 -> r11\n  step 2: r11 -> r12\n  blocked at r12: walk reached R12 with selected-successor evidence; rerun `walk step --watch --allow git-changes` to admit R13b handoff\n  step 3: r12 -> r13b",
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

## recent campaign files
2026-06-30 14:39:04.8993632800 146895 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/history/blocks/segment-000000.jsonl
2026-06-30 14:39:04.9073633650 216 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/history/index/by-hash.jsonl
2026-06-30 14:39:04.9143634390 155 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/history/index/by-lineage-height.jsonl
2026-06-30 14:39:04.9223635240 116 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/history/index/heads.json
2026-06-30 14:39:04.9223635240 1176 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/invocations/a96b5766-b5e0-43a8-99d8-b81936cb5449.json
2026-06-30 14:39:05.1033654510 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/streams/a96b5766-b5e0-43a8-99d8-b81936cb5449/stderr.log
2026-06-30 14:39:08.0153964230 680 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/channels/a96b5766-b5e0-43a8-99d8-b81936cb5449/child-to-parent.jsonl
2026-06-30 14:39:08.6314029690 55710 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/transition-journal.jsonl
2026-06-30 14:39:08.7154038620 1179 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/node.json
2026-06-30 14:39:08.9244060820 2088 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3.md
2026-06-30 14:39:08.9244060820 9067 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3.json
2026-06-30 14:39:09.1064080160 2091 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3-r2.md
2026-06-30 14:39:09.1064080160 9088 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3-r2.json
2026-06-30 14:39:09.2894099600 2091 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3-r3.md
2026-06-30 14:39:09.2894099600 9088 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/edit-harness-request/node-2fe75acd9e9cf6c3-r3.json
2026-06-30 14:39:09.4724119030 4005 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/pre-child-planning/node-2fe75acd9e9cf6c3.prompt.md
2026-06-30 14:39:11.1224294210 2639 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/messages/pre-child-planning/node-2fe75acd9e9cf6c3.json
2026-06-30 14:39:11.1575133810 74 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.git
2026-06-30 14:39:11.1575346110 74 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.git
2026-06-30 14:39:11.1575794560 71 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.git
2026-06-30 14:39:11.1824300580 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.agents.untracked-file-backup-20260426-1355
2026-06-30 14:39:11.1824300580 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.agents.untracked-file-backup-20260426-1355
2026-06-30 14:39:11.1824300580 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.agents.untracked-file-backup-20260426-1355
2026-06-30 14:39:11.1824300580 249952 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.agents/hyper-agents.txt
2026-06-30 14:39:11.1824300580 249952 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.agents/hyper-agents.txt
2026-06-30 14:39:11.1824300580 249952 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.agents/hyper-agents.txt
2026-06-30 14:39:11.1835360400 11370 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.agents/prototype1-score-grounding-handoff-2026-05-06.md
2026-06-30 14:39:11.1835360400 11370 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.agents/prototype1-score-grounding-handoff-2026-05-06.md
2026-06-30 14:39:11.1835360400 11370 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.agents/prototype1-score-grounding-handoff-2026-05-06.md
2026-06-30 14:39:11.1835360400 23753 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.agents/prototype1-hyperagents-handoff-2026-05-06.md
2026-06-30 14:39:11.1835360400 23753 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.agents/prototype1-hyperagents-handoff-2026-05-06.md
2026-06-30 14:39:11.1835360400 23753 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.agents/prototype1-hyperagents-handoff-2026-05-06.md
2026-06-30 14:39:11.1844300800 120 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.cargo/config.toml
2026-06-30 14:39:11.1844300800 120 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.cargo/config.toml
2026-06-30 14:39:11.1844300800 2240895 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.bacon-locations
2026-06-30 14:39:11.1844300800 2240895 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.bacon-locations
2026-06-30 14:39:11.1844300800 2240895 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.bacon-locations
2026-06-30 14:39:11.1848939050 120 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.cargo/config.toml
2026-06-30 14:39:11.1850921170 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.gitmodules
2026-06-30 14:39:11.1850921170 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.gitmodules
2026-06-30 14:39:11.1850921170 2382 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.gitignore
2026-06-30 14:39:11.1850921170 2382 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.gitignore
2026-06-30 14:39:11.1852154490 0 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.gitmodules
2026-06-30 14:39:11.1852154490 2382 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.gitignore
2026-06-30 14:39:11.1852732780 13145 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.orchestrator.archive-20260515-012614/board.json
2026-06-30 14:39:11.1852732780 13145 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.orchestrator.archive-20260515-012614/board.json
2026-06-30 14:39:11.1854106860 13145 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.orchestrator.archive-20260515-012614/board.json
2026-06-30 14:39:11.1855367140 97321 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.orchestrator.archive.2026-05-12-egui-task-readability/board.json
2026-06-30 14:39:11.1855367140 97321 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.orchestrator.archive.2026-05-12-egui-task-readability/board.json
2026-06-30 14:39:11.1856683110 97321 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.orchestrator.archive.2026-05-12-egui-task-readability/board.json
2026-06-30 14:39:11.1856827480 41013 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.orchestrator.archive.2026-05-12-egui-task-readability/graph-ingestion-wave2.json
2026-06-30 14:39:11.1856827480 41013 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.orchestrator.archive.2026-05-12-egui-task-readability/graph-ingestion-wave2.json
2026-06-30 14:39:11.1856827480 41013 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.orchestrator.archive.2026-05-12-egui-task-readability/graph-ingestion-wave2.json
2026-06-30 14:39:11.1864301000 913 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.tmp/msb-clap-5075-config.json
2026-06-30 14:39:11.1864301000 913 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.tmp/msb-clap-5075-config.json
2026-06-30 14:39:11.1864739060 12149 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/AGENTS.md
2026-06-30 14:39:11.1864739060 12149 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/AGENTS.md
2026-06-30 14:39:11.1864739060 12149 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/AGENTS.md
2026-06-30 14:39:11.1864739060 2010 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/TECH_DEBT.md
2026-06-30 14:39:11.1864739060 2010 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/TECH_DEBT.md
2026-06-30 14:39:11.1864739060 2010 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/TECH_DEBT.md
2026-06-30 14:39:11.1864739060 23714 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/README.md
2026-06-30 14:39:11.1864739060 23714 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/README.md
2026-06-30 14:39:11.1864739060 23714 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/README.md
2026-06-30 14:39:11.1864739060 2590 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/CLAUDE.md
2026-06-30 14:39:11.1864739060 2590 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/CLAUDE.md
2026-06-30 14:39:11.1864739060 2590 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/CLAUDE.md
2026-06-30 14:39:11.1864739060 26222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/PROPOSED_ARCH_V3.md
2026-06-30 14:39:11.1864739060 26222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/PROPOSED_ARCH_V3.md
2026-06-30 14:39:11.1864739060 26222 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/PROPOSED_ARCH_V3.md
2026-06-30 14:39:11.1864739060 265 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.zed/settings.json
2026-06-30 14:39:11.1864739060 265 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.zed/settings.json
2026-06-30 14:39:11.1864739060 265 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.zed/settings.json
2026-06-30 14:39:11.1864739060 291684 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/Cargo.lock
2026-06-30 14:39:11.1864739060 291684 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/Cargo.lock
2026-06-30 14:39:11.1864739060 291684 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/Cargo.lock
2026-06-30 14:39:11.1864739060 33022 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/LICENSE
2026-06-30 14:39:11.1864739060 33022 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/LICENSE
2026-06-30 14:39:11.1864739060 33022 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/LICENSE
2026-06-30 14:39:11.1864739060 5234 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/Cargo.toml
2026-06-30 14:39:11.1864739060 5234 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/Cargo.toml
2026-06-30 14:39:11.1864739060 5234 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/Cargo.toml
2026-06-30 14:39:11.1864739060 6153 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.tmp/msb-clap-5075-ground-truth.jsonl
2026-06-30 14:39:11.1864739060 6153 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.tmp/msb-clap-5075-ground-truth.jsonl
2026-06-30 14:39:11.1864739060 6153 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.tmp/msb-clap-5075-ground-truth.jsonl
2026-06-30 14:39:11.1864739060 887 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.tmp/msb-clap-5075-ground-truth-config.json
2026-06-30 14:39:11.1864739060 887 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/.tmp/msb-clap-5075-ground-truth-config.json
2026-06-30 14:39:11.1864739060 887 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/.tmp/msb-clap-5075-ground-truth-config.json
2026-06-30 14:39:11.1864739060 91 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/Trunk.toml
2026-06-30 14:39:11.1864739060 91 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/Trunk.toml
2026-06-30 14:39:11.1864739060 91 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/Trunk.toml
2026-06-30 14:39:11.1864739060 913 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/.tmp/msb-clap-5075-config.json
2026-06-30 14:39:11.1914301540 3286659 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/assets/ploke_rust_vector_embeddings.png
2026-06-30 14:39:11.1914301540 3286659 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/assets/ploke_rust_vector_embeddings.png
2026-06-30 14:39:11.1914301540 3286659 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/assets/ploke_rust_vector_embeddings.png
2026-06-30 14:39:11.1924301650 1518273 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/assets/ploki.png
2026-06-30 14:39:11.1924301650 1518273 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/assets/ploki.png
2026-06-30 14:39:11.1924301650 1518273 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/assets/ploki.png
2026-06-30 14:39:11.3574319160 89 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/fixture_test_crate/Cargo.toml
2026-06-30 14:39:11.3574319160 89 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/fixture_test_crate/Cargo.toml
2026-06-30 14:39:11.3575452750 89 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/fixture_test_crate/Cargo.toml
2026-06-30 14:39:11.3577748070 100291 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/package-lock.json
2026-06-30 14:39:11.3577748070 100291 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/package-lock.json
2026-06-30 14:39:11.3577748070 100291 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/package-lock.json
2026-06-30 14:39:11.3577748070 1136 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/install.sh
2026-06-30 14:39:11.3577748070 1136 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/install.sh
2026-06-30 14:39:11.3577748070 1136 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/install.sh
2026-06-30 14:39:11.3577748070 59 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/package.json
2026-06-30 14:39:11.3577748070 59 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/package.json
2026-06-30 14:39:11.3577748070 59 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/package.json
2026-06-30 14:39:11.3581596110 5101 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/gen_project_context.sh
2026-06-30 14:39:11.3582065590 1441 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/openrouter_pricing_sync.py
2026-06-30 14:39:11.3582065590 1441 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/openrouter_pricing_sync.py
2026-06-30 14:39:11.3582065590 1876 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/ploke_eval_doc_read_counts.sh
2026-06-30 14:39:11.3582065590 1876 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/ploke_eval_doc_read_counts.sh
2026-06-30 14:39:11.3582065590 1990 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/no_gratuitous_collect.sh
2026-06-30 14:39:11.3582065590 1990 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/no_gratuitous_collect.sh
2026-06-30 14:39:11.3582065590 2112 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/ploke_eval_doc_unread.sh
2026-06-30 14:39:11.3582065590 5101 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/gen_project_context.sh
2026-06-30 14:39:11.3582497900 12285 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/prototype1_eval_trend.py
2026-06-30 14:39:11.3582497900 12285 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/scripts/prototype1_eval_trend.py
2026-06-30 14:39:11.3582497900 2112 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/scripts/ploke_eval_doc_unread.sh
2026-06-30 14:39:11.3582737650 5101 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/gen_project_context.sh
2026-06-30 14:39:11.3582889040 1441 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/openrouter_pricing_sync.py
2026-06-30 14:39:11.3582889040 1876 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/ploke_eval_doc_read_counts.sh
2026-06-30 14:39:11.3582889040 1990 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/no_gratuitous_collect.sh
2026-06-30 14:39:11.3583248210 2112 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/ploke_eval_doc_unread.sh
2026-06-30 14:39:11.3583354410 12285 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/scripts/prototype1_eval_trend.py
2026-06-30 14:39:11.3617292350 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/xtask/Cargo.toml
2026-06-30 14:39:11.3617292350 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/xtask/Cargo.toml
2026-06-30 14:39:11.3617292350 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/tools/needed_files.sh
2026-06-30 14:39:11.3617292350 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/tools/needed_files.sh
2026-06-30 14:39:11.3617292350 378 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/tools/needed_files.sh
2026-06-30 14:39:11.3617662350 1463 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/xtask/Cargo.toml
2026-06-30 14:39:11.3617662350 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r2/xtask/README.md
2026-06-30 14:39:11.3617662350 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3-r3/xtask/README.md
2026-06-30 14:39:11.3617662350 8579 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/workspaces/edit-harness/node-2fe75acd9e9cf6c3/xtask/README.md
2026-06-30 14:39:21.6365407180 1966080 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite
2026-06-30 14:39:27.0725980430 5030 /home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/nodes/node-2fe75acd9e9cf6c3/streams/a96b5766-b5e0-43a8-99d8-b81936cb5449/stdout.log
