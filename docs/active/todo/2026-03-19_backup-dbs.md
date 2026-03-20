# TODO: Backup Dbs

We have been working on this new relation we are adding to `ploke-db`, and have
come accross a persistent issue I've stumbled across before, which we might as
well sort out now.

The issue is that we have some databases which we created using the `/index
start crates/ fixture_crates/<fixture_name>` command in `ploke-tui`, which uses
our ingestion pipeline from `syn_parser` -> `ploke-transform` ->
`ploke-embed`/`ploke-db`, which is then surfaced in the UI through `ploke-tui.

These databases are then saved using `/save db` to the default config dir, and
I then manually copy them over in the command line to the `tests/backup_dbs`
directory, and they are loaded from the backup_dbs directory for various
`ploke-db` and `ploke-tui` tests, among others.

The issue is that we are in rapid development (pre-release), and sometimes need
to make changes to the database schema, which causes our tests to fail because
they are not able to find an expected relation on loading or importing the
backup db.

What we need is four things:

1. A way to validate the presence of expected databases
2. A way to re-create each backup expected by the tests from the command line
3. Fixture usage tracking with clear update instructions and periodic review
   for consistency
4. Test helpers that live in a shared crate for db setup so we aren't
   hard-coding the fixture path in different crates

## Fixture usage tracking

- Create a document in `docs/testing` with a list of all backup db test fixtures and the following details:
  - filename of backup in `tests/backup_dbs`
  - parsed target(s): the workspace, crate, or crates used to create the database, including their relative path
  - tests utilizing the fixture, separated by crate
    - for each test, indicate whether the test requires mutable or immutable
    access to the database (e.g. is data being added or removed, or is the
    database only being used for search/retrieval?)
    - expected database config for that target, e.g. bm25 indexing setup, embedding model expected, vector embeddings present/absent
  - date of last update

- Additionally, add an instruction to AGENTS.md to remind the user if it has been longer than 7 days since the last backup review, asking if the user would like a review to be started now. This instruction for the agent should include a doc link to the fixture usage tracking document.

## Backup creation

- Use our `xtask` crate to (re)create expected backup db test fixtures
  - name of backup includes date created (yyyy-mm-dd)
- Checks the health of the expected test fixtures, validating that they can be loaded, imported, and saved as tests expect.
- On validation failure, provides clear instructions on how to backup the missing fixtures.

## Validate Expected Databases

- For each database fixture used in tests, check the presence of those
databases
- Create a clear document in `docs/how-to` with instructions on how to re-create the required databases using the command in (2).

## Test setup helpers

- Identify possibly extraneous test helpers

We have a number of test helpers in `crates/test-utils`, package name
"ploke-test-utils", which are helpful in database setup. However, due to
multiple refactors, there are likely more helpers than needed. If any test
helpers for the ingestion pipeline seem extraneous, or are being inconsistently
used, or are functionally duplicated or incorrect, identify and report them in
a new document in `docs/active/reports` called `test-helper-review.md`

- Create shared immutable database setup test helper

There are multiple locations in our crates where a lazy_static or static ref or
LazyStatic or similar is used to help cut down test time. Review the fixture
usage tracking document to identify which tests are using an immutable backup
db fixture, and create a single function or macro that can be used to either
create or reference the database (e.g. `OnceCell`, `LazyStatic`, etc).

Implement a `FixtureDb` struct that contains the required config info for the
backup db expected by our tests, such as embedding model expected, bm25 status,
etc, and use these in the tests to ensure the correct backup is being used.

Then, for each test that references the immutable backup db, use the test
helper instead, so we don't need to have ad hoc setup across our workspace
crates.

### Note on implementation

- On creating new helpers (outside of phase 4 above)

We have many methods and helpers for validating the contents of databases. Use
those helpers where possible, or the shared test helpers in `test-utils`.
Before creating a new helper, do a thorough search of related helpers, then ask
for explicit permission via a document you save in `docs/active/reports`, and
wait for permission before adding the helper. It must be clearly demonstrated
that no existing helpers would suffice and why a new helper must be added.

- On user communication

For any questions regarding the task which cannot be answered by exploring the code base, add them to `docs/active/agents/open-questions`

### On using sub-agents

#### If you are the main agent...

This is intended to be a long-running task, and you have permission to add git
commits periodically as the main agent at your discretion. These should help
serve both as identifiable chunks of progress and checkpoints for reverting if
the user wishes to revert changes.

use sub-agents for code exploration, code editing, and code review as much as
possible to limit your own context window.

For each task, pass it to a sub-agent and wait on their response before moving
forward. Only one editing sub-agent should be assigned at a time.

Sub-agents should be used for:
- code exploration
- code editing
- code review
- running and reporting on tests

As the main agent, your primary responsibility is to direct subagents, maintain
your context window, and keep sub-agents activity aligned with the overall
goals.

Additionally, there is a directory for use of main and sub-agents to share and
record progress or relevent doc/code links in `docs/active/agents/`,
which you may add to (but not remove from) with additional instructions for
sub-agents.

Whenever you have determined that a given phase is complete, ensure you spawn a
sub-agent for code review of the added/edited code to address cargo/clippy
warnings, code clarity and maintainability concerns, review the code for
correctness, and identify any inefficiencies or code smells to address (e.g.
unnecessary intermediate allocations, deeply nested if/else or conditionals,
etc). If any are reported, task another sub-agent to address the issues, and
repeat either until the code agent is satisfied or three reviews have been
performed. If possible, assign high or xhigh reasoning sub-agents for code
review tasks.

#### If you are the worker agent...

Add progress or shared information to `docs/active/agents/` with updates on your task or with documentation links and/or code locations and summaries.

Open questions may be addressed to either the main agent directly, or through `docs/active/agents/open_questions.md`. You may not direct a question directly to the user, but it will be reviewed and possibly bumped to the user if an answer cannot be found by the main agent.
