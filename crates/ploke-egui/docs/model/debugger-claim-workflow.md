# Debugger Claim Workflow

`ploke-egui` is a developer debugger, not only a UI. A visible claim in the
inspector or graph must be treated as something a developer may use to decide
what to implement next. The workflow for adding or repairing one of those
claims is therefore evidence-first and type-preserving.

## Workflow

1. Start from the visible claim.

   Name what the UI is claiming in domain terms, before touching widgets. For
   example: "this Artifact has been run as a Child runtime" is a debugger
   claim, not a row label.

2. Trace the claim back to persisted records.

   Identify the typed record that makes the claim true. For the role badge, the
   source is `ploke_records::invocation::InvocationRecord`: `role` says how the
   runtime was invoked, and `node.derived_artifact_id` or
   `request.derived_artifact_id` ties that invocation back to an Artifact.

3. Confirm ingestion into `ploke-tree`.

   `ploke-egui` should not reread files, scrape text, or infer from rendered
   labels. Make sure `ploke-records` shapes are loaded into `ploke-tree` and
   exposed from `ploke_tree::Graph` through typed accessors or typed graph
   facts.

4. State the graph invariant that validates the UI claim.

   The role badge uses the invariant that each Artifact maps to a Runtime. When
   an `InvocationRecord` says that Runtime was invoked as `Child`, and the
   record is tied to Artifact `A`, the UI may show `A` with a Child role badge.

5. Preserve the claim as a typed witness.

   Carry the claim through the inspector as a typed borrowed object, not as a
   string or row-shaped carrier. For the role badge, the witness is:

   ```rust
   Badge::Child(&ArtifactId)
   Badge::Parent(&ArtifactId)
   ```

   The borrowed `ArtifactId` is both the correctness anchor and the performance
   anchor. It proves the badge is attached to an underlying graph artifact, and
   it avoids cloning identity just to render a label.

6. Project only at the render boundary.

   Convert typed witnesses to egui-facing objects as late as possible:

   ```text
   ploke_tree::Graph
     -> typed graph fact
     -> typed inspector witness
     -> Badge
     -> BadgeText
     -> egui
   ```

   `RichText`, labels, colors, rows, and layout are render projections. They
   are not source facts.

7. Verify the claim path, not only the widget output.

   Tests should prove that the typed record evidence produces the typed witness
   the UI renders. The role badge test should build invocation evidence tied to
   an Artifact and assert that the inspector exposes `Badge::Child(&ArtifactId)`
   or `Badge::Parent(&ArtifactId)`.

## Row Boundary

Visual row layout is allowed inside the renderer. Rows must not be semantic
objects, cache entries, inspector facts, or projection carriers for Artifact,
Runtime, role, History, candidate, evidence, or source-status claims.

If the data answers a developer question, keep it typed until the final egui
render step.

## Badge Example

`crates/ploke-egui/src/ui/text/decor.rs` is the current small example of this
style. `Badge<'a>` is not decorative text. It is a borrowed role witness tied
to an `ArtifactId`. `BadgeText<'a>` is the egui-facing projection that can carry
rendering details while preserving the Artifact binding.

This shape is intended to generalize. Future badge-like claims should prefer a
trait or typed projection that preserves the same chain:

```text
record evidence -> Graph fact/invariant -> borrowed typed witness -> render object
```

