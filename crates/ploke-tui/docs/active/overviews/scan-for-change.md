# scan_for_change

Overall workflow of `scan_for_change` in `ploke_tui::app_state::database`

Summary of function:

1. gets info on the crate from crate_focus stored in state

2. gets file data from crate info

3. checks if files still exist

4. uses `scan_for_change_batch` method from `IoHandle` to:
  - send a message to `IoManager` internal handler
    - matches on `ScanChangeBatch` request.
    - dispatches `handle_scan_batch_with_roots`
      - note 1
      - inside an iterator, calls `check_file_hash_with_roots`
        - normalizes against the `SymlinkPolicy` (note 2)
        - which finally calls `check_file_hash`
  - `check_file_hash` uses parsed tokens to generate the `TrackingHash`
    - uses `read_file_to_string_abs` to read file.
      - warning: this does not use the technique we have used elsewhere, really
      just in `process_one_write`, which is to use a reference to a dashmap as a
      way of keeping track of which files are available for writes.
      - TODO:refactor evaluate our overall strategy on reads/writes. See note 3
      below. 
    - uses `parse_tokens_from_str`, which is just syn's `into_token_stream` with
    error handling

5. if all files still exist and no files have changed, returns

6. parse all target files
  - currently fails for any file failing (note 4)
  - This gives us the full module tree of the new changed state.

7. filter on the modules for those items that have changed to get module uuids.

8. for each module node, get all the primary node descendant node ids.

9. filter on the primary node descendent node ids (retains only those
   descendants)

10. Iterates over each of the node types to remove any nodes that are contained
    in the set of descendants of changed files.
    - calls `retract_embedded_files` (see note 5)

11. 


### End notes
- note 1: uses `futures::stream::iter` for kind of no reason. I think I was experimenting here. 
  - I mean there is kind of a reason, which is that we are trying
  to do file io in parallel via an iterator, which just seems kind
  of cool.

- note 2: I vaguely get the SymlinkPolicy, but am not firm on the details or
the particulars of what is and isn't being checked. This is more of a
gesture in the right direction, but I need to add a doc comment with clear
details on what is and isn't covered so we have a clear understanding of
expected behavior.
  - TODO:tests add tests for `SymlinkPolicy` variations, including edge cases,
  specifically around the function `normalize_against_roots_with_policy`. Tests
  should include:
    - attempting to read outside of allowed roots.
    - attempting to use relative file paths to read outside of allowed roots.
    - read file in another allowed root relative to the current allowed root
    - read file in parent directory (with relative traversal) when either:
      - parent is in allowed roots (should succeed)
      - parent is not in allowed roots (should fail)
    - others tbd

- note 3: Identify all the places where we are reading/writing to files, and try
to come up with either an overall strategy that will work (e.g. this strategy of
using dashmap), but does so with better type-safety. It would be best if we
could stop 

- note 4: Should we be failing on the parsing when any file fails to parse?

  Yes, because we can only guarentee the resolution of types, etc, if we know
  the contents of all files during the resolution phase. Otherwise we need to
  worry about handling the error state of a given code item, which could quickly
  become complex.

- note 5: on `retract_embedded_files`
  - states the ascestor rule
  - adds a rule for has_file_ancestor that checks those relations that have an
  ancestor that is a file-level module
    - TODO: refactor
