Types of Memory

Project
- Summary description of project
- "Table of Contents" with main parts of project + short descr of each file
- Recent changes

Operational
- LLM-facing info on how to:
  - use tools
  - address user
- Think of it as the LLM's "user manual" for Ploke.
? common failure modes ?

User
- Code Style + Convention
- Mastery level
- User-specified prefs, e.g. "Always remember to talk like a pirate"
- Long-term + Short-term goals
? likes/dislikes ?
? how to include things the user has accepted/rejected in the past ?
? if using sentiment analysis, maybe add extreme sentiments with messages + occasion ?

Task-specific
- Code worked on
- files in view
- goals/reqs

Ambient
- Events happening b/t tool calls
Depends on visible actions, but might be things like:
  - Files opened/closed/edited
  - Cursor time spent at location.
  - Inline marked questions/commands
  - Builds/runs/fails/tests


Questions regarding communicating system messages to LLM:

- For system messages, can I use LaTeX to concisely explain things with set theory annotation?
