# Design Philosophy: Static Dispatch via Monomorphization

## Core Principle

In the Ploke project, particularly within the `syn_parser` crate and its handling of code graph elements and identifiers, we prioritize **static dispatch** achieved through **monomorphization** wherever feasible. This means leveraging Rust's generic system and trait bounds to resolve method calls and generate specialized code at compile time, rather than relying on dynamic dispatch mechanisms (like trait objects with vtables) at runtime.

## Connection to the Typed ID System

Our strictly typed ID system (e.g., `ModuleNodeId`, `FunctionNodeId`, `PrimaryNodeId`, `AnyNodeId`) is a cornerstone of this philosophy.

1.  **Distinct Types:** Each specific ID (e.g., `FunctionNodeId`) is a unique type (a newtype wrapper).
2.  **Marker Traits:** Traits like `PrimaryNodeIdTrait`, `AssociatedItemNodeIdTrait` categorize these specific IDs.
3.  **Generic Functions:** We write generic functions that operate on IDs, using trait bounds to constrain the types they accept. For example, `SyntacticRelation::contains_target` allows finding a contained item *if* it matches the requested type `T`:
    ```rust
    // Simplified Example from relations.rs
    impl SyntacticRelation {
        // T must be a type that implements PrimaryNodeIdTrait
        pub fn contains_target<T: PrimaryNodeIdTrait>(&self, src: ModuleNodeId) -> Option<T> {
            match self {
                Self::Contains { source: s, target } if *s == src => {
                    // target is PrimaryNodeId
                    // Attempt conversion using TryFrom<PrimaryNodeId> for T
                    T::try_from(*target).ok() // Returns Some(T) or None
                }
                _ => None,
            }
        }
    }
    ```
    This pattern is then used idiomatically within iterator chains, often relying on type inference rather than explicit turbofish notation:
    ```rust
    // Example combining generic functions and iterators (like find_methods_in_module_improved)
    fn find_methods_in_module_improved(
        graph: &impl GraphAccess,
        module_id: ModuleNodeId,
    ) -> impl Iterator<Item = MethodNodeId> + '_ {
        // Find ImplNodeIds contained in the module (infers T=ImplNodeId for contains_target)
        let impl_ids = graph
            .relations()
            .iter()
            .filter_map(move |rel| rel.contains_target(module_id));

        // For each ImplNodeId, find its associated MethodNodeIds using flat_map
        impl_ids.flat_map(move |impl_id| { // Use flat_map for one-to-many
            graph.relations().iter().filter_map(move |rel| match rel {
                SyntacticRelation::ImplAssociatedItem { source, target } if *source == impl_id => {
                    // target is AssociatedItemNodeId
                    // try_into() infers target type MethodNodeId from flat_map's expected output
                    (*target).try_into().ok() // Convert Result to Option for filter_map
                }
                _ => None,
            })
        }) // Returns a lazy Iterator<Item = MethodNodeId>
    }
    ```

## Monomorphization Explained

When the Rust compiler encounters a call to a generic function like `contains_target::<FunctionNodeId>(...)`, it performs **monomorphization**:

*   It effectively creates a *new, specialized version* of that function specifically for the concrete type `FunctionNodeId`.
*   All generic type parameters (`T`) are replaced with the concrete type (`FunctionNodeId`).
*   Trait bound checks (`T: PrimaryNodeIdTrait`) are verified at compile time.
*   Method calls within the generic function (like `T::try_from`) are resolved to the specific implementation for the concrete type (`impl TryFrom<PrimaryNodeId> for FunctionNodeId`).

The compiler generates separate, optimized code for each concrete type the generic function is used with (e.g., one version for `FunctionNodeId`, another for `StructNodeId`, etc.).

## The Role of Type Inference (Avoiding Turbofish)

A key ergonomic benefit of this approach is that explicit type annotations using the "turbofish" syntax (e.g., `::<FunctionNodeId>`) are often **unnecessary** at the call site. Rust's type inference engine is usually powerful enough to deduce the concrete type `T` for the generic parameter based on the surrounding context:

*   **Return Type Context:** If the result is assigned to a variable with an explicit type (`let result: Option<FunctionNodeId> = ...`), the compiler infers `T`.
*   **Function Argument Context:** If the result is passed to another function expecting a specific type (`process_function(result)` where `process_function` takes `FunctionNodeId`), the compiler infers `T`.
*   **Method Call Context:** If a method specific to the concrete type is called on the result.
*   **Iterator Chains & Closures:** The expected input/output types of subsequent operations in an iterator chain or closure often constrain `T`. For example, if `try_into()` is used and the resulting `Ok(value)` is needed where a `ModuleNodeId` is expected (like in the `find_methods_in_module_improved` example's `flat_map`), the compiler infers that `try_into()` must attempt conversion *to* `ModuleNodeId`.

Turbofish (`::<>`) is only required when the compiler encounters ambiguity and cannot uniquely determine the concrete type `T` from the context alone. Our design leverages specific function signatures, trait bounds, and expected usage patterns to maximize the success of type inference, leading to cleaner and more readable code without sacrificing type safety.

## Benefits

1.  **Compile-Time Type Safety:** The compiler guarantees that only valid ID types (those satisfying the trait bounds) can be used with the generic function. Type mismatches are caught early. The use of `TryFrom` within the function ensures that even if a `PrimaryNodeId` is passed, it's explicitly checked against the *requested* specific type `T`.
2.  **Performance:** Static dispatch avoids the runtime overhead associated with dynamic dispatch (like vtable lookups). The specialized, monomorphized code can often be inlined and optimized further by the compiler, leading to performance comparable to hand-written non-generic code.
3.  **Clarity of Intent:** Generic functions with clear trait bounds explicitly document the *capabilities* required of the types they operate on. When calling such a function, type inference often makes the specific type argument implicit, yet the compiler still enforces the correct constraints, ensuring clarity without verbosity.

## Contrast with Dynamic Dispatch

The alternative would be to use trait objects (e.g., `&dyn PrimaryNodeIdTrait`). While powerful for heterogeneous collections or situations requiring runtime flexibility, dynamic dispatch incurs a runtime cost (vtable lookup) and often provides fewer compile-time guarantees about the specific underlying type. We prefer static dispatch for the core graph operations where performance and type rigidity are paramount.

## Tradeoffs

*   **Compile Time:** Monomorphization can increase compile times as the compiler generates code for each concrete type instantiation.
*   **Binary Size:** Generating multiple specialized versions of functions can potentially increase the final binary size, although optimizations often mitigate this.

## Conclusion

By embracing static dispatch via monomorphization, especially in conjunction with our typed ID system, we aim to build a `syn_parser` that is:

*   **Robust:** Catching type errors at compile time.
*   **Performant:** Avoiding runtime dispatch overhead for critical operations.
*   **Maintainable:** Providing clear type constraints and leveraging the compiler for verification.

This approach aligns with Rust's philosophy of zero-cost abstractions and compile-time guarantees.
