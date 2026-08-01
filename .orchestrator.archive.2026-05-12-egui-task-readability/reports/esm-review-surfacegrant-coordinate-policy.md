# Review: SurfaceGrant Coordinate/Policy Patch

Decision: fix before accept.

Findings:

- High: `Grant` represents two incompatible semantic states: authority-bearing
  surface grant and legacy material-only filter. `Option<GrantAuthority>` makes
  absence a normal state, and `EditableSurface::broad(...)` plus legacy
  constructors can still mint grants without runtime/policy authority.
- Medium: `Grant::check` erases the authority payload. A check from a
  coordinate-bound grant is not structurally distinguishable from a check from a
  material-only grant, so apply/History cannot cite the grant that authorized
  the edit.
- Low: tests cover target mismatch and exposed coordinate/policy, but do not
  prove broad/request admission is authority-bound, `narrow()` preserves
  authority, or checked apply retains coordinate/policy.

Required correction:

- The normal SurfaceGrant carrier must require coordinate and policy authority.
- If material-only bounds are still needed, isolate them behind a distinct
  non-authority adapter instead of a first-class alternate `Grant` state.
- Checks and downstream evidence must preserve the grant authority chain.
