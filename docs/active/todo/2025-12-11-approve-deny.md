• What I Found

  - Conversation rendering lives in crates/ploke-tui/src/app/view/components/
    conversation.rs + app/message_item.rs; it wraps plain text with a simple left
    gutter for selection. Colors are per MessageKind only; there’s no per-line styling,
    so diffs/code lose red/green context and wrapping can distort indentation.
  - Chat data model is in crates/ploke-tui/src/chat_history.rs (Message is plain text +
    kind/metadata). No typed “proposal message” exists; all proposal UX is out-of-band.
  - Code-edit proposals are staged in crates/ploke-tui/src/rag/tools.rs (see
    apply_code_edit_tool): they store EditProposal with DiffPreview in AppState,
    then post a SysInfo chat message string that includes a truncated preview and
    instructions to run edit approve/deny. Approve/deny application messages come from
    crates/ploke-tui/src/rag/editing.rs and are also plain SysInfo text.
  - Approval actions are currently accessed only through the overlay (ApprovalsState
    in app/view/components/approvals.rs + key handling in app/mod.rs), not through
    conversation. State commands to trigger apply/deny are StateCommand::ApproveEdits/
    ApproveCreations/Deny*.

  UX Gaps (re: inline diffs + actions)

  - The conversation only shows a textual summary; no actual diff with red/green, and
    wrapping can mangle code blocks.
  - There’s no inline affordance to approve/deny; users must jump to the overlay or
    type commands.
  - h/l are already mapped to branch navigation (input/keymap.rs), so left/right
    selection for buttons would need contextual override when a “proposal action” block
    is focused.

  Suggested Direction

  - Introduce a typed “proposal action” message variant (e.g., a new enum or metadata
    payload) that carries {request_id, status, DiffPreview, selected_action} instead of
    stringly text, keeping with the strong-typing rule in AGENTS.md.
  - Add a specialized renderer in conversation.rs/message_item.rs for this variant:
      - Render the diff using DiffPreview with per-line coloring (green +, red -,
        neutral gray context) and preserve newlines/indentation (skip textwrap for
        these blocks; compute height by lines).
      - Append a two-button row [ Approve ] [ Deny ], highlightable; intercept h/l (or
        left/right) and y/n/Enter when the selected message is a proposal-action block
        to send StateCommand::ApproveEdits or DenyEdits.
      - After action, replace the block with a condensed status line (approved <ts> -
        success|failed <err> or rejected <ts>), keeping the diff available above for
        context.
  - Keep the approvals overlay as the power-user bulk view; conversation blocks
    are a per-proposal inline affordance. Both should update from the same state
    (EditProposalStatus) so the UI reflects changes consistently.
  - Keybinding note: we can contextually override h/l only when the focused message is
    a proposal block, so branch navigation keeps working elsewhere.

  If You Want to Proceed (TDD sketch)

  1. Add a rendering/interaction test for a proposal-action message: ensures diff lines
     are color-tagged and the two-button row cycles selection and emits the correct
     StateCommand.
  2. Implement the typed message + renderer + input interception; adjust height
     measurement for pre-wrapped diff lines.
  3. Wire staging in rag/tools.rs to enqueue this new message type (with DiffPreview
     data) alongside the existing SysInfo summary, so conversation shows the actionable
     block automatically.

  Happy to refine the UX (e.g., exact colors, condensed vs. full diff) before coding.
Token usage: total=882,400 input=838,218 (+ 9,114,368 cached) output=44,182 (reasoning 22,848)
