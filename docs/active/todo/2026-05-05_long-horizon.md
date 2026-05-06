› capabilities to pull together the first real self-improvement loop now that we have proven we
  can at least maintain our Crown authority and parent/child handoff across 30 generations, which
  has already been accomplished.

  So I think next steps for us here are:
  1. send out sub-agents to survery crates/ploke-eval/src/protocol and crates/ploke-protocol along
  with crates/ploke-eval/src/cli/prototype1_state and the crates/ploke-eval/src/campaign.rs along
  with any other ploke-eval metrics we have on analyzing the success or failure of a given run,
  and put together a unified report on all the data sources that we already generate but are
  likely getting lost. Also in this we should chase down where all the files actually live for the
  fractured Report types and log types we are generating - I think there are over 20 different
  types for this and some have already been surveyed in docs/reports/prototype1-record-audit/
  history-admission-map.md and docs/active/agents/history-surface-admission-review-2026-04-30 but
  there may still be more. I'd like use to arrive at a definitive, complete report of what we have
  to draw from here, where it is stored, what the provenance is, whether it is stale in terms of
  file location etc, and then we can decide what is valuable for us - previously we were mostly
  saying we should just use the mechanized metrics only, but I think we probably want to reach for
  the llm-adjudicated stuff as well, just as long as we have clear provenance and can include/
  exclude what we want from our admissable evidence with provenance for child selection.
  2. use what we can from the .agents/hyper-agents.txt and analysis in docs/workflow/evalnomicon/
  chat-history/on-hyper-agents.md to just implement their mechanism for child selection, and we
  can develop other algorithms that we can use for this over time, but this seems like something
  other people have put some thought into so we can draw from their work and then just have a
  plug-and-play approach to our policy that selects from various approaches, and this can be the
  first comprehensive one.
  3. We will want to allow for the kinds of moves to be included in our policy enactment as code
  as described in the hyper-agent paper but also more broadly have a clear mechanism for traversal
  and relocation along the git tree, ensuring we retain provenance appropriately for both the case
  of a runtime that edits its own code and a runtime that edits another checkout, such that we can
  appropriately allow the kinds of traversal that permits revisiting old parents and rehydrating
  their runtimes along a different checkout, etc, so our selection mechanism can use that
  affordance to explore where it needs to.
  4. We have the beginning I think of a registry in crates/ploke-eval/src/successor_selection/
  registry.rs but we may want to build it out further. What we need here is the ability to select
  from among all the scores of previous children, and reference the provenance of those scores.
  5. While likely a relatively small change, we want to ensure our actual evaluation step allows
  for a setup that includes multiple eval isntances instead of just one, which is enforced in our
  code somewhere I think in the child execution path.
  6. We want to hook up ploke-tui to allow for a filter that can be papplies over a Surface that
  limits what can and cannot be edited, then fire up a sub-agent that can perform some exploration
  with some evidence over that surface and apply edits, which will then asin our curent
  architecture become the patch that is applied to create the child.
  7. we want to then ensure we are generating and have projections of the data that is being
  output from the loop such that we can evaluate the changes over time in reference botho the
  self-analysis metrics and the objective metrics on the eval, such that we can test against the
  oracle periodically with our highest-scoring memebers on more targets than usual, to optimize
  for a combination of cost and validity given our constraints.
  8. finally we want to actually run the loop end to end for a larger number of generations and be
  able to actually observe the output in a way that makes it clear to us whether the system is
  improvoing or degrading over a longer time horizon.
  9. Once we can show some stability and results with a single-active ruler we can begin to have
  multiple active rulers running simultaneously across our blockchain infrastructure, ideally in a
  way that scales up arbitrarily.
  10. once we have some amount of concurrent rulrs running simultaneously we can then improve on
  the inter-ruler communication and bolockchain backend to allow rulers to run on arbitrary
  machines and share information.
  11. Finally once this framework has the beginning capacity for reliable self-improvement on code
  tasks in rust we can begin to add more external oracles and consider more sophisticated methods
  of diversificartion and cross-breeding of essentially "alleles" and how we can isolate and
  strenghten different lineages for different targets by drawing from the cross-domain
  competencies in our project.
