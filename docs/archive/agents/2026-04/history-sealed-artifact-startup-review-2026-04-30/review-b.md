# Review B: sealed artifact startup gate

Commit reviewed: `21330672` (`history: verify sealed artifact at startup`)

Verdict: the commit implements a real successor-side admission check for the live `--handoff-invocation` path: it loads the current sealed History head, verifies the stored block hash, derives the current clean checkout tree key, and refuses `Parent<Ready>` if the sealed artifact claim is missing or mismatched. It does not yet make sealed History the universal startup boundary for every parent-capable runtime, and several type/visibility seams still allow future code to assemble or preserve authority-shaped data outside the intended transition.

## Findings

1. **High: non-handoff startup still bypasses sealed History admission.**

   The new gate is wired only inside successor handoff acknowledgement: `acknowledge_prototype1_state_handoff` calls `validate_prototype1_successor_history_admission` before returning `parent.ready()` when a successor invocation exists (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3117`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3130`). That verifier reads the configured store head, loads the sealed block, derives `clean_tree_key(repo_root)`, and verifies the admitted artifact claim (`crates/ploke-eval/src/cli/prototype1_process.rs:397`, `crates/ploke-eval/src/cli/prototype1_process.rs:404`, `crates/ploke-eval/src/cli/prototype1_process.rs:416`, `crates/ploke-eval/src/cli/prototype1_process.rs:419`, `crates/ploke-eval/src/cli/prototype1_process.rs:424`).

   The same function still returns `Parent<Ready>` without any History check when `--handoff-invocation` is absent (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3065`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3066`). The outer startup path only loads parent identity and validates checkout/scheduler facts before calling the handoff acknowledgement helper (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3216`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3219`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:347`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:353`). A manually invoked successor artifact, or a future caller that forgets to pass the invocation, can therefore enter the typed parent turn without proving that the current tree is admitted by the sealed head.

   Minimal fix: make `Parent<Checked> -> Parent<Ready>` consume an explicit startup-admission carrier. For non-handoff startup, require either an explicit genesis/bootstrap admission with an absent checked head, or the same sealed-head artifact check used by successor handoff.

2. **Medium: claim deserialization and `SealBlock` visibility leave a claim-injection path for future crate code.**

   `Claims` is documented as having no public constructor and not being an authority factory (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2051`), but it now derives `Deserialize` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2057`). `SealBlock` exposes its `claims` field crate-wide (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1985`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1989`). The live transition requires a Crown-admitted artifact claim, but it preserves any pre-existing claims while adding the artifact claim (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:197`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:217`).

   This means sibling module code could deserialize arbitrary policy/surface/manifest claims into `block::Claims`, assign them to `SealBlock.claims`, and then seal a hash-valid block through `seal_block_with_artifact`. Those preserved claims would not have been produced by `Crown<Ruling>::admit_claim`, despite being committed into the sealed header. This is not used by the live path, but it weakens the intended "durable record is a projection of an allowed transition" invariant.

   Minimal fix: keep `SealBlock.claims` private and make the handoff seal transition construct all claim storage internally from typed admitted claim inputs. If loader deserialization is needed, keep it behind a private verified-loader type rather than exposing `Deserialize` as a general crate-level constructor for `Claims`.

3. **Medium: the admitted artifact claim is checked against the current tree, but it is not structurally tied to the successor identity fields.**

   The parent computes `successor_artifact` as an `ArtifactRef` string from node metadata or branch fallback (`crates/ploke-eval/src/cli/prototype1_process.rs:921`, `crates/ploke-eval/src/cli/prototype1_process.rs:1084`) and separately computes `artifact_key` from the active checkout tree (`crates/ploke-eval/src/cli/prototype1_process.rs:1112`, `crates/ploke-eval/src/cli/prototype1_process.rs:1116`). The sealed block records `SuccessorRef` and `active_artifact` from the string ref (`crates/ploke-eval/src/cli/prototype1_process.rs:930`, `crates/ploke-eval/src/cli/prototype1_process.rs:934`), while the admitted claim stores the tree-key hash (`crates/ploke-eval/src/cli/prototype1_process.rs:951`, `crates/ploke-eval/src/cli/prototype1_process.rs:954`).

   The live sequence likely makes these agree because the selected artifact is installed before `handoff_block_fields` derives the tree key (`crates/ploke-eval/src/cli/prototype1_process.rs:512`, `crates/ploke-eval/src/cli/prototype1_process.rs:515`). But the block type does not encode that `selected_successor`, `active_artifact`, parent identity, installed branch, and admitted tree key describe one artifact. A stale artifact id or future alternate call site can produce a self-consistent block hash with inconsistent artifact identity fields.

   Minimal fix: introduce a small successor artifact commitment carrier built only after active-checkout install/validation. Use that one value to populate `SuccessorRef`, `SealBlock.active_artifact`, the admitted artifact claim, and successor invocation metadata.

