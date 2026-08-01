From [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:202), setup:

1. Loads/admites the run profile and prepares the batch/campaign.
2. Ensures baseline closure state exists.
3. Registers the root parent node.
4. Creates/checks out the parent branch.
5. Writes `.ploke/prototype1/parent_identity.json`.
6. Commits that identity file into the parent branch.
7. Prints the setup report from [cli_facing.rs](/home/brasides/code/ploke/crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:311).

That printed report is stdout-only, but it points at durable files: campaign manifest, closure state, scheduler, batch manifest, parent identity, node id, branch id, and profile commitment.
