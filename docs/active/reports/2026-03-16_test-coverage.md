Failure-State Synthesis
  Across the last 5 commits (94de83ee, 936a8bad, dc4d3043, 2e6fe064, 99103a95), the material
  fail states cluster into these groups:

  - parse_workspace boundary failures in lib.rs:66:
    missing/invalid workspace manifest, missing [workspace], selection mismatch, member parse
    failure, and DTO conversion failure in ParsedCrate::try_from:372.
  - Path-shape failures in normalize_selected_crates:146:
    lexical-only matching means crate_a/., crate_a/../crate_a, symlink-shaped paths, and
    duplicate selections can behave unexpectedly.
  - Ancestor workspace lookup failures in locate_workspace_manifest:130:
    ancestor read error, ancestor parse error, no workspace found, and wrong nearest-workspace
    behavior in real nested-workspace layouts.
  - Normalization-contract failures in try_parse_manifest:172:
    path, members, exclude, and [workspace.package] can be wrong, partial, or insufficiently
    normalized.
  - Discovery/cache integration failures in run_discovery_phase:42 and mod.rs:152:
    multi-crate same-workspace runs, order dependence, cache reuse, and mismatch behavior.
  - Version-inheritance precedence failure in PackageVersion::resolve:82:
    workspace = false can currently lose to lookup/version errors.
  - Error-conversion/reporting loss in discovery/error.rs:45:
    new workspace errors can flatten or lose context.
  - Downstream invariant failures in visitor/mod.rs:43 and visitor/mod.rs:553:
    wrong logical paths, or missing root crate_context.

  Coverage Synthesis
  Covered:

  - parse_workspace is now well covered in lib.rs:494:
    default-all-members, relative selection, absolute selection, missing [workspace], missing
    manifest, invalid manifest, mismatch details, member parse aggregation, empty selection,
    DTO population.
  - Workspace version inheritance success is covered in discovery/mod.rs:447.
  - Normalized membership lookup for one fixture member is covered in discovery/mod.rs:482.
  - Explicit WorkspacePathMismatch is covered in discovery/mod.rs:503.
  - Basic ancestor workspace lookup and nested member-path happy path are covered in discovery/
    workspace.rs:345.

  Partially covered:

  - try_parse_manifest normalization is only partially covered:
    normalized members are exercised, but exclude, workspace.path, and [workspace.package] are
    not directly asserted.
  - DTO success-path usability is covered, but DTO failure through ParsedCrate::try_from:372 is
    not.
  - Discovery in workspace mode is covered for single-crate cases, not multi-crate cache
    behavior.
  - The all-or-nothing parse_workspace wrapper is covered as an error wrapper, but not as an
    explicitly asserted policy choice.

  Not covered:

  - Non-canonical selected paths and duplicate selections in lib.rs:146.
  - Missing-crate_context DTO conversion failure in lib.rs:372.
  - Direct WorkspaceManifestRead, WorkspaceManifestParse, and WorkspaceManifestNotFound runtime
    tests for workspace.rs:130.
  - Real nested-workspace “nearest workspace wins” behavior in workspace.rs:130.
  - Direct normalization assertions for exclude, nested members, path, and [workspace.package]
    in workspace.rs:172.
  - Missing-version failure in resolve_workspace_version:83.
  - Multi-crate cache behavior and order independence in discovery/mod.rs:152.
  - PackageVersion::resolve explicit / workspace-true / workspace-false precedence in
    single_crate.rs:82.
  - Direct conversion tests for the new workspace-specific errors in discovery/error.rs:47.
  - Workspace-to-Phase-2 invariants around logical paths and exactly one root crate_context in
    parser/visitor/mod.rs:43.
  - MultipleWorkspacesDetected in discovery/mod.rs:225, which appears uncovered and may be
    unreachable.
