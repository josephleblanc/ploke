1. Fix issue with proposing code edits.
- the AI seems to be sending multiple responses repeatedly when asked to supply code edits.
- likely an issue with not supplying the AI with feedback that the code edit proposal has been made.
- todo: add a way to supply the AI with feedback on its submitted code edits

Update Oct 26: The issue with proposing code edits has been fixed.

2. Fix issue with syncing loaded database

- Issue occurs when files present in loaded database are not in local filesystem

- Q: What is the root cause of the issue?

It seems like the root cause is in `scan_for_change` in ploke-tui, in the file `app_state/database.rs`.

The issue seems to be that when scanning for a change, the `ploke-io` crate tries to read from files that do not exist, returning an error.

The desired behavior would be to check whether these files exist, and upon seeing that a file does not exist, to add that file to a list of files to be pruned from the database.

Adding this file to a list of files to be removed could happen in `scan_for_change`, but could also happen in the `ploke-io` function `scan_changes_batch`, which is called in `scan_for_change`.

Tracing the functions called to the error site:
  - `scan_for_change`
    - `state.io_handle.scan_changes_batch`
  - in ploke-io's handle.rs:
    - `scan_changes_batch`
    - `run`
    - `handle_request`
    - handle_scan_batch_with_roots
    - `check-file_hash_with_roots`

- Q: How do I remove a file from the database, along with all related edges contained in the file?

We can remove the file from the database by including all files which are not
found in the target directory in the "changed_filenames" within the
`scan_for_change` function in `app_state/database.rs`. This includes the
filenames in those files which are to be added to the nodes which are to be
removed from the database, along with a recursive depth-first traversal of each
node's contents.

NOTE: The algorithm within `scan_for_change` that traverses nodes and removes
them from the database is inefficient, taking a noticably long time to complete
for `ploke-tui`. This needs to be improved.
- Added to `ploke/docs/active/TECH_DEBT.md` oct 26

Update Oct 26: Immediate fix added, confirmed working correctly. Still needs to
be made more efficient, but it at least functions correctly to remove the nodes
of files that have been removed from the database now.

3. Add token cost estimator
- use the token amounts contained in the responses from the OpenRouter API
- first just keep a count of the tokens in the current conversation
- consider how to estimate cost. Where is this information contained?

Update oct 26: moved to `today-oct-26.md`

4. Migrate to Ratzilla
- evaluate which changes need to be made to implement a webassembly build of Ploke
- Q: is it as simple as replacing the ratatui rendering with ratzilla-based rendering?
- Q: are there other crates that will need to be changed or that won't work in a webassembly context?
- Q: is there anything extra that needs to be considered regarding file access in the webassembly build?
- Q: should the webassembly build be its own crate or should it be a build flag within ploke-tui?

Update oct 26: moved to `today-oct-26.md`