4. **Low: docs/comments are now internally stale in both directions.**

   The implementation now verifies sealed History for the live handoff path, but canonical comments still say the successor "does not verify that sealed head on startup" (`crates/ploke-eval/src/cli/prototype1_state/history.rs:13`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:17`) and that live successor validation still does not derive and check the current tree against the sealed head (`crates/ploke-eval/src/cli/prototype1_state/history.rs:273`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:280`). The v2 design note says live startup admission through sealed History is not implemented (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:41`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:83`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:94`). `prototype1_process.rs` also still documents successor startup as only "validate continuation" before ready acknowledgement (`crates/ploke-eval/src/cli/prototype1_process.rs:115`, `crates/ploke-eval/src/cli/prototype1_process.rs:117`).

   Conversely, the cross-runtime Crown prose remains stronger than the implementation where it says the only valid way to make the successor runtime executable is the predecessor move-only handoff transition (`crates/ploke-eval/src/cli/prototype1_state/history.rs:101`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:105`). That is true for the live spawn helper, but not for arbitrary local process execution or direct `prototype1-state` invocation.

   Minimal fix: update docs to distinguish "live successor handoff startup is gated" from "all parent-capable startup is gated." Keep the process-uniqueness and cross-runtime Crown claims explicitly out of scope until there is a lease/lock or equivalent.

## Invariant Coverage

- Successor handoff now seals and appends a History block before spawning the successor (`crates/ploke-eval/src/cli/prototype1_process.rs:941`, `crates/ploke-eval/src/cli/prototype1_process.rs:964`, `crates/ploke-eval/src/cli/prototype1_process.rs:966`).
- The artifact claim is required by the live `seal_block_with_artifact` transition signature and admitted through `Crown<Ruling>::admit_claim` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:170`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:177`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2597`).
- The successor check verifies the stored block hash and checks current checkout tree key equality against the sealed artifact claim (`crates/ploke-eval/src/cli/prototype1_state/history.rs:704`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:724`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2470`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2485`).
- `BlockHash` tightening improves the invariant without breaking normal storage compatibility: serialized form remains a hex string (`crates/ploke-eval/src/cli/prototype1_state/history.rs:391`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:396`), while deserialization now rejects non-64-hex payloads (`crates/ploke-eval/src/cli/prototype1_state/history.rs:400`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:426`). Existing valid SHA-256 hex block records should continue to load.

## Residual Risks

- No process-uniqueness guarantee: two processes running the same admitted checkout can both pass the artifact gate until a lease, lock, or consensus layer exists. The docs correctly note this limit (`crates/ploke-eval/src/cli/prototype1_state/history.rs:121`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:132`).
- No cryptographic signature or remote witness: local filesystem tampering by a process with write access remains outside the current tamper-evidence claim (`crates/ploke-eval/src/cli/prototype1_state/history.rs:183`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:189`).
- `sealed_head_block` currently rejects non-empty stored blocks (`crates/ploke-eval/src/cli/prototype1_state/history.rs:698`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:851`), so startup admission is limited to the current zero-entry handoff block shape.
- `Parent<Ruling>` still does not exist as a live structural carrier; Crown authority is minted from `Parent<Selectable>` lineage identity (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:214`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:215`).

## Verification

Ran `cargo test -p ploke-eval prototype1_state::history::tests:: --locked`: 29 history tests passed. The run emitted existing warning noise but no failures.
