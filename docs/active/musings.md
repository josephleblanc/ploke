LangGraph: DAG for agentic workflow
  - shared state (common file)
    - like a shared whiteboard: initial request, data requested, accumulated report, errors, "next action"
    - every agent updates the shared state
    - signal the next state
  - agent nodes are like functions
    - input, output, set next action
  - routing logic
    - takes current state as input, decides what runs next
    - can return "end" to stop the workflow
    - can branch based on runtime behavior: e.g. calling an "error handling" agent in failure states
  - agents run sequentially
  - react to each other's progress
  - modular design: each agent is a single node in the graph
  - dynamic routing: workflow can change paths based on runtime conditions
  - agent behavior can alter the direction of the workflow.
CrewAI: Open source python framework, strong emphasis on structured workflows
Microsoft's Autogen: emphasis on Inter-agent communication, agentic self-organizing
IBM

On agentic RAG more generally:
- Have a "fail state" that identifies when the query is not relevant to the RAG database, and can then respond that the query is not within the types of questions it would help answer.
