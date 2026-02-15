# IMMEDIATE NEXT STEPS

A document we can edit and add to as we work on various projects.

## Overview of Refactoring `NodeId` to include `cfg` attributes in hashed Uuid

We are refactoring the `NodeId` to include the `cfg` attributes in the UUIDv5
hash of `NodeId` to prevent conflation of ids and allow items with different
cfg flags to be included correctly in the code graph.

Okay, let's analyze the potential ripple effects of incorporating the `#[cfg(...)]` attribute string into the hash for `NodeId::generate_synthetic`.

This change directly addresses the known limitation documented in `ADR-009-Defer-Item-Level-Cfg-Handling.md`, where items differing only by `cfg` attributes currently receive the same `NodeId::Synthetic`.

Here are the likely ripple effects:

1.  **Node ID Changes for `cfg`-gated Items:**
    *   **Direct Impact:** Any item (function, struct, enum, etc.) that has one or more `#[cfg(...)]` attributes directly attached to it will now generate a **different** `NodeId::Synthetic` UUID than it did before this change. The hash input now includes the `cfg` string(s), altering the resulting UUIDv5.
    *   **No Impact on Non-`cfg` Items:** Items *without* `cfg` attributes will generate the *same* `NodeId::Synthetic` as before (assuming no other changes to the hashing inputs or method).

2.  **Resolution of ID Conflation:**
    *   **Core Benefit:** The primary positive effect is that items previously conflated because they only differed by `cfg` attributes (like `CfgGatedStruct` in `fixture_conflation`) will now receive **distinct** `NodeId::Synthetic` values. This makes the Phase 2 graph more accurately represent the distinct definitions present in the source code, even if those definitions are mutually exclusive in a final build.

3.  **Impact on Relations:**
    *   `Relation` structs created during Phase 2 (primarily `RelationKind::Contains` linking modules to items) will now use the *new*, distinct `NodeId::Synthetic` for any `cfg`-gated target items. This is generally correct, as the relation now points to the disambiguated item ID.

4.  **Impact on `ModuleNode.items`:**
    *   The `items` list within `ModuleNode` (for `Inline` or `FileBased` modules) will contain the *new*, distinct `NodeId::Synthetic` values for any `cfg`-gated items defined within that module.

5.  **Impact on `VisitorState.current_definition_scope`:**
    *   If a `cfg`-gated item (e.g., a `struct` or `impl` block with `#[cfg]`) is pushed onto the scope stack, its *new* `NodeId::Synthetic` will be used.
    *   When generating IDs for items *nested* within that `cfg`-gated scope (e.g., fields, methods), the `parent_scope_id` passed to `NodeId::generate_synthetic` will be this new ID. This ensures correct scoping relative to the disambiguated parent.

6.  **Impact on Phase 3 Resolution:**
    *   The mapping built in Phase 3 (`TemporarySynthId -> FinalPathId`) will now have separate entries for items previously conflated by `cfg`. For example, `#[cfg(a)] struct Foo` and `#[cfg(not(a))] struct Foo` will have different synthetic IDs mapping potentially to the same resolved path ID (if their logical path is the same). This increased granularity is accurate.
    *   The process of updating relations using this map should function correctly with the new distinct IDs.

7.  **Significant Impact on Testing (`paranoid` Helpers):**
    *   **Helper Logic:** The `find_*_node_paranoid` helper functions currently regenerate the expected `NodeId::Synthetic` based on context *excluding* `cfg` attributes. These helpers **must be updated**. They will now need to:
        1.  Find the candidate node(s) based on name/path.
        2.  Extract the `#[cfg(...)]` attributes directly from the found node instance(s) in the `CodeGraph`.
        3.  Canonicalize/stringify these attributes in the *exact same way* the visitor does.
        4.  Include this `cfg` string representation as part of the input when regenerating the expected `NodeId::Synthetic` hash.
    *   **Test Failures:**
        *   All existing paranoid tests targeting items *with* `cfg` attributes will **fail** until the helpers are updated, because the regenerated ID (without `cfg`) will not match the actual ID (with `cfg`).
        *   Tests that *specifically verified the conflation* (like `test_cfg_struct_node_id_conflation`) will now **fail** because the IDs will become distinct. These tests need to be rewritten to assert that the IDs for the different `cfg` branches are *different*.

8.  **Implementation Considerations:**
    *   **Accessing Attributes:** The `CodeVisitor` has access to `item.attrs` when visiting an item, so passing this information to the ID generation function is feasible.
    *   **Canonical `cfg` String:** A consistent method for converting the relevant `cfg` attributes into a string or byte sequence for hashing is crucial. Using `attr.to_token_stream().to_string()` for each `cfg` attribute (sorted perhaps by the attribute path string) might be the most direct way, but could be sensitive to formatting within the attribute arguments. Parsing the `syn::Meta` within the attribute to build a canonical string might be more robust but adds complexity.

