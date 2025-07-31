# Possible features

## Code Context Map
Crate a map that shows how the code snippets are connected in addition to
providing code snippets relevant to the user query. The code-graph structure
could prove to be an inherent strength to the project outside the explicit
functionality of enabling database queries.

If we had this feature, it might have helped to avoid the error linked below:
[See Potential Insights from E-DESIGN-DRIFT (Violating Core Design)] "Section 4 System/Tooling Factors"

## Cracked Smol Model

- Change the agentic framework to optimize for uptime, meaning the AI does as much as possible as often as possible.
- Rather than minimize the number of steps, instead it will
  - review + cargo check more often,
  - step through code every few lines,
  - always use a relatively large number of parallel attempts, either literally or prompting the same model multiple times with cycling histories
  - other cool shit!
- Goal is to use the small AI as usefully as possible within the given time frame, not to reduce token cost.
  - This does not mean inefficient processes