**In Summary:**

*   **Pro:** Resolves the known `NodeId` conflation issue for `cfg`-gated items, making the Phase 2 graph more precise.
*   **Con:** Requires significant updates to the `paranoid` test helpers and rewriting tests that previously relied on the conflation behavior. Introduces the need for a stable way to incorporate `cfg` attribute information into the hash.
*   **Overall:** This change improves the semantic accuracy of the generated IDs at the cost of increased complexity in testing and the ID generation logic itself. It's a necessary step if unique identification regardless of `cfg` is required before Phase 3.


--- 

## Using `rg` to find relevant files for refactoring and evaluating refactoring impact

### USER NOTE
> I've included the following message from earlier in the conversation
> so you (my AI code collaborator) can help remind me of the `rg` commands we can
> use to include relevant files in our refactoring without cluttering up the
> context window with noise introduced by tangential or irrelevant files. Please
> remind me of these commands as we continue to refactor.

Okay, dropping the large context dump and planning to add a curated list of affected files based on the ripple effect analysis is a much more manageable and effective approach.

Now, let's consider other `rg` searches that could be helpful for specific parts of the `NodeId` refactoring plan (incorporating `cfg` attributes into `generate_synthetic`), beyond the initial broad `rg -w -l NodeId .`:

The main areas identified in the ripple effect analysis were:

1.  The `generate_synthetic` function itself.
2.  The `paranoid` test helpers.
3.  Tests that use these helpers or assert specific NodeIds (especially for `cfg`-gated items).
4.  Places where `cfg` attributes are accessed or processed within the visitor (relevant for implementation).

Here are some targeted `rg` searches corresponding to these areas:

1.  **Locating the Core Function:**
    *   **Command:** `rg 'fn generate_synthetic\(' crates/ploke-core/src/lib.rs`
    *   **Purpose:** To quickly find the exact definition of the function we need to modify within `ploke-core`. (Adding `-l` isn't usually needed for a single known file/function).

2.  **Locating the `paranoid` Test Helpers:**
    *   **Command:** `rg -w -l 'find_.*_node_paranoid'`
    *   **Purpose:** To find all files containing the definitions or calls to the `find_*_node_paranoid` helper functions. These are the primary test utilities that *must* be updated to account for the `cfg` attribute hashing. Adding these files is crucial.

3.  **Locating Potentially Affected Tests:**
    *   **Command:** `rg -l 'test_cfg_struct_node_id_conflation'`
    *   **Purpose:** To find the specific test(s) mentioned previously that directly tested the *old* conflation behavior. These tests need to be rewritten to assert the *new* distinct ID behavior.
    *   **Command:** `rg -l 'use crate::common::uuid_ids_utils::.*paranoid;' tests/`
    *   **Purpose:** To find test files that explicitly import and likely use the `paranoid` helpers. These are prime candidates for needing updates or verification.
    *   **Command:** `rg -w -l '#\[cfg' tests/fixture_crates/fixture_conflation/`
    *   **Purpose:** To specifically list the files within the `fixture_conflation` crate that use `cfg`. Tests using this fixture are highly likely to be affected by the ID changes.
    *   **Command (Use with caution - potentially noisy):** `rg -w 'assert_eq!\(.*NodeId'`
    *   **Purpose:** To find assertions that directly compare `NodeId` values. This might catch tests that hardcoded expected IDs (though hopefully, the paranoid helpers were used instead). This can be noisy, so review the results carefully.

4.  **Locating Relevant Implementation Details (Attribute Handling):**
    *   **Command:** `rg -w -l 'item.attrs'` (Potentially narrow down path: `crates/ingest/syn_parser/src/parser/visitor/`)
    *   **Purpose:** To find where item attributes (`syn::Attribute`) are accessed within the visitor code. This helps locate the place where you'd need to extract the `cfg` attributes to pass them to the (modified) `generate_synthetic` function.

**How to Use These Searches:**

*   Run these commands (potentially adding `-l` to get file lists).
*   Compare the results against the curated list you're preparing based on the ripple effect analysis. These searches might help confirm your list or identify specific files related to testing that need attention.
*   Use the file lists generated by the `-l` flag to selectively `/add` the most relevant files (especially the `paranoid` helper definitions and key affected tests) to the chat when you're ready to discuss or implement the changes to those specific parts.

This more targeted approach, combined with your curated list, should provide sufficient context for tackling the different facets of the refactoring (modifying the core function, updating test helpers, fixing tests) without overwhelming the context window.
